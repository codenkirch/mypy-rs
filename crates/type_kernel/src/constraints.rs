//! Stage 4b constraint solver (constraints.rs) for Issue #84.
//!
//! Ports core constraint creation (`infer_constraints`, `Constraint`)
//! to Rust. Stage A handles the top-level TypeVarType template case
//! (PyO3 `rust_infer_constraints`); the full port mirrors the stock
//! `ConstraintBuilderVisitor` nominal-instance paths
//! (`rust_infer_constraints_full`), deferring to Python for the protocol,
//! callable/overloaded, and variadic branches.

use pyo3::prelude::*;

use std::cell::{Cell, RefCell};

use crate::argapprox::make_normalized_type_type;
use crate::checkexpr_functions::get_proper_or_expand;
use crate::constraints_select::{any_constraints_inner, ConstraintRep};
use crate::erase_typevars::erase_typevars_inner;
use crate::setops::{make_simplified_union, union_make_union};
use crate::subtypes::{
    is_subtype, map_instance_to_supertype, SubtypeContext, CONTRAVARIANT, COVARIANT,
};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::{
    find_unpack_in_list_inner, flatten_nested_tuples_inner, has_recursive_types_inner,
    split_with_prefix_and_suffix_inner, type_contains_erased,
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
///
/// `extra_tvars` is the @1427 kernel channel mirroring Python's
/// `Constraint.extra_tvars` (constraints.py:597): vars of a generic
/// actual callable attached by the polymorphic reverse-inference frame
/// (constraints.py:1810-1812). Rust-internal only; `write`/`read` stay
/// 3-field, so a decoded constraint carries an empty list.
#[derive(Debug, Clone)]
pub(crate) struct Constraint {
    pub origin_type_var: Type,
    pub op: i64, // SUBTYPE_OF or SUPERTYPE_OF
    pub target: Type,
    pub extra_tvars: Vec<Type>,
}

// Python `Constraint.__eq__` compares exactly (type_var, op, target)
// (constraints.py:608-611): `extra_tvars` must stay out of equality or the
// remove-set filters in solve would diverge whenever a constraint happens
// to carry extras.
impl PartialEq for Constraint {
    fn eq(&self, other: &Self) -> bool {
        self.origin_type_var == other.origin_type_var
            && self.op == other.op
            && self.target == other.target
    }
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
            extra_tvars: Vec::new(),
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
                extra_tvars: Vec::new(),
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------

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
    skip_neg_op: bool,
    erase_types: bool,
    strict_optional: bool,
    infer_polymorphic: bool,
) -> Option<Vec<Vec<u8>>> {
    let mut tb = ReadBuffer::new(template_bytes);
    let template = match read_type(&mut tb, None).ok() {
        Some(t) => t,
        None => {
            return None;
        }
    };
    let mut ab = ReadBuffer::new(actual_bytes);
    let actual = match read_type(&mut ab, None).ok() {
        Some(t) => t,
        None => {
            return None;
        }
    };
    // constraints.py:954 passes the ambient `type_state.infer_polymorphic`
    // faithfully; the tri-state mode hands it down through every nested
    // frame so the callable-vs-callable reverse gate sees what Python sees.
    let _poly = PolyModeGuard::install(infer_polymorphic);
    let constraints = infer_constraints_full_inner(
        &template,
        &actual,
        direction,
        resolver.resolver(),
        resolver.alias_resolver(),
        strict_optional,
        skip_neg_op,
        erase_types,
    )?;
    // The wire format stays 3-field: an extras-carrying constraint would
    // lose its `extra_tvars` in serialization (#1171), so defer the whole
    // call to Python instead (the Python body re-emits them as objects).
    if constraints.iter().any(|c| !c.extra_tvars.is_empty()) {
        return None;
    }
    let mut out = Vec::with_capacity(constraints.len());
    for c in constraints {
        let mut b = WriteBuffer::new();
        match c.write(&mut b) {
            Ok(()) => {}
            Err(_) => {
                return None;
            }
        }
        out.push(b.into_bytes());
    }
    Some(out)
}

/// Recursive core of the ported `ConstraintBuilderVisitor`. Mirrors
/// `_infer_constraints`'s dispatch (constraints.py:470) plus the wrapper's
/// `type_state.inferring` cycle guard, plus the visitor's per-shape
/// `visit_*` methods for the grabbable cases. Every unsupported shape
/// defers with `None` so Python runs its full visitor.
///
/// Guard order matches the Python source:
/// 1. Repeated (template, actual) pair -> no constraints (the
///    `type_state.inferring` mirror, constraints.py:729-731; without it a
///    self-recursive alias template never terminates, issue #1133).
/// 2. Proper-form expansion, union normalization (make_simplified_union),
///    suggestion-Any dismissal, the type[...]-union fixup, TypeVar
///    template emit, and actual-TypeVar rebinding (constraints.py:815-879).
/// 3. The four union branches (constraints.py:884-931).
/// 4. Per-shape visitor dispatch.
#[allow(clippy::too_many_arguments)]
pub(crate) fn infer_constraints_full_inner(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    skip_neg_op: bool,
    // The `visit_type_type` callable/overloaded arms consult this
    // (constraints.py:2050,2057). Internal recursion passes `true` like the
    // Python wrapper default (constraints.py:802), entry carries caller flag.
    erase_types: bool,
) -> Option<Vec<Constraint>> {
    // Mirror of constraints.py:729-753 (type_state.inferring, #1133): a
    // repeated (template, actual) pair yields no constraints; alias-bearing
    // templates are the ones that can regenerate their own shape on the wire.
    if INFERRING.with(|s| {
        s.borrow()
            .iter()
            .rev()
            .any(|(t, a)| t == template && a == actual)
    }) {
        return Some(vec![]);
    }
    let _guard = if crate::visitor::type_contains_alias(template) {
        INFERRING.with(|s| s.borrow_mut().push((template.clone(), actual.clone())));
        Some(InferringGuard)
    } else {
        None
    };
    infer_constraints_dispatch(
        template,
        actual,
        direction,
        resolver,
        aliases,
        strict_optional,
        skip_neg_op,
        erase_types,
    )
}

thread_local! {
    /// Mirror of `constraints.py` `type_state.inferring`: in-progress
    /// (template, actual) pairs for alias-bearing templates.
    static INFERRING: RefCell<Vec<(Type, Type)>> = const { RefCell::new(Vec::new()) };
    /// Tri-state mirror of `type_state.infer_polymorphic`
    /// (constraints.py:1711-1724): `0` = Unknown (no entry installed the
    /// Python flag, so the polymorphic reverse-inference frame defers the
    /// whole callable-vs-callable call, pre-@1427 behavior), `1` =
    /// Known(true) (the reverse frame fires and attaches `extra_tvars`),
    /// `2` = Known(false) (the fold is skipped, no extras). Only the seams
    /// that faithfully see the Python flag install Known
    /// (`rust_infer_constraints_full` = Known(param),
    /// `unify_generic_callable_core` = Known(true), accepted divergence
    /// under `old_type_inference`); every other entry point inherits
    /// Unknown so unverifiable mode defers to Python instead of guessing.
    static INFER_POLY: Cell<u8> = const { Cell::new(0) };
}

/// RAII tri-state install for [`INFER_POLY`] (panic-safe; the mode is
/// per-thread, so a guard must never outlive the frame that installed it).
pub(crate) struct PolyModeGuard;

impl PolyModeGuard {
    pub(crate) fn install(value: bool) -> Self {
        INFER_POLY.with(|c| c.set(if value { 1 } else { 2 }));
        Self
    }
}

impl Drop for PolyModeGuard {
    fn drop(&mut self) {
        INFER_POLY.with(|c| c.set(0));
    }
}

/// The installed tri-state (0/1/2) visible to the current frame.
fn infer_poly_mode() -> u8 {
    INFER_POLY.with(Cell::get)
}

/// RAII pop for [`INFERRING`] (panic-safe, mirrors the wrapper's
/// push/pop bracket around `_infer_constraints`).
struct InferringGuard;

impl Drop for InferringGuard {
    fn drop(&mut self) {
        INFERRING.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn infer_constraints_dispatch(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    skip_neg_op: bool,
    erase_types: bool,
) -> Option<Vec<Constraint>> {
    // `_infer_constraints` (constraints.py:815-943), top to bottom. `orig`
    // mirrors Python's `orig_template` (constraints.py:818): branch b
    // recurses against it raw, skipping the unwrap/normalize (897).
    let orig = template;
    let t = get_proper_or_expand(template, aliases)?;
    let a = get_proper_or_expand(actual, aliases)?;
    // constraints.py:825-828: "Type inference shouldn't be affected by
    // whether union types have been simplified" — normalize each union
    // operand (keep_erased=True).
    let (t, a, normalized_t, normalized_a) =
        normalize_union_operands(t, a, aliases, resolver, strict_optional)?;
    // keep_erased=True preserves ErasedType items, which Python's reader
    // cannot decode (tag 122): any constraint we emit against them fails
    // the shim's decode and wastes the native call.
    if (normalized_t && type_contains_erased(&t)) || (normalized_a && type_contains_erased(&a)) {
        return None;
    }
    // Ignore suggestion-engine Any types before any constraint is emitted:
    // constraints.py:835 runs before the TypeVar branch, so a recursive
    // `(T, Any_suggestion)` pair yields `[]`, never `T <: Any` (the #337 fix).
    if let Type::AnyType {
        type_of_any,
        source_any: None,
        missing_import_name: None,
    } = &a
    {
        if *type_of_any == ANY_SUGGESTION {
            return Some(vec![]);
        }
    }
    // constraints.py:843-847: `type[A | B]` is represented internally as
    // `type[A] | type[B]`; unwrap both sides when both are type[...]-ish.
    let t_is_tt = is_type_type_dispatch(&t, aliases)?;
    let a_is_tt = is_type_type_dispatch(&a, aliases)?;
    let type_type_unwrapped = t_is_tt && a_is_tt;
    let (t, a) = if type_type_unwrapped {
        let ut = unwrap_type_type_dispatch(&t, aliases)?;
        let ua = unwrap_type_type_dispatch(&a, aliases)?;
        (ut, ua)
    } else {
        (t, a)
    };
    // Template is a TypeVar -> single constraint (direction + target). This
    // runs on the unwrapped template (Python order: unwrap at 843, emit at
    // 858), so a tvar inside `type[T]` emits directly.
    if let Type::TypeVarType { .. } = &t {
        // `from_type_type` rides the wire now, so CallableType/Overloaded
        // targets no longer round-trip flagless (see infer_constraints_inner).
        return Some(vec![Constraint {
            origin_type_var: t.clone(),
            op: direction,
            target: a.clone(),
            extra_tvars: Vec::new(),
        }]);
    }
    // Actual TypeVar rebinding (constraints.py:866-879). Skipped when the
    // template is a union containing a type var (876-878): the union's own
    // tvar items must see the actual's shape, not its bound.
    let a = if let Type::TypeVarType {
        values,
        meta_level,
        upper_bound,
        ..
    } = &a
    {
        if values.is_empty() && *meta_level == 0 && direction == SUPERTYPE_OF {
            let template_union_has_tvar = match &t {
                Type::UnionType { items, .. } => items
                    .iter()
                    .any(|it| matches!(it, Type::TypeVarType { .. })),
                _ => false,
            };
            if !template_union_has_tvar {
                get_proper_or_expand(upper_bound, aliases)?
            } else {
                a.clone()
            }
        } else {
            a.clone()
        }
    } else {
        a
    };
    // Union branches (constraints.py:884-931). Direction splits them: for
    // SUBTYPE_OF the template union is handled first (a), then the actual
    // union (c); SUPERTYPE_OF is actual (b) first, then template (d).
    if direction == SUBTYPE_OF {
        if let Type::UnionType { items, .. } = &t {
            // Branch a (884-888): every item of the template union must be
            // a subtype of the actual; concatenate the per-item constraints.
            let mut res = Vec::new();
            for item in items {
                let cs = infer_constraints_full_inner(
                    item,
                    &a,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
                    false,
                    // Python branch a uses the wrapper (constraints.py:917).
                    true,
                )?;
                res.extend(cs);
            }
            return Some(res);
        }
        if let Type::UnionType { items, .. } = &a {
            // Branch c (905-915): find a union item the template is a
            // subtype of, inferring eagerly.
            let mut options: Vec<Option<Vec<Constraint>>> = Vec::with_capacity(items.len());
            for item in items {
                let inner = infer_constraints_if_possible_inner(
                    &t,
                    item,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
                )?;
                options.push(inner);
            }
            return run_any_constraints(options, true, strict_optional, resolver);
        }
    } else {
        if let Type::UnionType { items, .. } = &a {
            // Branch b (889-898): a supertype of a union is a supertype of
            // some item, so recurse item-wise against the raw template.
            let mut res = Vec::new();
            for item in items {
                let item = if type_type_unwrapped {
                    make_normalized_type_type(item.clone(), false)?
                } else {
                    item.clone()
                };
                let cs = infer_constraints_full_inner(
                    orig,
                    &item,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
                    false,
                    // Python branch b uses the wrapper (constraints.py:927).
                    true,
                )?;
                res.extend(cs);
            }
            return Some(res);
        }
        if let Type::UnionType { items, .. } = &t {
            // Branch d (916-931): with a union template we accept leaving
            // some type variables indeterminate; when every option comes
            // back empty, try the recursive-union split before giving up.
            let eager = matches!(a, Type::AnyType { .. });
            let mut options: Vec<Option<Vec<Constraint>>> = Vec::with_capacity(items.len());
            for item in items {
                let inner = infer_constraints_if_possible_inner(
                    item,
                    &a,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
                )?;
                options.push(inner);
            }
            let result = run_any_constraints(options, eager, strict_optional, resolver)?;
            if !result.is_empty() {
                return Some(result);
            }
            let t_rec = has_recursive_types_inner(&t);
            let a_rec = has_recursive_types_inner(&a);
            if t_rec && !a_rec {
                return handle_recursive_union_inner(
                    &t,
                    &a,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
                );
            }
            return Some(vec![]);
        }
    }
    match &t {
        Type::Instance { .. } => {
            visit_instance_native(&t, &a, direction, resolver, aliases, strict_optional)
        }
        Type::TupleType { .. } => {
            visit_tuple_native(&t, &a, direction, resolver, aliases, strict_optional)
        }
        Type::TypedDictType { .. } => {
            visit_typeddict_native(&t, &a, direction, resolver, aliases, strict_optional)
        }
        Type::TypeType { .. } => visit_type_type_native(
            &t,
            &a,
            direction,
            resolver,
            aliases,
            strict_optional,
            erase_types,
        ),
        Type::CallableType { .. } => visit_callable_native(
            &t,
            &a,
            direction,
            resolver,
            aliases,
            strict_optional,
            skip_neg_op,
        ),
        Type::Overloaded { .. } => {
            visit_overloaded_native(&t, &a, direction, resolver, aliases, strict_optional)
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
        // Union templates/actuals were consumed by the branches above for
        // both directions; unreachable safety fallback.
        Type::UnionType { .. } => Some(vec![]),
        // Unsupported template shapes: defer to Python.
        Type::TypeVarTupleType { .. } | Type::UnpackType { .. } | Type::Parameters(..) => None,
        Type::ErasedType => Some(vec![]),
    }
}

/// constraints.py:825-828: normalize each union operand through
/// `make_simplified_union(..., keep_erased=True)`. Per-item alias
/// expansion mirrors `flatten_nested_unions`'s `get_proper_type` walk
/// (the typeops.rs wrapper convention). Returns the normalized pair plus
/// flags saying whether normalization happened; `None` defers
/// (`union-normalize-fail`).
#[allow(clippy::type_complexity)]
fn normalize_union_operands(
    t: Type,
    a: Type,
    aliases: &crate::aliases::TypeAliasResolver,
    resolver: &TypeResolver,
    strict_optional: bool,
) -> Option<(Type, Type, bool, bool)> {
    // Matches the `_remove_redundant_union_items` convention in typeops.rs:
    // proper_subtype + ignore_promotions so Any items are not absorbed;
    // strict_optional follows the build state.
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    let normalize = |operand: Type| -> Option<(Type, bool)> {
        let Type::UnionType { items, .. } = &operand else {
            return Some((operand, false));
        };
        let mut expanded = Vec::with_capacity(items.len());
        for item in items {
            expanded.push(get_proper_or_expand(item, aliases)?);
        }
        let simplified = make_simplified_union(&expanded, &ctx, resolver, true, true)?;
        Some((simplified, true))
    };
    let (t, normalized_t) = normalize(t)?;
    let (a, normalized_a) = normalize(a)?;
    Some((t, a, normalized_t, normalized_a))
}

/// `_is_type_type` (constraints.py:946-963): `type[...]` or a union
/// thereof. Union items expand through the alias resolver like the
/// item-wise `get_proper_type` walk; an unresolvable item defers.
fn is_type_type_dispatch(tp: &Type, aliases: &crate::aliases::TypeAliasResolver) -> Option<bool> {
    match tp {
        Type::TypeType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            for item in items {
                let expanded = get_proper_or_expand(item, aliases)?;
                if !matches!(expanded, Type::TypeType { .. }) {
                    return Some(false);
                }
            }
            Some(true)
        }
        _ => Some(false),
    }
}

/// `_unwrap_type_type` (constraints.py:966-977): extract the inner type
/// from a `type[...]` expression or a union thereof.
fn unwrap_type_type_dispatch(
    tp: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Type> {
    match tp {
        Type::TypeType { item, .. } => Some((**item).clone()),
        Type::UnionType { items, .. } => {
            let mut inner = Vec::with_capacity(items.len());
            for item in items {
                match get_proper_or_expand(item, aliases)? {
                    Type::TypeType { item: it, .. } => inner.push(*it),
                    _ => return None,
                }
            }
            Some(union_make_union(inner))
        }
        _ => None,
    }
}

/// `infer_constraints_if_possible` (constraints.py:980-1002): the
/// satisfiability gates run through `erase_typevars` + `is_subtype`
/// before the recursive inference.
///
/// Outer `None` = defer (an undecidable gate, or the recursion itself
/// deferred); inner `None` = unsatisfiable, per Python.
#[allow(clippy::type_complexity)]
fn infer_constraints_if_possible_inner(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
) -> Option<Option<Vec<Constraint>>> {
    // erasetype.py:332 — TypeVarEraser replaces erased type vars with
    // AnyType(TypeOfAny.special_form) == 6; only the gate consumes this.
    let any_repl = Type::AnyType {
        type_of_any: 6,
        source_any: None,
        missing_import_name: None,
    };
    // Matches the pure-Python `is_subtype` defaults (subtypes.py:260-270):
    // all flags False except strict_optional.
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    if direction == SUBTYPE_OF {
        let erased = erase_typevars_inner(template, None, &any_repl)?;
        let gate = is_subtype(&erased, actual, &ctx, resolver)?;
        if !gate {
            return Some(None);
        }
    }
    if direction == SUPERTYPE_OF {
        let erased = erase_typevars_inner(template, None, &any_repl)?;
        let gate = is_subtype(actual, &erased, &ctx, resolver)?;
        if !gate {
            return Some(None);
        }
        // constraints.py:994-1001: not caught by the gate above because
        // erase_typevars turns the type var itself into Any.
        if let Type::TypeVarType { upper_bound, .. } = template {
            let erased_ub = erase_typevars_inner(upper_bound, None, &any_repl)?;
            let ub_gate = is_subtype(actual, &erased_ub, &ctx, resolver)?;
            if !ub_gate {
                return Some(None);
            }
        }
    }
    infer_constraints_full_inner(
        template,
        actual,
        direction,
        resolver,
        aliases,
        strict_optional,
        false,
        // `infer_constraints_if_possible` tail (constraints.py:1032).
        true,
    )
    .map(Some)
}

/// Wire conversion between the kernel `Constraint` and the
/// `any_constraints_inner` option representation.
fn constraint_to_rep(c: Constraint) -> ConstraintRep {
    ConstraintRep {
        origin: c.origin_type_var,
        op: c.op,
        target: c.target,
    }
}

fn rep_to_constraint(r: ConstraintRep) -> Constraint {
    Constraint {
        origin_type_var: r.origin,
        op: r.op,
        target: r.target,
        extra_tvars: Vec::new(),
    }
}

/// `any_constraints` body (constraints.py:1070+): convert the options,
/// run the shared kernel fold, convert back. `None` defers the branch to
/// Python (`any-constraints`).
fn run_any_constraints(
    options: Vec<Option<Vec<Constraint>>>,
    eager: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Constraint>> {
    // ConstraintRep is 3-field and drops `extra_tvars`; an option list
    // carrying extras would silently lose them, so defer instead.
    if options.iter().any(|opt| {
        opt.as_ref()
            .is_some_and(|cs| cs.iter().any(|c| !c.extra_tvars.is_empty()))
    }) {
        return None;
    }
    let reps: Vec<Option<Vec<ConstraintRep>>> = options
        .into_iter()
        .map(|opt| opt.map(|cs| cs.into_iter().map(constraint_to_rep).collect()))
        .collect();
    // filter_satisfiable uses the pure-Python `is_subtype` defaults.
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    let out = any_constraints_inner(&reps, eager, &ctx, resolver)?;
    Some(out.into_iter().map(rep_to_constraint).collect())
}

/// `handle_recursive_union` (constraints.py:1054-1067): split the
/// template union into non-type-var and type-var parts and try inferring
/// sequentially; the first non-empty result wins.
fn handle_recursive_union_inner(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    let items = match template {
        Type::UnionType { items, .. } => items,
        _ => return None,
    };
    let non_type_var: Vec<Type> = items
        .iter()
        .filter(|i| !matches!(i, Type::TypeVarType { .. }))
        .cloned()
        .collect();
    let type_var: Vec<Type> = items
        .iter()
        .filter(|i| matches!(i, Type::TypeVarType { .. }))
        .cloned()
        .collect();
    // Python calls make_union even for an empty part (UninhabitedType,
    // which infers no constraints) — union_make_union mirrors that.
    let first = infer_constraints_full_inner(
        &union_make_union(non_type_var),
        actual,
        direction,
        resolver,
        aliases,
        strict_optional,
        false,
        // handle_recursive_union mirror (constraints.py:1090).
        true,
    )?;
    if !first.is_empty() {
        return Some(first);
    }
    let second = infer_constraints_full_inner(
        &union_make_union(type_var),
        actual,
        direction,
        resolver,
        aliases,
        strict_optional,
        false,
        true,
    )?;
    if !second.is_empty() {
        return Some(second);
    }
    Some(vec![])
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
    strict_optional: bool,
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
    if matches!(actual, Type::LiteralType { .. }) {
        // constraints.py:1423: LiteralType unwraps to its fallback and takes
        // the nominal-Instance path; not ported this round.
        return None;
    }
    if matches!(actual, Type::TypeType { .. }) {
        // constraints.py:1400-1417: a protocol template extends the protocol
        // via members/its metaclass (deferred); any other template leaves
        // `actual` a TypeType and falls through the tail to `return []`.
        if template_snap.is_protocol {
            return None;
        }
        return Some(vec![]);
    }
    if let Type::Instance { type_ref, args, .. } = actual {
        let a_snap = resolver.get(type_ref).map_or_else(|| None, Some)?;
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
            );
            let mapped = mapped?;
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
                                strict_optional,
                            )?);
                        }
                        if tvar.1 != COVARIANT {
                            res.extend(push_inner(
                                mapped_arg.clone(),
                                inst_arg.clone(),
                                neg_op(direction),
                                resolver,
                                aliases,
                                strict_optional,
                            )?);
                        }
                    }
                    // ParamSpecType (kind 1): defer (needs Parameters slicing).
                    // ParamSpecType (kind 1): defer (needs Parameters slicing).
                    1 => {
                        return None;
                    }
                    // TypeVarTupleType (kind 2): covariant-ish single direction.
                    2 => {
                        res.extend(push_inner(
                            mapped_arg.clone(),
                            inst_arg.clone(),
                            direction,
                            resolver,
                            aliases,
                            strict_optional,
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
            let mapped = map_instance_to_supertype(type_ref, args, template_ref, resolver);
            let mapped = mapped?;
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
                                strict_optional,
                            )?);
                        }
                        if tvar.1 != COVARIANT {
                            res.extend(push_inner(
                                template_arg.clone(),
                                mapped_arg.clone(),
                                neg_op(direction),
                                resolver,
                                aliases,
                                strict_optional,
                            )?);
                        }
                    }
                    1 => {
                        return None;
                    }
                    2 => {
                        res.extend(push_inner(
                            template_arg.clone(),
                            mapped_arg.clone(),
                            SUBTYPE_OF,
                            resolver,
                            aliases,
                            strict_optional,
                        )?);
                        res.extend(push_inner(
                            template_arg.clone(),
                            mapped_arg.clone(),
                            SUPERTYPE_OF,
                            resolver,
                            aliases,
                            strict_optional,
                        )?);
                    }
                    _ => {}
                }
            }
            return Some(res);
        }
        // Structural-protocol branch (constraints.py:1540-1581): the
        // SUPERTYPE_OF arm with a non-protocol instance is decided in
        // Rust; protocol-left and other actual kinds still defer.
        if template_snap.is_protocol || a_snap.is_protocol {
            if template_snap.is_protocol && !a_snap.is_protocol {
                if direction == SUPERTYPE_OF {
                    return visit_instance_protocol_supertype_native(
                        template,
                        actual,
                        original_actual,
                        direction,
                        resolver,
                        aliases,
                        strict_optional,
                    );
                }
                if direction == SUBTYPE_OF {
                    // Python: both nominal branches miss, the SUPERTYPE_OF
                    // structural arm does not fire, and the tail's elif
                    // chain matches no Instance arm, so `return []`.
                    return Some(vec![]);
                }
            }
            // Protocol-template / protocol-actual pairs beyond the ported
            // arms are Python-side (the structural protocol tail).
            return None;
        }
        // Fall through to the tail (actual is a non-protocol instance).
    }
    visit_instance_tail_native(
        template,
        actual,
        direction,
        resolver,
        aliases,
        strict_optional,
    )
}

thread_local! {
    /// Per-protocol-class inferring mirror (constraints.py `template.type.
    /// inferring`): a template already on the stack suppresses the
    /// structural protocol arm (`any(template == t for t in
    /// reversed(...))`, verified by structural equality, and the
    /// type_ref inside the Instance keeps entries of different protocol
    /// classes apart).
    static PROTOCOL_INFERRING: RefCell<Vec<Type>> = const { RefCell::new(Vec::new()) };
}

/// RAII guard mirroring `template.type.inferring.append/pop` around the
/// member loop (constraints.py:1553-1571).
struct ProtocolInferringPush;

impl ProtocolInferringPush {
    fn new(template: &Type) -> Self {
        PROTOCOL_INFERRING.with(|s| s.borrow_mut().push(template.clone()));
        Self
    }
}

impl Drop for ProtocolInferringPush {
    fn drop(&mut self) {
        PROTOCOL_INFERRING.with(|s| s.borrow_mut().pop());
    }
}

/// The SUPERTYPE_OF structural-protocol arm of `visit_instance`
/// (constraints.py:1540-1573): `template` is a protocol Instance, `actual`
/// is a non-protocol Instance, and both nominal branches missed. Decided
/// when `is_protocol_implementation(actual, erased(template),
/// skip=["__call__"])` succeeds (Rust engine, parity-tested) and the
/// member loop is `find_member`-based, which the parity-tested
/// `get_protocol_member_inner` covers for the shapes this path reaches.
fn visit_instance_protocol_supertype_native(
    template: &Type,
    actual: &Type,
    original_actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    // The engine and the member loop both need the live TypeInfo map for
    // the dependency record + member-flag reads (subtypes.py:1885,
    // :2025-2055); without it, defer straight to the pure-Python body.
    if !resolver.has_live_info_map() {
        return None;
    }
    // Inferring guard (constraints.py:1549).
    let already_on_stack = PROTOCOL_INFERRING.with(|s| s.borrow().contains(template));
    if already_on_stack {
        // Python falls out of the instance block with an empty `res`,
        // reaches the tail, which returns [] for an Instance actual.
        return Some(vec![]);
    }
    // erased = erase_typevars(template) (constraints.py:1422).
    let erased = erase_typevars_inner(template, None, &crate::erase_typevars::make_any());
    let erased = erased?;
    let skip = vec!["__call__".to_string()];
    let ctx = SubtypeContext::default();
    let verdict = pyo3::Python::with_gil(|py| {
        crate::protocols::is_protocol_implementation_inner(
            py, actual, actual, &erased, &skip, &ctx, resolver,
        )
    });
    match verdict {
        Some(false) => {
            // Python: the arm's `and` chain fails, control falls out of
            // the instance block; the tail then returns [] for an
            // Instance actual (constraints.py:1583-1641).
            visit_instance_tail_native(
                template,
                actual,
                direction,
                resolver,
                aliases,
                strict_optional,
            )
        }
        Some(true) => {
            let _guard = ProtocolInferringPush::new(template);
            // Python passes `(instance, template, original_actual,
            // template)` — subtype is the original actual, protocol is
            // the template itself.
            let res = pyo3::Python::with_gil(|py| {
                infer_constraints_from_protocol_members_native(
                    py,
                    actual,
                    template,
                    original_actual,
                    template,
                    false,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
                    None,
                )
            })?;
            Some(res)
        }
        None => None,
    }
}

/// Port of `infer_constraints_from_protocol_members` (constraints.py:
/// 1643-1682). For every protocol member, fetch the member type on the
/// instance side (`find_member(member, instance, subtype,
/// class_obj=class_obj)`) and on the template side (`find_member(member,
/// template, subtype)`), then infer constraints recursively; settable
/// members also emit opposite-direction constraints.
///
/// Member fetches ride the parity-tested `get_protocol_member_inner`
/// (the same fetch `is_protocol_implementation`'s member loop performs).
/// Two `find_member` divergences are known and mirrored:
///   * `__call__` on a metaclass instance: `get_protocol_member` answers
///     None where `find_member` finds the metaclass `__call__`, but the
///     constraints loop skips metaclass `__call__` members either way
///     (constraints.py:1676) — the inst-side fetch matches Python's
///     outcome.
///   * `__call__` on a metaclass TEMPLATE side would produce a spurious
///     `continue`; unreachable for protocol templates, and the rare
///     scaler-shape defers to Python instead.
///
/// A fetch `Defer` (property, plugin hook, not-ready var, ...) bubbles up
/// as a whole-call deferral, so Python re-runs the exact original loop.
#[allow(clippy::too_many_arguments)]
fn infer_constraints_from_protocol_members_native(
    py: Python<'_>,
    instance: &Type,
    template: &Type,
    subtype: &Type,
    protocol: &Type,
    class_obj: bool,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    protocol_snap: Option<&crate::typeinfo::TypeInfoSnapshot>,
) -> Option<Vec<Constraint>> {
    use crate::checker_helpers::{get_protocol_member_inner, GetProtocolMemberResult};

    let protocol_ref = get_type_ref(protocol)?;
    let protocol_snap = match protocol_snap {
        // Engine already resolved the protocol snapshot.
        Some(s) => s,
        None => resolver.get(protocol_ref)?,
    };
    let mut res: Vec<Constraint> = Vec::new();
    for member in &protocol_snap.protocol_members {
        // `__call__` on a metaclass template would diverge (see doc).
        if member == "__call__"
            && (protocol_snap.fullname == "builtins.type"
                || protocol_snap.fullname == "abc.ABCMeta")
        {
            return None;
        }
        let inst = match get_protocol_member_inner(
            py, instance, subtype, member, class_obj, false, resolver,
        ) {
            Some(GetProtocolMemberResult::Found(t)) => Some(t),
            Some(GetProtocolMemberResult::NoneVal) => None,
            // Defer / no-answer: fall back to the pure-Python loop.
            _ => {
                return None;
            }
        };
        let temp = match get_protocol_member_inner(
            py, template, subtype, member, false, false, resolver,
        ) {
            Some(GetProtocolMemberResult::Found(t)) => Some(t),
            Some(GetProtocolMemberResult::NoneVal) => None,
            _ => {
                return None;
            }
        };
        let (inst, temp) = match (inst, temp) {
            (Some(i), Some(t)) => (i, t),
            // Either side missing:
            (Some(_), None) | (None, Some(_)) | (None, None) => {
                if member == "__call__" {
                    continue;
                }
                // See #11020: a missing (non-__call__) member produces no
                // constraints at all — decided.
                return Some(vec![]);
            }
        };
        if class_obj {
            // constraints.py:1667-1674. `is_subtype(inst, erase_typevars(
            // temp), ignore_pos_arg_names=True)`; skipping is decided.
            let erased_temp =
                erase_typevars_inner(&temp, None, &crate::erase_typevars::make_any())?;
            let ctx = SubtypeContext::with_callable_flags(
                false,
                false,
                false,
                false,
                false,
                strict_optional,
                true,
                false,
            );
            match is_subtype(&inst, &erased_temp, &ctx, resolver) {
                Some(true) => {}
                Some(false) => continue,
                None => return None,
            }
        }
        res.extend(push_inner(
            temp.clone(),
            inst.clone(),
            direction,
            resolver,
            aliases,
            strict_optional,
        )?);
        // Settable members are invariant: add opposite-direction
        // constraints (constraints.py:1679-1681).
        let protocol_info = resolver.live_typeinfo(py, protocol_ref);
        let protocol_info = protocol_info?;
        let flags = crate::member_flags::get_member_flags_inner_pub(
            py,
            protocol_info,
            member,
            false,
            None,
            strict_optional,
            resolver,
        )?;
        if flags.contains(&crate::member_flags::IS_SETTABLE) {
            res.extend(push_inner(
                temp,
                inst,
                neg_op(direction),
                resolver,
                aliases,
                strict_optional,
            )?);
        }
    }
    Some(res)
}

fn visit_instance_tail_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    let template_args = match template {
        Type::Instance { args, .. } => args,
        _ => return None,
    };
    if let Type::AnyType { .. } = actual {
        return infer_against_any_native(
            template_args,
            actual,
            direction,
            resolver,
            aliases,
            strict_optional,
        );
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
                    Type::UnpackType { typ, .. } => match typ.as_ref() {
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
                        strict_optional,
                    )?);
                }
            }
            return Some(res);
        }
        // Both Python tuple branches require SUPERTYPE_OF
        // (constraints.py:1607, 1626), so any other direction falls out the
        // end of the elif chain to `return []`.
        if direction != SUPERTYPE_OF {
            return Some(vec![]);
        }
        // constraints.py:1626-1644: constrain the template against the
        // tuple's fallback instance.
        let fallback = crate::typeops::tuple_fallback(actual, resolver)?;
        let any_repl = Type::AnyType {
            type_of_any: 6, // TypeOfAny.special_form
            source_any: None,
            missing_import_name: None,
        };
        let erased = erase_typevars_inner(template, None, &any_repl)?;
        if !matches!(erased, Type::Instance { .. }) {
            // Python asserts the erased template is a bare Instance
            // (constraints.py:1628-1629); anything else is out of contract.
            return None;
        }
        let tail_tref = get_type_ref(template)?;
        let tail_snap = (resolver.get(tail_tref))?;
        if tail_snap.is_protocol {
            // Protocol special-case (constraints.py:1630-1643) needs
            // protocol members; not ported this round.
            return None;
        }
        return push_inner(
            template.clone(),
            fallback,
            direction,
            resolver,
            aliases,
            strict_optional,
        );
    }
    if let Type::TypeVarType {
        values,
        meta_level,
        upper_bound,
        ..
    } = actual
    {
        return if values.is_empty() && *meta_level == 0 {
            infer_constraints_full_inner(
                template,
                upper_bound,
                direction,
                resolver,
                aliases,
                strict_optional,
                false,
                // Tail mirror of the wrapper call at constraints.py:1647.
                true,
            )
        } else {
            Some(vec![])
        };
    }
    if let Type::ParamSpecType { upper_bound, .. } = actual {
        return infer_constraints_full_inner(
            template,
            upper_bound,
            direction,
            resolver,
            aliases,
            strict_optional,
            false,
            // Tail mirror of the wrapper call at constraints.py:1650.
            true,
        );
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
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    let template_items = match template {
        Type::TupleType { items, .. } => items,
        _ => return None,
    };
    let unpack_index = find_unpack_in_list_inner(template_items);
    let is_varlength = match actual {
        Type::Instance { type_ref, .. } => resolver
            .get(type_ref)
            .map(|s| s.has_base("builtins.tuple"))?,
        _ => false,
    };
    if !(matches!(actual, Type::TupleType { .. }) || is_varlength) {
        if let Type::AnyType { .. } = actual {
            return infer_against_any_native(
                template_items,
                actual,
                direction,
                resolver,
                aliases,
                strict_optional,
            );
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
                Type::UnpackType { typ, .. } => match typ.as_ref() {
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
                        extra_tvars: Vec::new(),
                    });
                }
                Type::Instance { type_ref, .. } if type_ref == "builtins.tuple" => {
                    res.extend(push_inner(
                        unmapped_inner.clone(),
                        mapped_instance.clone(),
                        direction,
                        resolver,
                        aliases,
                        strict_optional,
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
                    strict_optional,
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
                strict_optional,
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
                strict_optional,
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
            strict_optional,
        );
    }
    let a_unpack = &a_items[a_unpack_index as usize];
    let a_unpacked = match a_unpack {
        Type::UnpackType { typ, .. } => typ.as_ref(),
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
            strict_optional,
        );
    }
    // The actual-unpack middle only constrains when the unpacked is a
    // homogeneous `*tuple[X, ...]` instance and get_proper_type may expand
    // an alias through the resolver; a non-tuple target still runs the tail.
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
        let mid_arg = a_mid_args.first().cloned()?;
        for tm in t_middle {
            res.extend(push_inner(
                tm,
                mid_arg.clone(),
                direction,
                resolver,
                aliases,
                strict_optional,
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
        strict_optional,
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
    strict_optional: bool,
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
                    strict_optional,
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
                strict_optional,
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
            strict_optional,
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
    strict_optional: bool,
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
                Type::UnpackType { typ, .. } => typ.as_ref(),
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
                    extra_tvars: Vec::new(),
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
                strict_optional,
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
                    strict_optional,
                )?);
            }
        }
        // Constraint(s) for the variadic item when possible. `t_unpack` is
        // set from the template item (constraints.py:2079).
        t_unpack = Some(&template_args[template_unpack as usize]);
        let inner = match t_unpack.unwrap() {
            Type::UnpackType { typ, .. } => typ.as_ref(),
            _ => return None,
        };
        match inner {
            Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                let inner_arg = args.first().cloned()?;
                // Homogeneous case *tuple[T, ...] <: [X, Y, Z, ...].
                res.extend(constrain_homogeneous_middle(
                    &middle,
                    &inner_arg,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
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
                    extra_tvars: Vec::new(),
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
            strict_optional,
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
                strict_optional,
            )?);
        }
    }
    if let Some(tu) = t_unpack {
        let inner = match tu {
            Type::UnpackType { typ, .. } => typ.as_ref(),
            _ => return None,
        };
        match inner {
            Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                let inner_arg = args.first().cloned()?;
                // Homogeneous case *tuple[T, ...] <: [X, Y, Z, ...].
                res.extend(constrain_homogeneous_middle(
                    &middle,
                    &inner_arg,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
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
                    extra_tvars: Vec::new(),
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
            Type::UnpackType { typ, .. } => typ.as_ref(),
            _ => return None,
        };
        // Only a *tuple[A, ...] actual unpack produces constraints.
        // TypeVarTuple actual unpack yields nothing (constraints.py:2137)
        // unless get_proper_type expands the alias to a tuple target.
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
            Type::UnpackType { typ, .. } => typ.as_ref(),
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
                    strict_optional,
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
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    let mut res = Vec::new();
    for a in middle {
        match a {
            Type::UnpackType { typ, .. } => {
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
                        strict_optional,
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
                    strict_optional,
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
    strict_optional: bool,
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
                        strict_optional,
                    )?);
                }
            }
            Some(res)
        }
        Type::AnyType { .. } => {
            let values: Vec<Type> = t_items.iter().map(|(_, v)| v.clone()).collect();
            infer_against_any_native(
                &values,
                actual,
                direction,
                resolver,
                aliases,
                strict_optional,
            )
        }
        _ => Some(vec![]),
    }
}

/// Port of `visit_type_type` (constraints.py:2046). TypeType, Any, and the
/// empty case are portable, plus the CallableType/Overloaded actuals via the
/// existing `callable_compat::is_type_obj` + `get_proper_or_expand` ports
/// (Python `get_instance_type`, types.py:2528). Defers when the resolver
/// cannot decide `is_type_obj` (missing fallback snapshot) or an alias
/// `ret_type` cannot expand.
fn visit_type_type_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    erase_types: bool,
) -> Option<Vec<Constraint>> {
    let template_item = match template {
        Type::TypeType { item, .. } => item.as_ref(),
        _ => return None,
    };
    // Python erasetype.py:331 erase_typevars replacement is
    // `AnyType(TypeOfAny.special_form)`; ensure this isn't confused with
    // the unmodeled `make_any` variant used elsewhere.
    let any_sp = Type::AnyType {
        type_of_any: 6,
        source_any: None,
        missing_import_name: None,
    };
    match actual {
        Type::CallableType {
            instance_type,
            ret_type,
            ..
        } => {
            // constraints.py:2048-2053: is_type_obj -> get_instance_type();
            // else infer against the raw ret_type (no get_proper_type).
            let is_tobj = crate::callable_compat::is_type_obj(actual, resolver)?;
            let mut target = if is_tobj {
                match instance_type {
                    Some(t) => (**t).clone(),
                    None => (get_proper_or_expand(ret_type, aliases))?,
                }
            } else {
                (**ret_type).clone()
            };
            if is_tobj && erase_types {
                target = erase_typevars_inner(&target, None, &any_sp)?;
            }
            push_inner(
                template_item.clone(),
                target,
                direction,
                resolver,
                aliases,
                strict_optional,
            )
        }
        Type::Overloaded { items } => {
            // constraints.py:2054-2060; Overloaded.is_type_obj is
            // items[0].is_type_obj() (types.py:2993).
            let first = items.first()?;
            let Some(Type::CallableType {
                instance_type,
                ret_type,
                ..
            }) = Some(first)
            else {
                return None;
            };
            let is_tobj = crate::callable_compat::is_type_obj(first, resolver)?;
            let mut target = if is_tobj {
                match instance_type {
                    Some(t) => (**t).clone(),
                    None => (get_proper_or_expand(ret_type, aliases))?,
                }
            } else {
                (**ret_type).clone()
            };
            if is_tobj && erase_types {
                target = erase_typevars_inner(&target, None, &any_sp)?;
            }
            push_inner(
                template_item.clone(),
                target,
                direction,
                resolver,
                aliases,
                strict_optional,
            )
        }
        Type::TypeType { item, .. } => push_inner(
            template_item.clone(),
            item.as_ref().clone(),
            direction,
            resolver,
            aliases,
            strict_optional,
        ),
        Type::AnyType { .. } => push_inner(
            template_item.clone(),
            actual.clone(),
            direction,
            resolver,
            aliases,
            strict_optional,
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
#[allow(clippy::too_many_arguments)]
fn visit_callable_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    skip_neg_op: bool,
) -> Option<Vec<Constraint>> {
    // The CallableType actual takes the full template-vs-callable port
    // (constraints.py:1656-1780), which must see the raw template: the
    // param-spec/unpack-formal gates below only guard the Any branch.
    if matches!(actual, Type::CallableType { .. }) {
        return callable_vs_callable_native(
            template,
            actual,
            direction,
            resolver,
            aliases,
            strict_optional,
            skip_neg_op,
        );
    }
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
    // Only the AnyType actual branch is portable; every other actual shape
    // (callable, overloaded, typetype, instance) defers to Python.
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
            strict_optional,
        )?);
    } else {
        let ps = param_spec.clone()?;
        // Build Parameters([any, any], [ARG_STAR, ARG_STAR2], [None, None])
        // with imprecise_arg_kinds=True (constraints.py:1485-1491).
        let target = Type::Parameters(crate::wire::Parameters {
            arg_types: vec![any_type.clone(), any_type.clone()],
            arg_kinds: vec![2 /* ARG_STAR */, 4 /* ARG_STAR2 */],
            arg_names: vec![None, None],
            variables: Vec::new(),
            imprecise_arg_kinds: true,
            is_ellipsis_args: false,
        });
        res.push(Constraint {
            origin_type_var: ps,
            op: SUBTYPE_OF,
            target,
            extra_tvars: Vec::new(),
        });
    }
    res.extend(push_inner(
        ret_type.as_ref().clone(),
        any_type.clone(),
        direction,
        resolver,
        aliases,
        strict_optional,
    )?);
    Some(res)
}

/// `CallableType.param_spec()` (types.py:2696-2712), returning the real
/// ParamSpecType: flavor forced to BARE and prefix rebuilt as
/// `Parameters(arg_types[:-2], arg_kinds[:-2], arg_names[:-2])`.
pub(crate) fn param_spec_of(
    arg_types: &[Type],
    arg_kinds: &[i64],
    arg_names: &[Option<String>],
) -> Option<Type> {
    if arg_types.len() < 2 {
        return None;
    }
    let n = arg_kinds.len();
    // ARG_STAR = 2, ARG_STAR2 = 4 (nodes.py:2486,2490).
    if arg_kinds[n - 2] != 2 || arg_kinds[n - 1] != 4 {
        return None;
    }
    match &arg_types[arg_types.len() - 2] {
        Type::ParamSpecType {
            prefix: _,
            name,
            fullname,
            raw_id,
            namespace,
            flavor: _,
            upper_bound,
            default,
            meta_level,
        } => {
            let m = arg_types.len();
            let names_len = arg_names.len();
            Some(Type::ParamSpecType {
                prefix: Box::new(crate::wire::Parameters {
                    arg_types: arg_types[..m - 2].to_vec(),
                    arg_kinds: arg_kinds[..n - 2].to_vec(),
                    arg_names: arg_names[..names_len.saturating_sub(2)].to_vec(),
                    variables: Vec::new(),
                    imprecise_arg_kinds: false,
                    is_ellipsis_args: false,
                }),
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                flavor: 0, // ParamSpecFlavor.BARE
                upper_bound: upper_bound.clone(),
                default: default.clone(),
                meta_level: *meta_level,
            })
        }
        _ => None,
    }
}

/// `get_tuple_fallback_from_unpack` (constraints.py:2111-2124) on the wire
/// form: only the shapes whose fallback is decidable without the live MRO.
fn tuple_fallback_ref_from_unpack(t: &Type) -> Option<String> {
    match t {
        Type::UnpackType { typ, .. } => match typ.as_ref() {
            Type::Instance { type_ref, .. } if type_ref == "builtins.tuple" => {
                Some(type_ref.clone())
            }
            Type::TypeVarTupleType { tuple_fallback, .. } => match tuple_fallback.as_ref() {
                Type::Instance { type_ref, .. } => Some(type_ref.clone()),
                _ => None,
            },
            Type::TupleType {
                partial_fallback, ..
            } => match partial_fallback.as_ref() {
                // Python walks `partial_fallback.type.mro` for builtins.tuple;
                // the wire cannot walk that MRO, so only the exact fallback
                // is decidable here.
                Type::Instance { type_ref, .. } if type_ref == "builtins.tuple" => {
                    Some(type_ref.clone())
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// `repack_callable_args` (constraints.py:2132-2155) on the wire form.
/// The tuple fallback only matters when re-wrapping a bare star type.
fn repack_callable_args_wire(
    arg_types: &[Type],
    arg_kinds: &[i64],
    tuple_ref: &str,
) -> Option<Vec<Type>> {
    let star_index = match arg_kinds.iter().position(|k| *k == 2) {
        Some(i) => i,
        // Python: `if ARG_STAR not in callable.arg_kinds: return callable.arg_types`.
        None => return Some(arg_types.to_vec()),
    };
    let mut out: Vec<Type> = arg_types[..star_index].to_vec();
    let star_out: Type = match &arg_types[star_index] {
        Type::UnpackType { typ, .. } => match typ.as_ref() {
            Type::TupleType { items, .. } => {
                // Python asserts the first item is itself an UnpackType,
                // splices it as the middle and keeps the rest as the suffix.
                if !matches!(items[0], Type::UnpackType { .. }) {
                    return None;
                }
                out.push(items[0].clone());
                out.extend(items[1..].iter().cloned());
                return Some(out);
            }
            Type::TypeAliasType { .. } => return None, // get_proper_type needs the live alias
            // Python keeps the star UnpackType untouched when its proper
            // type is not a TupleType (constraints.py:2165-2170).
            _ => arg_types[star_index].clone(),
        },
        // Re-normalize *args: X -> *args: *tuple[X, ...].
        not_unpack => Type::UnpackType {
            typ: Box::new(Type::Instance {
                type_ref: tuple_ref.to_string(),
                args: vec![not_unpack.clone()],
                last_known_value: None,
                extra_attrs: None,
            }),
            from_star_syntax: false,
        },
    };
    out.push(star_out);
    Some(out)
}

/// Port of the `isinstance(self.actual, CallableType)` branch of
/// `visit_callable_type` (constraints.py:1656-1813): normalize both sides
/// (`with_unpacked_kwargs().with_normalized_var_args()`), infer the
/// ret-type constraint (with type_guard/type_is arms), then either the
/// reverse gate + args, or (template ParamSpec) the reverse gate +
/// prefix + `param_spec_target` construction.
///
/// The ambient polymorphic state rides the thread-local tri-state
/// (PolyModeGuard), mirroring the Python ambient `type_state.infer_
/// polymorphic` read at constraints.py:1712/1768/1795: Unknown mode keeps
/// the pre-#1427 defer front, Known(true) fires the opposite-direction
/// reverse frame plus the extras tail (constraints.py:1710-1733,
/// 1768-1775, 1810-1812), Known(false) keeps both gates off.
///
/// Defers (`None`) when either side fails to normalize, any nested
/// constraint step defers, or the actual carries type variables in
/// Unknown mode (the wire historically had no representation for the
/// resulting `extra_tvars`; #1171). Nested Rust recursions have no mode
/// install, so they read Unknown and keep the old
/// skip_neg_op=False/erase_types=True defaults; the tri-state read sites
/// mirror constraints.py:1712/1768/1795.
#[allow(clippy::too_many_arguments)]
fn callable_vs_callable_native(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    skip_neg_op: bool,
) -> Option<Vec<Constraint>> {
    let templ = match crate::checkcall::normalize_callable(template) {
        Ok(t) => t,
        Err(_) => return None,
    };
    let cact = match crate::checkcall::normalize_callable(actual) {
        Ok(t) => t,
        Err(_) => return None,
    };
    let Type::CallableType {
        arg_types: t_args,
        arg_kinds: t_kinds,
        arg_names: t_names,
        ret_type: t_ret,
        is_ellipsis_args: t_ellipsis,
        type_guard: t_guard,
        type_is: t_is,
        ..
    } = &templ
    else {
        return None;
    };
    let Type::CallableType {
        arg_types: a_args,
        arg_kinds: a_kinds,
        arg_names: a_names,
        ret_type: a_ret,
        variables: a_vars,
        type_guard: a_guard,
        type_is: a_is,
        imprecise_arg_kinds: a_imprecise,
        ..
    } = &cact
    else {
        return None;
    };

    // Defer front parity (pre-#1427): the reverse gates need the ambient
    // mode; the Unknown mode with a generic actual can't evaluate the
    // infer_polymorphic conditionals at constraints.py:1711-1724/1768.
    let mode = infer_poly_mode();
    if !a_vars.is_empty() && !skip_neg_op && mode == 0 {
        return None;
    }
    let mut extras_fired = false;

    // Ret constraint with the type_guard / type_is arms
    // (constraints.py:1662-1672).
    let mut t_ret_ref: &Type = t_ret;
    let mut a_ret_ref: &Type = a_ret;
    if let (Some(tg), Some(ag)) = (t_guard, a_guard) {
        t_ret_ref = tg;
        a_ret_ref = ag;
    }
    if let (Some(ti), Some(ai)) = (t_is, a_is) {
        t_ret_ref = ti;
        a_ret_ref = ai;
    }
    let res = push_inner(
        t_ret_ref.clone(),
        a_ret_ref.clone(),
        direction,
        resolver,
        aliases,
        strict_optional,
    );
    let mut res = res?;

    let param_spec = param_spec_of(t_args, t_kinds, t_names);
    if param_spec.is_none() {
        // Opposite-direction inference marks extra type variables for
        // the solver (constraints.py:1711-1733); a self-id var leaking
        // through an ellipsis template is the only exception.
        if mode == 1
            && !a_vars.is_empty()
            && !skip_neg_op
            && !(a_vars
                .iter()
                .any(|v| matches!(v, Type::TypeVarType { raw_id: 0, .. }))
                && *t_ellipsis)
        {
            let cs = infer_constraints_full_inner(
                &cact,
                &templ,
                neg_op(direction),
                resolver,
                aliases,
                strict_optional,
                true,
                true,
            )?;
            res.extend(cs);
            extras_fired = true;
        }
        // We can't infer constraints from arguments if the template is
        // Callable[..., T] (constraints.py:1694-1695).
        if !*t_ellipsis {
            let unpack_present = find_unpack_in_list_inner(t_args);
            let cactual_ps = param_spec_of(a_args, a_kinds, a_names);
            if unpack_present >= 0 && cactual_ps.is_none() {
                // Re-normalize args to the tuple form and use the same
                // helper as for tuple types (constraints.py:1698-1726).
                let tuple_ref =
                    match tuple_fallback_ref_from_unpack(&t_args[unpack_present as usize]) {
                        Some(r) => r,
                        None => {
                            return None;
                        }
                    };
                let template_types = repack_callable_args_wire(t_args, t_kinds, &tuple_ref)?;
                let actual_types = repack_callable_args_wire(a_args, a_kinds, &tuple_ref)?;
                let simple_attempt = simple_unpack_native(
                    &template_types,
                    &actual_types,
                    neg_op(direction),
                    resolver,
                    aliases,
                    strict_optional,
                )?;
                res.extend(simple_attempt);
            } else {
                let new_args = infer_callable_arguments_constraints_core(
                    &templ,
                    &cact,
                    direction,
                    resolver,
                    aliases,
                    strict_optional,
                )?;
                res.extend(new_args);
            }
        }
        if extras_fired {
            for c in &mut res {
                c.extra_tvars.extend(a_vars.iter().cloned());
            }
        }
        return Some(res);
    }
    // ParamSpec prefix branch (constraints.py:1719-1779).
    let Some(param_spec) = param_spec else {
        unreachable!("is_none branch returned above");
    };
    let Type::ParamSpecType { prefix, .. } = &param_spec else {
        unreachable!("param_spec_of returns a ParamSpecType");
    };
    let prefix_len = prefix.arg_types.len();
    let cactual_ps = param_spec_of(a_args, a_kinds, a_names);

    // No self/ellipsis exception here: the ps-branch gate is the plain
    // conjunction (constraints.py:1768-1775).
    if mode == 1 && !a_vars.is_empty() && !skip_neg_op {
        let cs = infer_constraints_full_inner(
            &cact,
            &templ,
            neg_op(direction),
            resolver,
            aliases,
            strict_optional,
            true,
            true,
        )?;
        res.extend(cs);
        extras_fired = true;
    }

    // Compare prefixes as well (constraints.py:1743-1754).
    let mut cbase = match crate::checkcall::callable_base(&cact) {
        Ok(b) => b,
        Err(_) => return None,
    };
    cbase.arg_types.truncate(prefix_len);
    cbase.arg_kinds.truncate(prefix_len);
    cbase.arg_names.truncate(prefix_len);
    let cactual_prefix = cbase.into_type();
    let prefix_type = Type::Parameters((**prefix).clone());
    res.extend(infer_callable_arguments_constraints_core(
        &prefix_type,
        &cactual_prefix,
        direction,
        resolver,
        aliases,
        strict_optional,
    )?);

    let param_spec_target: Option<Type> = match &cactual_ps {
        None => {
            // max_prefix_len = number of positional+optional kinds
            // (constraints.py:1756-1765); ARG_POS = 0, ARG_OPT = 1.
            let max_prefix_len = a_kinds.iter().filter(|k| **k == 0 || **k == 1).count();
            let pl = prefix_len.min(max_prefix_len);
            Some(Type::Parameters(crate::wire::Parameters {
                arg_types: a_args[pl..].to_vec(),
                arg_kinds: a_kinds[pl..].to_vec(),
                arg_names: a_names[pl..].to_vec(),
                // constraints.py:1795 reads the ambient flag directly,
                // NOT the fired flag: a Known(true) skip_neg_op=True
                // frame still yields an empty variables list here.
                variables: match mode {
                    1 => Vec::new(),
                    _ => a_vars.clone(),
                },
                imprecise_arg_kinds: *a_imprecise,
                is_ellipsis_args: false,
            }))
        }
        Some(cp) => {
            let Type::ParamSpecType {
                prefix: cp_prefix,
                name,
                fullname,
                raw_id,
                namespace,
                flavor,
                upper_bound,
                default,
                meta_level,
            } = cp
            else {
                unreachable!("cactual_ps is a ParamSpecType");
            };
            if prefix_len <= cp_prefix.arg_types.len() {
                Some(Type::ParamSpecType {
                    prefix: Box::new(crate::wire::Parameters {
                        arg_types: cp_prefix.arg_types[prefix_len..].to_vec(),
                        arg_kinds: cp_prefix.arg_kinds[prefix_len..].to_vec(),
                        arg_names: cp_prefix.arg_names[prefix_len..].to_vec(),
                        variables: Vec::new(),
                        imprecise_arg_kinds: cp_prefix.imprecise_arg_kinds,
                        is_ellipsis_args: false,
                    }),
                    name: name.clone(),
                    fullname: fullname.clone(),
                    raw_id: *raw_id,
                    namespace: namespace.clone(),
                    flavor: *flavor,
                    upper_bound: upper_bound.clone(),
                    default: default.clone(),
                    meta_level: *meta_level,
                })
            } else {
                None
            }
        }
    };
    if let Some(target) = param_spec_target {
        res.push(Constraint {
            origin_type_var: param_spec,
            op: direction,
            target,
            extra_tvars: Vec::new(),
        });
    }
    if extras_fired {
        for c in &mut res {
            c.extra_tvars.extend(a_vars.iter().cloned());
        }
    }
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
            meta_level,
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
                    is_ellipsis_args: false,
                }),
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                flavor: 0,
                upper_bound: upper_bound.clone(),
                default: default.clone(),
                meta_level: *meta_level,
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
    strict_optional: bool,
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
            strict_optional,
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
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    let mut active = Vec::new();
    let flat = flatten_nested_tuples_inner(
        items,
        true,
        Some(aliases as &dyn crate::aliases::AliasLookup),
        &mut active,
    )?;
    let mut res = Vec::new();
    for t in flat {
        match t {
            Type::UnpackType { typ, .. } => match typ.as_ref() {
                Type::TypeVarTupleType { .. } => {
                    res.push(Constraint {
                        origin_type_var: typ.as_ref().clone(),
                        op: direction,
                        target: any_type.clone(),
                        extra_tvars: Vec::new(),
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
                    strict_optional,
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
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    infer_constraints_full_inner(
        &t,
        &other,
        direction,
        resolver,
        aliases,
        strict_optional,
        false,
        // Every visit_* nested recursion mirrors Python's top-level
        // `infer_constraints` call, whose wrapper default is True.
        true,
    )
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
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let template = read_type(&mut ReadBuffer::new(template_bytes), None).ok()?;
    let actual = read_type(&mut ReadBuffer::new(actual_bytes), None).ok()?;
    let res = infer_callable_arguments_constraints_core(
        &template,
        &actual,
        direction,
        resolver.resolver(),
        resolver.alias_resolver(),
        strict_optional,
    )?;
    // The write loop is 3-field: an extras-carrying constraint would lose
    // its `extra_tvars` in serialization, so defer to Python instead.
    if res.iter().any(|c| !c.extra_tvars.is_empty()) {
        return None;
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

/// Shared core of `infer_callable_arguments_constraints`
/// (constraints.py:2032-2102), callable from both the standalone pyfunction
/// and the callable-vs-callable branch of the full constraint builder.
pub(crate) fn infer_callable_arguments_constraints_core(
    template: &Type,
    actual: &Type,
    direction: i64,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
) -> Option<Vec<Constraint>> {
    use crate::callable_compat::{
        argument_by_name, argument_by_position, callable_corresponding_argument, formal_arguments,
        kind_is_positional, kind_is_star, kw_arg, try_synthesizing_arg_from_kwarg,
        try_synthesizing_arg_from_vararg, var_arg,
    };

    // Mirror `direction == SUBTYPE_OF: left, right = template, actual` else
    // `left, right = actual, template` (constraints.py:2039-2042).
    let (left, right): (&Type, &Type) = if direction == SUBTYPE_OF {
        (template, actual)
    } else {
        (actual, template)
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
            resolver,
            aliases,
            strict_optional,
        )?);
    }
    if let (Some(ls2), Some(rs2)) = (&left_star2, &right_star2) {
        res.extend(infer_directed_arg_constraints_native(
            ls2.typ.clone(),
            rs2.typ.clone(),
            direction,
            resolver,
            aliases,
            strict_optional,
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
                Err(_) => {
                    return None;
                }
            };
        res.extend(infer_directed_arg_constraints_native(
            left_arg.typ.clone(),
            right_arg.typ.clone(),
            direction,
            resolver,
            aliases,
            strict_optional,
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
                resolver,
                aliases,
                strict_optional,
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
                    resolver,
                    aliases,
                    strict_optional,
                )?);
            }
        }
    }
    Some(res)
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
    strict_optional: bool,
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
    infer_constraints_full_inner(
        &template,
        &actual,
        inferred_dir,
        resolver,
        aliases,
        strict_optional,
        false,
        // Python `infer_constraints` wrapper default (constraints.py:802).
        true,
    )
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
            extra_tvars: Vec::new(),
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
            special_sig: None,
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
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "mod.P".to_string(),
            raw_id: 5,
            namespace: "fn".to_string(),
            flavor: 1,
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            meta_level: 0,
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
            true,
            false,
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
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
        )
        .is_none());
    }

    #[test]
    fn test_visit_callable_any_defers_unpack_formal() {
        let resolver = crate::typeinfo::TypeResolver::new();
        let unpack = Type::UnpackType {
            typ: Box::new(type_var(1, "T")),
            from_star_syntax: false,
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
            special_sig: None,
        };
        assert!(visit_callable_native(
            &template,
            &any_type(),
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
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
                    is_ellipsis_args: false,
                }),
                name: "P".to_string(),
                fullname: "mod.P".to_string(),
                raw_id: 5,
                namespace: "fn".to_string(),
                flavor: 0,
                upper_bound: Box::new(any_type()),
                default: Box::new(any_type()),
                meta_level: 0,
            }],
            type_guard: None,
            type_is: None,
            special_sig: None,
        };
        assert!(visit_callable_native(
            &template,
            &any_type(),
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
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
            &crate::aliases::TypeAliasResolver::new(),
            true
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
            true,
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
            is_recursive: false,
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

    fn instance_str() -> Type {
        Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn generic_list(arg: Type) -> Type {
        Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![arg],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn union_type(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        }
    }

    fn encode_type(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        crate::wire::write_type(&mut buf, t).expect("type must encode");
        buf.into_bytes()
    }

    fn nominal_snap_n(fullname: &str, n_tvars: usize) -> crate::typeinfo::TypeInfoSnapshot {
        let mut s = crate::typeinfo::TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        for _ in 0..n_tvars {
            s.type_vars.push("T".to_string());
            s.type_vars_with_variance.push(("T".to_string(), 1, 0)); // COVARIANT, TypeVarType
            s.type_var_raw_ids
                .push(s.type_vars_with_variance.len() as i64 - 1);
            s.type_var_upper_bounds.push(Vec::new());
        }
        if fullname != "builtins.object" {
            s.mro.push("builtins.object".to_string());
            s.has_base.insert("builtins.object".to_string());
            s.bases.push(encode_type(&Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }));
        }
        s
    }

    fn builtin_resolver() -> TypeResolver {
        let mut r = TypeResolver::new();
        for (name, n) in [
            ("builtins.object", 0),
            ("builtins.int", 0),
            ("builtins.str", 0),
            ("builtins.list", 1),
        ] {
            r.insert(name.to_string(), nominal_snap_n(name, n));
        }
        r
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
        let res = infer_constraints_full_inner(
            &template,
            &any_type(),
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
            true,
        );
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
        let res = infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
            true,
        );
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
            true,
            false,
            true,
        )
        .is_none());
    }

    #[test]
    fn test_top_level_alias_expanding_to_union_emits() {
        // An alias whose target is a union expands before the union
        // dispatch: the normalized union flows into the TypeVar branch and
        // emits the constraint directly (mirrors constraints.py:825-859).
        let resolver = builtin_resolver();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.UAlias".to_string(),
            alias_snap(
                "mod.UAlias",
                &union_type(vec![instance_int(), Type::NoneType]),
            ),
        );
        let template = type_var(1, "T");
        let actual = alias_type("mod.UAlias");
        let res = infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
            true,
        );
        let constraints = res.expect("union actual must emit natively now");
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].op, SUPERTYPE_OF);
        assert_eq!(constraints[0].origin_type_var, template);
        if let Type::UnionType { items, .. } = &constraints[0].target {
            assert!(items.contains(&instance_int()));
            assert!(items.contains(&Type::NoneType));
        } else {
            panic!("target not a union");
        }
    }

    #[test]
    fn test_union_dispatch_suggestion_any_actual_returns_empty() {
        // constraints.py:835: a suggestion-engine Any actual short-circuits
        // to [] before any branch runs.
        let resolver = crate::typeinfo::TypeResolver::new();
        let template = instance_int();
        let actual = Type::AnyType {
            type_of_any: ANY_SUGGESTION,
            source_any: None,
            missing_import_name: None,
        };
        assert!(infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
            true,
        )
        .is_some_and(|c| c.is_empty()));
    }

    #[test]
    fn test_union_erased_item_defers() {
        // keep_erased normalization preserves ErasedType items, which the
        // Python-side wire reader cannot decode: defer instead of emitting
        // an undecodable constraint.
        let resolver = crate::typeinfo::TypeResolver::new();
        let template = union_type(vec![instance_int(), Type::ErasedType]);
        let actual = instance_int();
        assert!(infer_constraints_full_inner(
            &template,
            &actual,
            SUBTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
            true,
        )
        .is_none());
    }

    #[test]
    fn test_branch_a_template_union_subtypes_all_items() {
        // constraints.py:884-888 (branch a): SUBTYPE_OF with a union
        // template constrains each item against the actual.
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = union_type(vec![generic_list(type_var(1, "T")), instance_str()]);
        let actual = generic_list(instance_int());
        let res = infer_constraints_full_inner(
            &template, &actual, SUBTYPE_OF, &resolver, &aliases, true, false, true,
        );
        let constraints = res.expect("branch a must compute natively");
        assert_eq!(constraints.len(), 1);
        assert_eq!(constraints[0].op, SUBTYPE_OF);
        assert_eq!(constraints[0].target, instance_int());
        if let Type::TypeVarType { name, .. } = &constraints[0].origin_type_var {
            assert_eq!(name, "T");
        } else {
            panic!("origin not a TypeVarType");
        }
    }

    #[test]
    fn test_branch_c_actual_union_finds_compatible_item() {
        // constraints.py:905-915 (branch c): SUBTYPE_OF with a union
        // actual; only compatible items become constraints.
        let resolver = builtin_resolver();
        let template = instance_int();
        let actual = union_type(vec![instance_int(), instance_str()]);
        let res = infer_constraints_full_inner(
            &template,
            &actual,
            SUBTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
            true,
        );
        assert!(res.is_some_and(|c| c.is_empty()));
    }

    #[test]
    fn test_branch_b_actual_union_supertypes_recurses_orig() {
        // constraints.py:889-898 (branch b): SUPERTYPE_OF with a union
        // actual recurses item-wise against the raw template.
        let resolver = builtin_resolver();
        let template = generic_list(type_var(1, "T"));
        let actual = union_type(vec![
            generic_list(instance_int()),
            generic_list(instance_str()),
        ]);
        let res = infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
            true,
        );
        let constraints = res.expect("branch b must compute natively");
        assert_eq!(constraints.len(), 2);
        for c in &constraints {
            assert_eq!(c.op, SUPERTYPE_OF);
            if let Type::TypeVarType { name, .. } = &c.origin_type_var {
                assert_eq!(name, "T");
            } else {
                panic!("origin not a TypeVarType");
            }
        }
        assert!(constraints.iter().any(|c| c.target == instance_int()));
        assert!(constraints.iter().any(|c| c.target == instance_str()));
    }

    #[test]
    fn test_branch_d_if_possible_gate_defers_without_snapshot() {
        // A union template whose item gates need an unsnapshotted nominal
        // subtype check defers to Python.
        let resolver = crate::typeinfo::TypeResolver::new();
        let template = union_type(vec![instance_int(), instance_str()]);
        let actual = Type::Instance {
            type_ref: "foo.Bar".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &crate::aliases::TypeAliasResolver::new(),
            true,
            false,
            true,
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
                from_star_syntax: false,
            }],
            implicit: false,
        };
        let res =
            visit_instance_tail_native(&template, &actual, SUPERTYPE_OF, &resolver, &aliases, true);
        assert!(res.is_some());
    }

    // -- type_state.inferring cycle guard (issue #1133) --

    fn insert_recursive_tree_alias(aliases: &mut crate::aliases::TypeAliasResolver) {
        // `Tree = TypedDict('Tree', {'left': 'Tree[T]'})` — the snapshot
        // target carries a lazy self-reference, so both operands
        // regenerate their own shape on expansion.
        let target = Type::TypedDictType {
            fallback: Box::new(instance_builtins_object()),
            items: vec![(
                "left".to_string(),
                Type::TypeAliasType {
                    args: vec![type_var(1, "T")],
                    type_ref: "mod.Tree".to_string(),
                    is_recursive: false,
                },
            )],
            required_keys: std::collections::HashSet::new(),
            readonly_keys: std::collections::HashSet::new(),
            is_closed: false,
        };
        aliases.insert("mod.Tree".to_string(), alias_snap("mod.Tree", &target));
    }

    #[test]
    fn test_recursive_alias_pair_returns_no_constraints() {
        // Without the INFERRING mirror this pair recursed to stack overflow
        // once the alias snapshot made it reachable (#1133); the guard must
        // catch the repeat and mirror constraints.py:729-731 (empty list).
        let resolver = crate::typeinfo::TypeResolver::new();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_recursive_tree_alias(&mut aliases);
        let template = Type::TypeAliasType {
            args: vec![type_var(1, "T")],
            type_ref: "mod.Tree".to_string(),
            is_recursive: false,
        };
        let actual = Type::TypeAliasType {
            args: vec![instance_int()],
            type_ref: "mod.Tree".to_string(),
            is_recursive: false,
        };
        let res = infer_constraints_full_inner(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
            true,
        );
        assert!(res.is_some_and(|c| c.is_empty()));
    }

    #[test]
    fn test_alias_guard_stack_is_popped_between_calls() {
        // A second identical call must not see the first call's pushed
        // pair; a leaked INFERRING entry would turn the second call into
        // a spurious repeat (empty result).
        let resolver = crate::typeinfo::TypeResolver::new();
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Alias".to_string(),
            alias_snap("mod.Alias", &type_var(1, "T")),
        );
        let template = alias_type("mod.Alias");
        let actual = instance_int();
        for _ in 0..2 {
            let res = infer_constraints_full_inner(
                &template,
                &actual,
                SUPERTYPE_OF,
                &resolver,
                &aliases,
                true,
                false,
                true,
            );
            let constraints = res.expect("non-recursive call must compute natively");
            assert_eq!(constraints.len(), 1);
        }
    }

    // -- issue #1259 round 3: decided type-object/callable actual,
    //    NamedTuple tuple-fallback, and type-type actual arms --

    fn tobject_snap() -> crate::typeinfo::TypeInfoSnapshot {
        nominal_snap_n("builtins.type", 0)
    }

    fn type_object_callable(instance_type: Option<Type>, ret: Type) -> Type {
        Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.type".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: instance_type.map(Box::new),
            is_ellipsis_args: false,
            implicit: false,
            is_bound: true,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: true,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(ret),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn assert_tvar_origin(constraint: &Constraint, name: &str, raw_id: i64) {
        if let Type::TypeVarType {
            name: got_name,
            raw_id: got_id,
            ..
        } = &constraint.origin_type_var
        {
            assert_eq!(got_name, name);
            assert_eq!(*got_id, raw_id);
        } else {
            panic!("origin not a TypeVarType");
        }
    }

    #[test]
    fn test_tt_actual_type_obj_callable_erase_flag() {
        // constraints.py:2046-2053: `type[list[T]]` vs a type-object callable
        // constrains list[T] against the callable's instance_type; with
        // erase_types the tvars become AnyType(TypeOfAny.special_form).
        let mut resolver = builtin_resolver();
        resolver.insert("builtins.type".to_string(), tobject_snap());
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = Type::TypeType {
            item: Box::new(generic_list(type_var(1, "T"))),
            is_type_form: false,
        };
        let actual = type_object_callable(Some(generic_list(type_var(2, "V"))), instance_int());
        let erased = visit_type_type_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            true, // erase_types: the seamless entry's caller flag
        )
        .expect("type-object callable actual must decide natively");
        assert_eq!(erased.len(), 1);
        assert_eq!(erased[0].op, SUPERTYPE_OF);
        assert_tvar_origin(&erased[0], "T", 1);
        assert!(matches!(
            &erased[0].target,
            Type::AnyType { type_of_any: 6, .. }
        ));

        // Without erase_types the tvar in instance_type survives.
        let kept = visit_type_type_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("type-object callable actual must decide natively");
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].op, SUPERTYPE_OF);
        assert_tvar_origin(&kept[0], "T", 1);
        if let Type::TypeVarType { name, raw_id, .. } = &kept[0].target {
            assert_eq!(name, "V");
            assert_eq!(*raw_id, 2);
        } else {
            panic!("target not the instance tvar");
        }
    }

    #[test]
    fn test_tail_tuple_namedtuple_fallback_supertype() {
        // constraints.py:1626-1654 tail: a NamedTuple instance vs its
        // generic NamedTuple base constrains the base's tvars against the
        // concrete args Riding the fallback mapping.
        let mut resolver = builtin_resolver();
        resolver.insert("nt.X".to_string(), nominal_snap_n("nt.X", 1));
        let mut nt_snap = nominal_snap_n("nt.NT", 2);
        nt_snap.mro.insert(1, "nt.X".to_string());
        nt_snap.has_base.insert("nt.X".to_string());
        nt_snap.bases.insert(
            0,
            encode_type(&Type::Instance {
                type_ref: "nt.X".to_string(),
                args: vec![instance_int(), instance_str()],
                last_known_value: None,
                extra_attrs: None,
            }),
        );
        resolver.insert("nt.NT".to_string(), nt_snap);
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = Type::Instance {
            type_ref: "nt.X".to_string(),
            args: vec![type_var(1, "T")],
            last_known_value: None,
            extra_attrs: None,
        };
        let actual = Type::TupleType {
            items: vec![instance_int(), instance_str()],
            partial_fallback: Box::new(Type::Instance {
                type_ref: "nt.NT".to_string(),
                args: vec![instance_int(), instance_str()],
                last_known_value: None,
                extra_attrs: None,
            }),
            implicit: false,
        };
        let res =
            visit_instance_tail_native(&template, &actual, SUPERTYPE_OF, &resolver, &aliases, true)
                .expect("NamedTuple tuple fallback must decide natively");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].op, SUPERTYPE_OF);
        assert_tvar_origin(&res[0], "T", 1);
        assert_eq!(res[0].target, instance_int());
    }

    #[test]
    fn test_inst_typetype_actual_decided_by_protocol_flag() {
        // constraints.py:1400-1417 + tail: a non-protocol template leaves
        // the type[...] actual to fall out to `return []`; a protocol
        // template defers to the unported member walk.
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = generic_list(type_var(1, "T"));
        let actual = Type::TypeType {
            item: Box::new(instance_int()),
            is_type_form: false,
        };
        let res =
            visit_instance_native(&template, &actual, SUPERTYPE_OF, &resolver, &aliases, true)
                .expect("non-protocol template with type actual must decide natively");
        assert!(res.is_empty());

        let mut proto_snap = nominal_snap_n("mod.P", 0);
        proto_snap.is_protocol = true;
        let mut proto_resolver = builtin_resolver();
        proto_resolver.insert("mod.P".to_string(), proto_snap);
        let proto_template = Type::Instance {
            type_ref: "mod.P".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(visit_instance_native(
            &proto_template,
            &actual,
            SUPERTYPE_OF,
            &proto_resolver,
            &aliases,
            true,
        )
        .is_none());
    }

    // -- callable_vs_callable_native (visit_callable_type actual-Callable
    //    arm, constraints.py:1656-1780, issue #1194) --

    #[allow(clippy::too_many_arguments)]
    fn cb_callable(
        arg_types: Vec<Type>,
        arg_kinds: Vec<i64>,
        arg_names: Vec<Option<String>>,
        ret: Type,
        variables: Vec<Type>,
        is_ellipsis_args: bool,
    ) -> Type {
        Type::CallableType {
            fallback: Box::new(instance_builtins_object()),
            instance_type: None,
            is_ellipsis_args,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type: Box::new(ret),
            name: None,
            variables,
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn cb_param_spec(name: &str, raw_id: i64, prefix_types: Vec<Type>) -> Type {
        Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: prefix_types.clone(),
                arg_kinds: vec![0; prefix_types.len()], // ARG_POS
                arg_names: vec![None; prefix_types.len()],
                variables: Vec::new(),
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: name.to_string(),
            fullname: format!("mod.{}", name),
            raw_id,
            namespace: "fn".to_string(),
            flavor: 0, // BARE
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            meta_level: 0,
        }
    }

    #[test]
    fn test_cb_plain_callable_ret_and_args() {
        let resolver = crate::typeinfo::TypeResolver::new();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        // template: Callable[[T], T]; actual: Callable[[str], int].
        let template = cb_callable(
            vec![tv.clone()],
            vec![0],
            vec![None],
            tv.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            Vec::new(),
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("plain callable-vs-callable must decide natively");
        assert_eq!(res.len(), 2);
        let ret_c = res
            .iter()
            .find(|c| {
                matches!(&c.target, Type::Instance { type_ref, .. } if type_ref == "builtins.int")
            })
            .expect("ret constraint targets the actual ret type");
        assert_eq!(ret_c.op, SUPERTYPE_OF);
        assert_eq!(ret_c.origin_type_var, tv);
        let arg_c = res
            .iter()
            .find(|c| {
                matches!(&c.target, Type::Instance { type_ref, .. } if type_ref == "builtins.str")
            })
            .expect("arg constraint targets the actual arg type");
        assert_eq!(arg_c.origin_type_var, tv);
        // The formal is invariant-facing: the argument-side constraint flips
        // the direction (infer_directed_arg_constraints core).
        assert_eq!(arg_c.op, SUBTYPE_OF);
    }

    #[test]
    fn test_cb_ellipsis_template_infers_ret_only() {
        // constraints.py:1694-1696: no arg constraints when the template is
        // Callable[..., T] (literal ellipsis).
        let resolver = crate::typeinfo::TypeResolver::new();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        let template = cb_callable(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            tv.clone(),
            Vec::new(),
            true,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            Vec::new(),
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("ellipsis template must decide natively");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].op, SUPERTYPE_OF);
        assert_eq!(res[0].origin_type_var, tv);
        assert_eq!(res[0].target, instance_int());
    }

    #[test]
    fn test_cb_unpack_template_simple_unpack() {
        // constraints.py:1698-1726: an Unpack formal routes through repack +
        // build_constraints_for_simple_unpack. Template: Callable[[int,
        // *tuple[T, ...]], T]; actual: Callable[[str, *tuple[str, ...]], int].
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        let tuple_unpack = |inner: Type| Type::UnpackType {
            typ: Box::new(Type::Instance {
                type_ref: "builtins.tuple".to_string(),
                args: vec![inner],
                last_known_value: None,
                extra_attrs: None,
            }),
            from_star_syntax: false,
        };
        let template = cb_callable(
            vec![instance_int(), tuple_unpack(tv.clone())],
            vec![0, 2],
            vec![None, None],
            tv.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str(), tuple_unpack(instance_str())],
            vec![0, 2],
            vec![None, None],
            instance_int(),
            Vec::new(),
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("unpack template must decide natively");
        // Ret constraint (T vs int, SUPERTYPE_OF) plus the unpack-middle
        // arg constraint (T vs str, direction-inverted to SUBTYPE_OF);
        // constraints.py:1662-1672 + 1698-1726.
        assert_eq!(res.len(), 2);
        let arg_c = res
            .iter()
            .find(|c| matches!(&c.target, Type::Instance { type_ref, .. } if type_ref == "builtins.str"))
            .expect("unpack-middle constraint targets the actual arg type");
        assert_eq!(arg_c.op, SUBTYPE_OF);
        assert_eq!(arg_c.origin_type_var, tv);
        assert_eq!(arg_c.target, instance_str());
    }

    #[test]
    fn test_cb_param_spec_prefix_and_target_from_plain_actual() {
        // constraints.py:1719-1765: template ParamSpec prefix comparison plus
        // the Parameters(param_spec_target) arm for a non-ParamSpec actual.
        // Template: Callable[[int, *P, **P], T]; actual: Callable[[int, str], str].
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        let ps = cb_param_spec("P", 5, vec![instance_int()]);
        let template = cb_callable(
            vec![instance_int(), ps.clone(), any_type()],
            vec![0, 2, 4],
            vec![None, None, None],
            tv.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_int(), instance_str()],
            vec![0, 0],
            vec![None, None],
            instance_str(),
            Vec::new(),
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("ParamSpec template must decide natively");
        assert_eq!(res.len(), 2);
        let ret_c = res
            .iter()
            .find(|c| matches!(&c.target, Type::Instance { type_ref, .. } if type_ref == "builtins.str"))
            .expect("ret constraint targets the actual ret type");
        assert_eq!(ret_c.op, SUPERTYPE_OF);
        assert_eq!(ret_c.origin_type_var, tv);
        let ps_c = res
            .iter()
            .find(|c| matches!(&c.target, Type::Parameters(_)))
            .expect("param-spec constraint targets the captured Parameters");
        assert_eq!(ps_c.op, SUPERTYPE_OF);
        assert_eq!(ps_c.origin_type_var, ps);
        let Type::Parameters(target) = &ps_c.target else {
            panic!("param-spec target not Parameters");
        };
        // The captured target is the actual's args past the template prefix.
        assert_eq!(target.arg_types, vec![instance_str()]);
        assert_eq!(target.arg_kinds, vec![0]);
    }

    #[test]
    fn test_cb_param_spec_target_from_actual_param_spec() {
        // constraints.py:1767-1776: the actual also carries a ParamSpec, so
        // the target is the actual's ParamSpec with the template prefix
        // stripped from its own prefix (template arg prefix [int]).
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let ps = cb_param_spec("P", 5, vec![instance_int()]);
        let ps_q = cb_param_spec("Q", 8, vec![instance_int(), instance_str()]);
        let template = cb_callable(
            vec![instance_int(), ps.clone(), any_type()],
            vec![0, 2, 4],
            vec![None, None, None],
            instance_int(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_int(), instance_str(), ps_q.clone(), any_type()],
            vec![0, 0, 2, 4],
            vec![None, None, None, None],
            instance_int(),
            Vec::new(),
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("both-ParamSpec pair must decide natively");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].op, SUPERTYPE_OF);
        assert_eq!(res[0].origin_type_var, ps);
        let Type::ParamSpecType { prefix, .. } = &res[0].target else {
            panic!("param-spec target not a ParamSpecType");
        };
        // Q's prefix loses the one template-prefix arg: [int, str] -> [str].
        assert_eq!(prefix.arg_types, vec![instance_str()]);
    }

    #[test]
    fn test_cb_generic_actual_defers() {
        // constraints.py:1674-1686: polymorphic inference on a generic
        // actual attaches extra_tvars the wire cannot represent (#1171);
        // the port must defer.
        let resolver = crate::typeinfo::TypeResolver::new();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        let template = cb_callable(
            vec![tv.clone()],
            vec![0],
            vec![None],
            tv.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            vec![type_var(2, "U")],
            false,
        );
        assert!(callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .is_none());
    }

    #[test]
    fn test_cb_generic_actual_skip_neg_op_proceeds() {
        // The sole skip_neg_op=True callers (constraints.py:1740/1782) live
        // inside the polymorphic block, which Python also enters only when
        // skip_neg_op is False, so a generic actual proceeds natively (#1226).
        let resolver = crate::typeinfo::TypeResolver::new();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        let template = cb_callable(
            vec![tv.clone()],
            vec![0],
            vec![None],
            tv.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            vec![type_var(2, "U")],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            true,
        )
        .expect("skip_neg_op=true must run the generic actual natively");
        // Per-arg constraint (T <: str under the arg direction) plus the
        // ret constraint (T :> int under the call direction).
        assert_eq!(res.len(), 2);
        for c in &res {
            assert_eq!(c.origin_type_var, tv);
        }
        assert!(res
            .iter()
            .any(|c| c.op == SUPERTYPE_OF && c.target == instance_int()));
        assert!(res
            .iter()
            .any(|c| c.op == SUBTYPE_OF && c.target == instance_str()));
    }

    #[test]
    fn test_cb_param_spec_target_variables_kept_when_not_polymorphic() {
        // constraints.py:1845 keeps `cactual.variables` in the captured
        // Parameters target when infer_polymorphic is off; with a generic
        // actual and skip_neg_op=true the walk still engages.
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        let ps = cb_param_spec("P", 5, vec![instance_int()]);
        let template = cb_callable(
            vec![instance_int(), ps.clone(), any_type()],
            vec![0, 2, 4],
            vec![None, None, None],
            tv.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_str(),
            vec![type_var(2, "U")],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            true,
        )
        .expect("param-spec target with generic actual must engage (skip=true)");
        let ps_c = res
            .iter()
            .find(|c| matches!(&c.target, Type::Parameters(_)))
            .expect("param-spec constraint targets the captured Parameters");
        assert_eq!(ps_c.origin_type_var, ps);
        let Type::Parameters(target) = &ps_c.target else {
            panic!("param-spec target not Parameters");
        };
        assert_eq!(target.variables, vec![type_var(2, "U")]);
    }

    #[test]
    fn test_cb_param_spec_target_variables_empty_when_polymorphic() {
        // Same shape under infer_polymorphic=true: the target carries no
        // variables (they live in extra_tvars the wire cannot express;
        // #1171).
        let _poly = PolyModeGuard::install(true);
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv = type_var(7, "T");
        let ps = cb_param_spec("P", 5, vec![instance_int()]);
        let template = cb_callable(
            vec![instance_int(), ps.clone(), any_type()],
            vec![0, 2, 4],
            vec![None, None, None],
            tv.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_str(),
            vec![type_var(2, "U")],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            true,
        )
        .expect("param-spec target with generic actual must engage (skip=true)");
        let ps_c = res
            .iter()
            .find(|c| matches!(&c.target, Type::Parameters(_)))
            .expect("param-spec constraint targets the captured Parameters");
        let Type::Parameters(target) = &ps_c.target else {
            panic!("param-spec target not Parameters");
        };
        assert!(target.variables.is_empty());
    }

    // ---- extra_tvars reverse frames (#1427) ----

    #[test]
    fn test_cb_extras_reverse_frame_fires_when_polymorphic() {
        let _poly = PolyModeGuard::install(true);
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv7 = type_var(7, "T");
        let tv2 = type_var(2, "U");
        let template = cb_callable(
            vec![tv7.clone()],
            vec![0],
            vec![None],
            tv7.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            vec![tv2.clone()],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("polymorphic reverse frame must engage");
        assert!(!res.is_empty());
        // constraints.py:1710-1733: every emitted constraint marks the
        // actual's own type variables for the solver.
        for c in &res {
            assert_eq!(c.extra_tvars, vec![tv2.clone()]);
        }
        assert!(res.iter().any(|c| c.op == SUBTYPE_OF));
    }

    #[test]
    fn test_cb_extras_absent_when_not_polymorphic() {
        let _poly = PolyModeGuard::install(false);
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv7 = type_var(7, "T");
        let template = cb_callable(
            vec![tv7.clone()],
            vec![0],
            vec![None],
            tv7.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            vec![type_var(2, "U")],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("Known(false) keeps pre-#1427 engagement (extras never attach)");
        for c in &res {
            assert!(c.extra_tvars.is_empty());
        }
    }

    #[test]
    fn test_cb_extras_frame_respects_skip_neg_op() {
        let _poly = PolyModeGuard::install(true);
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let tv7 = type_var(7, "T");
        let template = cb_callable(
            vec![tv7.clone()],
            vec![0],
            vec![None],
            tv7.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            vec![type_var(2, "U")],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            // skip_neg_op=True suppresses the reverse frame
            // (constraints.py:1711) even under Known(true).
            true,
        )
        .expect("skip_neg_op must not defer the call");
        for c in &res {
            assert!(c.extra_tvars.is_empty());
        }
    }

    #[test]
    fn test_cb_extras_self_id_ellipsis_exception() {
        let _poly = PolyModeGuard::install(true);
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        // constraints.py:1724-1731: a self-id variable carried by the
        // raw_id==0 var leaking through an ellipsis template is the only
        // exception to the reverse gate.
        let template = cb_callable(vec![], vec![], vec![], type_var(1, "R"), Vec::new(), true);
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            vec![type_var(0, "S")],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("the exception arm must keep the call engaged");
        for c in &res {
            assert!(c.extra_tvars.is_empty());
        }
    }

    #[test]
    fn test_cb_extras_ps_branch_reverse_gate() {
        let _poly = PolyModeGuard::install(true);
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let ps = cb_param_spec("P", 5, vec![instance_int()]);
        let template = cb_callable(
            vec![instance_int(), ps, any_type()],
            vec![0, 2, 4],
            vec![None, None, None],
            type_var(7, "T"),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_int()],
            vec![0],
            vec![None],
            instance_str(),
            vec![type_var(2, "U")],
            false,
        );
        let res = callable_vs_callable_native(
            &template,
            &actual,
            SUPERTYPE_OF,
            &resolver,
            &aliases,
            true,
            false,
        )
        .expect("ps-branch reverse gate must engage under Known(true)");
        // constraints.py:1768-1775 + the extras tail.
        for c in &res {
            assert_eq!(c.extra_tvars, vec![type_var(2, "U")]);
        }
        // constraints.py:1795 reads the ambient mode, not the fired flag.
        let ps_c = res
            .iter()
            .find(|c| matches!(&c.target, Type::Parameters(_)))
            .expect("param-spec constraint targets the captured Parameters");
        let Type::Parameters(target) = &ps_c.target else {
            panic!("param-spec target not Parameters");
        };
        assert!(target.variables.is_empty());
    }

    #[test]
    fn test_run_any_constraints_defers_on_extras() {
        let resolver = builtin_resolver();
        let tv2 = type_var(2, "U");
        let c = Constraint {
            origin_type_var: type_var(7, "T"),
            op: SUPERTYPE_OF,
            target: instance_int(),
            extra_tvars: vec![tv2],
        };
        assert!(
            run_any_constraints(vec![Some(vec![c])], true, true, &resolver).is_none(),
            "extras-carrying constraint blobs cannot survive the 3-field wire"
        );
        // The clean sibling still engages.
        let c_clean = Constraint {
            origin_type_var: type_var(7, "T"),
            op: SUPERTYPE_OF,
            target: instance_int(),
            extra_tvars: Vec::new(),
        };
        let res = run_any_constraints(vec![Some(vec![c_clean])], true, true, &resolver)
            .expect("clean single option engages");
        assert_eq!(res.len(), 1);
        assert!(res[0].extra_tvars.is_empty());
    }

    #[test]
    fn test_poly_mode_guard_sequential_unwind() {
        // The RAII guard resets to Unknown (0), not to a saved previous
        // value: nesting is unsupported, sequential installs unwind fully.
        assert_eq!(infer_poly_mode(), 0);
        let g = PolyModeGuard::install(true);
        assert_eq!(infer_poly_mode(), 1);
        drop(g);
        assert_eq!(infer_poly_mode(), 0);
        let g2 = PolyModeGuard::install(false);
        assert_eq!(infer_poly_mode(), 2);
        drop(g2);
        assert_eq!(infer_poly_mode(), 0);
    }

    #[test]
    fn test_ffi_infer_polymorphic_serialization_guard() {
        let nat = NativeTypeResolver::new(
            builtin_resolver(),
            crate::aliases::TypeAliasResolver::default(),
        );
        let tv7 = type_var(7, "T");
        let template = cb_callable(
            vec![tv7.clone()],
            vec![0],
            vec![None],
            tv7.clone(),
            Vec::new(),
            false,
        );
        let actual = cb_callable(
            vec![instance_str()],
            vec![0],
            vec![None],
            instance_int(),
            vec![type_var(2, "U")],
            false,
        );
        let mut tb = WriteBuffer::new();
        write_type(&mut tb, &template).unwrap();
        let template_bytes = tb.into_bytes();
        let mut ab = WriteBuffer::new();
        write_type(&mut ab, &actual).unwrap();
        let actual_bytes = ab.into_bytes();

        // infer_polymorphic=false: decisions come out clean on the wire.
        let clean = rust_infer_constraints_full(
            &nat,
            &template_bytes,
            &actual_bytes,
            SUPERTYPE_OF,
            false,
            true,
            true,
            false,
        )
        .expect("non-polymorphic pair serializes");
        assert!(!clean.is_empty());

        // infer_polymorphic=true: the emissions carry extra_tvars, which
        // the 3-field wire cannot express, so defer (None).
        assert!(
            rust_infer_constraints_full(
                &nat,
                &template_bytes,
                &actual_bytes,
                SUPERTYPE_OF,
                false,
                true,
                true,
                true,
            )
            .is_none(),
            "extras-carrying output must defer at the FFI boundary"
        );
    }

    // ---- visit_type_type_native: type[T] vs a type-object callable ----

    fn tt_template(item: Type) -> Type {
        Type::TypeType {
            item: Box::new(item),
            is_type_form: true,
        }
    }

    fn ctor_meta_resolver() -> TypeResolver {
        // builtin_resolver plus a snapshot whose has_base contains
        // builtins.type, so crate::callable_compat::is_type_obj decides
        // Some(true) for a callable with that fallback.
        let mut r = builtin_resolver();
        let mut meta = nominal_snap_n("mod.Ctor", 0);
        meta.has_base.insert("builtins.type".to_string());
        r.insert("mod.Ctor".to_string(), meta);
        r
    }

    fn ctor_callable(fallback_ref: &str, ret: Type, instance_type: Option<Type>) -> Type {
        Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: fallback_ref.to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: instance_type.map(Box::new),
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(ret),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn tt_constraints(
        resolver: &TypeResolver,
        aliases: &crate::aliases::TypeAliasResolver,
        template: &Type,
        actual: &Type,
        erase_types: bool,
    ) -> Option<Vec<Constraint>> {
        visit_type_type_native(
            template,
            actual,
            SUBTYPE_OF,
            resolver,
            aliases,
            true,
            erase_types,
        )
    }

    #[test]
    fn test_tt_actual_type_object_ctor_erases_per_flag() {
        // echoes constraints.py visit_type_type (2033-2040);
        // both flag settings are observable in the emitted constraint.
        let resolver = ctor_meta_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = tt_template(type_var(1, "T"));
        let actual = ctor_callable(
            "mod.Ctor",
            instance_int(),
            Some(generic_list(type_var(2, "U"))),
        );
        let erased = tt_constraints(&resolver, &aliases, &template, &actual, true)
            .expect("type-object ctor arm must engage");
        assert_eq!(erased.len(), 1);
        assert_eq!(erased[0].op, SUBTYPE_OF);
        assert_eq!(erased[0].origin_type_var, type_var(1, "T"));
        assert!(
            matches!(&erased[0].target, Type::Instance { args, .. }
                if matches!(args[0], Type::AnyType { .. })),
            "erase_types=true must erase the ctor typevar, got {:?}",
            erased[0].target
        );
        let preserved = tt_constraints(&resolver, &aliases, &template, &actual, false)
            .expect("erase_types=false arm must engage too");
        assert_eq!(preserved.len(), 1);
        assert!(
            matches!(&preserved[0].target, Type::Instance { args, .. }
                if matches!(&args[0], Type::TypeVarType { name, .. } if name == "U")),
            "erase_types=false must preserve the ctor typevar, got {:?}",
            preserved[0].target
        );
    }

    #[test]
    fn test_tt_actual_type_object_no_instance_type_uses_ret() {
        // get_instance_type historic fallback: instance_type None -> the
        // proper ret_type stands in (types.py get_instance_type), then the
        // erase flag applies.
        let resolver = ctor_meta_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = tt_template(type_var(1, "T"));
        let actual = ctor_callable("mod.Ctor", instance_int(), None);
        let res = tt_constraints(&resolver, &aliases, &template, &actual, true)
            .expect("ret-type fallback arm must engage");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].target, instance_int());
    }

    #[test]
    fn test_tt_actual_non_type_object_recurse_raw_ret() {
        // is_type_obj() false (fallback builtins.object): recursion against
        // the raw ret_type with no erase (constraints.py:2040).
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = tt_template(type_var(1, "T"));
        let actual = ctor_callable("builtins.object", instance_str(), None);
        let res = tt_constraints(&resolver, &aliases, &template, &actual, true)
            .expect("non-type-object arm must engage");
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].target, instance_str());
    }

    #[test]
    fn test_tt_actual_unsnapshotted_fallback_defers() {
        // A fallback Instance missing from the resolver leaves is_type_obj
        // undecided (callable_compat::is_type_obj -> None): the whole arm
        // defers and Python re-runs -- the pre-port behavior.
        let resolver = builtin_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = tt_template(type_var(1, "T"));
        let actual = ctor_callable("mod.Missing", instance_int(), None);
        assert!(tt_constraints(&resolver, &aliases, &template, &actual, true).is_none());
    }

    #[test]
    fn test_tt_actual_type_object_alias_ret_defers() {
        // is_type_obj true, instance_type None, ret is an alias with no
        // snapshot: get_proper_type cannot expand -> defer (tt-inst-ret).
        let resolver = ctor_meta_resolver();
        let aliases = crate::aliases::TypeAliasResolver::new();
        let template = tt_template(type_var(1, "T"));
        let actual = ctor_callable("mod.Ctor", alias_type("mod.Missing"), None);
        assert!(tt_constraints(&resolver, &aliases, &template, &actual, true).is_none());
    }
}
