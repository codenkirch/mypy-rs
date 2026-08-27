//! Stage 4b constraint solver (constraints.rs) for Issue #84.
//!
//! Ports core constraint creation (`infer_constraints`, `Constraint`)
//! to Rust. Stage A handles the top-level TypeVarType template case
//! (PyO3 `rust_infer_constraints`); the full port mirrors the stock
//! `ConstraintBuilderVisitor` nominal-instance paths
//! (`rust_infer_constraints_full`), deferring to Python for the protocol,
//! callable/overloaded, and variadic branches.

use pyo3::prelude::*;

use crate::checkexpr_functions::get_proper_or_expand;
use crate::subtypes::{map_instance_to_supertype, CONTRAVARIANT, COVARIANT};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::{
    find_unpack_in_list_inner, flatten_nested_tuples_inner, split_with_prefix_and_suffix_inner,
};
use crate::wire::{
    read_int, read_type, write_int, write_type, ReadBuffer, Type, WireError, WriteBuffer,
};

// Used only by the wire round-trip tests.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const SUPERTYPE_OF: i64 = 1;
pub(crate) const SUBTYPE_OF: i64 = 0;
const ANY_SUGGESTION: i64 = 9;
const TUPLE_LIKE_INSTANCE_NAMES: [&str; 5] = [
    "builtins.tuple",
    "typing.Iterable",
    "typing.Container",
    "typing.Sequence",
    "typing.Reversible",
];

/// Mirrors `mypy.utils.neg_op` (constraints.py:1617).
pub(crate) fn neg_op(op: i64) -> i64 {
    if op == SUBTYPE_OF {
        SUPERTYPE_OF
    } else {
        SUBTYPE_OF
    }
}

/// A representation of a type constraint (T <: type or T :> type).
///
/// Unlike the earlier wire format (which dropped `origin_type_var`, the
/// full `TypeVarType`), this carries the origin so the Python solver can
/// use `values`/`upper_bound`/`variance`/`meta_level` when grouping and
/// solving. The TypeVarType wire round-trip is complete (see #177).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Constraint {
    pub origin_type_var: Type,
    pub op: i64, // SUBTYPE_OF or SUPERTYPE_OF
    pub target: Type,
}

impl Constraint {
    pub(crate) fn write(&self, buf: &mut WriteBuffer) -> Result<(), WireError> {
        write_type(buf, &self.origin_type_var)?;
        write_int(buf, self.op)?;
        write_type(buf, &self.target)?;
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn read(buf: &mut ReadBuffer<'_>) -> Result<Self, WireError> {
        let origin_type_var = read_type(buf, None)?;
        let op = read_int(buf)?;
        let target = read_type(buf, None)?;
        Ok(Self {
            origin_type_var,
            op,
            target,
        })
    }
}

/// PyO3 entry point for `infer_constraints`.
///
/// Only the top-level TypeVarType template case is handled (matching the
/// Python O(1) branch in `_infer_constraints`); everything else defers to
/// Python with `None`. Serializes the resulting `Constraint` list
/// (origin + op + target) so the shim can reconstruct full objects.
#[pyfunction]
pub(crate) fn rust_infer_constraints(
    template_bytes: &[u8],
    actual_bytes: &[u8],
    direction: i64,
) -> Option<Vec<Vec<u8>>> {
    let mut template_buf = ReadBuffer::new(template_bytes);
    let template = read_type(&mut template_buf, None).ok()?;
    let mut actual_buf = ReadBuffer::new(actual_bytes);
    let actual = read_type(&mut actual_buf, None).ok()?;

    let constraint = infer_constraints_inner(&template, &actual, direction)?;
    let mut buf = WriteBuffer::new();
    constraint.write(&mut buf).ok()?;
    Some(vec![buf.into_bytes()])
}

/// Emit a constraint for the top-level `TypeVarType` template, mirroring
/// `_infer_constraints`'s first branch. Defer (None) on anything else.
fn infer_constraints_inner(template: &Type, actual: &Type, direction: i64) -> Option<Constraint> {
    // Unions must be normalized before emitting, which Rust cannot do — defer
    // so Python's `_infer_constraints` runs make_simplified_union first.
    if matches!(
        template,
        Type::UnionType { .. } | Type::TypeAliasType { .. }
    ) || matches!(actual, Type::UnionType { .. } | Type::TypeAliasType { .. })
    {
        return None;
    }
    match template {
        Type::TypeVarType { .. } => {
            // `from_type_type` now rides the CallableType wire flags
            // (wire.rs), so a CallableType/Overloaded target round-trips
            // without flipping abstract-class checks (issue #388).
            Some(Constraint {
                origin_type_var: template.clone(),
                op: direction,
                target: actual.clone(),
            })
        }
        _ => None,
    }
}

/// PyO3 entry point for the full `ConstraintBuilderVisitor` port.
///
/// Serializes the resulting constraint list (origin Type | op int | target
/// Type), or returns `None` to defer the whole call to Python. Mirrors
/// `constraints.py::infer_constraints` + `_infer_constraints` for the
/// dispatchable cases; the recursion always routes through
/// `infer_constraints_full_inner`, never through this FFI boundary (Rust
/// recursion avoids Python type_state.inferring re-entry).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_infer_constraints_full(
    resolver: &NativeTypeResolver,
    template_bytes: &[u8],
    actual_bytes: &[u8],
    direction: i64,
    _skip_neg_op: bool,
    _erase_types: bool,
) -> Option<Vec<Vec<u8>>> {
    let mut tb = ReadBuffer::new(template_bytes);
    let template = read_type(&mut tb, None).ok()?;
    let mut ab = ReadBuffer::new(actual_bytes);
    let actual = read_type(&mut ab, None).ok()?;
    let constraints = infer_constraints_full_inner(
        &template,
        &actual,
        direction,
        resolver.resolver(),
        resolver.alias_resolver(),
    )?;
    let mut out = Vec::with_capacity(constraints.len());
    for c in constraints {
        let mut b = WriteBuffer::new();
        c.write(&mut b).ok()?;
        out.push(b.into_bytes());
    }
    Some(out)
}

/// Recursive core of the ported `ConstraintBuilderVisitor`. Mirrors
/// `_infer_constraints`'s dispatch (constraints.py:470) minus the wrapper's
/// `type_state.inferring` bookkeeping, plus the visitor's per-shape
/// `visit_*` methods for the grabbable cases. Every unsupported shape
/// defers with `None` so Python runs its full visitor.
///
/// Guard order matches the Python source:
/// 1. Union/alias deferral on either side (Python normalizes via
///    make_simplified_union; Rust cannot, so it must run before the TypeVar
///    and visitor dispatch).
/// 2. TypeVar template -> single constraint.
/// 3. Any-suggestion empty result, actual-TypeVar rebinding.
/// 4. Per-shape visitor dispatch.
pub(crate) fn infer_constraints_full_inner(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    // `_infer_constraints` calls get_proper_type on both operands at the top
    // (constraints.py:548-549), so a TypeAliasType operand (reaching this core
    // via `push_inner`) expands through the alias resolver here.
    let template = get_proper_or_expand(template, aliases)?;
    let actual = get_proper_or_expand(actual, aliases)?;
    // Python re-normalizes unions via make_simplified_union at the top of every
    // `_infer_constraints` entry (constraints.py:552-558); Rust cannot re-run
    // that. Defer if either side is a union (incl. an alias expanding into one).
    if matches!(template, Type::UnionType { .. }) || matches!(actual, Type::UnionType { .. }) {
        return None;
    }
    // Otherwise Rust would emit constraints against an unsimplified target
    // (`T <: bool | int | float` instead of `T <: int | float`) and change
    // solver results.
    // Ignore suggestion-engine Any types before any constraint is emitted:
    // constraints.py:546 runs before the TypeVar branch, so a recursive
    // `(T, Any_suggestion)` pair yields `[]`, never `T <: Any` (the #337 fix).
    if let Type::AnyType {
        type_of_any,
        source_any: None,
        missing_import_name: None,
    } = &actual
    {
        if *type_of_any == ANY_SUGGESTION {
            return Some(vec![]);
        }
    }
    // Template is a TypeVar -> single constraint (direction + target).
    if let Type::TypeVarType { .. } = &template {
        // `from_type_type` rides the wire now, so CallableType/Overloaded
        // targets no longer round-trip flagless (see infer_constraints_inner).
        return Some(vec![Constraint {
            origin_type_var: template.clone(),
            op: direction,
            target: actual.clone(),
        }]);
    }
    // Actual TypeVar rebinding (constraints.py:509-515).
    if let Type::TypeVarType {
        values,
        meta_level,
        upper_bound,
        ..
    } = &actual
    {
        if values.is_empty() && *meta_level == 0 && direction == SUPERTYPE_OF {
            return infer_constraints_full_inner(
                &template,
                upper_bound,
                direction,
                resolver,
                aliases,
            );
        }
    }
    match &template {
        Type::Instance { .. } => {
            visit_instance_native(&template, &actual, direction, resolver, aliases)
        }
        Type::TupleType { .. } => {
            visit_tuple_native(&template, &actual, direction, resolver, aliases)
        }
        Type::TypedDictType { .. } => {
            visit_typeddict_native(&template, &actual, direction, resolver, aliases)
        }
        Type::TypeType { .. } => {
            visit_type_type_native(&template, &actual, direction, resolver, aliases)
        }
        Type::CallableType { .. } => {
            visit_callable_native(&template, &actual, direction, resolver, aliases)
        }
        Type::Overloaded { .. } => {
            visit_overloaded_native(&template, &actual, direction, resolver, aliases)
        }
        Type::AnyType { .. }
        | Type::NoneType
        | Type::UnboundType { .. }
        | Type::UninhabitedType { .. }
        | Type::DeletedType { .. }
        | Type::LiteralType { .. } => Some(vec![]),
        Type::ParamSpecType { .. } => Some(vec![]),
        Type::TypeVarType { .. } => {
            unreachable!("TypeVarType template handled above")
        }
        // The top-level get_proper_or_expand guarantees a non-alias template
        // here; an unresolvable alias already deferred at the top.
        Type::TypeAliasType { .. } => None,
        // UnionType is deferred at the top-level check (constraints.py:547-550),
        // these arms are unreachable. Return [] as a safety fallback.
        Type::UnionType { .. } => Some(vec![]),
        // Unsupported template shapes: defer to Python.
        Type::TypeVarTupleType { .. } | Type::UnpackType { .. } | Type::Parameters(..) => None,
        Type::ErasedType => Some(vec![]),
    }
}

/// Port of `ConstraintBuilderVisitor.visit_instance` (constraints.py:917),
/// keeping only the grabbable nominal-instance paths. Any branch that needs
/// TypeInfo graph data Rust does not snapshot (protocol members, callable
/// fallbacks, `Parameters` variance) defers with `None` so the Python
/// visitor runs the full case.
fn visit_instance_native(
    template: &Type,
    original_actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let template_args = match template {
        Type::Instance { args, .. } => args,
        _ => return None,
    };
    let template_snap = resolver.get(get_type_ref(template)?)?;
    let mut actual = original_actual;
    // Callable actuals: only defer if the template is a protocol. The
    // dominant non-protocol-vs-callable case uses the callable's fallback.
    if matches!(actual, Type::CallableType { .. }) {
        if template_snap.is_protocol {
            return None;
        }
        if let Type::CallableType { fallback, .. } = actual {
            actual = fallback;
        }
    }
    // Overloaded actual: no fallback in the dense wire transport, defer.
    if matches!(actual, Type::Overloaded { .. }) {
        return None;
    }
    // TypedDict actuals, Protocol-typed actuals: defer.
    if let Type::TypedDictType { .. } = actual {
        return None;
    }
    if matches!(actual, Type::TypeType { .. } | Type::LiteralType { .. }) {
        return None;
    }
    if let Type::Instance { type_ref, args, .. } = actual {
        let a_snap = resolver.get(type_ref)?;
        // SUBTYPE_OF direction: template is a base of actual (fast path).
        if direction == SUBTYPE_OF && template_snap.has_base(type_ref) {
            if template_snap.has_type_var_tuple_type || a_snap.has_type_var_tuple_type {
                return None;
            }
            let mapped = map_instance_to_supertype(
                get_type_ref(template)?,
                template_args,
                type_ref,
                resolver,
            )?;
            let mut res = Vec::new();
            for (tvar, mapped_arg, inst_arg) in zip3(tvars_of(a_snap), &mapped, args) {
                match tvar.2 {
                    // TypeVarType (kind 0): variance-aware.
                    0 => {
                        if tvar.1 != CONTRAVARIANT {
                            res.extend(push_inner(
                                mapped_arg.clone(),
                                inst_arg.clone(),
                                direction,
                                resolver,
                                aliases,
                            )?);
                        }
                        if tvar.1 != COVARIANT {
                            res.extend(push_inner(
                                mapped_arg.clone(),
                                inst_arg.clone(),
                                neg_op(direction),
                                resolver,
                                aliases,
                            )?);
                        }
                    }
                    // ParamSpecType (kind 1): defer (needs Parameters slicing).
                    1 => return None,
                    // TypeVarTupleType (kind 2): covariant-ish single direction.
                    2 => {
                        res.extend(push_inner(
                            mapped_arg.clone(),
                            inst_arg.clone(),
                            direction,
                            resolver,
                            aliases,
                        )?);
                    }
                    _ => {}
                }
            }
            return Some(res);
        }
        // SUPERTYPE_OF direction: actual is a base of template.
        if direction == SUPERTYPE_OF && a_snap.has_base(get_type_ref(template)?) {
            if template_snap.has_type_var_tuple_type || a_snap.has_type_var_tuple_type {
                return None;
            }
            let template_ref = get_type_ref(template)?;
            let mapped = map_instance_to_supertype(type_ref, args, template_ref, resolver)?;
            let mut res = Vec::new();
            for (tvar, template_arg, mapped_arg) in
                zip3(tvars_of(template_snap), template_args, &mapped)
            {
                match tvar.2 {
                    0 => {
                        if tvar.1 != CONTRAVARIANT {
                            res.extend(push_inner(
                                template_arg.clone(),
                                mapped_arg.clone(),
                                direction,
                                resolver,
                                aliases,
                            )?);
                        }
                        if tvar.1 != COVARIANT {
                            res.extend(push_inner(
                                template_arg.clone(),
                                mapped_arg.clone(),
                                neg_op(direction),
                                resolver,
                                aliases,
                            )?);
                        }
                    }
                    1 => return None,
                    2 => {
                        res.extend(push_inner(
                            template_arg.clone(),
                            mapped_arg.clone(),
                            SUBTYPE_OF,
                            resolver,
                            aliases,
                        )?);
                        res.extend(push_inner(
                            template_arg.clone(),
                            mapped_arg.clone(),
                            SUPERTYPE_OF,
                            resolver,
                            aliases,
                        )?);
                    }
                    _ => {}
                }
            }
            return Some(res);
        }
        // Structural-protocol branches: need `find_member` + `is_protocol_implementation`,
        // not snapshot-able, so defer.
        if template_snap.is_protocol || a_snap.is_protocol {
            return None;
        }
        // Fall through to the tail (actual is a non-protocol instance).
    }
    visit_instance_tail_native(template, actual, direction, resolver, aliases)
}

/// Port of the tail of `visit_instance` (constraints.py:1156-1205),
/// reached after the nominal branches. `actual` here is the possibly-rewritten
/// (fallback-unwrapped) actual; if it is no longer an Instance, we did not
/// fall through from the nominal block above.
fn visit_instance_tail_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let template_args = match template {
        Type::Instance { args, .. } => args,
        _ => return None,
    };
    if let Type::AnyType { .. } = actual {
        return infer_against_any_native(template_args, actual, direction, resolver, aliases);
    }
    if let Type::TupleType { items, .. } = actual {
        let template_ref = get_type_ref(template)?;
        if direction == SUPERTYPE_OF && TUPLE_LIKE_INSTANCE_NAMES.contains(&template_ref) {
            if template_args.is_empty() {
                return None;
            }
            let mut res = Vec::new();
            for item in items {
                let constrained = match item {
                    // Python: `unpacked = get_proper_type(item.type)`; an
                    // alias expands through the resolver (constraints.py:1459).
                    Type::UnpackType { typ } => match typ.as_ref() {
                        Type::TypeVarTupleType { .. } => None,
                        Type::Instance { args, .. } if get_type_ref(typ)? == "builtins.tuple" => {
                            args.first().cloned()
                        }
                        Type::TypeAliasType { .. } => {
                            let expanded = get_proper_or_expand(typ, aliases)?;
                            match expanded {
                                Type::TypeVarTupleType { .. } => None,
                                Type::Instance { args, .. }
                                    if get_type_ref(&expanded)? == "builtins.tuple" =>
                                {
                                    args.first().cloned()
                                }
                                _ => return None,
                            }
                        }
                        _ => return None,
                    },
                    other => Some(other.clone()),
                };
                if let Some(ci) = constrained {
                    res.extend(push_inner(
                        template_args.first()?.clone(),
                        ci,
                        direction,
                        resolver,
                        aliases,
                    )?);
                }
            }
            return Some(res);
        }
        // Tuple actual against a non-tuple template, SUPERTYPE_OF: parent
        // uses typeops.tuple_fallback, too complex to port. Defer.
        return None;
    }
    if let Type::TypeVarType {
        values,
        meta_level,
        upper_bound,
        ..
    } = actual
    {
        return if values.is_empty() && *meta_level == 0 {
            infer_constraints_full_inner(template, upper_bound, direction, resolver, aliases)
        } else {
            Some(vec![])
        };
    }
    if let Type::ParamSpecType { upper_bound, .. } = actual {
        return infer_constraints_full_inner(template, upper_bound, direction, resolver, aliases);
    }
    // TypeVarTupleType actual raises NotImplementedError in Python.
    if matches!(actual, Type::TypeVarTupleType { .. }) {
        return None;
    }
    Some(vec![])
}

/// Port of `visit_tuple_type` (constraints.py:1731-1835).
///
/// Covers the variadic cases the earlier port deferred: template-with-Unpack
/// against a varlength `tuple[X, ...]` instance, template-with-Unpack against
/// a `TupleType` actual (via `build_constraints_for_simple_unpack`), and a
/// template without Unpack against an actual with an internal Unpack (the
/// `Tuple[T, S, U] <: tuple[X, *tuple[Y, ...], Z]` split). The named-tuple
/// early return and the per-item/fallback tail match constraints.py:1813-1831.
fn visit_tuple_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let template_items = match template {
        Type::TupleType { items, .. } => items,
        _ => return None,
    };
    let unpack_index = find_unpack_in_list_inner(template_items);
    let is_varlength = match actual {
        Type::Instance { type_ref, .. } => resolver.get(type_ref)?.has_base("builtins.tuple"),
        _ => false,
    };
    if !(matches!(actual, Type::TupleType { .. }) || is_varlength) {
        if let Type::AnyType { .. } = actual {
            return infer_against_any_native(template_items, actual, direction, resolver, aliases);
        }
        return Some(vec![]);
    }
    // Actual is a TupleType or a varlength tuple instance.
    let mut res = Vec::new();
    if unpack_index >= 0 {
        if is_varlength {
            // Template has an Unpack and the actual is a variable-length tuple
            // (constraints.py:1742-1768). Map the actual up to builtins.tuple
            // and constrain the unpacked type against the mapped instance.
            let unpack_type = &template_items[unpack_index as usize];
            // get_proper_type(rhs): expand a top-level alias through the
            // resolver; an unresolvable alias (missing snapshot) defers.
            let unmapped_inner = match unpack_type {
                Type::UnpackType { typ } => match typ.as_ref() {
                    Type::TypeAliasType { .. } => get_proper_or_expand(typ, aliases)?,
                    other => other.clone(),
                },
                _ => return None,
            };
            let actual_args = match actual {
                Type::Instance { args, .. } => args,
                _ => return None,
            };
            let mapped = map_instance_to_supertype(
                get_type_ref(actual)?,
                actual_args,
                "builtins.tuple",
                resolver,
            )?;
            if mapped.len() != 1 {
                return None;
            }
            let mapped_instance = Type::Instance {
                type_ref: "builtins.tuple".to_string(),
                args: mapped.clone(),
                last_known_value: None,
                extra_attrs: None,
            };
            match &unmapped_inner {
                Type::TypeVarTupleType { .. } => {
                    res.push(Constraint {
                        origin_type_var: unmapped_inner.clone(),
                        op: direction,
                        target: mapped_instance.clone(),
                    });
                }
                Type::Instance { type_ref, .. } if type_ref == "builtins.tuple" => {
                    res.extend(push_inner(
                        unmapped_inner.clone(),
                        mapped_instance.clone(),
                        direction,
                        resolver,
                        aliases,
                    )?);
                }
                _ => return None,
            }
            // Constrain the non-unpack template items against the mapped arg:
            // `ti <: X` for every `ti` in Tuple[T, *Ts, S] <: tuple[X, ...].
            let mapped_arg = mapped.first()?.clone();
            for (i, ti) in template_items.iter().enumerate() {
                if i as i64 == unpack_index {
                    continue;
                }
                res.extend(push_inner(
                    ti.clone(),
                    mapped_arg.clone(),
                    direction,
                    resolver,
                    aliases,
                )?);
            }
            return Some(res);
        }
        // Template has an Unpack and the actual is a fixed TupleType: the
        // simple-unpack inference port (constraints.py:1770-1776).
        if let Type::TupleType { items: a_items, .. } = actual {
            res.extend(simple_unpack_native(
                template_items,
                a_items,
                direction,
                resolver,
                aliases,
            )?);
        } else {
            return None;
        }
        // Only the fallback-vs-fallback constraint is appended here: the
        // per-item loop is skipped because actual_items/template_items stay
        // empty after the unpack branch (constraints.py:1774-1831).
        if let (
            Type::TupleType {
                partial_fallback: t_fb,
                ..
            },
            Type::TupleType {
                partial_fallback: a_fb,
                ..
            },
        ) = (template, actual)
        {
            res.extend(push_inner(
                t_fb.as_ref().clone(),
                a_fb.as_ref().clone(),
                direction,
                resolver,
                aliases,
            )?);
        }
        return Some(res);
    }
    // Template has no Unpack.
    let (a_items, t_items) = match actual {
        Type::TupleType { items: ai, .. } => (ai, template_items),
        _ => return Some(vec![]), // varlength instance with no template unpack
    };
    // Actual tuple with an internal Unpack: the split-based inference path
    // (constraints.py:1778-1804).
    let a_unpack_index = find_unpack_in_list_inner(a_items);
    if a_unpack_index < 0 {
        return visit_tuple_tail_native(
            template,
            actual,
            t_items.to_vec(),
            a_items.to_vec(),
            direction,
            resolver,
            aliases,
        );
    }
    let a_unpack = &a_items[a_unpack_index as usize];
    let a_unpacked = match a_unpack {
        Type::UnpackType { typ } => typ.as_ref(),
        _ => return None,
    };
    if a_items.len() + 1 > t_items.len() {
        // Actual and template lengths are incompatible: no per-item
        // constraints, but the fallback tail still runs.
        return visit_tuple_tail_native(
            template,
            actual,
            Vec::new(),
            Vec::new(),
            direction,
            resolver,
            aliases,
        );
    }
    // The actual-unpack middle only constrains when the unpacked is a
    // homogenous `*tuple[X, ...]` instance; get_proper_type(a_unpack.type)
    // may expand a TypeAliasType through the resolver. An alias resolving to
    // a non-tuple target still runs the tail, matching Python.
    let a_unpacked: Option<Vec<Type>> = match a_unpacked {
        Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => Some(args.clone()),
        Type::TypeAliasType { .. } => match get_proper_or_expand(a_unpacked, aliases)? {
            Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => Some(args),
            _ => None,
        },
        Type::TupleType { .. } => return None,
        _ => None,
    };
    let a_prefix_len = a_unpack_index as usize;
    let a_suffix_len = a_items.len() - a_unpack_index as usize - 1;
    let (t_prefix, t_middle, t_suffix) =
        split_with_prefix_and_suffix_inner(t_items, a_prefix_len, a_suffix_len);
    let mut actual_items: Vec<Type> = a_items[..a_prefix_len].to_vec();
    if a_suffix_len > 0 {
        actual_items.extend_from_slice(&a_items[a_items.len() - a_suffix_len..]);
    }
    let mut template_items: Vec<Type> = t_prefix;
    template_items.extend_from_slice(&t_suffix);
    if let Some(a_mid_args) = a_unpacked {
        // Tuple[T, S, U] <: tuple[X, *tuple[Y, ...], Z]: T <: X, S <: Y,
        // U <: Z.
        let mid_arg = a_mid_args.first()?;
        for tm in t_middle {
            res.extend(push_inner(
                tm,
                mid_arg.clone(),
                direction,
                resolver,
                aliases,
            )?);
        }
    }
    // Fall through to the equal-length per-item + fallback tail; the
    // middle constraints above must be preserved.
    res.extend(visit_tuple_tail_native(
        template,
        actual,
        template_items,
        actual_items,
        direction,
        resolver,
        aliases,
    )?);
    Some(res)
}

/// Per-item + fallback tail of `visit_tuple_type` (constraints.py:1813-1831),
/// shared by the plain and actual-has-Unpack paths. The caller passes
/// pre-computed (template, actual) item lists for the non-plain paths; the
/// fallback-pair constraint is always appended.
#[allow(clippy::too_many_arguments)]
fn visit_tuple_tail_native(
    template: &Type,
    actual: &Type,
    template_items: Vec<Type>,
    actual_items: Vec<Type>,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let mut res = Vec::new();
    // Named-tuple early return (constraints.py:1814-1821): if both are named
    // tuples, constrain only the fallbacks and return immediately, skipping
    // the per-item constraints.
    if actual_items.len() == template_items.len() {
        if let (
            Type::TupleType {
                partial_fallback: t_fb,
                ..
            },
            Type::TupleType {
                partial_fallback: a_fb,
                ..
            },
        ) = (template, actual)
        {
            let is_named = match (t_fb.as_ref(), a_fb.as_ref()) {
                (Type::Instance { type_ref: t_tr, .. }, Type::Instance { type_ref: a_tr, .. }) => {
                    let t_snap = resolver.get(t_tr)?;
                    let a_snap = resolver.get(a_tr)?;
                    t_snap.is_named_tuple && a_snap.is_named_tuple
                }
                _ => false,
            };
            if is_named {
                return push_inner(
                    t_fb.as_ref().clone(),
                    a_fb.as_ref().clone(),
                    direction,
                    resolver,
                    aliases,
                );
            }
        }
    }
    // Per-item constraints for equal-length tuples.
    if actual_items.len() == template_items.len() {
        for (t_i, a_i) in template_items.iter().zip(actual_items.iter()) {
            res.extend(push_inner(
                t_i.clone(),
                a_i.clone(),
                direction,
                resolver,
                aliases,
            )?);
        }
    }
    // Always append the fallback-vs-fallback constraint.
    if let (
        Type::TupleType {
            partial_fallback: t_fb,
            ..
        },
        Type::TupleType {
            partial_fallback: a_fb,
            ..
        },
    ) = (template, actual)
    {
        res.extend(push_inner(
            t_fb.as_ref().clone(),
            a_fb.as_ref().clone(),
            direction,
            resolver,
            aliases,
        )?);
    }
    Some(res)
}

/// Port of `build_constraints_for_simple_unpack` (constraints.py:2050-2143).
///
/// Infers constraints between two lists of types with a variadic item in the
/// template. Only callable when a variadic item is present in the template;
/// defers (None) when the template has no Unpack or any constraint step hits
/// an unresolvable shape. Mirrors the Python source exactly:
///
/// 1. If the actual has no Unpack: if the template prefix+suffix exceeds the
///    actual length, return fast (TypeVarTuple -> empty TupleType target;
///    else empty); otherwise constrain template prefix/suffix against the
///    actual prefix/suffix and the template Unpack against the actual middle.
/// 2. If the actual has an Unpack: constrain the common prefix/suffix only,
///    then handle the template Unpack against the actual middle.
fn simple_unpack_native(
    template_args: &[Type],
    actual_args: &[Type],
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let template_unpack = find_unpack_in_list_inner(template_args);
    if template_unpack < 0 {
        return None;
    }
    let template_prefix = template_unpack as usize;
    let template_suffix = template_args.len() - template_prefix - 1;

    let mut t_unpack: Option<&Type> = None;
    let mut res = Vec::new();

    let actual_unpack = find_unpack_in_list_inner(actual_args);
    if actual_unpack < 0 {
        // Template has an Unpack, actual has none.
        if template_prefix + template_suffix > actual_args.len() {
            // These can't be subtypes of each-other, return fast. If the
            // unpacked is a TypeVarTuple, set it to empty to improve error
            // messages; otherwise return the empty list.
            let t_unpack_item = &template_args[template_unpack as usize];
            let inner = match t_unpack_item {
                Type::UnpackType { typ } => typ.as_ref(),
                _ => return None,
            };
            return match inner {
                Type::TypeVarTupleType { .. } => Some(vec![Constraint {
                    origin_type_var: inner.clone(),
                    op: direction,
                    target: Type::TupleType {
                        partial_fallback: inner_tvt_fallback(inner)?,
                        items: Vec::new(),
                        implicit: false,
                    },
                }]),
                _ => Some(vec![]),
            };
        }
        // Otherwise constrain the template prefix/suffix against the actual
        // and the template Unpack against the actual middle.
        let (start, middle, end) =
            split_with_prefix_and_suffix_inner(actual_args, template_prefix, template_suffix);
        for (t, a) in template_args[..template_prefix].iter().zip(start.iter()) {
            res.extend(push_inner(
                t.clone(),
                a.clone(),
                direction,
                resolver,
                aliases,
            )?);
        }
        if template_suffix > 0 {
            for (t, a) in template_args[template_args.len() - template_suffix..]
                .iter()
                .zip(end.iter())
            {
                res.extend(push_inner(
                    t.clone(),
                    a.clone(),
                    direction,
                    resolver,
                    aliases,
                )?);
            }
        }
        // Constraint(s) for the variadic item when possible. `t_unpack` is
        // set from the template item (constraints.py:2079).
        t_unpack = Some(&template_args[template_unpack as usize]);
        let inner = match t_unpack.unwrap() {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => return None,
        };
        match inner {
            Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                let inner_arg = args.first()?;
                // Homogeneous case *tuple[T, ...] <: [X, Y, Z, ...].
                res.extend(constrain_homogeneous_middle(
                    &middle, inner_arg, direction, resolver, aliases,
                )?);
            }
            Type::TypeVarTupleType { .. } => {
                let target = Type::TupleType {
                    partial_fallback: inner_tvt_fallback(inner)?,
                    items: middle,
                    implicit: false,
                };
                res.push(Constraint {
                    origin_type_var: inner.clone(),
                    op: direction,
                    target,
                });
            }
            _ => return None,
        }
        return Some(res);
    }
    // Actual has an Unpack: constrain the common prefix/suffix first, then
    // the template Unpack against the actual middle.
    let actual_prefix = actual_unpack as usize;
    let actual_suffix = actual_args.len() - actual_prefix - 1;
    let common_prefix = std::cmp::min(template_prefix, actual_prefix);
    let common_suffix = std::cmp::min(template_suffix, actual_suffix);
    if actual_prefix >= template_prefix && actual_suffix >= template_suffix {
        // Only case where we can guarantee there will be no partial overlap
        // (note partial overlap is OK for variadic tuples, handled below).
        t_unpack = Some(&template_args[template_unpack as usize]);
    }
    let (start, middle, end) =
        split_with_prefix_and_suffix_inner(actual_args, common_prefix, common_suffix);
    for (t, a) in template_args[..common_prefix].iter().zip(start.iter()) {
        res.extend(push_inner(
            t.clone(),
            a.clone(),
            direction,
            resolver,
            aliases,
        )?);
    }
    if common_suffix > 0 {
        for (t, a) in template_args[template_args.len() - common_suffix..]
            .iter()
            .zip(end.iter())
        {
            res.extend(push_inner(
                t.clone(),
                a.clone(),
                direction,
                resolver,
                aliases,
            )?);
        }
    }
    if let Some(tu) = t_unpack {
        let inner = match tu {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => return None,
        };
        match inner {
            Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                let inner_arg = args.first()?;
                // Homogeneous case *tuple[T, ...] <: [X, Y, Z, ...].
                res.extend(constrain_homogeneous_middle(
                    &middle, inner_arg, direction, resolver, aliases,
                )?);
            }
            Type::TypeVarTupleType { .. } => {
                let target = Type::TupleType {
                    partial_fallback: inner_tvt_fallback(inner)?,
                    items: middle,
                    implicit: false,
                };
                res.push(Constraint {
                    origin_type_var: inner.clone(),
                    op: direction,
                    target,
                });
            }
            _ => {}
        }
    } else if actual_unpack >= 0 {
        // A special case for a variadic tuple unpack, we simply infer
        // T <: X from Tuple[..., *tuple[T, ...], ...] <:
        // Tuple[..., *tuple[X, ...], ...] (constraints.py:2131-2142).
        let actual_unpack_type = &actual_args[actual_unpack as usize];
        let a_unpacked = match actual_unpack_type {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => return None,
        };
        // Only an *tuple[A, ...] actual unpack produces constraints. A
        // TypeVarTuple actual unpack yields nothing (constraints.py:2137).
        // get_proper_type(a_unpack_type.type) expands an alias through the
        // resolver; a non-tuple target yields nothing.
        let a_inner_args: Option<Vec<Type>> = match a_unpacked {
            Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                Some(args.clone())
            }
            Type::TypeAliasType { .. } => match get_proper_or_expand(a_unpacked, aliases)? {
                Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => Some(args),
                _ => None,
            },
            _ => None,
        };
        let t_unpack_item = &template_args[template_unpack as usize];
        let t_inner = match t_unpack_item {
            Type::UnpackType { typ } => typ.as_ref(),
            _ => return None,
        };
        let t_inner_args: Option<Vec<Type>> = match t_inner {
            Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                Some(args.clone())
            }
            Type::TypeAliasType { .. } => match get_proper_or_expand(t_inner, aliases)? {
                Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => Some(args),
                _ => None,
            },
            _ => None,
        };
        if let (Some(t_args), Some(a_args)) = (t_inner_args, a_inner_args) {
            if let (Some(t_arg), Some(a_arg)) = (t_args.first(), a_args.first()) {
                res.extend(push_inner(
                    t_arg.clone(),
                    a_arg.clone(),
                    direction,
                    resolver,
                    aliases,
                )?);
            } else {
                // Empty tuple args: Python would IndexError on args[0]; defer.
                return None;
            }
        }
    }
    Some(res)
}

/// Homogeneous `*tuple[T, ...]` middle inference, shared by both branches of
/// `build_constraints_for_simple_unpack` (constraints.py:2118-2128): a
/// non-Unpack middle item constrains `T <: X`, an internal `*tuple[A, ...]`
/// constrains `T <: A`. Non-tuple internal Unpack items are silently skipped.
fn constrain_homogeneous_middle(
    middle: &[Type],
    inner_arg: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let mut res = Vec::new();
    for a in middle {
        match a {
            Type::UnpackType { typ } => {
                // *tuple[T, ...] <: *tuple[A, ...].
                let a_inner = match typ.as_ref() {
                    Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                        args.first().cloned()
                    }
                    // get_proper_type(item.type): expand a top-level alias; a
                    // non-tuple target is silently skipped (constraints.py:2121).
                    Type::TypeAliasType { .. } => match get_proper_or_expand(typ, aliases)? {
                        Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                            args.first().cloned()
                        }
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(a_arg) = a_inner {
                    res.extend(push_inner(
                        inner_arg.clone(),
                        a_arg.clone(),
                        direction,
                        resolver,
                        aliases,
                    )?);
                }
            }
            non_unpack => {
                res.extend(push_inner(
                    inner_arg.clone(),
                    non_unpack.clone(),
                    direction,
                    resolver,
                    aliases,
                )?);
            }
        }
    }
    Some(res)
}

/// `TypeVarTupleType.tuple_fallback` clone (types.py:991-1001). The wire
/// type carries the fallback as an Instance.
fn inner_tvt_fallback(inner: &Type) -> Option<Box<Type>> {
    match inner {
        Type::TypeVarTupleType { tuple_fallback, .. } => Some(tuple_fallback.clone()),
        _ => None,
    }
}

/// Port of `visit_typeddict_type` (constraints.py:1542).
fn visit_typeddict_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let t_items = match template {
        Type::TypedDictType { items, .. } => items,
        _ => return None,
    };
    match actual {
        Type::TypedDictType { items: a_items, .. } => {
            // Python's `template.zip(actual)` iterates over template items,
            // pairing with the matching actual key (ignoring non-matching keys).
            let mut res = Vec::new();
            for (name, t_v) in t_items {
                if let Some(a_v) = a_items.iter().find(|(n, _)| n == name).map(|(_, v)| v) {
                    res.extend(push_inner(
                        t_v.clone(),
                        a_v.clone(),
                        direction,
                        resolver,
                        aliases,
                    )?);
                }
            }
            Some(res)
        }
        Type::AnyType { .. } => {
            let values: Vec<Type> = t_items.iter().map(|(_, v)| v.clone()).collect();
            infer_against_any_native(&values, actual, direction, resolver, aliases)
        }
        _ => Some(vec![]),
    }
}

/// Port of `visit_type_type` (constraints.py:1594). The callable/overloaded
/// branches need `is_type_obj()` / `get_instance_type()`; Rust only has the
/// dense wire `Type::CallableType`/`Overloaded`, so those defer. TypeType,
/// Any, and the empty case are portable.
fn visit_type_type_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let template_item = match template {
        Type::TypeType { item, .. } => item.as_ref(),
        _ => return None,
    };
    match actual {
        Type::CallableType { .. } | Type::Overloaded { .. } => None,
        Type::TypeType { item, .. } => push_inner(
            template_item.clone(),
            item.as_ref().clone(),
            direction,
            resolver,
            aliases,
        ),
        Type::AnyType { .. } => push_inner(
            template_item.clone(),
            actual.clone(),
            direction,
            resolver,
            aliases,
        ),
        _ => Some(vec![]),
    }
}

/// Port of `visit_callable_type` (constraints.py:1350-1523). Only the
/// `AnyType` actual branch is portable: it is a pure Type->Type transform
/// with no `is_subtype`/`find_member`/`type_object_type` calls. Everything
/// else (CallableType, Overloaded, TypeType, Instance actuals) needs live
/// graph data Rust does not snapshot, so it defers with `None`.
///
/// The Any branch mirrors constraints.py:1478-1493:
///  - If `param_spec is None`: `infer_against_any(arg_types, any)` +
///    `infer_constraints(ret_type, any_type, direction)`.
///  - If `param_spec is not None`: emit a `Constraint(param_spec, SUBTYPE_OF,
///    Parameters([any, any], [ARG_STAR, ARG_STAR2], [None, None]))` +
///    `infer_constraints(ret_type, any_type, direction)`. The Parameters
///    construction requires `imprecise_arg_kinds=True` (constraints.py:1489).
fn visit_callable_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let callee = match template {
        Type::CallableType {
            arg_types,
            arg_kinds,
            ret_type,
            variables,
            ..
        } => {
            // ParamSpec/TypeVarTuple variables use the deferred constraint
            // paths (constraints.py:509-512 filter_imprecise_kinds): defer.
            if variables.iter().any(|v| {
                matches!(
                    v,
                    Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. }
                )
            }) {
                return None;
            }
            // UnpackType formals use the star-unpack branch
            // (constraints.py:376-458): defer.
            if arg_types
                .iter()
                .any(|t| matches!(t, Type::UnpackType { .. }))
            {
                return None;
            }
            (arg_types, arg_kinds, ret_type)
        }
        _ => return None,
    };
    let (formal_types, formal_kinds, ret_type) = callee;
    // Only the AnyType actual branch is portable.
    if !matches!(actual, Type::AnyType { .. }) {
        return None;
    }
    // Build the derived Any: type_of_any=from_another_any, source_any=actual
    // (constraints.py:1480-1481). Mirrors AnyType(TypeOfAny.from_another_any,
    // source_any=self.actual).
    let any_type = Type::AnyType {
        type_of_any: 7, // TypeOfAny.from_another_any
        source_any: Some(Box::new(actual.clone())),
        missing_import_name: None,
    };
    // Detect ParamSpec (constraints.py:1361, types.py:2480-2497): the
    // two final params must be ARG_STAR + ARG_STAR2, and the last-but-one
    // arg type must be a ParamSpecType.
    let param_spec = detect_param_spec(formal_types, formal_kinds);
    let mut res = Vec::new();
    if param_spec.is_none() {
        res.extend(infer_against_any_native(
            formal_types,
            &any_type,
            direction,
            resolver,
            aliases,
        )?);
    } else {
        let ps = param_spec?;
        // Build Parameters([any, any], [ARG_STAR, ARG_STAR2], [None, None])
        // with imprecise_arg_kinds=True (constraints.py:1485-1491).
        let target = Type::Parameters(crate::wire::Parameters {
            arg_types: vec![any_type.clone(), any_type.clone()],
            arg_kinds: vec![2 /* ARG_STAR */, 4 /* ARG_STAR2 */],
            arg_names: vec![None, None],
            variables: Vec::new(),
            imprecise_arg_kinds: true,
        });
        res.push(Constraint {
            origin_type_var: ps,
            op: SUBTYPE_OF,
            target,
        });
    }
    res.extend(push_inner(
        ret_type.as_ref().clone(),
        any_type.clone(),
        direction,
        resolver,
        aliases,
    )?);
    Some(res)
}

/// Detect a ParamSpec on a CallableType (mirrors `CallableType.param_spec()`
/// in types.py:2480-2497). Returns the ParamSpecType (with flavor set to
/// BARE=0) if the last two arg_kinds are [ARG_STAR, ARG_STAR2] and the
/// last-but-one arg_type is a ParamSpecType, else None.
fn detect_param_spec(arg_types: &[Type], arg_kinds: &[i64]) -> Option<Type> {
    if arg_types.len() < 2 || arg_kinds.len() < 2 {
        return None;
    }
    let n = arg_kinds.len();
    // ARG_STAR = 2, ARG_STAR2 = 4 (nodes.py:2486,2490).
    if arg_kinds[n - 2] != 2 || arg_kinds[n - 1] != 4 {
        return None;
    }
    match &arg_types[arg_types.len() - 2] {
        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor: _,
            upper_bound,
            default,
        } => {
            // ParamSpecFlavor.BARE = 0 (types.py:2447).
            let _ = prefix;
            Some(Type::ParamSpecType {
                prefix: Box::new(crate::wire::Parameters {
                    arg_types: Vec::new(),
                    arg_kinds: Vec::new(),
                    arg_names: Vec::new(),
                    variables: Vec::new(),
                    imprecise_arg_kinds: false,
                }),
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                flavor: 0,
                upper_bound: upper_bound.clone(),
                default: default.clone(),
            })
        }
        _ => None,
    }
}

/// Port of `visit_overloaded` (constraints.py:1686-1694). Only the `else`
/// branch (actual is not a CallableType) is portable: it iterates over
/// `template.items` and recurses via `infer_constraints`. The
/// `find_matching_overload_items` branch (actual IS CallableType) needs
/// `is_callable_compatible`, which is not snapshot-able, so it defers.
fn visit_overloaded_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let items = match template {
        Type::Overloaded { items } => items,
        _ => return None,
    };
    // find_matching_overload_items needs is_callable_compatible: defer.
    if matches!(actual, Type::CallableType { .. }) {
        return None;
    }
    let mut res = Vec::new();
    for t in items {
        // Each item is asserted to be a CallableType (wire.rs:1002).
        if !matches!(t, Type::CallableType { .. }) {
            return None;
        }
        res.extend(push_inner(
            t.clone(),
            actual.clone(),
            direction,
            resolver,
            aliases,
        )?);
    }
    Some(res)
}

/// Port of `infer_against_any` (constraints.py:1565).
fn infer_against_any_native(
    items: &[Type],
    any_type: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    let flat = flatten_nested_tuples_inner(items, true)?;
    let mut res = Vec::new();
    for t in flat {
        match t {
            Type::UnpackType { typ } => match typ.as_ref() {
                Type::TypeVarTupleType { .. } => {
                    res.push(Constraint {
                        origin_type_var: typ.as_ref().clone(),
                        op: direction,
                        target: any_type.clone(),
                    });
                }
                _ => return None,
            },
            other => {
                res.extend(push_inner(
                    other,
                    any_type.clone(),
                    direction,
                    resolver,
                    aliases,
                )?);
            }
        }
    }
    Some(res)
}

/// Single recursion step: mirror `infer_constraints(t, other, direction)`,
/// which routes back into the full port.
fn push_inner(
    t: Type,
    other: Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    infer_constraints_full_inner(&t, &other, direction, resolver, aliases)
}

/// std-only 3-way zip (avoids an itertools dependency).
fn zip3<'a, A, B, C>(
    a: &'a [A],
    b: &'a [B],
    c: &'a [C],
) -> impl Iterator<Item = (&'a A, &'a B, &'a C)> + 'a {
    a.iter().zip(b).zip(c).map(|((x, y), z)| (x, y, z))
}

/// Borrow the `type_vars_with_variance` slice of a snapshot.
fn tvars_of(snap: &crate::typeinfo::TypeInfoSnapshot) -> &[(String, i64, i64)] {
    &snap.type_vars_with_variance
}

/// Read the `type_ref` of an Instance / TypeAliasType / TypedDict fallback.
fn get_type_ref(t: &Type) -> Option<&str> {
    match t {
        Type::Instance { type_ref, .. } | Type::TypeAliasType { type_ref, .. } => Some(type_ref),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// infer_callable_arguments_constraints (constraints.py:2032-2102)
// ---------------------------------------------------------------------------

/// `NormalizedCallableType | Parameters` arg fields, shared accessor for
/// `infer_callable_arguments_constraints`. Both `CallableType` and
/// `Parameters` carry `arg_types`/`arg_kinds`/`arg_names`; every other
/// variant defers.
#[allow(clippy::type_complexity)] // Mirrors the three field slices of the Python counterpart.
fn callable_arg_fields(t: &Type) -> Option<(&[Type], &[i64], &[Option<String>])> {
    match t {
        Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            ..
        }
        | Type::Parameters(crate::wire::Parameters {
            arg_types,
            arg_kinds,
            arg_names,
            ..
        }) => Some((arg_types, arg_kinds, arg_names)),
        _ => None,
    }
}

/// `infer_callable_arguments_constraints` (constraints.py:2032-2102).
///
/// Infers constraints between the argument types of two callables by
/// extracting the four argument-matching phases of
/// `subtypes.are_parameters_compatible` and substituting an
/// `infer_constraints` call for each subtype check. Every phase routes
/// through `infer_constraints_full_inner` (the direction-inverting
/// `infer_directed_arg_constraints` core), so constraint identity is
/// preserved via the existing `wire::Constraint` write path.
///
/// Wire layout in: template Type | actual Type | direction int.
/// Wire layout out: count (bare int) + N× [origin Type | op int | target Type].
///
/// Defers (`None`) when either side is not a CallableType/Parameters, or
/// any argument-matching step hits an unresolvable shape.
#[pyfunction]
pub(crate) fn rust_infer_callable_arguments_constraints(
    resolver: &NativeTypeResolver,
    template_bytes: &[u8],
    actual_bytes: &[u8],
    direction: i64,
) -> Option<Vec<u8>> {
    use crate::callable_compat::{
        argument_by_name, argument_by_position, callable_corresponding_argument, formal_arguments,
        kind_is_positional, kind_is_star, kw_arg, try_synthesizing_arg_from_kwarg,
        try_synthesizing_arg_from_vararg, var_arg,
    };

    let template = read_type(&mut ReadBuffer::new(template_bytes), None).ok()?;
    let actual = read_type(&mut ReadBuffer::new(actual_bytes), None).ok()?;

    // Mirror `direction == SUBTYPE_OF: left, right = template, actual` else
    // `left, right = actual, template` (constraints.py:2039-2042).
    let (left, right): (&Type, &Type) = if direction == SUBTYPE_OF {
        (&template, &actual)
    } else {
        (&actual, &template)
    };
    let (left_types, left_kinds, left_names) = callable_arg_fields(left)?;
    let (right_types, right_kinds, right_names) = callable_arg_fields(right)?;

    let left_star = var_arg(left_types, left_kinds);
    let left_star2 = kw_arg(left_types, left_kinds);
    let right_star = var_arg(right_types, right_kinds);
    let right_star2 = kw_arg(right_types, right_kinds);

    let mut res: Vec<Constraint> = Vec::new();

    // Phase 1a: compare star vs star arguments.
    if let (Some(ls), Some(rs)) = (&left_star, &right_star) {
        res.extend(infer_directed_arg_constraints_native(
            ls.typ.clone(),
            rs.typ.clone(),
            direction,
            resolver.resolver(),
            resolver.alias_resolver(),
        )?);
    }
    if let (Some(ls2), Some(rs2)) = (&left_star2, &right_star2) {
        res.extend(infer_directed_arg_constraints_native(
            ls2.typ.clone(),
            rs2.typ.clone(),
            direction,
            resolver.resolver(),
            resolver.alias_resolver(),
        )?);
    }

    // Phase 1b: compare left args with corresponding non-star right args.
    for right_arg in formal_arguments(right_types, right_kinds, right_names) {
        let left_arg =
            match callable_corresponding_argument(left_types, left_kinds, left_names, &right_arg) {
                // Python: `if left_arg is None: continue`.
                Ok(None) => continue,
                Ok(Some(a)) => a,
                // The by-name/by-pos merge case needs `meet_types`, which the
                // kernel cannot reconstruct: defer the whole call so constraints
                // are neither dropped nor changed.
                Err(_) => return None,
            };
        res.extend(infer_directed_arg_constraints_native(
            left_arg.typ.clone(),
            right_arg.typ.clone(),
            direction,
            resolver.resolver(),
            resolver.alias_resolver(),
        )?);
    }

    // Phase 1c: compare left args with right *args.
    if let Some(rs) = &right_star {
        // Python asserts non-None here; right_star Some guarantees var_arg
        // Some, so this cannot fail.
        let right_by_position = try_synthesizing_arg_from_vararg(right_types, right_kinds, None)?;
        let i = rs.pos?;
        let mut j = i;
        while j < left_kinds.len() && kind_is_positional(left_kinds[j], false) {
            let left_by_position = argument_by_position(left_types, left_kinds, left_names, j)?;
            res.extend(infer_directed_arg_constraints_native(
                left_by_position.typ.clone(),
                right_by_position.typ.clone(),
                direction,
                resolver.resolver(),
                resolver.alias_resolver(),
            )?);
            j += 1;
        }
    }

    // Phase 1d: compare left args with right **kwargs.
    if right_star2.is_some() {
        let right_names_set: std::collections::HashSet<&str> =
            right_names.iter().filter_map(|n| n.as_deref()).collect();
        let mut left_only_names: Vec<String> = Vec::new();
        for (name, kind) in left_names.iter().zip(left_kinds.iter()) {
            if let Some(name) = name {
                if !kind_is_star(*kind) && !right_names_set.contains(name.as_str()) {
                    left_only_names.push(name.clone());
                }
            }
        }
        if !left_only_names.is_empty() {
            let right_by_name = try_synthesizing_arg_from_kwarg(right_types, right_kinds, None)?;
            for name in &left_only_names {
                let left_by_name = argument_by_name(left_types, left_kinds, left_names, name)?;
                res.extend(infer_directed_arg_constraints_native(
                    left_by_name.typ.clone(),
                    right_by_name.typ.clone(),
                    direction,
                    resolver.resolver(),
                    resolver.alias_resolver(),
                )?);
            }
        }
    }

    let mut output = WriteBuffer::new();
    crate::wire::write_int_bare(&mut output, res.len() as i64).ok()?;
    for c in &res {
        write_type(&mut output, &c.origin_type_var).ok()?;
        write_int(&mut output, c.op).ok()?;
        write_type(&mut output, &c.target).ok()?;
    }
    Some(output.into_bytes())
}

/// `infer_directed_arg_constraints` core (constraints_filter.rs:310-365):
/// ParamSpec/Unpack on either side -> [], else `infer_constraints_full_inner`
/// on the inverted direction (argument contravariance). Reused by both the
/// standalone `rust_infer_directed_arg_constraints` and the callable-args
/// port so the constraint emission stays identical.
pub(crate) fn infer_directed_arg_constraints_native(
    left: Type,
    right: Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<Constraint>> {
    if matches!(left, Type::ParamSpecType { .. } | Type::UnpackType { .. })
        || matches!(right, Type::ParamSpecType { .. } | Type::UnpackType { .. })
    {
        return Some(vec![]);
    }
    let (template, actual, inferred_dir) = if direction == SUBTYPE_OF {
        (left, right, neg_op(direction))
    } else {
        (right, left, neg_op(direction))
    };
    infer_constraints_full_inner(&template, &actual, inferred_dir, resolver, aliases)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn type_var(raw_id: i64, name: &str) -> Type {
        Type::TypeVarType {
            name: name.to_string(),
            fullname: format!("mod.{}", name),
            raw_id,
            namespace: "fn".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            variance: 1, // COVARIANT
            meta_level: 1,
        }
    }

    #[test]
    fn test_constraint_roundtrip_full_typevar() {
        let tv = type_var(42, "T");
        let c = Constraint {
            origin_type_var: tv.clone(),
            op: SUPERTYPE_OF,
            target: any_type(),
        };
        let mut buf = WriteBuffer::new();
        c.write(&mut buf).unwrap();

        let binding = buf.into_bytes();
        let mut read_buf = ReadBuffer::new(&binding);
        let c2 = Constraint::read(&mut read_buf).unwrap();
        assert_eq!(c, c2);

        // The round-tripped origin must still be a TypeVarType with the
        // same identity fields.
        if let Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } = &c2.origin_type_var
        {
            assert_eq!(*raw_id, 42);
            assert_eq!(*meta_level, 1);
            assert_eq!(namespace, "fn");
        } else {
            panic!("origin_type_var not a TypeVarType");
        }
    }

    #[test]
    fn test_infer_constraints_top_level_typevar() {
        let tv = type_var(7, "T");
        let actual = any_type();
        let c = infer_constraints_inner(&tv, &actual, SUPERTYPE_OF).unwrap();
        assert_eq!(c.op, SUPERTYPE_OF);
        assert_eq!(c.origin_type_var, tv);
        assert_eq!(c.target, actual);
    }

    #[test]
    fn test_constraint_wire_bytes_ready() {
        // Round-trip through the pyfunction-shaped path: the emitted
        // blob can be read back to a Constraint.
        let tv = type_var(9, "U");
        let c = infer_constraints_inner(&tv, &any_type(), 0 /* SUBTYPE_OF */).unwrap();
        let mut buf = WriteBuffer::new();
        c.write(&mut buf).unwrap();
        let binding = buf.into_bytes();
        let mut read_buf = ReadBuffer::new(&binding);
        let c2 = Constraint::read(&mut read_buf).unwrap();
        assert_eq!(c2.origin_type_var, tv);
        assert_eq!(c2.target, any_type());
    }

    fn instance_builtins_object() -> Type {
        Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn minimal_callable(ret: Type) -> Type {
        Type::CallableType {
            fallback: Box::new(instance_builtins_object()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![type_var(1, "T")],
            arg_kinds: vec![0], // ARG_POS
            arg_names: vec![None],
            ret_type: Box::new(ret),
            name: None,
            variables: vec![type_var(1, "T")],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn test_detect_param_spec_none_for_plain_callable() {
        let types = vec![type_var(1, "T")];
        let kinds = vec![0]; // ARG_POS only
        assert!(detect_param_spec(&types, &kinds).is_none());
    }

    #[test]
    fn test_detect_param_spec_found_for_star_star2() {
        let ps = Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: Vec::new(),
                arg_kinds: Vec::new(),
                arg_names: Vec::new(),
                variables: Vec::new(),
                imprecise_arg_kinds: false,
            }),
            name: "P".to_string(),
            fullname: "mod.P".to_string(),
            raw_id: 5,
            namespace: "fn".to_string(),
            flavor: 1,
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
        };
        let types = vec![ps, any_type()];
        let kinds = vec![2, 4];
        let detected = detect_param_spec(&types, &kinds).unwrap();
        if let Type::ParamSpecType {
            name,
            raw_id,
            flavor,
            ..
        } = &detected
        {
            assert_eq!(name, "P");
            assert_eq!(*raw_id, 5);
            assert_eq!(*flavor, 0); // BARE
        } else {
            panic!("not a ParamSpecType");
        }
    }

    #[test]
    fn test_visit_callable_any_no_param_spec() {
        // Callable[[T], T] against Any -> [T <: Any, T :> Any via ret]
        let resolver = crate::typeinfo::TypeResolver::new();
        let template = minimal_callable(type_var(1, "T"));
        let res = visit_callable_native(
            &template,
            &any_type(),
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
        );
        assert!(res.is_some());
        let constraints = res.unwrap();
        // arg_types=[T] -> infer_against_any yields T :> Any.
        // ret_type=T -> infer_constraints(T, Any, SUPERTYPE_OF) yields T :> Any.
        // Total: 2 constraints (duplicates are kept).
        assert!(constraints
            .iter()
            .all(|c| matches!(c.origin_type_var, Type::TypeVarType { .. })));
        assert!(constraints
            .iter()
            .all(|c| matches!(c.target, Type::AnyType { .. })));
    }

    #[test]
    fn test_visit_callable_any_defers_non_any_actual() {
        let resolver = crate::typeinfo::TypeResolver::new();
        let template = minimal_callable(type_var(1, "T"));
        // Instance actual -> defer (not AnyType).
        assert!(visit_callable_native(
            &template,
            &instance_builtins_object(),
            SUBTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new()
        )
        .is_none());
    }

    #[test]
    fn test_visit_callable_any_defers_unpack_formal() {
        let resolver = crate::typeinfo::TypeResolver::new();
        let unpack = Type::UnpackType {
            typ: Box::new(type_var(1, "T")),
        };
        let template = Type::CallableType {
            fallback: Box::new(instance_builtins_object()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![unpack],
            arg_kinds: vec![0],
            arg_names: vec![None],
            ret_type: Box::new(type_var(1, "T")),
            name: None,
            variables: vec![type_var(1, "T")],
            type_guard: None,
            type_is: None,
        };
        assert!(visit_callable_native(
            &template,
            &any_type(),
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new()
        )
        .is_none());
    }

    #[test]
    fn test_visit_callable_any_defers_paramspec_variable() {
        let resolver = crate::typeinfo::TypeResolver::new();
        // A CallableType with a ParamSpec in variables -> defer.
        let template = Type::CallableType {
            fallback: Box::new(instance_builtins_object()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![type_var(1, "T")],
            arg_kinds: vec![0],
            arg_names: vec![None],
            ret_type: Box::new(type_var(1, "T")),
            name: None,
            variables: vec![Type::ParamSpecType {
                prefix: Box::new(crate::wire::Parameters {
                    arg_types: Vec::new(),
                    arg_kinds: Vec::new(),
                    arg_names: Vec::new(),
                    variables: Vec::new(),
                    imprecise_arg_kinds: false,
                }),
                name: "P".to_string(),
                fullname: "mod.P".to_string(),
                raw_id: 5,
                namespace: "fn".to_string(),
                flavor: 0,
                upper_bound: Box::new(any_type()),
                default: Box::new(any_type()),
            }],
            type_guard: None,
            type_is: None,
        };
        assert!(visit_callable_native(
            &template,
            &any_type(),
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new()
        )
        .is_none());
    }

    #[test]
    fn test_visit_overloaded_defers_callable_actual() {
        let resolver = crate::typeinfo::TypeResolver::new();
        let template = Type::Overloaded {
            items: vec![minimal_callable(type_var(1, "T"))],
        };
        // CallableType actual -> defer (find_matching_overload_items needed).
        let actual = minimal_callable(any_type());
        assert!(visit_overloaded_native(
            &template,
            &actual,
            SUBTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new()
        )
        .is_none());
    }

    #[test]
    fn test_visit_overloaded_any_actual_iterates_items() {
        let resolver = crate::typeinfo::TypeResolver::new();
        let template = Type::Overloaded {
            items: vec![minimal_callable(type_var(1, "T"))],
        };
        // AnyType actual -> each item (CallableType) recurses via
        // visit_callable_native, which handles Any.
        let res = visit_overloaded_native(
            &template,
            &any_type(),
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
        );
        assert!(res.is_some());
        let constraints = res.unwrap();
        assert!(constraints
            .iter()
            .all(|c| matches!(c.target, Type::AnyType { .. })));
    }

    // -- alias expansion through the resolver (issue #869) --

    fn alias_snap(fullname: &str, target: &Type) -> crate::aliases::TypeAliasSnapshot {
        let mut buf = WriteBuffer::new();
        crate::wire::write_type(&mut buf, target).expect("alias target must encode");
        crate::aliases::TypeAliasSnapshot {
            fullname: fullname.to_string(),
            target: buf.into_bytes(),
            ..Default::default()
        }
    }

    fn alias_type(type_ref: &str) -> Type {
        Type::TypeAliasType {
            args: vec![],
            type_ref: type_ref.to_string(),
        }
    }

    fn instance_int() -> Type {
        Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn test_top_level_alias_template_expands_to_typevar() {
        // A TypeAliasType template whose target is a TypeVarType expands
        // through the resolver, so the TypeVar branch emits the constraint
        // (the pre-#869 code deferred the whole call on any alias operand).
        let resolver = crate::typeinfo::TypeResolver::new();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Alias".to_string(),
            alias_snap("mod.Alias", &type_var(1, "T")),
        );
        let template = alias_type("mod.Alias");
        let res =
            infer_constraints_full_inner(&template, &any_type(), SUPERTYPE_OF, &resolver, &aliases);
        let constraints = res.expect("alias template must resolve natively");
        assert_eq!(constraints.len(), 1);
        if let Type::TypeVarType { name, .. } = &constraints[0].origin_type_var {
            assert_eq!(name, "T");
        } else {
            panic!("origin not a TypeVarType");
        }
        assert_eq!(constraints[0].op, SUPERTYPE_OF);
    }

    #[test]
    fn test_top_level_alias_actual_expands_in_typevar_target() {
        // A TypeAliasType actual expands before the TypeVar template branch,
        // so the emitted constraint target is the expanded Instance rather
        // than the (unresolvable) alias placeholder.
        let resolver = crate::typeinfo::TypeResolver::new();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Alias".to_string(),
            alias_snap("mod.Alias", &instance_int()),
        );
        let template = type_var(1, "T");
        let actual = alias_type("mod.Alias");
        let res =
            infer_constraints_full_inner(&template, &actual, SUPERTYPE_OF, &resolver, &aliases);
        let constraints = res.expect("alias actual must resolve natively");
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].target, instance_int());
    }

    #[test]
    fn test_top_level_alias_missing_snapshot_defers() {
        // An alias with no resolver snapshot still defers the whole call,
        // preserving the pre-#869 fallback to Python.
        let resolver = crate::typeinfo::TypeResolver::new();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = type_var(1, "T");
        let actual = alias_type("mod.Missing");
        assert!(infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
        )
        .is_none());
    }

    #[test]
    fn test_top_level_alias_expanding_to_union_defers() {
        // An alias whose target is a union would need make_simplified_union,
        // so it defers (the union check runs after alias expansion).
        let resolver = crate::typeinfo::TypeResolver::new();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.UAlias".to_string(),
            alias_snap(
                "mod.UAlias",
                &Type::UnionType {
                    items: vec![instance_int(), Type::NoneType],
                    uses_pep604_syntax: false,
                    can_be_true: true,
                    can_be_false: true,
                },
            ),
        );
        let template = type_var(1, "T");
        let actual = alias_type("mod.UAlias");
        assert!(infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
        )
        .is_none());
    }

    #[test]
    fn test_visit_instance_tail_alias_unpack_target_expands() {
        // visit_instance_tail_native: a tuple-item Unpack whose inner is a
        // TypeAliasType resolves to a typevar-tuple / tuple instance instead
        // of deferring on the alias placeholder.
        let resolver = crate::typeinfo::TypeResolver::new();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        // tuple[A, ...] template against *Alias inside a TupleType actual.
        aliases.insert(
            "mod.TAlias".to_string(),
            alias_snap(
                "mod.TAlias",
                &Type::Instance {
                    type_ref: "builtins.tuple".to_string(),
                    args: vec![type_var(2, "A")],
                    last_known_value: None,
                    extra_attrs: None,
                },
            ),
        );
        let template = Type::Instance {
            type_ref: "typing.Iterable".to_string(),
            args: vec![type_var(1, "T")],
            last_known_value: None,
            extra_attrs: None,
        };
        // Actual is a TupleType with an unpack element `*Alias` that resolves
        // to `tuple[A, ...]`; Python's visit_instance constrains T against
        // the alias's first arg (A) rather than skipping/deferring.
        let actual = Type::TupleType {
            partial_fallback: Box::new(instance_builtins_object()),
            items: vec![Type::UnpackType {
                typ: Box::new(alias_type("mod.TAlias")),
            }],
            implicit: false,
        };
        let res = visit_instance_tail_native(&template, &actual, SUPERTYPE_OF, &resolver, &aliases);
        assert!(res.is_some());
    }
}
