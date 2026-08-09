//! Stage 4b constraint solver (constraints.rs) for Issue #84.
//!
//! Ports core constraint creation (`infer_constraints`, `Constraint`)
//! to Rust. Stage A handles the top-level TypeVarType template case
//! (PyO3 `rust_infer_constraints`); the full port mirrors the stock
//! `ConstraintBuilderVisitor` nominal-instance paths
//! (`rust_infer_constraints_full`), deferring to Python for the protocol,
//! callable/overloaded, and variadic branches.

use pyo3::prelude::*;

use crate::subtypes::{map_instance_to_supertype, CONTRAVARIANT, COVARIANT};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::{find_unpack_in_list_inner, flatten_nested_tuples_inner};
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
            // The CallableType wire format carries only 6 flags
            // (wire.rs:539) and drops `from_type_type`; any CallableType
            // target reconstructed on the Python side decodes with
            // `from_type_type = False`. That would spuriously trigger
            // abstract-class instantiation errors (checkexpr's
            // `cannot_instantiate_abstract_class` is gated on
            // `from_type_type`). Defer so Python's `_infer_constraints`
            // emits the constraint against the original object.
            // Mirrors the documented setops.rs join limitation.
            if matches!(actual, Type::CallableType { .. } | Type::Overloaded { .. }) {
                return None;
            }
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
    let constraints =
        infer_constraints_full_inner(&template, &actual, direction, resolver.resolver())?;
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
) -> Option<Vec<Constraint>> {
    // Python re-normalizes unions at the top of every `_infer_constraints`
    // entry (constraints.py:547-550) and resolves type aliases via
    // get_proper_type. Rust cannot re-run make_simplified_union, so defer any
    // union or alias on either side before the TypeVar/visitor dispatch:
    // Rust would otherwise emit a constraint against an unsimplified target
    // (`T <: bool | int | float` instead of `T <: int | float`) and change
    // solver results. The old union defer below this point ran too late,
    // because the TypeVar branch above returns first.
    if matches!(
        template,
        Type::UnionType { .. } | Type::TypeAliasType { .. }
    ) || matches!(actual, Type::UnionType { .. } | Type::TypeAliasType { .. })
    {
        return None;
    }
    // Ignore suggestion-engine Any types before any constraint is emitted:
    // constraints.py:546 runs before the TypeVar branch, so a recursive
    // `(T, Any_suggestion)` pair yields `[]`, never `T <: Any` (the #337 fix).
    if let Type::AnyType {
        type_of_any,
        source_any: None,
        missing_import_name: None,
    } = actual
    {
        if *type_of_any == ANY_SUGGESTION {
            return Some(vec![]);
        }
    }
    // Template is a TypeVar -> single constraint (direction + target).
    if let Type::TypeVarType { .. } = template {
        // Same `from_type_type` wire loss as `infer_constraints_inner`:
        // CallableType/Overloaded targets round-trip flagless, which would
        // spuriously flip abstract-class checks. Defer to Python's visitor.
        if matches!(actual, Type::CallableType { .. } | Type::Overloaded { .. }) {
            return None;
        }
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
    } = actual
    {
        if values.is_empty() && *meta_level == 0 && direction == SUPERTYPE_OF {
            return infer_constraints_full_inner(template, upper_bound, direction, resolver);
        }
    }
    match template {
        Type::Instance { .. } => visit_instance_native(template, actual, direction, resolver),
        Type::TupleType { .. } => visit_tuple_native(template, actual, direction, resolver),
        Type::TypedDictType { .. } => visit_typeddict_native(template, actual, direction, resolver),
        Type::TypeType { .. } => visit_type_type_native(template, actual, direction, resolver),
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
        Type::UnionType { .. } | Type::TypeAliasType { .. } => None,
        // Unsupported template shapes: defer to Python.
        Type::CallableType { .. }
        | Type::Overloaded { .. }
        | Type::TypeVarTupleType { .. }
        | Type::UnpackType { .. }
        | Type::Parameters(..) => None,
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
                            )?);
                        }
                        if tvar.1 != COVARIANT {
                            res.extend(push_inner(
                                mapped_arg.clone(),
                                inst_arg.clone(),
                                neg_op(direction),
                                resolver,
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
                            )?);
                        }
                        if tvar.1 != COVARIANT {
                            res.extend(push_inner(
                                template_arg.clone(),
                                mapped_arg.clone(),
                                neg_op(direction),
                                resolver,
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
                        )?);
                        res.extend(push_inner(
                            template_arg.clone(),
                            mapped_arg.clone(),
                            SUPERTYPE_OF,
                            resolver,
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
    visit_instance_tail_native(template, actual, direction, resolver)
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
) -> Option<Vec<Constraint>> {
    let template_args = match template {
        Type::Instance { args, .. } => args,
        _ => return None,
    };
    if let Type::AnyType { .. } = actual {
        return infer_against_any_native(template_args, actual, direction, resolver);
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
                    Type::UnpackType { typ } => match typ.as_ref() {
                        Type::TypeVarTupleType { .. } => None,
                        Type::Instance { args, .. } if get_type_ref(typ)? == "builtins.tuple" => {
                            args.first().cloned()
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
            infer_constraints_full_inner(template, upper_bound, direction, resolver)
        } else {
            Some(vec![])
        };
    }
    if let Type::ParamSpecType { upper_bound, .. } = actual {
        return infer_constraints_full_inner(template, upper_bound, direction, resolver);
    }
    // TypeVarTupleType actual raises NotImplementedError in Python.
    if matches!(actual, Type::TypeVarTupleType { .. }) {
        return None;
    }
    Some(vec![])
}

/// Port of `visit_tuple_type` (constraints.py:1436).
fn visit_tuple_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
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
            return infer_against_any_native(template_items, actual, direction, resolver);
        }
        return Some(vec![]);
    }
    // Actual is a TupleType or a varlength tuple instance.
    let mut res = Vec::new();
    if unpack_index >= 0 {
        // Template has an Unpack; the exact handling needs the variadic mapping
        // machinery. Defer.
        return None;
    }
    let (a_items, t_items) = match actual {
        Type::TupleType { items: ai, .. } => (ai, template_items),
        _ => return Some(vec![]), // varlength instance with no template unpack
    };
    // Actual tuple with an internal Unpack: the simple-unpack inference path.
    if find_unpack_in_list_inner(a_items) >= 0 {
        return None;
    }

    // Named-tuple early return (constraints.py:1518-1526): if both are named
    // tuples, constrain only the fallbacks and return immediately, skipping
    // the per-item constraints.
    if a_items.len() == t_items.len() {
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
                res = push_inner(
                    t_fb.as_ref().clone(),
                    a_fb.as_ref().clone(),
                    direction,
                    resolver,
                )?;
                return Some(res);
            }
        }
    }

    // Per-item constraints for equal-length tuples.
    if a_items.len() == t_items.len() {
        for (t_i, a_i) in t_items.iter().zip(a_items.iter()) {
            res.extend(push_inner(t_i.clone(), a_i.clone(), direction, resolver)?);
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
        )?);
    }
    Some(res)
}

/// Port of `visit_typeddict_type` (constraints.py:1542).
fn visit_typeddict_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
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
                    res.extend(push_inner(t_v.clone(), a_v.clone(), direction, resolver)?);
                }
            }
            Some(res)
        }
        Type::AnyType { .. } => {
            let values: Vec<Type> = t_items.iter().map(|(_, v)| v.clone()).collect();
            infer_against_any_native(&values, actual, direction, resolver)
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
        ),
        Type::AnyType { .. } => {
            push_inner(template_item.clone(), actual.clone(), direction, resolver)
        }
        _ => Some(vec![]),
    }
}

/// Port of `infer_against_any` (constraints.py:1565).
fn infer_against_any_native(
    items: &[Type],
    any_type: &Type,
    direction: i64,
    resolver: &TypeResolver,
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
                res.extend(push_inner(other, any_type.clone(), direction, resolver)?);
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
) -> Option<Vec<Constraint>> {
    infer_constraints_full_inner(&t, &other, direction, resolver)
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
            raw_id: raw_id,
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
}
