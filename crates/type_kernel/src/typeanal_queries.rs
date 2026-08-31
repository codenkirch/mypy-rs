#![allow(non_local_definitions)]

//! Native port of Type-query helpers and `TypeAnalyser.anal_type` hot path
//! from `mypy/typeanal.py`.
//!
//! Query helpers (no semantic context needed):
//! - `has_explicit_any` — does the type carry `AnyType(TypeOfAny.explicit)`?
//! - `has_any_from_unimported_type` — same for `from_unimported_type`.
//! - `collect_all_inner_types` — list of every type `t` contains.
//! - `make_optional_type` — `Optional[t]` without union simplification.
//!
//! Hot path (`rust_type_analyze`): mirrors `TypeAnalyser.anal_type`, analyzing
//! already-bound types (Instance, Callable, TypeVar, etc.) by recursing into
//! children and rebuilding. Returns `None` for types needing semantic context
//! (UnboundType, PlaceholderType) — exactly the deferral pattern Python uses
//! (lookup_qualified, plugin hooks). `TypeAliasType` is passed through
//! unchanged: `visit_type_alias_type` in `mypy/typeanal.py` is a pure
//! passthrough that returns `t` as-is (args untouched), so the kernel
//! mirrors that instead of expanding or re-analyzing args (re-analysis
//! would rebuild already-bound args, e.g. a `Self`, as fresh wire copies
//! and break object identity; expansion needs the live alias target).
//!
//! The four query helpers defer on `TypeAliasType` (they need alias
//! expansion); `rust_type_analyze` passes it through unchanged instead
//! and defers only on UnboundType and PlaceholderType (symbol lookup,
//! plugin hooks).
//!
//! Live-object queries (Stage 18):
//! - `find_self_type` — BoolTypeQuery with lookup callback, checks SELF_TYPE_NAMES.
//!   `TypeAliasType` expands through the alias snapshot (issue #1157).
//! - `check_vec_type_args` — vec type argument validation.
//! - `is_typevar_default_recursive` — BFS over `default_depends` graph.
//! - `detect_diverging_alias` — DivergingAliasDetector visitor.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple};

use crate::aliases::TypeAliasResolver;
use crate::argmap::{ARG_STAR, ARG_STAR2};
use crate::checkexpr_functions::expanded_alias_target;
use crate::refs::{is_instance, TypeRefs};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{read_type, write_type, ExtraAttrs, Parameters, ReadBuffer, Type, WriteBuffer};

// TypeOfAny constants (mirror mypy/types.py:213-239).
pub(crate) const EXPLICIT: i64 = 2;
pub(crate) const FROM_UNIMPORTED_TYPE: i64 = 3;
const SPECIAL_FORM: i64 = 6;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

// ---------------------------------------------------------------------------
// has_explicit_any / has_any_from_unimported_type
// ---------------------------------------------------------------------------

/// `mypy.typeanal.has_explicit_any` — true if `t` is, or contains, an
/// `AnyType` with `type_of_any == explicit`.
///
/// Mirrors `HasExplicitAny` (typeanal.py:2561-2570): `BoolTypeQuery` with
/// `ANY_STRATEGY`, `visit_any` compares the explicit flag, and
/// `visit_typeddict_type` returns False (TypedDict is checked during its
/// declaration, not here).
#[pyfunction]
pub(crate) fn rust_has_explicit_any(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(has_explicit_any_inner(&t, EXPLICIT)),
        None => Ok(None),
    }
}

/// `mypy.typeanal.has_any_from_unimported_type` — like
/// `has_explicit_any`, but for `type_of_any == from_unimported_type`.
#[pyfunction]
pub(crate) fn rust_has_any_from_unimported_type(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(has_explicit_any_inner(&t, FROM_UNIMPORTED_TYPE)),
        None => Ok(None),
    }
}

/// ANY_STRATEGY (default False) bool query over the wire type, comparing
/// `AnyType.type_of_any` against `wanted`. Any child deferring (alias) makes
/// the whole result defer (`None`) to match the Python fallback.
pub(crate) fn has_explicit_any_inner(t: &Type, wanted: i64) -> Option<bool> {
    // TypeAliasType needs the live target to expand (mirrors
    // rust_has_recursive_types); can't conclude from the wire.
    if matches!(t, Type::TypeAliasType { .. }) {
        return None;
    }
    // visit_typeddict_type -- TypeQuery descends, but HasExplicitAny and
    // HasAnyFromUnimportedType override it to return False.
    if matches!(t, Type::TypedDictType { .. }) {
        return Some(false);
    }
    if let Type::AnyType { type_of_any, .. } = t {
        return Some(*type_of_any == wanted);
    }
    for child in query_children_bool(t) {
        match has_explicit_any_inner(child, wanted) {
            // ANY_STRATEGY: parent true if any child true.
            Some(true) => return Some(true),
            // Defer on any child that needs the alias target.
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// has_explicit_any / has_any_from_unimported_type (resolver-backed)
// ---------------------------------------------------------------------------

/// Resolver-backed `rust_has_explicit_any`: a `TypeAliasType` where the
/// byte-only seam defers (the alias target is not on the wire) expands
/// through the `NativeTypeResolver` alias snapshot, mirroring
/// `BoolTypeQuery.visit_type_alias_type` (type_visitor.py:599-617):
/// the substituted target, then `t.args` for new-style (PEP 695) aliases.
/// A repeated alias on a descent short-circuits to the ANY_STRATEGY
/// default (false), matching `seen_aliases`. Any expansion the kernel
/// cannot perform exactly, or a missing snapshot, defers (`None`) and the
/// Python shim falls back to the pure-Python visitor (parity-safe).
#[pyfunction]
pub(crate) fn rust_has_explicit_any_live(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(has_explicit_any_live_inner(
            &t,
            EXPLICIT,
            resolver.alias_resolver(),
        )),
        None => Ok(None),
    }
}

/// Resolver-backed `rust_has_any_from_unimported_type` — same expansion,
/// different `type_of_any` constant.
#[pyfunction]
pub(crate) fn rust_has_any_from_unimported_type_live(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(has_explicit_any_live_inner(
            &t,
            FROM_UNIMPORTED_TYPE,
            resolver.alias_resolver(),
        )),
        None => Ok(None),
    }
}

fn has_explicit_any_live_inner(t: &Type, wanted: i64, aliases: &TypeAliasResolver) -> Option<bool> {
    let mut seen: Vec<String> = Vec::new();
    has_explicit_any_live_seen(t, wanted, aliases, &mut seen)
}

fn has_explicit_any_live_seen(
    t: &Type,
    wanted: i64,
    aliases: &TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Option<bool> {
    if let Type::TypeAliasType { type_ref, .. } = t {
        if seen.contains(type_ref) {
            return Some(false);
        }
        seen.push(type_ref.clone());
        let (target, args, python_3_12) = expanded_alias_target(t, aliases)?;
        if has_explicit_any_live_seen(&target, wanted, aliases, seen)? {
            return Some(true);
        }
        if python_3_12 {
            for arg in &args {
                if has_explicit_any_live_seen(arg, wanted, aliases, seen)? {
                    return Some(true);
                }
            }
        }
        return Some(false);
    }
    // visit_typeddict_type -- both visitors override it to False (TypedDict
    // is checked during its declaration, not here).
    if matches!(t, Type::TypedDictType { .. }) {
        return Some(false);
    }
    if let Type::AnyType { type_of_any, .. } = t {
        return Some(*type_of_any == wanted);
    }
    for child in query_children_bool(t) {
        if has_explicit_any_live_seen(child, wanted, aliases, seen)? {
            return Some(true);
        }
    }
    Some(false)
}

/// Direct children for a `BoolTypeQuery` traversal (ANY_STRATEGY default
/// False leaves return no children). Mirrors `BoolTypeQuery.visit_*`
/// (type_visitor.py:517-597), including the callable's unconditional
/// instance_type descent and the typevar upper_bound/default/values descent.
fn query_children_bool(t: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match t {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ } => out.push(typ),
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(values.iter());
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(prefix.arg_types.iter());
        }
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
        }
        Type::Parameters(p) => out.extend(p.arg_types.iter()),
        Type::Instance { args, .. } => out.extend(args.iter()),
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            out.extend(arg_types.iter());
            out.push(ret_type);
            if let Some(it) = instance_type {
                out.push(it);
            }
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            out.push(partial_fallback);
            out.extend(items.iter());
        }
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::Overloaded { items } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        Type::AnyType {
            source_any: Some(sa),
            ..
        } => out.push(sa),
        Type::AnyType {
            source_any: None, ..
        } => {}
        // TypeAliasType is handled by the caller (deferred). NoneType,
        // UninhabitedType, DeletedType: no children (default False).
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// collect_all_inner_types
// ---------------------------------------------------------------------------

/// `mypy.typeanal.collect_all_inner_types` — all types `t` contains, in
/// query order: each child's collected inners first, then the direct
/// children. The root is excluded. Deferral is by inner function (`None`)
/// rather than by list return, matching the Python `Option` convention.
#[pyfunction]
pub(crate) fn rust_collect_all_inner_types(type_bytes: &[u8]) -> PyResult<Option<Vec<Vec<u8>>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(collect_all_inner_types_inner(&t).map(|ts| ts.iter().filter_map(encode_type).collect()))
}

/// `CollectAllInnerTypesQuery` (typeanal.py:2601-2607): `query_types`
/// chains each child's collected inners, then appends the direct children.
/// Deferral (`None`) propagates on any TypeAliasType, since its target is
/// a live alias the wire cannot expand.
fn collect_all_inner_types_inner(t: &Type) -> Option<Vec<Type>> {
    if matches!(t, Type::TypeAliasType { .. }) {
        return None;
    }
    let children = query_children_type(t);
    let mut out = Vec::new();
    for child in &children {
        out.extend(collect_all_inner_types_inner(child)?);
    }
    // Direct children of `t` (not `t` itself) are the query result.
    for child in children {
        out.push(child.clone());
    }
    Some(out)
}

/// Resolver-backed `rust_collect_all_inner_types`: a `TypeAliasType`
/// visits the expanded target (mirroring `TypeQuery.visit_type_alias_type`,
/// type_visitor.py:459-469, which is `get_proper_type(t).accept(self)`),
/// with a type_ref-keyed seen set so a repeated alias on a descent returns
/// `strategy([])` = empty, terminating recursive aliases. New-style aliases
/// do NOT additionally visit `t.args` here (TypeQuery has no args visit;
/// the args are already inside the substituted target).
#[pyfunction]
pub(crate) fn rust_collect_all_inner_types_live(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<Vec<Vec<u8>>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let mut seen: Vec<String> = Vec::new();
    Ok(
        collect_all_inner_types_live_inner(&t, resolver.alias_resolver(), &mut seen)
            .map(|ts| ts.iter().filter_map(encode_type).collect()),
    )
}

fn collect_all_inner_types_live_inner(
    t: &Type,
    aliases: &TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Option<Vec<Type>> {
    if let Type::TypeAliasType { type_ref, .. } = t {
        if seen.contains(type_ref) {
            return Some(Vec::new());
        }
        seen.push(type_ref.clone());
        let (target, _, _) = expanded_alias_target(t, aliases)?;
        return collect_all_inner_types_live_inner(&target, aliases, seen);
    }
    let children = query_children_type(t);
    let mut out = Vec::new();
    for child in &children {
        out.extend(collect_all_inner_types_live_inner(child, aliases, seen)?);
    }
    // Direct children of `t` (not `t` itself) are the query result.
    for child in children {
        out.push(child.clone());
    }
    Some(out)
}

/// Direct children for a `TypeQuery` traversal. Mirrors
/// `TypeQuery.visit_*` (type_visitor.py:378-468): arg_types + ret (+
/// instance_type only when not equal to ret_type) for callables, partial
/// fallback + items for tuples, values only for typeddicts, upper +
/// default + values for typevars, prefix arg_types for param specs.
fn query_children_type(t: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match t {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ } => out.push(typ),
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(values.iter());
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
            out.extend(prefix.arg_types.iter());
        }
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            out.push(default);
        }
        Type::Parameters(p) => out.extend(p.arg_types.iter()),
        Type::Instance { args, .. } => out.extend(args.iter()),
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            out.extend(arg_types.iter());
            out.push(ret_type);
            if let Some(it) = instance_type {
                if it.as_ref() != ret_type.as_ref() {
                    out.push(it);
                }
            }
        }
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            out.push(partial_fallback);
            out.extend(items.iter());
        }
        Type::TypedDictType { items, .. } => out.extend(items.iter().map(|(_, t)| t)),
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::Overloaded { items } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        // TypeAliasType is deferred by the caller. AnyType, NoneType,
        // UninhabitedType, DeletedType: no children.
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// make_optional_type
// ---------------------------------------------------------------------------

/// `mypy.typeanal.make_optional_type` — `Optional[t]` without union
/// simplification (typeanal.py:2609-2623).
///
/// - NoneType returns itself.
/// - UnionType keeps items that are not None (via `get_proper_type` in
///   Python; via the wire's own NoneType variant here), prepends a fresh
///   NoneType, and reuses the union's line/column. Truthiness flags are
///   recomputed over the new items exactly like the Python constructor
///   computes after `flatten_nested_unions`.
/// - Any other type (including TypeAliasType, which is not a ProperType
///   and hits the `else` branch) wraps as `UnionType([t, NoneType])`.
///
/// Defers (`None`) when the input union contains an alias (Python filters
/// aliases by expanding them, which the wire cannot do), and when
/// truthiness reconstruction would need the live alias target.
#[pyfunction]
pub(crate) fn rust_make_optional_type(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(make_optional_type_inner(&t).and_then(|res| encode_type(&res)))
}

fn make_optional_type_inner(t: &Type) -> Option<Type> {
    // isinstance(t, ProperType) and isinstance(t, NoneType): return t.
    if matches!(t, Type::NoneType) {
        return Some(t.clone());
    }
    // isinstance(t, ProperType) and isinstance(t, UnionType): filter None
    // items, then append a fresh NoneType.
    if let Type::UnionType {
        items,
        uses_pep604_syntax,
        ..
    } = t
    {
        let mut kept = Vec::with_capacity(items.len() + 1);
        for item in items {
            // Python: `not isinstance(get_proper_type(item), NoneType)`.
            // Defer when expansion could change the answer (alias target
            // unknown on the wire).
            if matches!(item, Type::TypeAliasType { .. }) {
                return None;
            }
            if !matches!(item, Type::NoneType) {
                kept.push(item.clone());
            }
        }
        kept.push(Type::NoneType);
        let can_be_true = kept.iter().any(crate::setops::union_item_can_be_true);
        let can_be_false = kept.iter().any(crate::setops::union_item_can_be_false);
        return Some(Type::UnionType {
            items: kept,
            uses_pep604_syntax: *uses_pep604_syntax,
            can_be_true,
            can_be_false,
        });
    }
    // else: wrap Non-None-type values in a UnionType.
    let items = vec![t.clone(), Type::NoneType];
    let can_be_true = items.iter().any(crate::setops::union_item_can_be_true);
    let can_be_false = items.iter().any(crate::setops::union_item_can_be_false);
    Some(Type::UnionType {
        items,
        uses_pep604_syntax: false,
        can_be_true,
        can_be_false,
    })
}

/// Resolver-backed `rust_make_optional_type`: when filtering the items of
/// an input union, an alias item expands through the snapshot to decide
/// whether `get_proper_type(item)` is a `NoneType` (dropped, absorbed by
/// the appended fresh `NoneType`). Non-None aliases are kept in the output
/// AS-IS (Python keeps the original item, not the expansion). An expansion
/// the kernel cannot perform exactly defers the whole call (parity-safe).
#[pyfunction]
pub(crate) fn rust_make_optional_type_live(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<Vec<u8>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(make_optional_type_live_inner(&t, resolver.alias_resolver())
        .and_then(|res| encode_type(&res)))
}

fn make_optional_type_live_inner(t: &Type, aliases: &TypeAliasResolver) -> Option<Type> {
    // isinstance(t, ProperType) and isinstance(t, NoneType): return t.
    if matches!(t, Type::NoneType) {
        return Some(t.clone());
    }
    if let Type::UnionType {
        items,
        uses_pep604_syntax,
        ..
    } = t
    {
        let mut kept = Vec::with_capacity(items.len() + 1);
        for item in items {
            match item {
                Type::TypeAliasType { .. } => {
                    let (target, _, _) = expanded_alias_target(item, aliases)?;
                    if matches!(&target, Type::NoneType) {
                        continue;
                    }
                    kept.push(item.clone());
                }
                Type::NoneType => {}
                _ => kept.push(item.clone()),
            }
        }
        kept.push(Type::NoneType);
        let can_be_true = kept.iter().any(crate::setops::union_item_can_be_true);
        let can_be_false = kept.iter().any(crate::setops::union_item_can_be_false);
        return Some(Type::UnionType {
            items: kept,
            uses_pep604_syntax: *uses_pep604_syntax,
            can_be_true,
            can_be_false,
        });
    }
    // else: wrap Non-None-type values in a UnionType.
    let items = vec![t.clone(), Type::NoneType];
    let can_be_true = items.iter().any(crate::setops::union_item_can_be_true);
    let can_be_false = items.iter().any(crate::setops::union_item_can_be_false);
    Some(Type::UnionType {
        items,
        uses_pep604_syntax: false,
        can_be_true,
        can_be_false,
    })
}

// ---------------------------------------------------------------------------
// unknown_unpack
// ---------------------------------------------------------------------------

/// `mypy.typeanal.unknown_unpack` — true if `t` is an unpack of an unknown
/// type: `UnpackType` whose proper type is `AnyType(TypeOfAny.special_form)`.
///
/// Mirrors typeanal.py:2856-2867. Pure query: returns `None` (defer) when
/// the unpack target is a `TypeAliasType`, whose proper expansion is not
/// available on the wire (`get_proper_type` needs the live alias target).
#[pyfunction]
pub(crate) fn rust_unknown_unpack(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(unknown_unpack_inner(&t)),
        None => Ok(None),
    }
}

fn unknown_unpack_inner(t: &Type) -> Option<bool> {
    let Type::UnpackType { typ } = t else {
        return Some(false);
    };
    // get_proper_type(UnpackType.typ): non-alias wire types are already
    // proper, so only the alias case is indeterminate on the wire.
    match typ.as_ref() {
        Type::TypeAliasType { .. } => None,
        // isinstance(unpacked, AnyType) and type_of_any == special_form.
        Type::AnyType { type_of_any, .. } => Some(*type_of_any == SPECIAL_FORM),
        _ => Some(false),
    }
}

/// Resolver-backed `rust_unknown_unpack`: the unpack target's alias chain
/// expands through the snapshot (Python `get_proper_type(t.type)`), then
/// the same special-form AnyType check as the byte-only seam. A missing
/// snapshot or an undecidable expansion defers (parity-safe).
#[pyfunction]
pub(crate) fn rust_unknown_unpack_live(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    match decode_type(type_bytes) {
        Some(t) => Ok(unknown_unpack_live_inner(&t, resolver.alias_resolver())),
        None => Ok(None),
    }
}

fn unknown_unpack_live_inner(t: &Type, aliases: &TypeAliasResolver) -> Option<bool> {
    let Type::UnpackType { typ } = t else {
        return Some(false);
    };
    match typ.as_ref() {
        Type::TypeAliasType { .. } => {
            let (target, _, _) = expanded_alias_target(typ, aliases)?;
            match &target {
                Type::AnyType { type_of_any, .. } => Some(*type_of_any == SPECIAL_FORM),
                _ => Some(false),
            }
        }
        Type::AnyType { type_of_any, .. } => Some(*type_of_any == SPECIAL_FORM),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// find_self_type (live-object BoolTypeQuery with lookup callback)
// ---------------------------------------------------------------------------

/// `mypy.typeanal.find_self_type` — true if `typ` contains a Self type.
///
/// Mirrors `HasSelfType` (typeanal.py:4361-4370): a `BoolTypeQuery` with
/// `ANY_STRATEGY` that overrides `visit_unbound_type` to check if the
/// unbound name resolves to a symbol in `SELF_TYPE_NAMES`. All other
/// type variants use the default `BoolTypeQuery` descent (ANY_STRATEGY
/// over children).
///
/// `lookup` is a Python callable `Callable[[str], SymbolTableNode | None]`.
/// Returns `None` (defer) when a type variant is not handled by Rust, or
/// when a `TypeAliasType` cannot be expanded over live objects. With no
/// resolver installed (issue #1308), the live alias expansion still
/// decides `UnboundType`-carrying shapes; the Python fallback then runs
/// the full `HasSelfType` visitor only in genuinely undecidable
/// window cases.
#[pyfunction]
pub(crate) fn rust_find_self_type(
    py: Python<'_>,
    typ: &PyAny,
    lookup: &PyAny,
) -> PyResult<Option<bool>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let mut ctx = SelfTypeCtx {
        refs: &refs,
        lookup: &SelfLookup::Py(lookup),
        aliases: None,
        seen_aliases: Vec::new(),
        subst: Vec::new(),
    };
    run_find_self_type(py, &mut ctx, typ)
}

/// Resolver-backed `rust_find_self_type` (issue #1157): the live walk is
/// identical, but a `TypeAliasType` expands through the
/// `NativeTypeResolver` alias snapshot, mirroring
/// `BoolTypeQuery.visit_type_alias_type` (type_visitor.py:599-617):
/// the substituted target, plus `t.args` for new-style (PEP 695)
/// aliases, folded with ANY_STRATEGY. A missing snapshot, an unreadable
/// alias node, or an undecidable expansion defers (`None`) to the
/// pure-Python visitor.
#[pyfunction]
pub(crate) fn rust_find_self_type_live(
    py: Python<'_>,
    resolver: &NativeTypeResolver,
    typ: &PyAny,
    lookup: &PyAny,
) -> PyResult<Option<bool>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let mut ctx = SelfTypeCtx {
        refs: &refs,
        lookup: &SelfLookup::Py(lookup),
        aliases: Some(resolver.alias_resolver()),
        seen_aliases: Vec::new(),
        subst: Vec::new(),
    };
    run_find_self_type(py, &mut ctx, typ)
}

fn run_find_self_type(
    py: Python<'_>,
    ctx: &mut SelfTypeCtx<'_>,
    typ: &PyAny,
) -> PyResult<Option<bool>> {
    match find_self_type_inner(py, typ, ctx) {
        Ok(b) => Ok(Some(b)),
        Err(DeferError) => Ok(None),
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct DeferError;

struct SelfTypeCtx<'a> {
    refs: &'a TypeRefs<'a>,
    lookup: &'a SelfLookup<'a>,
    /// Alias snapshots for the resolver-backed `TypeAliasType` expansion;
    /// `None` (the pre-first-SCC semanal window) takes the live expansion.
    aliases: Option<&'a TypeAliasResolver>,
    /// `self.seen_aliases`: identity guard over the whole query,
    /// push-never-pop like the Python set (type_visitor.py:602-608).
    seen_aliases: Vec<usize>,
    /// Alias-tvar substitution for a live no-snapshot alias expansion
    /// (`(tvar.id, t.args[i])` pairs); empty when unsubstituted.
    subst: Vec<(Py<PyAny>, Py<PyAny>)>,
}

/// `HasSelfType.lookup` shim: the production seam takes a live Python
/// callable; the cfg(test) unit tests use a plain name-to-fullname table.
enum SelfLookup<'a> {
    Py(&'a PyAny),
    #[cfg(test)]
    Table(std::collections::HashMap<String, String>),
}

impl SelfLookup<'_> {
    /// `sym.fullname` for `lookup(name)`; `Ok(None)` on a miss, defer
    /// on an unreadable symbol or a raising callback.
    fn fullname(&self, name: &str) -> Result<Option<String>, DeferError> {
        match self {
            SelfLookup::Py(cb) => {
                let sym = cb.call1((name,)).map_err(|_| DeferError)?;
                if sym.is_none() {
                    return Ok(None);
                }
                let fullname = sym.getattr("fullname").map_err(|_| DeferError)?;
                if fullname.is_none() {
                    return Ok(None);
                }
                let s = fullname.downcast::<PyString>().map_err(|_| DeferError)?;
                s.to_str()
                    .map(|f| Some(f.to_string()))
                    .map_err(|_| DeferError)
            }
            #[cfg(test)]
            SelfLookup::Table(table) => Ok(table.get(name).cloned()),
        }
    }
}

/// `SELF_TYPE_NAMES` (typeanal.py:146).
fn is_self_fullname(fullname: &str) -> bool {
    fullname == "typing.Self" || fullname == "typing_extensions.Self"
}

fn find_self_type_inner(
    py: Python<'_>,
    obj: &PyAny,
    ctx: &mut SelfTypeCtx<'_>,
) -> Result<bool, DeferError> {
    // UnboundType: lookup name, check SELF_TYPE_NAMES, then descend args.
    if class_name_is(obj, "UnboundType") {
        return self_type_visit_unbound(py, obj, ctx);
    }
    // TypeVar-like types: query the Python child lists (issue #1122),
    // mirroring visit_type_var / visit_param_spec / visit_type_var_tuple
    // in type_visitor.py.
    if is_instance(obj, ctx.refs.type_var_type) {
        // Live alias substitution: an alias tvar occurrence is replaced by
        // the corresponding `t.args` element (InstantiateAliasVisitor
        // semantics via TypeVarId equality).
        if !ctx.subst.is_empty() {
            let id = obj.getattr("id").map_err(|_| DeferError)?;
            for (key, arg) in &ctx.subst {
                if key.as_ref(py).eq(id).map_err(|_| DeferError)? {
                    let arg = arg.clone();
                    // The substituted arg is already instantiated
                    // (expand_type semantics): never re-walk it under the
                    // same subst, or `A[T]` with args `[T]` recurses forever.
                    let outer = std::mem::take(&mut ctx.subst);
                    let out = find_self_type_inner(py, arg.as_ref(py), ctx);
                    ctx.subst = outer;
                    return out;
                }
            }
        }
        let upper = get_attr_or_defer(obj, "upper_bound")?;
        if find_self_type_inner(py, upper, ctx)? {
            return Ok(true);
        }
        let default = get_attr_or_defer(obj, "default")?;
        if find_self_type_inner(py, default, ctx)? {
            return Ok(true);
        }
        let values = get_attr_or_defer(obj, "values")?;
        return self_type_any_seq(py, values, ctx);
    }
    if is_instance(obj, ctx.refs.param_spec_type) {
        let upper = get_attr_or_defer(obj, "upper_bound")?;
        if find_self_type_inner(py, upper, ctx)? {
            return Ok(true);
        }
        let default = get_attr_or_defer(obj, "default")?;
        if find_self_type_inner(py, default, ctx)? {
            return Ok(true);
        }
        let prefix = get_attr_or_defer(obj, "prefix")?;
        return find_self_type_inner(py, prefix, ctx);
    }
    if is_instance(obj, ctx.refs.type_var_tuple_type) {
        let upper = get_attr_or_defer(obj, "upper_bound")?;
        if find_self_type_inner(py, upper, ctx)? {
            return Ok(true);
        }
        let default = get_attr_or_defer(obj, "default")?;
        return find_self_type_inner(py, default, ctx);
    }
    // AnyType: no Self type.
    if is_instance(obj, ctx.refs.any_type) {
        return Ok(false);
    }
    // NoneType, UninhabitedType, DeletedType, LiteralType: no children.
    if is_instance(obj, ctx.refs.none_type)
        || is_instance(obj, ctx.refs.uninhabited_type)
        || is_instance(obj, ctx.refs.deleted_type)
        || is_instance(obj, ctx.refs.literal_type)
    {
        return Ok(false);
    }
    // UnpackType: recurse into typ.
    if is_instance(obj, ctx.refs.unpack_type) {
        let typ = get_attr_or_defer(obj, "type")?;
        return find_self_type_inner(py, typ, ctx);
    }
    // Instance: recurse into args + last_known_value.
    if is_instance(obj, ctx.refs.instance) {
        return self_type_any_children(py, obj, &["args"], ctx, true);
    }
    // CallableType: recurse arg_types, ret_type, instance_type.
    if is_instance(obj, ctx.refs.callable_type) {
        return self_type_callable(py, obj, ctx);
    }
    // Overloaded: recurse items.
    if is_instance(obj, ctx.refs.overloaded) {
        let items = get_attr_or_defer(obj, "items")?;
        return self_type_any_seq(py, items, ctx);
    }
    // TupleType: recurse items + partial_fallback.
    if is_instance(obj, ctx.refs.tuple_type) {
        let items = get_attr_or_defer(obj, "items")?;
        if self_type_any_seq(py, items, ctx)? {
            return Ok(true);
        }
        let fb = get_attr_or_defer(obj, "partial_fallback")?;
        return find_self_type_inner(py, fb, ctx);
    }
    // TypedDictType: recurse items values only (visit_typeddict_type in
    // type_visitor.py does not descend into the fallback; #1122).
    if is_instance(obj, ctx.refs.typed_dict_type) {
        let items = get_attr_or_defer(obj, "items")?;
        let dict: &PyDict = items.downcast().map_err(|_| DeferError)?;
        for (_, v) in dict.iter() {
            if find_self_type_inner(py, v, ctx)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    // UnionType: recurse items (ANY_STRATEGY).
    if is_instance(obj, ctx.refs.union_type) {
        let items = get_attr_or_defer(obj, "items")?;
        return self_type_any_seq(py, items, ctx);
    }
    // TypeType: recurse item.
    if is_instance(obj, ctx.refs.type_type) {
        let item = get_attr_or_defer(obj, "item")?;
        return find_self_type_inner(py, item, ctx);
    }
    // TypeAliasType: expand through the alias snapshot (issue #1157);
    // with no snapshot (the pre-first-SCC semanal window) take the live
    // expansion instead (see self_type_visit_alias_live).
    if is_instance(obj, ctx.refs.type_alias_type) {
        if ctx.aliases.is_some() {
            return self_type_visit_alias(py, obj, ctx);
        }
        return self_type_visit_alias_live(py, obj, ctx);
    }
    // Parameters: recurse arg_types.
    if class_name_is(obj, "Parameters") {
        let arg_types = get_attr_or_defer(obj, "arg_types")?;
        return self_type_any_seq(py, arg_types, ctx);
    }
    // TypeList: query_types(t.items), matching BoolTypeQuery's
    // visit_type_list (syntactic list inside Callable arg position).
    if class_name_is(obj, "TypeList") {
        let items = get_attr_or_defer(obj, "items")?;
        return self_type_any_seq(py, items, ctx);
    }
    // EllipsisType: strategy([]) -> False (bare Callable[..., T]).
    if class_name_is(obj, "EllipsisType") {
        return Ok(false);
    }
    // RawExpressionType: strategy([]) -> False (invalid type literals).
    if class_name_is(obj, "RawExpressionType") {
        return Ok(false);
    }
    // Unknown type variant: defer.
    Err(DeferError)
}

/// `visit_unbound_type`: lookup name, check SELF_TYPE_NAMES, then args.
fn self_type_visit_unbound(
    py: Python<'_>,
    obj: &PyAny,
    ctx: &mut SelfTypeCtx<'_>,
) -> Result<bool, DeferError> {
    let name = obj.getattr("name").map_err(|_| DeferError)?;
    let name_str: String = if name.is_none() {
        return Err(DeferError);
    } else {
        let s = name.downcast::<PyString>().map_err(|_| DeferError)?;
        s.to_str().map(|s| s.to_string()).map_err(|_| DeferError)?
    };
    // Call lookup(name) -> SymbolTableNode | None.
    if let Some(fullname) = ctx.lookup.fullname(&name_str)? {
        if is_self_fullname(&fullname) {
            return Ok(true);
        }
    }
    // super().visit_unbound_type(t) -> query_types(t.args).
    let args = obj.getattr("args").map_err(|_| DeferError)?;
    self_type_any_seq(py, args, ctx)
}

/// ANY_STRATEGY over a sequence of child types.
fn self_type_any_seq(
    py: Python<'_>,
    seq: &PyAny,
    ctx: &mut SelfTypeCtx<'_>,
) -> Result<bool, DeferError> {
    for child in iter_seq(seq)? {
        if find_self_type_inner(py, child, ctx)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// ANY_STRATEGY over named attrs, optionally also last_known_value.
fn self_type_any_children(
    py: Python<'_>,
    obj: &PyAny,
    attrs: &[&str],
    ctx: &mut SelfTypeCtx<'_>,
    check_lkv: bool,
) -> Result<bool, DeferError> {
    for attr_name in attrs {
        let seq = get_attr_or_defer(obj, attr_name)?;
        if self_type_any_seq(py, seq, ctx)? {
            return Ok(true);
        }
    }
    if check_lkv {
        let lkv = obj.getattr("last_known_value").map_err(|_| DeferError)?;
        if !lkv.is_none() && find_self_type_inner(py, lkv, ctx)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// ANY_STRATEGY over callable's arg_types, ret_type, instance_type.
fn self_type_callable(
    py: Python<'_>,
    obj: &PyAny,
    ctx: &mut SelfTypeCtx<'_>,
) -> Result<bool, DeferError> {
    let arg_types = get_attr_or_defer(obj, "arg_types")?;
    if self_type_any_seq(py, arg_types, ctx)? {
        return Ok(true);
    }
    let ret = get_attr_or_defer(obj, "ret_type")?;
    if find_self_type_inner(py, ret, ctx)? {
        return Ok(true);
    }
    let inst = obj.getattr("instance_type").map_err(|_| DeferError)?;
    if !inst.is_none() && find_self_type_inner(py, inst, ctx)? {
        return Ok(true);
    }
    Ok(false)
}

/// `BoolTypeQuery.visit_type_alias_type` (type_visitor.py:599-617)
/// resolver-backed: expand the proper target through the alias snapshot,
/// then re-query the node's own args for new-style aliases. Seen-guard
/// keyed by node identity, push-never-pop like the Python set. Called
/// only with a snapshot installed; missing snapshots, substituted
/// expansions (the live path handles those), and unreadable facts defer.
fn self_type_visit_alias(
    py: Python<'_>,
    obj: &PyAny,
    ctx: &mut SelfTypeCtx<'_>,
) -> Result<bool, DeferError> {
    let key = obj.as_ptr() as usize;
    if ctx.seen_aliases.contains(&key) {
        return Ok(false);
    }
    ctx.seen_aliases.push(key);
    let aliases = match ctx.aliases {
        Some(a) => a,
        None => {
            return Err(DeferError);
        }
    };
    let alias = obj.getattr("alias").map_err(|_| DeferError)?;
    if alias.is_none() {
        // Python asserts `t.alias is not None`; defer so the fallback
        // surfaces the same error.
        return Err(DeferError);
    }
    let fullname = alias.getattr("fullname").map_err(|_| DeferError)?;
    let type_ref = fullname
        .downcast::<PyString>()
        .map_err(|_| DeferError)?
        .to_str()
        .map_err(|_| DeferError)?
        .to_string();
    let args_seq = get_attr_or_defer(obj, "args")?;
    let args = iter_seq(args_seq)?;
    let snap = match aliases.get(&type_ref) {
        Some(s) => s,
        None => {
            return Err(DeferError);
        }
    };
    if snap.no_args && !args.is_empty() {
        // Python copies the live args into the bare target
        // (types.py:453-457) — not representable from the snapshot.
        return Err(DeferError);
    }
    if !snap.no_args && !snap.alias_tvars.is_empty() {
        // Substitution needs the live args on the wire; conservative
        // defer, the Python visitor answers identically from the live
        // expansion.
        return Err(DeferError);
    }
    let bare = Type::TypeAliasType {
        args: Vec::new(),
        type_ref: type_ref.clone(),
    };
    let (target, _, py312) = match expanded_alias_target(&bare, aliases) {
        Some(t) => t,
        None => {
            return Err(DeferError);
        }
    };
    let target = if snap.no_args {
        // `_expand_once` for a no_args alias asserts an Instance target
        // and writes `args=[]` over it (types.py:453-457, the live args
        // are empty here).
        match target {
            Type::Instance {
                type_ref,
                last_known_value,
                extra_attrs,
                ..
            } => Type::Instance {
                type_ref,
                args: Vec::new(),
                last_known_value,
                extra_attrs,
            },
            _ => return Err(DeferError),
        }
    } else {
        target
    };
    let mut wire_seen = Vec::new();
    if find_self_type_wire(&target, ctx.lookup, aliases, &mut wire_seen)? {
        return Ok(true);
    }
    if py312 && !args.is_empty() {
        // Weird edge case in the Python visitor (type_visitor.py:610-615):
        // if the expansion found nothing, still visit the node's own
        // arguments — done only for new-style aliases.
        return self_type_any_seq(py, args_seq, ctx);
    }
    Ok(false)
}

/// Substitute-substitution chain resolution for the live alias expansion
/// (OCR finding, issue #1311): a nested alias's arg is an occurrence in
/// the OUTER expansion context, so before it becomes the inner map's
/// value it must be resolved through the outer subst, mirroring
/// InstantiateAliasVisitor eagerly rewriting nested alias args under the
/// outer variable map (e.g. `A[T] = B[T]`, `B[S] = list[S]`, use site
/// `A[Self]` -> `list[Self]`). Acyclic chains are bounded by
/// `subst.len()`; a cycle defers (parity-safe).
fn resolve_subst_chain(
    py: Python<'_>,
    arg: &PyAny,
    subst: &[(Py<PyAny>, Py<PyAny>)],
) -> Result<Py<PyAny>, DeferError> {
    let mut cur = arg.into_py(py);
    let mut steps = 0;
    while steps <= subst.len() {
        steps += 1;
        let mut matched: Option<Py<PyAny>> = None;
        if let Ok(id) = cur.as_ref(py).getattr("id") {
            for (key, val) in subst {
                if key.as_ref(py).eq(id).map_err(|_| DeferError)? {
                    matched = Some(val.clone());
                    break;
                }
            }
        }
        match matched {
            Some(v) => cur = v,
            None => return Ok(cur),
        }
    }
    Err(DeferError)
}

/// No-snapshot live alias expansion for the pre-first-SCC semanal window
/// (issue #1308): mirrors `BoolTypeQuery.visit_type_alias_type`
/// (type_visitor.py:599-617) plus `TypeAliasType._expand_once`
/// (types.py:436-461) over live objects instead of the resolver snapshot.
/// Decisions:
/// - `alias.tvar_tuple_index` set: the middle-split mapping is not
///   representable in the tvar map; defer.
/// - `no_args`: the target is asserted to be an `Instance` and the query
///   is over `t.args` (the substituted instance's only queried children).
/// - otherwise: walk `alias.target` with the `(tvar.id -> t.args[i])`
///   substitution map (zipped, matching Python's `zip` truncation), then
///   `t.args` for new-style aliases when the target found nothing.
/// - `alias is None` or an unreadable fact: defer (the Python fallback
///   surfaces the same assert/error).
fn self_type_visit_alias_live(
    py: Python<'_>,
    obj: &PyAny,
    ctx: &mut SelfTypeCtx<'_>,
) -> Result<bool, DeferError> {
    let key = obj.as_ptr() as usize;
    if ctx.seen_aliases.contains(&key) {
        return Ok(false);
    }
    ctx.seen_aliases.push(key);
    let alias = obj.getattr("alias").map_err(|_| DeferError)?;
    if alias.is_none() {
        // Python asserts `t.alias is not None`; defer so the fallback
        // surfaces the same error.
        return Err(DeferError);
    }
    let target = alias.getattr("target").map_err(|_| DeferError)?;
    let py312 = alias
        .getattr("python_3_12_type_alias")
        .map_err(|_| DeferError)?
        .is_true()
        .map_err(|_| DeferError)?;
    let no_args = alias
        .getattr("no_args")
        .map_err(|_| DeferError)?
        .is_true()
        .map_err(|_| DeferError)?;
    let args_seq = get_attr_or_defer(obj, "args")?;
    if no_args {
        // Python asserts an Instance target and copies `t.args` over it
        // (types.py:441-444); the substituted instance's only queried
        // children are its args.
        let is_inst = is_instance(target, ctx.refs.instance);
        if !is_inst {
            return Err(DeferError);
        }
        return self_type_any_seq(py, args_seq, ctx);
    }
    let tvt_index = alias.getattr("tvar_tuple_index").map_err(|_| DeferError)?;
    if !tvt_index.is_none() {
        // split_with_prefix_and_suffix mapping: defer (parity-safe).
        return Err(DeferError);
    }
    let tvars = alias.getattr("alias_tvars").map_err(|_| DeferError)?;
    let tvars: Vec<Py<PyAny>> = iter_seq(tvars)?
        .into_iter()
        .map(|t| t.into_py(py))
        .collect();
    let args: Vec<Py<PyAny>> = iter_seq(args_seq)?
        .into_iter()
        .map(|t| t.into_py(py))
        .collect();
    let outer = std::mem::take(&mut ctx.subst);
    let mut inner = Vec::new();
    for (tv, arg) in tvars.iter().zip(args.iter()) {
        let id = tv.as_ref(py).getattr("id").map_err(|_| DeferError)?;
        let id = id.into_py(py);
        inner.push((id, resolve_subst_chain(py, arg.as_ref(py), &outer)?));
    }
    ctx.subst = inner;
    let out = find_self_type_inner(py, target, ctx);
    ctx.subst = outer;
    let res = out?;
    if !res && py312 && !args.is_empty() {
        // Weird edge case (type_visitor.py:610-615): visit the node's own
        // arguments when the expansion found nothing, new-style only.
        return self_type_any_seq(py, args_seq, ctx);
    }
    Ok(res)
}

/// Wire-mode `HasSelfType` query over an expanded alias target with no
/// live Python types. Decides the same leaf shapes as the live walk and
/// defers on anything else. The `seen` set substitutes for the Python
/// node-identity guard: aliases with substituted or copied args are
/// already excluded upstream, so a type_ref's expansion is fixed for
/// the whole query.
fn find_self_type_wire(
    t: &Type,
    lookup: &SelfLookup<'_>,
    aliases: &TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Result<bool, DeferError> {
    match t {
        Type::TypeAliasType { args, type_ref } => {
            if seen.contains(type_ref) {
                return Ok(false);
            }
            seen.push(type_ref.clone());
            let (target, _, py312) = match expanded_alias_target(t, aliases) {
                Some(x) => x,
                None => {
                    return Err(DeferError);
                }
            };
            if find_self_type_wire(&target, lookup, aliases, seen)? {
                return Ok(true);
            }
            if py312 && !args.is_empty() {
                return find_self_type_wire_seq(args, lookup, aliases, seen);
            }
            Ok(false)
        }
        Type::UnboundType { name, args, .. } => {
            if let Some(fullname) = lookup.fullname(name)? {
                if is_self_fullname(&fullname) {
                    return Ok(true);
                }
            }
            find_self_type_wire_seq(args, lookup, aliases, seen)
        }
        Type::Instance {
            args,
            last_known_value,
            ..
        } => {
            if find_self_type_wire_seq(args, lookup, aliases, seen)? {
                return Ok(true);
            }
            match last_known_value {
                Some(lkv) => find_self_type_wire(lkv, lookup, aliases, seen),
                None => Ok(false),
            }
        }
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            if find_self_type_wire_seq(arg_types, lookup, aliases, seen)? {
                return Ok(true);
            }
            if find_self_type_wire(ret_type, lookup, aliases, seen)? {
                return Ok(true);
            }
            match instance_type {
                Some(inst) => find_self_type_wire(inst, lookup, aliases, seen),
                None => Ok(false),
            }
        }
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            if find_self_type_wire_seq(items, lookup, aliases, seen)? {
                return Ok(true);
            }
            find_self_type_wire(partial_fallback, lookup, aliases, seen)
        }
        Type::TypedDictType { items, .. } => {
            for (_, item) in items {
                if find_self_type_wire(item, lookup, aliases, seen)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Type::UnionType { items, .. } | Type::Overloaded { items } => {
            find_self_type_wire_seq(items, lookup, aliases, seen)
        }
        Type::TypeType { item, .. } => find_self_type_wire(item, lookup, aliases, seen),
        Type::UnpackType { typ } => find_self_type_wire(typ, lookup, aliases, seen),
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            if find_self_type_wire(upper_bound, lookup, aliases, seen)? {
                return Ok(true);
            }
            if find_self_type_wire(default, lookup, aliases, seen)? {
                return Ok(true);
            }
            find_self_type_wire_seq(values, lookup, aliases, seen)
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            if find_self_type_wire(upper_bound, lookup, aliases, seen)? {
                return Ok(true);
            }
            if find_self_type_wire(default, lookup, aliases, seen)? {
                return Ok(true);
            }
            find_self_type_wire_seq(&prefix.arg_types, lookup, aliases, seen)
        }
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            if find_self_type_wire(upper_bound, lookup, aliases, seen)? {
                return Ok(true);
            }
            find_self_type_wire(default, lookup, aliases, seen)
        }
        Type::Parameters(p) => find_self_type_wire_seq(&p.arg_types, lookup, aliases, seen),
        Type::AnyType { .. }
        | Type::NoneType
        | Type::UninhabitedType { .. }
        | Type::ErasedType
        | Type::DeletedType { .. }
        // visit_literal_type: strategy([]), the fallback is not queried.
        | Type::LiteralType { .. } => Ok(false),
    }
}

/// ANY_STRATEGY fold over a wire sequence of child types.
fn find_self_type_wire_seq(
    types: &[Type],
    lookup: &SelfLookup<'_>,
    aliases: &TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Result<bool, DeferError> {
    for child in types {
        if find_self_type_wire(child, lookup, aliases, seen)? {
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// validate_instance (live Instance argument count/position validation)
// ---------------------------------------------------------------------------

/// `mypy.typeanal.validate_instance` — check well-formedness of an
/// `Instance` with respect to argument count/positions.
///
/// Mirrors typeanal.py:2768-2839. Returns `None` (defer) when any
/// sub-check needs information Rust can't access (e.g. unknown_unpack
/// on a type the wire can't decode), so the Python caller re-runs the
/// full validation. When the check passes or fails deterministically,
/// returns `Some(true)` or `Some(false)`. The `fail` callback is a
/// Python `MsgCallback` that takes `(message, context, code=...)`.
#[pyfunction]
pub(crate) fn rust_validate_instance(
    py: Python<'_>,
    t: &PyAny,
    fail: &PyAny,
    indexed: bool,
) -> PyResult<Option<bool>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    match validate_instance_inner(py, t, fail, indexed, &refs) {
        Ok(b) => Ok(Some(b)),
        Err(DeferError) => Ok(None),
    }
}

fn validate_instance_inner(
    py: Python<'_>,
    t: &PyAny,
    fail: &PyAny,
    indexed: bool,
    refs: &TypeRefs<'_>,
) -> Result<bool, DeferError> {
    let args = get_attr_or_defer(t, "args")?;
    let arg_list = iter_seq(args)?;
    // any(unknown_unpack(a) for a in t.args) — defer on unknown types.
    for a in &arg_list {
        if !is_instance(a, refs.unpack_type) {
            continue;
        }
        // unknown_unpack: UnpackType whose proper type is
        // AnyType(special_form). Defer on alias target.
        let unpacked = a.getattr("type").map_err(|_| DeferError)?;
        if is_instance(unpacked, refs.type_alias_type) {
            return Err(DeferError);
        }
        if is_instance(unpacked, refs.any_type) {
            let toa = unpacked.getattr("type_of_any").map_err(|_| DeferError)?;
            let sf = refs
                .type_of_any
                .getattr("special_form")
                .map_err(|_| DeferError)?;
            if toa.eq(sf).unwrap_or(false) {
                return Ok(false);
            }
        }
    }
    let empty_tuple_index = indexed && arg_list.is_empty();
    let typ_type = get_attr_or_defer(t, "type")?;

    let has_tvt = typ_type
        .getattr("has_type_var_tuple_type")
        .map_err(|_| DeferError)?;
    let has_tvt: bool = has_tvt.is_true().map_err(|_| DeferError)?;

    if has_tvt {
        return validate_instance_variadic(py, t, fail, &arg_list, empty_tuple_index, typ_type);
    }

    // Non-variadic path.
    if arg_list.iter().any(|a| is_instance(a, refs.unpack_type)) {
        // Variadic unpack in fixed-size instance.
        fail_call(py, fail, "Invalid unpack position", t);
        // t.args = () — set on the live object.
        let _ = t.setattr("args", PyList::empty(py));
        return Ok(false);
    }

    let type_vars = typ_type.getattr("type_vars").map_err(|_| DeferError)?;
    let tv_list = iter_seq(type_vars)?;
    let expected = tv_list.len();
    if arg_list.len() != expected {
        // Check min_tv_count and emit error, but always return false.
        let defn = typ_type.getattr("defn").map_err(|_| DeferError)?;
        let defn_type_vars = defn.getattr("type_vars").map_err(|_| DeferError)?;
        let defn_list = iter_seq(defn_type_vars)?;
        let min_tv_count = defn_list
            .iter()
            .filter(|tv| {
                let has_def = tv
                    .call_method0("has_default")
                    .map(|v| v.is_true().unwrap_or(true))
                    .unwrap_or(true);
                !has_def
            })
            .count();
        let arg_count = arg_list.len();
        if (arg_count > 0 || empty_tuple_index)
            && (arg_count < min_tv_count || arg_count > expected)
        {
            let type_name = typ_type.getattr("name").map_err(|_| DeferError)?;
            let name_str = type_name
                .downcast::<PyString>()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let msg = wrong_type_arg_count_msg(min_tv_count, expected, arg_count, &name_str);
            fail_call(py, fail, &msg, t);
        }
        return Ok(false);
    }
    Ok(true)
}

/// Variadic instance validation (has_type_var_tuple_type == True).
fn validate_instance_variadic(
    py: Python<'_>,
    t: &PyAny,
    fail: &PyAny,
    arg_list: &[&PyAny],
    empty_tuple_index: bool,
    typ_type: &PyAny,
) -> Result<bool, DeferError> {
    let refs = TypeRefs::try_new(py).map_err(|_| DeferError)?;
    let defn = typ_type.getattr("defn").map_err(|_| DeferError)?;
    let defn_type_vars = defn.getattr("type_vars").map_err(|_| DeferError)?;
    let defn_list = iter_seq(defn_type_vars)?;
    let min_tv_count = defn_list
        .iter()
        .filter(|tv| {
            let has_def = tv
                .call_method0("has_default")
                .map(|v| v.is_true().unwrap_or(true))
                .unwrap_or(true);
            let is_tvt = is_instance(tv, refs.type_var_tuple_type);
            !has_def && !is_tvt
        })
        .count();
    // Python: correct = len(t.args) >= min_tv_count
    //         if any(unpack-Instance): correct = True
    let mut correct = arg_list.len() >= min_tv_count;
    for a in arg_list {
        if !is_instance(a, refs.unpack_type) {
            continue;
        }
        let inner = match a.getattr("type") {
            Ok(i) => i,
            Err(_) => continue,
        };
        if is_instance(inner, refs.type_alias_type) {
            return Err(DeferError);
        }
        if is_instance(inner, refs.instance) {
            correct = true;
            break;
        }
    }

    if arg_list.is_empty() {
        if !(empty_tuple_index && {
            let tvs = typ_type.getattr("type_vars").map_err(|_| DeferError)?;
            iter_seq(tvs)?.len() == 1
        }) {
            if empty_tuple_index && min_tv_count > 0 {
                let msg = format!("At least {min_tv_count} type argument(s) expected, none given");
                fail_call(py, fail, &msg, t);
            }
            return Ok(false);
        }
        return Ok(true);
    }
    if !correct {
        let msg = format!(
            "Bad number of arguments, expected: at least {min_tv_count}, \
             given: {}",
            arg_list.len()
        );
        fail_call(py, fail, &msg, t);
        return Ok(false);
    }
    // Check TypeVarTuple split.
    let unpack_idx = arg_list
        .iter()
        .position(|a| is_instance(a, refs.unpack_type));
    if let Some(idx) = unpack_idx {
        let unpack_arg = arg_list[idx];
        let inner = unpack_arg.getattr("type").map_err(|_| DeferError)?;
        if is_instance(inner, refs.type_var_tuple_type) {
            let exp_prefix = typ_type
                .getattr("type_var_tuple_prefix")
                .map_err(|_| DeferError)?;
            if exp_prefix.is_none() {
                return Err(DeferError);
            }
            let exp_suffix = typ_type
                .getattr("type_var_tuple_suffix")
                .map_err(|_| DeferError)?;
            if exp_suffix.is_none() {
                return Err(DeferError);
            }
            let exp_prefix: i64 = exp_prefix.extract().map_err(|_| DeferError)?;
            let exp_suffix: i64 = exp_suffix.extract().map_err(|_| DeferError)?;
            let act_prefix = idx as i64;
            let act_suffix = (arg_list.len() - idx - 1) as i64;
            if act_prefix < exp_prefix || act_suffix < exp_suffix {
                fail_call(py, fail, "TypeVarTuple cannot be split", t);
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn wrong_type_arg_count_msg(min: usize, max: usize, given: usize, type_name: &str) -> String {
    let s = if min == max {
        if min == 0 {
            "no type arguments".to_string()
        } else if min == 1 {
            "1 type argument".to_string()
        } else {
            format!("{min} type arguments")
        }
    } else {
        format!("between {min} and {max} type arguments")
    };
    let given_str = if given == 0 {
        "none"
    } else {
        &given.to_string()
    };
    format!("\"{type_name}\" expects {s}, but {given_str} given")
}

fn fail_call(py: Python<'_>, fail: &PyAny, msg: &str, context: &PyAny) {
    let _ = py.import("mypy.messages").and_then(|m| {
        let code = m.getattr("codes")?.getattr("TYPE_ARG")?;
        // MsgCallback.__call__ has `code` as keyword-only: (msg, ctx, *, code=None).
        let kwargs = PyDict::new(py);
        kwargs.set_item("code", code).ok();
        let _ = fail.call((msg, context), Some(kwargs));
        Ok::<(), pyo3::PyErr>(())
    });
}

// ---------------------------------------------------------------------------
// check_vec_type_args (live type argument validation for 'vec')
// ---------------------------------------------------------------------------

/// `mypy.typeanal.check_vec_type_args` — report an error if type args
/// for 'vec' are invalid. Returns `None` to defer.
///
/// Mirrors typeanal.py:3038-3086. The `api` object must provide
/// `is_stub_file` (bool) and `fail` (callable). Recurses on
/// optional unions.
#[pyfunction]
pub(crate) fn rust_check_vec_type_args(
    py: Python<'_>,
    args: &PyAny,
    ctx: &PyAny,
    api: &PyAny,
) -> PyResult<Option<bool>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    match check_vec_type_args_inner(py, args, ctx, api, &refs) {
        Ok(b) => Ok(Some(b)),
        Err(DeferError) => Ok(None),
    }
}

fn check_vec_type_args_inner(
    py: Python<'_>,
    args: &PyAny,
    ctx: &PyAny,
    api: &PyAny,
    refs: &TypeRefs<'_>,
) -> Result<bool, DeferError> {
    let arg_list = iter_seq(args)?;
    let mut ok = true;
    if arg_list.len() != 1 {
        ok = false;
    } else {
        let arg = arg_list[0];
        // get_proper_type — defer on alias.
        if is_instance(arg, refs.type_alias_type) {
            return Err(DeferError);
        }
        if is_instance(arg, refs.instance) {
            let typ = arg.getattr("type").map_err(|_| DeferError)?;
            let fullname = typ.getattr("fullname").map_err(|_| DeferError)?;
            let fn_str = fullname.downcast::<PyString>().map_err(|_| DeferError)?;
            let fn_s = fn_str.to_str().map_err(|_| DeferError)?;
            if fn_s == "builtins.int" {
                ok = false;
            }
        } else if is_instance(arg, refs.union_type) {
            // Distinguish Instance-in-deny-list (outer emits) from
            // non-Instance recursive call (already emitted, skip).
            let union_ok = check_vec_union(py, arg, ctx, api, refs)?;
            if !union_ok {
                let items = get_attr_or_defer(arg, "items")?;
                let item_list = iter_seq(items)?;
                if item_list.len() == 2 {
                    let i0 = item_list[0];
                    let i1 = item_list[1];
                    let nopt = if is_instance(i0, refs.none_type) {
                        Some(i1)
                    } else if is_instance(i1, refs.none_type) {
                        Some(i0)
                    } else {
                        None
                    };
                    if let Some(n) = nopt {
                        if !is_instance(n, refs.instance) {
                            return Ok(false);
                        }
                    }
                }
                ok = false;
            }
        } else if is_instance(arg, refs.type_var_type) {
            let is_stub = api.getattr("is_stub_file").map_err(|_| DeferError)?;
            if !is_stub.is_true().map_err(|_| DeferError)? {
                ok = false;
            }
        } else {
            ok = false;
        }
    }
    if !ok {
        let _ = api
            .getattr("fail")
            .and_then(|f| f.call1(("Invalid item type for \"vec\"", ctx)));
    }
    Ok(ok)
}

/// Check vec args for a UnionType (Optional handling).
fn check_vec_union(
    py: Python<'_>,
    arg: &PyAny,
    ctx: &PyAny,
    api: &PyAny,
    refs: &TypeRefs<'_>,
) -> Result<bool, DeferError> {
    let items = get_attr_or_defer(arg, "items")?;
    let item_list = iter_seq(items)?;
    if item_list.len() != 2 {
        return Ok(false);
    }
    let i0 = item_list[0];
    let i1 = item_list[1];
    // get_proper_type on each — defer on alias.
    if is_instance(i0, refs.type_alias_type) || is_instance(i1, refs.type_alias_type) {
        return Err(DeferError);
    }
    let non_optional: Option<&PyAny> = if is_instance(i0, refs.none_type) {
        Some(i1)
    } else if is_instance(i1, refs.none_type) {
        Some(i0)
    } else {
        None
    };
    if non_optional.is_none() {
        return Ok(false);
    }
    let nopt = non_optional.unwrap();
    if is_instance(nopt, refs.instance) {
        let typ = nopt.getattr("type").map_err(|_| DeferError)?;
        let fullname = typ.getattr("fullname").map_err(|_| DeferError)?;
        let fn_str = fullname.downcast::<PyString>().map_err(|_| DeferError)?;
        let fn_s = fn_str.to_str().map_err(|_| DeferError)?;
        if fn_s == "mypy_extensions.i64"
            || fn_s == "mypy_extensions.i32"
            || fn_s == "mypy_extensions.i16"
            || fn_s == "mypy_extensions.u8"
            || fn_s == "builtins.int"
            || fn_s == "builtins.float"
            || fn_s == "builtins.bool"
            || fn_s == "librt.vecs.vec"
        {
            return Ok(false);
        }
        return Ok(true);
    }
    // Recurse: check_vec_type_args([non_optional], ctx, api)
    let single = PyList::new(py, [nopt]);
    check_vec_type_args_inner(py, single.as_ref(), ctx, api, refs)
}

// ---------------------------------------------------------------------------
// check_unpacks_in_list (filter variadic Unpack items in a type list)
// ---------------------------------------------------------------------------

/// `mypy.typeanal.TypeAnalyser.check_unpacks_in_list` — filter a type-arg
/// list that must carry at most one non-tuple `Unpack` item.
///
/// Mirrors typeanal.py:2991-3006: an item is a variadic unpack when it is
/// an `UnpackType` whose proper type is not a `TupleType`; the first such
/// item is kept and later ones are dropped. The inner type is resolved via
/// Python's `get_proper_type` (alias expansion must consult the live
/// resolver), matching the erase.rs pattern.
///
/// Returns `(kept_indices, final_unpack_index)` where `final_unpack_index`
/// is `Some` only when more than one variadic unpack was seen; Python
/// applies the "More than one variadic Unpack" fail with the final unpack's
/// inner type as context and rebuilds the item list from the indices.
/// `None` (defer) on unreadable facts (non-sequence input, missing `type`
/// attr, `get_proper_type` failure).
#[pyfunction]
pub(crate) fn rust_check_unpacks_in_list(
    py: Python<'_>,
    items: &PyAny,
) -> PyResult<Option<(Vec<usize>, Option<usize>)>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    match check_unpacks_in_list_inner(py, items, &refs) {
        Ok(r) => Ok(Some(r)),
        Err(DeferError) => Ok(None),
    }
}

fn check_unpacks_in_list_inner(
    py: Python<'_>,
    items: &PyAny,
    refs: &TypeRefs<'_>,
) -> Result<(Vec<usize>, Option<usize>), DeferError> {
    let list = iter_seq(items)?;
    let get_proper_type = py
        .import("mypy.types")
        .and_then(|m| m.getattr("get_proper_type"))
        .map_err(|_| DeferError)?;
    let mut variadic: Vec<bool> = Vec::with_capacity(list.len());
    for item in list.iter() {
        let mut is_variadic = false;
        if is_instance(item, refs.unpack_type) {
            let inner = item.getattr("type").map_err(|_| DeferError)?;
            let proper = get_proper_type.call1((inner,)).map_err(|_| DeferError)?;
            if !is_instance(proper, refs.tuple_type) {
                is_variadic = true;
            }
        }
        variadic.push(is_variadic);
    }
    Ok(filter_variadic_unpacks(&variadic))
}

/// Pure index fold over per-item "is a variadic unpack" flags: the first
/// variadic unpack is kept, later ones are dropped, and the returned
/// `final_unpack_index` is set only when more than one was seen.
fn filter_variadic_unpacks(variadic: &[bool]) -> (Vec<usize>, Option<usize>) {
    let mut kept: Vec<usize> = Vec::with_capacity(variadic.len());
    let mut num_unpacks = 0usize;
    let mut final_unpack: Option<usize> = None;
    for (i, &is_variadic) in variadic.iter().enumerate() {
        if is_variadic {
            if num_unpacks == 0 {
                kept.push(i);
            }
            num_unpacks += 1;
            final_unpack = Some(i);
        } else {
            kept.push(i);
        }
    }
    let final_out = if num_unpacks > 1 { final_unpack } else { None };
    (kept, final_out)
}

// ---------------------------------------------------------------------------
// is_typevar_default_recursive (BFS over default_depends graph)
// ---------------------------------------------------------------------------

/// `mypy.typeanal.is_typevar_default_recursive` — check if the type
/// variable can lead to infinite recursion via defaults.
///
/// Mirrors typeanal.py:2478-2495. Pure BFS over the `default_depends`
/// dict on the `start` object (TypeInfo or TypeAlias). Returns `None`
/// (defer) on any attribute access failure.
#[pyfunction]
pub(crate) fn rust_is_typevar_default_recursive(
    py: Python<'_>,
    tv_fname: &str,
    start: &PyAny,
) -> PyResult<Option<bool>> {
    match is_typevar_default_recursive_inner(py, tv_fname, start) {
        Ok(b) => Ok(Some(b)),
        Err(DeferError) => Ok(None),
    }
}

fn is_typevar_default_recursive_inner(
    _py: Python<'_>,
    tv_fname: &str,
    start: &PyAny,
) -> Result<bool, DeferError> {
    let dd = start.getattr("default_depends").map_err(|_| DeferError)?;
    let dd: &PyDict = dd.downcast().map_err(|_| DeferError)?;
    // tv_fname not in start.default_depends -> False
    let key = PyString::new(_py, tv_fname);
    if !dd.contains(key).unwrap_or(false) {
        return Ok(false);
    }
    let initial = dd.get_item(key).map_err(|_| DeferError)?;
    let initial = match initial {
        Some(i) if !i.is_none() => i,
        _ => return Ok(false),
    };
    let mut todo: Vec<PyObject> = {
        let set: &PySet = initial.downcast().map_err(|_| DeferError)?;
        set.iter().map(|o| o.into()).collect()
    };
    let start_ptr = start.as_ptr() as usize;
    let mut seen: HashSet<usize> = HashSet::new();
    while let Some(node_obj) = todo.pop() {
        let node: &PyAny = node_obj.as_ref(_py);
        if node.as_ptr() as usize == start_ptr {
            return Ok(true);
        }
        let ptr = node.as_ptr() as usize;
        if seen.contains(&ptr) {
            continue;
        }
        seen.insert(ptr);
        let node_dd = node.getattr("default_depends").map_err(|_| DeferError)?;
        let node_dd: &PyDict = node_dd.downcast().map_err(|_| DeferError)?;
        for (_, dep_set) in node_dd.iter() {
            if let Ok(s) = dep_set.downcast::<PySet>() {
                for dep in s.iter() {
                    todo.push(dep.into());
                }
            } else if let Ok(l) = dep_set.downcast::<PyList>() {
                for dep in l.iter() {
                    todo.push(dep.into());
                }
            }
        }
    }
    Ok(false)
}

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// detect_diverging_alias (DivergingAliasDetector visitor)
// ---------------------------------------------------------------------------

/// Collect-aliases walk over live Type objects, a mirror of
/// `CollectAliasesVisitor` (types.py:4463-4483): for a `TypeAliasType`
/// whose alias node was not seen yet, record it and recurse into the
/// alias node's target and then the TypeAliasType args (args are walked
/// for already-seen aliases too); every other kind walks its component
/// children.
fn collect_aliases_visit(
    py: Python<'_>,
    obj: &PyAny,
    alias_seen: &mut HashSet<usize>,
    aliases: &mut HashSet<usize>,
    refs: &TypeRefs<'_>,
) -> Result<(), DeferError> {
    if is_instance(obj, refs.type_alias_type) {
        let alias = obj.getattr("alias").map_err(|_| DeferError)?;
        if alias.is_none() {
            return Err(DeferError); // Python asserts t.alias is not None
        }
        let alias_ptr = alias.as_ptr() as usize;
        if !alias_seen.contains(&alias_ptr) {
            alias_seen.insert(alias_ptr);
            aliases.insert(alias_ptr);
            let target = alias.getattr("target").map_err(|_| DeferError)?;
            collect_aliases_visit(py, target, alias_seen, aliases, refs)?;
        }
        let args = obj.getattr("args").map_err(|_| DeferError)?;
        for arg in iter_seq(args)? {
            collect_aliases_visit(py, arg, alias_seen, aliases, refs)?;
        }
        return Ok(());
    }
    for child in type_query_children(py, obj, refs)? {
        collect_aliases_visit(py, child, alias_seen, aliases, refs)?;
    }
    Ok(())
}

/// `mypy.typeanal.detect_diverging_alias` — detect type aliases that
/// will diverge during type checking.
///
/// Mirrors typeanal.py:2528-2551. `node` is a `TypeAlias` Python object,
/// `target` is a `Type` Python object. Returns `None` to defer.
#[pyfunction]
pub(crate) fn rust_detect_diverging_alias(
    py: Python<'_>,
    node: &PyAny,
    target: &PyAny,
) -> PyResult<Option<bool>> {
    let refs = match TypeRefs::try_new(py) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    match detect_diverging_alias_inner(py, node, target, &refs) {
        Ok(b) => Ok(Some(b)),
        Err(DeferError) => Ok(None),
    }
}

fn detect_diverging_alias_inner(
    py: Python<'_>,
    node: &PyAny,
    target: &PyAny,
    refs: &TypeRefs<'_>,
) -> Result<bool, DeferError> {
    // is_recursive = node._is_recursive
    let is_recursive = node.getattr("_is_recursive").map_err(|_| DeferError)?;
    let is_rec: bool = if is_recursive.is_none() {
        // node._is_recursive is None: compute via a Rust port of
        // CollectAliasesVisitor (types.py:4463-4483) walking node.target
        // (typeanal.py:3691-3693). No cache write: Python only caches the
        // positive case on the node (and that write happens below).
        let node_target = node.getattr("target").map_err(|_| DeferError)?;
        let mut alias_seen: HashSet<usize> = HashSet::new();
        let mut aliases: HashSet<usize> = HashSet::new();
        collect_aliases_visit(py, node_target, &mut alias_seen, &mut aliases, refs)?;
        aliases.contains(&(node.as_ptr() as usize))
    } else {
        is_recursive.is_true().map_err(|_| DeferError)?
    };
    if !is_rec {
        return Ok(false);
    }
    // node._is_recursive = True (cache positive case).
    let _ = node.setattr("_is_recursive", true);
    // visitor = DivergingAliasDetector({node}); target.accept(visitor).
    let seen: HashSet<usize> = std::iter::once(node.as_ptr() as usize).collect();
    let mut diverging = false;
    diverging_alias_visit(py, target, &seen, &mut diverging, refs)?;
    Ok(diverging)
}

/// `mypy.types.get_proper_type` via PyO3. The Python DivergingAliasDetector
/// calls it inside its walk (typeanal.py:3652), so invoking it here at the
/// same point reproduces the expansion (and any state it touches) exactly.
/// Any error defers to the pure-Python body.
fn get_proper_type_py<'a>(py: Python<'a>, obj: &'a PyAny) -> Result<&'a PyAny, DeferError> {
    let types_mod = py.import("mypy.types").map_err(|_| DeferError)?;
    let gpt = types_mod
        .getattr("get_proper_type")
        .map_err(|_| DeferError)?;
    let expanded = gpt.call1((obj,)).map_err(|_| DeferError)?;
    Ok(expanded)
}

/// Recursively walk `obj` looking for diverging alias expansions.
/// Mirrors DivergingAliasDetector.visit_type_alias_type.
fn diverging_alias_visit(
    py: Python<'_>,
    obj: &PyAny,
    seen: &HashSet<usize>,
    diverging: &mut bool,
    refs: &TypeRefs<'_>,
) -> Result<(), DeferError> {
    if *diverging {
        return Ok(());
    }
    // TypeAliasType: the key visit.
    if is_instance(obj, refs.type_alias_type) {
        return diverging_visit_alias_type(py, obj, seen, diverging, refs);
    }
    // Recurse into children for all other composite types.
    diverging_recurse_children(py, obj, seen, diverging, refs)
}

/// DivergingAliasDetector.visit_type_alias_type.
fn diverging_visit_alias_type(
    py: Python<'_>,
    obj: &PyAny,
    seen: &HashSet<usize>,
    diverging: &mut bool,
    refs: &TypeRefs<'_>,
) -> Result<(), DeferError> {
    let alias = obj.getattr("alias").map_err(|_| DeferError)?;
    if alias.is_none() {
        return Err(DeferError);
    }
    let alias_ptr = alias.as_ptr() as usize;
    if seen.contains(&alias_ptr) {
        // Check each arg: if it's not TypeVarLike / Unpack(TypeVarLike)
        // and has_type_vars(arg) -> diverging = True.
        let args = obj.getattr("args").map_err(|_| DeferError)?;
        let arg_list = iter_seq(args)?;
        for arg in arg_list {
            if !is_typevarlike_or_unpack_tvl(arg, refs) && has_type_vars_live(py, arg, refs)? {
                *diverging = true;
                return Ok(());
            }
        }
        return Ok(());
    }
    // Not in seen: mirror typeanal.py:3652 —
    // `get_proper_type(t).accept(visitor)`. Expand via the real Python
    // get_proper_type (substitutes t.args into the alias target, no_args
    // handling included) instead of reading `alias.target` raw, then walk
    // the expansion with the extended seen set. Python does not visit
    // t.args separately on this branch; the expansion already carries them.
    let mut new_seen = seen.clone();
    new_seen.insert(alias_ptr);
    let expanded = get_proper_type_py(py, obj)?;
    diverging_alias_visit(py, expanded, &new_seen, diverging, refs)?;
    Ok(())
}

/// Check if `obj` is a TypeVarLikeType or UnpackType(TypeVarLikeType).
fn is_typevarlike_or_unpack_tvl(obj: &PyAny, refs: &TypeRefs<'_>) -> bool {
    if is_instance(obj, refs.type_var_type)
        || is_instance(obj, refs.param_spec_type)
        || is_instance(obj, refs.type_var_tuple_type)
    {
        return true;
    }
    if is_instance(obj, refs.unpack_type) {
        let inner = obj.getattr("type").ok();
        if let Some(inner) = inner {
            return is_instance(inner, refs.type_var_type)
                || is_instance(inner, refs.param_spec_type)
                || is_instance(inner, refs.type_var_tuple_type);
        }
    }
    false
}

/// has_type_vars on a live Python Type object.
/// Mirrors mypy.types.has_type_vars (BoolTypeQuery, ANY_STRATEGY).
#[allow(clippy::only_used_in_recursion)]
fn has_type_vars_live(
    py: Python<'_>,
    obj: &PyAny,
    refs: &TypeRefs<'_>,
) -> Result<bool, DeferError> {
    if is_instance(obj, refs.type_var_type)
        || is_instance(obj, refs.param_spec_type)
        || is_instance(obj, refs.type_var_tuple_type)
    {
        return Ok(true);
    }
    if is_instance(obj, refs.unpack_type) {
        let typ = get_attr_or_defer(obj, "type")?;
        return has_type_vars_live(py, typ, refs);
    }
    if is_instance(obj, refs.instance) {
        let args = get_attr_or_defer(obj, "args")?;
        for a in iter_seq(args)? {
            if has_type_vars_live(py, a, refs)? {
                return Ok(true);
            }
        }
        let lkv = obj.getattr("last_known_value").map_err(|_| DeferError)?;
        if !lkv.is_none() {
            return has_type_vars_live(py, lkv, refs);
        }
        return Ok(false);
    }
    if is_instance(obj, refs.callable_type) {
        let arg_types = get_attr_or_defer(obj, "arg_types")?;
        for a in iter_seq(arg_types)? {
            if has_type_vars_live(py, a, refs)? {
                return Ok(true);
            }
        }
        let ret = get_attr_or_defer(obj, "ret_type")?;
        if has_type_vars_live(py, ret, refs)? {
            return Ok(true);
        }
        let inst = obj.getattr("instance_type").map_err(|_| DeferError)?;
        if !inst.is_none() && has_type_vars_live(py, inst, refs)? {
            return Ok(true);
        }
        return Ok(false);
    }
    if is_instance(obj, refs.union_type) {
        let items = get_attr_or_defer(obj, "items")?;
        for a in iter_seq(items)? {
            if has_type_vars_live(py, a, refs)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if is_instance(obj, refs.tuple_type) {
        let items = get_attr_or_defer(obj, "items")?;
        for a in iter_seq(items)? {
            if has_type_vars_live(py, a, refs)? {
                return Ok(true);
            }
        }
        let fb = get_attr_or_defer(obj, "partial_fallback")?;
        return has_type_vars_live(py, fb, refs);
    }
    if is_instance(obj, refs.type_type) {
        let item = get_attr_or_defer(obj, "item")?;
        return has_type_vars_live(py, item, refs);
    }
    if is_instance(obj, refs.overloaded) {
        let items = get_attr_or_defer(obj, "items")?;
        for a in iter_seq(items)? {
            if has_type_vars_live(py, a, refs)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if is_instance(obj, refs.typed_dict_type) {
        let items = get_attr_or_defer(obj, "items")?;
        let dict: &PyDict = items.downcast().map_err(|_| DeferError)?;
        for (_, v) in dict.iter() {
            if has_type_vars_live(py, v, refs)? {
                return Ok(true);
            }
        }
        let fb = get_attr_or_defer(obj, "fallback")?;
        return has_type_vars_live(py, fb, refs);
    }
    if is_instance(obj, refs.literal_type) {
        let fb = get_attr_or_defer(obj, "fallback")?;
        return has_type_vars_live(py, fb, refs);
    }
    if is_instance(obj, refs.any_type) {
        let sa = obj.getattr("source_any").map_err(|_| DeferError)?;
        if !sa.is_none() {
            return has_type_vars_live(py, sa, refs);
        }
        return Ok(false);
    }
    if is_instance(obj, refs.type_alias_type) {
        let args = get_attr_or_defer(obj, "args")?;
        for a in iter_seq(args)? {
            if has_type_vars_live(py, a, refs)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    // NoneType, UninhabitedType, DeletedType, UnboundType, Parameters:
    // no type vars.
    Ok(false)
}

/// Enumerate the component types of a live Type object for TypeQuery-based
/// visitors (the shared `type_visitor.py query_types` child machinery the
/// DivergingAliasDetector and CollectAliasesVisitor both inherit);
/// `TypeAliasType` children are routed by the callers' alias visits and are
/// never enumerated here. Keep the kind order and attribute reads identical
/// to the previous longhand recurse walk.
fn type_query_children<'a>(
    _py: Python<'a>,
    obj: &'a PyAny,
    refs: &TypeRefs<'a>,
) -> Result<Vec<&'a PyAny>, DeferError> {
    let mut out: Vec<&PyAny> = Vec::new();
    if is_instance(obj, refs.instance) {
        let args = get_attr_or_defer(obj, "args")?;
        out.extend(iter_seq(args)?);
        return Ok(out);
    }
    if is_instance(obj, refs.callable_type) {
        let arg_types = get_attr_or_defer(obj, "arg_types")?;
        out.extend(iter_seq(arg_types)?);
        let ret = get_attr_or_defer(obj, "ret_type")?;
        out.push(ret);
        let inst = obj.getattr("instance_type").map_err(|_| DeferError)?;
        if !inst.is_none() {
            out.push(inst);
        }
        return Ok(out);
    }
    if is_instance(obj, refs.union_type) {
        let items = get_attr_or_defer(obj, "items")?;
        out.extend(iter_seq(items)?);
        return Ok(out);
    }
    if is_instance(obj, refs.tuple_type) {
        let items = get_attr_or_defer(obj, "items")?;
        out.extend(iter_seq(items)?);
        let fb = get_attr_or_defer(obj, "partial_fallback")?;
        out.push(fb);
        return Ok(out);
    }
    if is_instance(obj, refs.typed_dict_type) {
        let items = get_attr_or_defer(obj, "items")?;
        let dict: &PyDict = items.downcast().map_err(|_| DeferError)?;
        for (_, v) in dict.iter() {
            out.push(v);
        }
        let fb = get_attr_or_defer(obj, "fallback")?;
        out.push(fb);
        return Ok(out);
    }
    if is_instance(obj, refs.type_type) {
        let item = get_attr_or_defer(obj, "item")?;
        out.push(item);
        return Ok(out);
    }
    if is_instance(obj, refs.overloaded) {
        let items = get_attr_or_defer(obj, "items")?;
        out.extend(iter_seq(items)?);
        return Ok(out);
    }
    if is_instance(obj, refs.literal_type) {
        let fb = get_attr_or_defer(obj, "fallback")?;
        out.push(fb);
        return Ok(out);
    }
    if is_instance(obj, refs.unpack_type) {
        let typ = get_attr_or_defer(obj, "type")?;
        out.push(typ);
        return Ok(out);
    }
    if is_instance(obj, refs.any_type) {
        let sa = obj.getattr("source_any").map_err(|_| DeferError)?;
        if !sa.is_none() {
            out.push(sa);
        }
        return Ok(out);
    }
    if is_instance(obj, refs.type_var_type)
        || is_instance(obj, refs.param_spec_type)
        || is_instance(obj, refs.type_var_tuple_type)
    {
        // TypeVar-like: recurse upper_bound, default, values.
        let ub = get_attr_or_defer(obj, "upper_bound")?;
        out.push(ub);
        let default = get_attr_or_defer(obj, "default")?;
        out.push(default);
        let values = get_attr_or_defer(obj, "values")?;
        out.extend(iter_seq(values)?);
        return Ok(out);
    }
    // NoneType, UninhabitedType, DeletedType, UnboundType, Parameters:
    // no children to recurse.
    Ok(out)
}

/// Recurse into all children of `obj` for diverging alias detection.
fn diverging_recurse_children(
    py: Python<'_>,
    obj: &PyAny,
    seen: &HashSet<usize>,
    diverging: &mut bool,
    refs: &TypeRefs<'_>,
) -> Result<(), DeferError> {
    for child in type_query_children(py, obj, refs)? {
        diverging_alias_visit(py, child, seen, diverging, refs)?;
        if *diverging {
            return Ok(());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers (live-object traversal)
// ---------------------------------------------------------------------------

fn get_attr_or_defer<'a>(obj: &'a PyAny, name: &str) -> Result<&'a PyAny, DeferError> {
    obj.getattr(name).map_err(|_| DeferError)
}

pub(crate) fn iter_seq(obj: &PyAny) -> Result<Vec<&PyAny>, DeferError> {
    if let Ok(list) = obj.downcast::<PyList>() {
        Ok(list.iter().collect())
    } else if let Ok(tuple) = obj.downcast::<PyTuple>() {
        Ok(tuple.iter().collect())
    } else {
        Err(DeferError)
    }
}

fn class_name_is(obj: &PyAny, expected: &str) -> bool {
    let class = match obj.getattr("__class__") {
        Ok(c) => c,
        Err(_) => return false,
    };
    let name = match class.getattr("__name__") {
        Ok(n) => n,
        Err(_) => return false,
    };
    match name.downcast::<PyString>() {
        Ok(s) => s.to_str().unwrap_or("") == expected,
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_any(type_of_any: i64) -> Type {
        Type::AnyType {
            type_of_any,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_callable(arg_types: Vec<Type>, ret_type: Type) -> Type {
        make_callable_with_instance(None, arg_types, ret_type)
    }

    fn make_callable_with_instance(
        instance_type: Option<Box<Type>>,
        arg_types: Vec<Type>,
        ret_type: Type,
    ) -> Type {
        Type::CallableType {
            fallback: Box::new(make_any(SPECIAL_FORM)),
            instance_type,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(ret_type),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
        }
    }

    fn make_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }
    }

    fn make_typeddict(items: Vec<(String, Type)>) -> Type {
        Type::TypedDictType {
            fallback: Box::new(make_instance("builtins.dict", vec![])),
            items,
            required_keys: HashSet::new(),
            readonly_keys: HashSet::new(),
            is_closed: true,
        }
    }

    use std::collections::HashSet;

    #[test]
    fn test_has_explicit_any_true_for_explicit() {
        assert_eq!(
            has_explicit_any_inner(&make_any(EXPLICIT), EXPLICIT),
            Some(true)
        );
        assert_eq!(
            has_explicit_any_inner(&make_any(EXPLICIT), FROM_UNIMPORTED_TYPE),
            Some(false)
        );
    }

    #[test]
    fn test_has_explicit_any_false_for_special_and_unimported() {
        assert_eq!(
            has_explicit_any_inner(&make_any(SPECIAL_FORM), EXPLICIT),
            Some(false)
        );
        assert_eq!(
            has_explicit_any_inner(&make_any(FROM_UNIMPORTED_TYPE), EXPLICIT),
            Some(false)
        );
    }

    #[test]
    fn test_has_explicit_any_in_union() {
        let t = make_union(vec![make_any(SPECIAL_FORM), make_any(EXPLICIT)]);
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), Some(true));
    }

    #[test]
    fn test_has_explicit_any_in_instance_args() {
        let t = make_instance("builtins.list", vec![make_any(EXPLICIT)]);
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), Some(true));
    }

    #[test]
    fn test_has_explicit_any_typeddict_is_false() {
        let t = make_typeddict(vec![("x".to_string(), make_any(EXPLICIT))]);
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), Some(false));
    }

    #[test]
    fn test_has_explicit_any_callable_instance_type() {
        let with_inst = make_callable_with_instance(
            Some(Box::new(make_any(EXPLICIT))),
            vec![make_any(SPECIAL_FORM)],
            make_any(SPECIAL_FORM),
        );
        assert_eq!(has_explicit_any_inner(&with_inst, EXPLICIT), Some(true));
        let no_inst = make_callable(vec![make_any(SPECIAL_FORM)], make_any(SPECIAL_FORM));
        assert_eq!(has_explicit_any_inner(&no_inst, EXPLICIT), Some(false));
    }

    #[test]
    fn test_has_any_unimported_true() {
        let t = make_union(vec![make_any(SPECIAL_FORM), make_any(FROM_UNIMPORTED_TYPE)]);
        assert_eq!(has_explicit_any_inner(&t, FROM_UNIMPORTED_TYPE), Some(true));
    }

    #[test]
    fn test_has_explicit_any_deferred_on_alias() {
        let t = Type::TypeAliasType {
            args: vec![make_any(EXPLICIT)],
            type_ref: "m.T".to_string(),
        };
        assert_eq!(has_explicit_any_inner(&t, EXPLICIT), None);
        // Alias nested inside a union propagates the deferral.
        let u = make_union(vec![make_any(SPECIAL_FORM), t]);
        assert_eq!(has_explicit_any_inner(&u, EXPLICIT), None);
    }

    #[test]
    fn test_collect_all_inner_union_children() {
        let t = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_any(SPECIAL_FORM),
        ]);
        let result = collect_all_inner_types_inner(&t).unwrap();
        // [int, any] -- both are leaves, so no inner, direct children only.
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], Type::Instance { .. }));
        assert!(matches!(result[1], Type::AnyType { .. }));
    }

    #[test]
    fn test_collect_all_inner_nested_union_inner_ordering() {
        let inner = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.str", vec![]),
        ]);
        let t = make_instance("builtins.list", vec![inner.clone()]);
        let result = collect_all_inner_types_inner(&t).unwrap();
        assert_eq!(result.len(), 3);
        // Query order: inner union's children first, then the direct child.
        assert!(matches!(result[2], Type::UnionType { .. }));
    }

    #[test]
    fn test_collect_all_inner_children_none_does_not_include_root() {
        let t = make_instance("builtins.int", vec![]);
        assert_eq!(collect_all_inner_types_inner(&t).unwrap().len(), 0);
    }

    #[test]
    fn test_collect_all_inner_callable_instance_not_double_counted() {
        let ret = make_instance("builtins.str", vec![]);
        let t = make_callable_with_instance(
            Some(Box::new(ret.clone())),
            vec![make_instance("builtins.int", vec![])],
            ret.clone(),
        );
        let result = collect_all_inner_types_inner(&t).unwrap();
        // children: int (arg), str (ret), str (instance == ret, not dup).
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_collect_all_inner_deferred_on_alias() {
        let t = make_union(vec![
            make_any(SPECIAL_FORM),
            Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "m.T".to_string(),
            },
        ]);
        assert_eq!(collect_all_inner_types_inner(&t), None);
    }

    #[test]
    fn test_make_optional_none_identity() {
        assert_eq!(
            make_optional_type_inner(&Type::NoneType),
            Some(Type::NoneType)
        );
    }

    #[test]
    fn test_make_optional_instance_wraps() {
        let t = make_instance("builtins.int", vec![]);
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType {
                items,
                can_be_true,
                can_be_false,
                ..
            } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::Instance { .. }));
                assert!(matches!(items[1], Type::NoneType));
                // int can be true and false.
                assert!(can_be_true);
                assert!(can_be_false);
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_union_strips_none() {
        let t = make_union(vec![make_instance("builtins.int", vec![]), Type::NoneType]);
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::Instance { .. }));
                assert!(matches!(items[1], Type::NoneType));
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_none_only_union_truthiness() {
        let t = make_union(vec![Type::NoneType]);
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType {
                items,
                can_be_true,
                can_be_false,
                ..
            } => {
                assert_eq!(items.len(), 1);
                assert!(matches!(items[0], Type::NoneType));
                // Union[None] is always false-y: can be false, never true.
                assert!(!can_be_true);
                assert!(can_be_false);
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_alias_nested_union_defers() {
        let t = make_union(vec![
            make_any(SPECIAL_FORM),
            Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "m.T".to_string(),
            },
        ]);
        assert_eq!(make_optional_type_inner(&t), None);
    }

    #[test]
    fn test_make_optional_alias_wraps_in_else_branch() {
        // A TypeAliasType is not a ProperType, so Python takes the else
        // branch and wraps it without inspecting its target.
        let t = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "m.T".to_string(),
        };
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert!(matches!(items[0], Type::TypeAliasType { .. }));
                assert!(matches!(items[1], Type::NoneType));
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_make_optional_alias_wraps_truthiness_true() {
        // Else-branch wrap: setops truthiness helpers fall back to the
        // per-variant default for aliases, so a plain TypeAliasType has
        // can_be_true/can_be_false both true.
        let t = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "m.T".to_string(),
        };
        let result = make_optional_type_inner(&t).unwrap();
        match result {
            Type::UnionType {
                items,
                can_be_true,
                can_be_false,
                ..
            } => {
                assert_eq!(items.len(), 2);
                assert!(can_be_true);
                assert!(can_be_false);
            }
            _ => panic!("expected UnionType"),
        }
    }

    #[test]
    fn test_unknown_unpack_false_for_non_unpack() {
        assert_eq!(unknown_unpack_inner(&make_any(SPECIAL_FORM)), Some(false));
        assert_eq!(
            unknown_unpack_inner(&make_instance("m.B", Vec::new())),
            Some(false)
        );
    }

    #[test]
    fn test_unknown_unpack_matches_special_form_any() {
        let unpacked = Type::UnpackType {
            typ: Box::new(make_any(SPECIAL_FORM)),
        };
        assert_eq!(unknown_unpack_inner(&unpacked), Some(true));
    }

    #[test]
    fn test_unknown_unpack_other_any_is_false() {
        let unpacked = Type::UnpackType {
            typ: Box::new(make_any(EXPLICIT)),
        };
        assert_eq!(unknown_unpack_inner(&unpacked), Some(false));
    }

    #[test]
    fn test_unknown_unpack_defers_on_alias_target() {
        let unpacked = Type::UnpackType {
            typ: Box::new(Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "m.T".to_string(),
            }),
        };
        assert_eq!(unknown_unpack_inner(&unpacked), None);
    }

    fn make_tuple(items: Vec<Type>, implicit: bool) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items,
            implicit,
        }
    }

    #[test]
    fn test_analyze_tuple_implicit_defers_without_tuple_literal() {
        // visit_tuple_type: implicit && !allow_tuple_literal emits an
        // error and returns Any(from_error); Rust must defer, not return
        // the tuple unchanged.
        let t = make_tuple(vec![make_instance("builtins.int", vec![])], true);
        assert_eq!(analyze_type_inner(&t, false, false, false), None);
        // When tuple literals are allowed, implicit tuples analyze fine.
        let t2 = make_tuple(vec![make_instance("builtins.int", vec![])], true);
        assert!(analyze_type_inner(&t2, true, false, false).is_some());
        // Explicit tuples need no guard.
        let t3 = make_tuple(vec![make_instance("builtins.int", vec![])], false);
        assert!(analyze_type_inner(&t3, false, false, false).is_some());
    }

    #[test]
    fn test_analyze_union_unpack_child_defers() {
        // visit_union_type analyzes items with allow_unpack=False
        // regardless of the outer flag, so an UnpackType child must
        // defer to Python's error path.
        let unpacked = Type::UnpackType {
            typ: Box::new(make_instance("builtins.int", vec![])),
        };
        let t = make_union(vec![make_instance("builtins.str", vec![]), unpacked]);
        assert_eq!(analyze_type_inner(&t, false, false, true), None);
        // A union without Unpack children still analyzes (outer flag
        // irrelevant for plain items).
        let t2 = make_union(vec![
            make_instance("builtins.str", vec![]),
            make_instance("builtins.int", vec![]),
        ]);
        assert!(analyze_type_inner(&t2, false, false, true).is_some());
    }

    // find_self_type wire-mode walk over expanded alias targets (#1157).
    // These run without a GIL: lookups go through SelfLookup::Table and
    // alias snapshots are built from direct `Type` values.

    use std::collections::HashMap;

    use crate::aliases::TypeAliasSnapshot;

    fn self_table(entries: &[(&str, &str)]) -> SelfLookup<'static> {
        let mut map = HashMap::new();
        for (name, fullname) in entries {
            map.insert((*name).to_string(), (*fullname).to_string());
        }
        SelfLookup::Table(map)
    }

    fn alias_resolver_with(fullname: &str, target: &Type) -> TypeAliasResolver {
        let mut resolver = TypeAliasResolver::new();
        resolver.insert(
            fullname.to_string(),
            TypeAliasSnapshot {
                fullname: fullname.to_string(),
                target: encode_type(target).expect("target encodes"),
                ..Default::default()
            },
        );
        resolver
    }

    fn make_unbound(name: &str) -> Type {
        Type::UnboundType {
            name: name.to_string(),
            args: Vec::new(),
            original_str_expr: None,
            original_str_fallback: None,
        }
    }

    fn make_alias(type_ref: &str, args: Vec<Type>) -> Type {
        Type::TypeAliasType {
            args,
            type_ref: type_ref.to_string(),
        }
    }

    #[test]
    fn test_find_self_type_wire_alias_target_is_self() {
        let aliases = alias_resolver_with("mod.X", &make_unbound("Self"));
        let lookup = self_table(&[("Self", "typing.Self")]);
        let t = make_alias("mod.X", vec![]);
        assert_eq!(
            find_self_type_wire(&t, &lookup, &aliases, &mut Vec::new()),
            Ok(true)
        );
        // Plain unbound target does not fire, even with the same table.
        let aliases_plain = alias_resolver_with("mod.X", &make_unbound("int"));
        assert_eq!(
            find_self_type_wire(&t, &lookup, &aliases_plain, &mut Vec::new()),
            Ok(false)
        );
    }

    #[test]
    fn test_find_self_type_wire_alias_cycle_defers() {
        // Snapshot target re-encodes the alias itself: the chain cycle
        // guard must defer, not loop.
        let aliases = alias_resolver_with("mod.A", &make_alias("mod.A", vec![]));
        let lookup = self_table(&[]);
        let t = make_alias("mod.A", vec![]);
        assert!(find_self_type_wire(&t, &lookup, &aliases, &mut Vec::new()).is_err());
    }

    #[test]
    fn test_find_self_type_wire_missing_snapshot_defers() {
        let aliases = alias_resolver_with("mod.X", &make_unbound("Self"));
        let lookup = self_table(&[("Self", "typing.Self")]);
        let t = make_alias("mod.MISSING", vec![]);
        assert!(find_self_type_wire(&t, &lookup, &aliases, &mut Vec::new()).is_err());
    }

    #[test]
    fn test_find_self_type_wire_py312_args_queried() {
        // New-style alias: the target is plain, so the Self must come
        // from the node's own args via the python_3_12 arm.
        let _aliases = alias_resolver_with("mod.Y", &make_any(SPECIAL_FORM));
        let mut snap = TypeAliasSnapshot {
            fullname: "mod.Y".to_string(),
            target: encode_type(&make_any(SPECIAL_FORM)).expect("target encodes"),
            ..Default::default()
        };
        snap.python_3_12_type_alias = true;
        let mut resolver = TypeAliasResolver::new();
        resolver.insert("mod.Y".to_string(), snap);
        let lookup = self_table(&[("Self", "typing.Self")]);
        let t = make_alias("mod.Y", vec![make_unbound("Self")]);
        assert_eq!(
            find_self_type_wire(&t, &lookup, &resolver, &mut Vec::new()),
            Ok(true)
        );
        // Same alias with a non-Self arg stays False.
        let t_plain = make_alias("mod.Y", vec![make_instance("builtins.int", vec![])]);
        assert_eq!(
            find_self_type_wire(&t_plain, &lookup, &resolver, &mut Vec::new()),
            Ok(false)
        );
    }

    #[test]
    fn test_find_self_type_wire_seen_alias_returns_default() {
        // A second occurrence of the same alias inside the expansion
        // returns the ANY_STRATEGY default (False), like the Python
        // seen_aliases cache.
        let inner = make_instance("builtins.list", vec![make_alias("mod.A", vec![])]);
        let aliases = alias_resolver_with("mod.A", &inner);
        let lookup = self_table(&[]);
        let t = make_alias("mod.A", vec![]);
        assert_eq!(
            find_self_type_wire(&t, &lookup, &aliases, &mut Vec::new()),
            Ok(false)
        );
    }
}

// ---------------------------------------------------------------------------
// rust_type_analyze — hot path mirroring TypeAnalyser.anal_type
// ---------------------------------------------------------------------------

/// Mirrors `TypeAnalyser.anal_type` for types that can be analyzed without
/// semantic context (Instance, Callable, TypeVar, Tuple, TypedDict, Union,
/// TypeType, Literal, etc.). Returns `None` for types requiring symbol lookup
/// (UnboundType), alias expansion (TypeAliasType), or placeholder resolution
/// (PlaceholderType), matching Python's deferral semantics exactly.
///
/// Flags control tuple literal syntax, ParamSpec literal syntax, and unpack
/// handling — these are the same options the Python visitor carries.
#[pyfunction]
#[pyo3(signature = (type_bytes, allow_tuple_literal=false, allow_param_spec_literals=false, allow_unpack=false))]
pub(crate) fn rust_type_analyze(
    type_bytes: &[u8],
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> PyResult<Option<Vec<u8>>> {
    let t = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let analyzed = match analyze_type_inner(
        &t,
        allow_tuple_literal,
        allow_param_spec_literals,
        allow_unpack,
    ) {
        Some(result) => result,
        None => return Ok(None),
    };
    Ok(encode_type(&analyzed))
}

/// Core analysis of a single Type value. Returns `None` when the type requires
/// semantic context (symbol lookup, alias expansion, etc.).
fn analyze_type_inner(
    t: &Type,
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> Option<Type> {
    match t {
        // Already-bound types: analyze children.
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            let args = analyze_type_list(
                args,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let lkv = match last_known_value {
                Some(v) => Some(Box::new(analyze_type_inner(
                    v,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args,
                last_known_value: lkv,
                extra_attrs: extra_attrs.as_ref().and_then(|ea| {
                    let mut attrs = std::collections::HashMap::with_capacity(ea.attrs.len());
                    for (k, v) in &ea.attrs {
                        attrs.insert(
                            k.clone(),
                            analyze_type_inner(
                                v,
                                allow_tuple_literal,
                                allow_param_spec_literals,
                                allow_unpack,
                            )?,
                        );
                    }
                    Some(ExtraAttrs {
                        attrs,
                        immutable: ea.immutable.clone(),
                        mod_name: ea.mod_name.clone(),
                    })
                }),
            })
        }

        Type::TypeAliasType { args, type_ref } => {
            // Mirror `visit_type_alias_type` (typeanal.py): a pure
            // passthrough returning the alias unchanged, not an
            // expansion (needs the live target) nor arg re-analysis.
            Some(Type::TypeAliasType {
                args: args.clone(),
                type_ref: type_ref.clone(),
            })
        }

        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default,
            variance,
            meta_level,
        } => {
            let values = analyze_type_list(
                values,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let upper_bound = Box::new(analyze_type_inner(
                upper_bound,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let default = Box::new(analyze_type_inner(
                default,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::TypeVarType {
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                values,
                upper_bound,
                default,
                variance: *variance,
                meta_level: *meta_level,
            })
        }

        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound,
            default,
        } => {
            let prefix = Box::new(Parameters {
                arg_types: analyze_type_list(
                    &prefix.arg_types,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?,
                arg_kinds: prefix.arg_kinds.clone(),
                arg_names: prefix.arg_names.clone(),
                variables: analyze_type_var_likes(
                    &prefix.variables,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?,
                imprecise_arg_kinds: prefix.imprecise_arg_kinds,
                is_ellipsis_args: prefix.is_ellipsis_args,
            });
            let upper_bound = Box::new(analyze_type_inner(
                upper_bound,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let default = Box::new(analyze_type_inner(
                default,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::ParamSpecType {
                prefix,
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                flavor: *flavor,
                upper_bound,
                default,
            })
        }

        Type::TypeVarTupleType {
            tuple_fallback,
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound,
            default,
            min_len,
        } => {
            let tuple_fallback = Box::new(analyze_type_inner(
                tuple_fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let upper_bound = Box::new(analyze_type_inner(
                upper_bound,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let default = Box::new(analyze_type_inner(
                default,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::TypeVarTupleType {
                tuple_fallback,
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                upper_bound,
                default,
                min_len: *min_len,
            })
        }

        Type::UnpackType { typ } => {
            // Unpack analysis needs allow_unpack context and nesting_level tracking.
            // Defer to Python unless allow_unpack is set.
            if !allow_unpack {
                return None;
            }
            let typ = Box::new(analyze_type_inner(
                typ,
                allow_tuple_literal,
                allow_param_spec_literals,
                true,
            )?);
            Some(Type::UnpackType { typ })
        }

        Type::Parameters(p) => Some(Type::Parameters(Parameters {
            arg_types: analyze_type_list(
                &p.arg_types,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?,
            arg_kinds: p.arg_kinds.clone(),
            arg_names: p.arg_names.clone(),
            variables: analyze_type_var_likes(
                &p.variables,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?,
            imprecise_arg_kinds: p.imprecise_arg_kinds,
            is_ellipsis_args: p.is_ellipsis_args,
        })),

        Type::UnboundType { .. } => {
            // UnboundType needs symbol lookup (lookup_qualified) in the
            // semantic analyzer. Python handles this by looking up the name
            // and dispatching to ParamSpecExpr/TypeVarExpr/TypeInfo/etc.

            // Rust has no access to the symbol table.
            None
        }

        Type::AnyType {
            type_of_any,
            source_any,
            missing_import_name,
        } => {
            let source_any = match source_any {
                Some(sa) => {
                    let analyzed = analyze_type_inner(
                        sa,
                        allow_tuple_literal,
                        allow_param_spec_literals,
                        allow_unpack,
                    )?;
                    Some(Box::new(analyzed))
                }
                None => None,
            };
            Some(Type::AnyType {
                type_of_any: *type_of_any,
                source_any,
                missing_import_name: missing_import_name.clone(),
            })
        }

        Type::NoneType => Some(Type::NoneType),

        Type::ErasedType => Some(Type::ErasedType),

        Type::UninhabitedType { ambiguous } => Some(Type::UninhabitedType {
            ambiguous: *ambiguous,
        }),

        Type::DeletedType { source } => Some(Type::DeletedType {
            source: source.clone(),
        }),

        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            // Callable analysis: mirror visit_callable_type (typeanal.py:1873).
            // Re-analyze arg_types, ret_type, variables, type guards.
            // Bind-vars + guard/is analysis needs live context: defer.
            let ret = analyze_type_inner(
                ret_type,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            // Star-arg handling (typeanal.py:1886-1903): bound
            // P.args/P.kwargs pass through unchanged (substitution would
            // explode the ParamSpec); other stars analyze with allow_unpack.
            let mut arg_types_out: Vec<Type> = Vec::with_capacity(arg_types.len());
            for (kind, at) in arg_kinds.iter().zip(arg_types.iter()) {
                if (*kind == ARG_STAR || *kind == ARG_STAR2)
                    && matches!(at, Type::ParamSpecType { .. })
                {
                    // Bound P.args/P.kwargs: pass through as-is.
                    arg_types_out.push(at.clone());
                } else {
                    let analyzed = analyze_type_inner(
                        at,
                        allow_tuple_literal,
                        allow_param_spec_literals,
                        *kind == ARG_STAR || *kind == ARG_STAR2 || allow_unpack,
                    )?;
                    arg_types_out.push(analyzed);
                }
            }
            let variables = analyze_type_var_likes(
                variables,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            let type_guard = match type_guard {
                Some(g) => Some(Box::new(analyze_type_inner(
                    g,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            let type_is = match type_is {
                Some(ti) => Some(Box::new(analyze_type_inner(
                    ti,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            let instance_type = match instance_type {
                Some(it) => Some(Box::new(analyze_type_inner(
                    it,
                    allow_tuple_literal,
                    allow_param_spec_literals,
                    allow_unpack,
                )?)),
                None => None,
            };
            let fallback = Box::new(analyze_type_inner(
                fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::CallableType {
                fallback,
                instance_type,
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                from_type_type: *from_type_type,
                arg_types: arg_types_out,
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: Box::new(ret),
                name: name.clone(),
                variables,
                type_guard,
                type_is,
            })
        }

        Type::Overloaded { items } => {
            // Each overloaded item is a CallableType (Python asserts this).
            let items: Vec<Type> = items
                .iter()
                .map(|item| match item {
                    Type::CallableType { .. } => analyze_type_inner(
                        item,
                        allow_tuple_literal,
                        allow_param_spec_literals,
                        allow_unpack,
                    ),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::Overloaded { items })
        }

        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            // visit_tuple_type errors on implicit tuples unless tuple
            // literals are allowed and returns Any(from_error). Rust
            // cannot reproduce that side effect, so defer.
            if *implicit && !allow_tuple_literal {
                return None;
            }
            let partial_fallback = Box::new(analyze_type_inner(
                partial_fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let items = analyze_type_list(
                items,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?;
            Some(Type::TupleType {
                partial_fallback,
                items,
                implicit: *implicit,
            })
        }

        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            let fallback = Box::new(analyze_type_inner(
                fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            let items_out: Vec<_> = items
                .iter()
                .map(|(k, v)| {
                    analyze_type_inner(
                        v,
                        allow_tuple_literal,
                        allow_param_spec_literals,
                        allow_unpack,
                    )
                    .map(|t| (k.clone(), t))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::TypedDictType {
                fallback,
                items: items_out,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }

        Type::LiteralType { fallback, value } => {
            let fallback = Box::new(analyze_type_inner(
                fallback,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::LiteralType {
                fallback,
                value: value.clone(),
            })
        }

        Type::UnionType {
            items,
            uses_pep604_syntax,
            can_be_true,
            can_be_false,
        } => {
            // visit_union_type analyzes items with allow_unpack=False
            // (anal_array default) regardless of the outer flag, so an
            // UnpackType child defers to Python's error path.
            let items =
                analyze_type_list(items, allow_tuple_literal, allow_param_spec_literals, false)?;
            Some(Type::UnionType {
                items,
                uses_pep604_syntax: *uses_pep604_syntax,
                can_be_true: *can_be_true,
                can_be_false: *can_be_false,
            })
        }

        Type::TypeType { item, is_type_form } => {
            let item = Box::new(analyze_type_inner(
                item,
                allow_tuple_literal,
                allow_param_spec_literals,
                allow_unpack,
            )?);
            Some(Type::TypeType {
                item,
                is_type_form: *is_type_form,
            })
        }
    }
}

/// Analyze a list of types, returning `None` if any child defers.
fn analyze_type_list(
    types: &[Type],
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(types.len());
    for t in types {
        out.push(analyze_type_inner(
            t,
            allow_tuple_literal,
            allow_param_spec_literals,
            allow_unpack,
        )?);
    }
    Some(out)
}

/// Analyze TypeVar-like types (TypeVarType, ParamSpecType, TypeVarTupleType).
fn analyze_type_var_likes(
    types: &[Type],
    allow_tuple_literal: bool,
    allow_param_spec_literals: bool,
    allow_unpack: bool,
) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(types.len());
    for t in types {
        out.push(analyze_type_inner(
            t,
            allow_tuple_literal,
            allow_param_spec_literals,
            allow_unpack,
        )?);
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// check_unpacks_in_list pure decision tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod check_unpacks_tests {
    use super::filter_variadic_unpacks;

    #[test]
    fn test_no_unpacks() {
        assert_eq!(
            filter_variadic_unpacks(&[false, false, false]),
            (vec![0, 1, 2], None)
        );
    }

    #[test]
    fn test_first_variadic_unpack_kept() {
        assert_eq!(filter_variadic_unpacks(&[true, false]), (vec![0, 1], None));
    }

    #[test]
    fn test_second_variadic_unpack_dropped() {
        assert_eq!(
            filter_variadic_unpacks(&[true, false, true]),
            (vec![0, 1], Some(2))
        );
    }

    #[test]
    fn test_three_variadic_unpacks_keep_first_only() {
        assert_eq!(
            filter_variadic_unpacks(&[true, true, true]),
            (vec![0], Some(2))
        );
    }

    #[test]
    fn test_tuple_unpack_is_ordinary() {
        assert_eq!(filter_variadic_unpacks(&[false, false]), (vec![0, 1], None));
    }
}
