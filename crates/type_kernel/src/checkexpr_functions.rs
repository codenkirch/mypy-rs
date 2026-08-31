//! Native port of standalone type-helper functions from `mypy/checkexpr.py`
//! and `mypy/checker.py` (Stage 9).
//!
//! These are pure-logic functions that operate on the wire-format `Type`
//! enum without needing live Python checker state. Each is exposed as a
//! `#[pyfunction]` with a Python-side strangler-fig gate.
//!
//! Deferred (return None) cases:
//!   * Functions that call `get_proper_type` (alias expansion) defer on
//!     `TypeAliasType` since the wire format has no resolved alias target.

use std::collections::HashSet;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyFrozenSet, PyList, PyModule, PyString, PyTuple, PyType};

use crate::operators::is_operator_method_name;
use crate::setops::is_type_obj_callable;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, LiteralValue, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// `TypeOfAny.special_form` == 6. Special forms are not real Any types.
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

/// `TypeOfAny.from_another_any` == 7. The source is the Any operand.
const TYPE_OF_ANY_FROM_ANOTHER_ANY: i64 = 7;

/// Same-type-with-args join disc 4: the joined arg is
/// `AnyType(from_another_any, <Any side>)`, t-preferred
/// (join.py:131-135, :282-295, :335-338); s verbatim when t is not Any.
fn joined_arg_any(src_s: &Type, src_t: &Type) -> Type {
    let src = if matches!(src_t, Type::AnyType { .. }) {
        src_t.clone()
    } else {
        src_s.clone()
    };
    Type::AnyType {
        type_of_any: TYPE_OF_ANY_FROM_ANOTHER_ANY,
        source_any: Some(Box::new(src)),
        missing_import_name: None,
    }
}

/// `TypeOfAny.unannotated` == 1.
const TYPE_OF_ANY_UNANNOTATED: i64 = 1;

/// `TypeOfAny.from_omitted_generics` == 4. Mypy uses this as the "no
/// default" sentinel for TypeVar-like types: `has_default()` returns
/// False exactly when the default is this Any, so the default must not
/// be treated as a real Any by `has_any_type`.
const TYPE_OF_ANY_FROM_OMITTED_GENERICS: i64 = 4;

/// `ArgKind.ARG_POS` = 0. `ArgKind.ARG_OPT` = 1. `ArgKind.ARG_STAR` = 2.
/// `ArgKind.ARG_NAMED` = 3. `ArgKind.ARG_STAR2` = 4. `ArgKind.ARG_NAMED_OPT` = 5.
const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

// ---------------------------------------------------------------------------
// Wire helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// `get_proper_type` for the wire format. Expands TypeAliasType by
/// returning None (defer) since the wire format has no alias target.
/// For all other types, returns the type as-is (they are already proper).
fn get_proper_or_none(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ),
    }
}

/// Resolving variant of `get_proper_or_none`: expands a `TypeAliasType`
/// through the alias resolver (chain-resolving, argument-substituting).
/// Returns an owned expanded type, or `None` to defer (missing snapshot,
/// cycle, undecodable target, unsupported substitution). Used by seams
/// whose Python mirror calls `get_proper_type` on the value (checkexpr
/// callers and checker_helpers alias expansion).
pub(crate) fn get_proper_or_expand(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Type> {
    match typ {
        Type::TypeAliasType { .. } => {
            let (target, _, _) = expanded_alias_target(typ, aliases)?;
            Some(target)
        }
        _ => Some(typ.clone()),
    }
}

/// Expand a `TypeAliasType` wire node to its frozen `target` snapshot
/// (B3a). This is a *shape-only* expansion: the target is decoded as-is
/// and the alias's type arguments are NOT substituted. That is parity-safe
/// for shape predicates (`is_type_type_context`, `method_fullname`)
/// because the answer depends only on the target's type class and name,
/// never on `Instance` args; exactly the difference between `A = Type[X]`
/// and `B = List[X]`. (`has_any_type` switched to the substituting
/// `expanded_alias_target` in B3b since it must see type args.)
///
/// Aliasing a chain (`B = A`, `A = Type[X]`) is legal in mypy, so the
/// snapshots are followed until a non-alias target is found. Recursion
/// would loop forever on `A = Tuple[A, ...]`, so aliases already seen on
/// this chain cause a defer (Python's `get_proper_type` also cannot
/// terminate those, and the shape predicates preserve Python behavior by
/// deferring). Returns `Some(expanded)` when the chain resolves, `None`
/// to defer (missing snapshot, undecodable target, or a cycle).
pub(crate) fn expand_alias_shape(
    typ: &Type,
    aliases: &dyn crate::aliases::AliasLookup,
) -> Option<Type> {
    let mut current = typ.clone();
    let mut seen: Vec<String> = Vec::new();
    loop {
        let type_ref = match &current {
            Type::TypeAliasType { type_ref, .. } => type_ref.clone(),
            _ => return Some(current),
        };
        if seen.contains(&type_ref) {
            return None;
        }
        seen.push(type_ref.clone());
        let snap = aliases.get(&type_ref)?;
        let mut buf = ReadBuffer::new(&snap.target);
        current = read_type(&mut buf, None).ok()?;
    }
}

/// Follow an alias chain to its **raw** snapshot target, without
/// substituting the alias's type arguments (unlike `expanded_alias_target`).
/// Returns the first non-alias wire type: for `B = A`, `A = list[A]`, the
/// chain resolves to `list[A]` with no typevar left behind. Used by the
/// union-expansion seams, where the result feeds `make_simplified_union` /
/// `_remove_redundant_union_items` and structural parity is what matters;
/// substituting into a generic target is both unnecessary there and
/// wrong for chains of different arities (it leaves a phantom typevar).
///
/// This RAW expansion is only valid when the alias needs no argument
/// substitution: a root `TypeAliasType` with empty `args` and a chain of
/// snapshots with empty `alias_tvars`. Any substitution requirement
/// (e.g. `Second[str]` where `Second = Node[List[int], List[T]]`) means
/// Python's `_expand_once` substitution must run, so we defer (return
/// `None`) and let the caller fall back to Python.
/// Returns `None` to defer on a missing snapshot, an alias cycle, or a
/// substitution-requiring alias.
pub(crate) fn expand_alias_target_raw(
    typ: &Type,
    aliases: &dyn crate::aliases::AliasLookup,
) -> Option<Type> {
    let mut current = typ.clone();
    let mut seen: Vec<String> = Vec::new();
    loop {
        let type_ref = match &current {
            Type::TypeAliasType { type_ref, args } => {
                if !args.is_empty() {
                    return None;
                }
                type_ref.clone()
            }
            _ => return Some(current),
        };
        if seen.contains(&type_ref) {
            return None;
        }
        seen.push(type_ref.clone());
        let snap = aliases.get(&type_ref)?;
        if !snap.alias_tvars.is_empty() {
            return None;
        }
        let mut buf = ReadBuffer::new(&snap.target);
        current = read_type(&mut buf, None).ok()?;
    }
}

/// Expand a `TypeAliasType` wire node to its **type-argument-substituted**
/// target, mirroring `TypeAliasType._expand_once` (types.py:361-392) then
/// `get_proper_type`'s chain loop. This is what the `BoolTypeQuery`
/// predicates need: Python's base `visit_type_alias_type` visits
/// `get_proper_type(t)` (the substituted target) and `t.args`.
///
/// Returns `(target, args, python_3_12)` where `target` is the
/// chain-resolved target with the alias's type arguments substituted for
/// its declared typevars (`no_args` aliases keep the target as-is), `args`
/// are the top-level `TypeAliasType`'s own arguments, and
/// `python_3_12` is the top-level snapshot's `python_3_12_type_alias`
/// flag. Returns `None` to defer: missing snapshot, undecodable target,
/// an alias cycle, or a substitution the kernel cannot perform exactly
/// (ParamSpec/TypeVarTuple, leftover typevar, etc.). Deferral is
/// parity-safe: the caller falls back to the pure-Python visitor.
pub(crate) fn expanded_alias_target(
    typ: &Type,
    aliases: &dyn crate::aliases::AliasLookup,
) -> Option<(Type, Vec<Type>, bool)> {
    let mut current = typ.clone();
    let mut args: Vec<Type> = Vec::new();
    let mut python_3_12 = false;
    let mut seen: Vec<String> = Vec::new();
    loop {
        let (type_ref, cur_args) = match &current {
            Type::TypeAliasType { type_ref, args } => (type_ref.clone(), args.clone()),
            _ => return Some((current, args, python_3_12)),
        };
        if seen.contains(&type_ref) {
            return None;
        }
        seen.push(type_ref.clone());
        let snap = aliases.get(&type_ref)?;
        // The outermost TypeAliasType's args + python_3_12 flag drive the
        // substitution and the args-visit below.
        if args.is_empty() {
            args = cur_args;
        }
        if !python_3_12 {
            python_3_12 = snap.python_3_12_type_alias;
        }
        let mut buf = ReadBuffer::new(&snap.target);
        let target = read_type(&mut buf, None).ok()?;
        if snap.no_args {
            return Some((target, args, python_3_12));
        }
        if snap.alias_tvars.is_empty() {
            current = target;
            continue;
        }
        // Build the substitution env from zip(alias_tvars, args),
        // mirroring _expand_once's `v.id: s ...}` mapping. If the arg
        // count mismatches, Python's zip truncates and leaves some

        // typevars unbound, which would leave TypeVar nodes in the
        // target that the predicate can safely treat as non-Any. The
        // exact flag is a defer (conservative).
        let env = build_alias_env(&snap.alias_tvars, &args)?;
        let substituted = crate::expandtype::expand_type_inner(&target, &env, true)?;
        current = substituted;
    }
}

/// Build the `TypeVarId -> Type` substitution map for an alias
/// application, mirroring `_expand_once` (types.py:375-385). Returns
/// `None` if the arg count mismatches the declared typevar count or an
/// identity is missing (defer).
fn build_alias_env(
    alias_tvars: &[crate::aliases::AliasTvar],
    args: &[Type],
) -> Option<std::collections::HashMap<crate::expandtype::EnvKey, Type>> {
    use std::collections::HashMap;
    let mut env = HashMap::with_capacity(alias_tvars.len());
    for (tv, arg) in alias_tvars.iter().zip(args) {
        env.insert(
            (tv.raw_id, tv.meta_level, tv.namespace.clone()),
            arg.clone(),
        );
    }
    Some(env)
}

/// Whether a CallableType is a type object (i.e. its fallback is
/// `builtins.type`). Mirrors `CallableType.is_type_obj()` — the wire
/// format stores `fallback` + `from_concatenate` but not the computed
/// `is_type_obj` boolean, so we reconstruct it here.
fn is_type_obj(fallback: &Type, from_concatenate: bool) -> bool {
    if from_concatenate {
        return false;
    }
    matches!(
        fallback,
        Type::Instance { type_ref, .. } if type_ref == "builtins.type"
    )
}

// ---------------------------------------------------------------------------
// has_any_type: BoolTypeQuery (ANY_STRATEGY)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_any_type` — whether a type contains an Any type.
/// Special forms (type_of_any == 6) are not counted as real Any.
///
/// Mirrors `HasAnyType` (checkexpr.py:7890-7926) with the resolver for
/// alias expansion (B3b). Deferral is parity-safe.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_any_type(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    ignore_in_type_obj: bool,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_any_type_inner(
        &typ,
        ignore_in_type_obj,
        resolver.alias_resolver(),
    ))
}

pub(crate) fn has_any_type_inner(
    typ: &Type,
    ignore_in_type_obj: bool,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    // `seen` holds alias fullnames already visited on this descent,
    // mirroring BoolTypeQuery.seen_aliases (type_visitor.py:604-607):

    // a repeated alias short-circuits to the strategy default (false
    // for ANY_STRATEGY). This terminates recursive aliases like
    // `A = List[A]` that `expanded_alias_target`'s per-call chain

    // detection cannot see across the has_any_type recursion boundary.
    let mut seen: Vec<String> = Vec::new();
    has_any_type_inner_seen(typ, ignore_in_type_obj, aliases, &mut seen)
}

fn has_any_type_inner_seen(
    typ: &Type,
    ignore_in_type_obj: bool,
    aliases: &crate::aliases::TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Option<bool> {
    if let Type::TypeAliasType { type_ref, .. } = typ {
        // Python's BoolTypeQuery.visit_type_alias_type (type_visitor.py:598)
        // visits the substituted target and (for new-style aliases only)

        // the args. `expanded_alias_target` substitutes exactly like
        // TypeAliasType._expand_once; when the substitution defers we
        // defer the whole query (parity-safe).
        if seen.contains(type_ref) {
            return Some(false);
        }
        seen.push(type_ref.clone());
        let (target, args, python_3_12) = expanded_alias_target(typ, aliases)?;
        if has_any_type_inner_seen(&target, ignore_in_type_obj, aliases, seen)? {
            return Some(true);
        }
        if python_3_12 {
            for arg in &args {
                if has_any_type_inner_seen(arg, ignore_in_type_obj, aliases, seen)? {
                    return Some(true);
                }
            }
        }
        return Some(false);
    }
    match typ {
        Type::AnyType { type_of_any, .. } => Some(*type_of_any != TYPE_OF_ANY_SPECIAL_FORM),
        Type::CallableType {
            arg_types,
            ret_type,
            variables,
            instance_type,
            fallback,
            from_concatenate,
            ..
        } => {
            if ignore_in_type_obj && is_type_obj(fallback, *from_concatenate) {
                return Some(false);
            }
            for t in arg_types {
                match has_any_type_inner_seen(t, ignore_in_type_obj, aliases, seen) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            match has_any_type_inner_seen(ret_type, ignore_in_type_obj, aliases, seen) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
            for v in variables {
                match has_any_type_inner_seen(v, ignore_in_type_obj, aliases, seen) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            if let Some(it) = instance_type {
                return has_any_type_inner_seen(it, ignore_in_type_obj, aliases, seen);
            }
            Some(false)
        }
        _ => {
            for child in children(typ) {
                match has_any_type_inner_seen(child, ignore_in_type_obj, aliases, seen) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
    }
}

/// Yield direct child types (same as visitor::children, duplicated here
/// to keep this module self-contained).
fn children(typ: &Type) -> Vec<&Type> {
    let mut out = Vec::new();
    match typ {
        Type::UnboundType { args, .. } => out.extend(args.iter()),
        Type::UnpackType { typ } => out.push(typ),
        Type::Instance {
            args,
            last_known_value,
            ..
        } => {
            out.extend(args.iter());
            if let Some(lkv) = last_known_value {
                out.push(lkv);
            }
        }
        Type::Overloaded { items } => out.extend(items.iter()),
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            out.push(partial_fallback);
            out.extend(items.iter());
        }
        Type::TypedDictType {
            items, fallback, ..
        } => {
            out.push(fallback);
            out.extend(items.iter().map(|(_, t)| t));
        }
        Type::LiteralType { fallback, .. } => out.push(fallback),
        Type::UnionType { items, .. } => out.extend(items.iter()),
        Type::TypeType { item, .. } => out.push(item),
        Type::AnyType {
            source_any: Some(sa),
            ..
        } => out.push(sa),
        Type::AnyType {
            source_any: None, ..
        } => {}
        // TypeVar families carry bounds/defaults/values that must be
        // queried: Python's BoolTypeQuery.visit_type_var / param_spec /
        // type_var_tuple visit upper_bound + default (only when

        // has_default()) + values/prefix. The no-default sentinel
        // (from_omitted_generics) is not a real Any and is skipped.
        // TypeVarTupleType's tuple_fallback is NOT visited by Python.
        Type::TypeVarType {
            values,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            if !matches!(
                default.as_ref(),
                Type::AnyType {
                    type_of_any: TYPE_OF_ANY_FROM_OMITTED_GENERICS,
                    ..
                }
            ) {
                out.push(default);
            }
            out.extend(values.iter());
        }
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            if !matches!(
                default.as_ref(),
                Type::AnyType {
                    type_of_any: TYPE_OF_ANY_FROM_OMITTED_GENERICS,
                    ..
                }
            ) {
                out.push(default);
            }
            out.extend(prefix.arg_types.iter());
        }
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => {
            out.push(upper_bound);
            if !matches!(
                default.as_ref(),
                Type::AnyType {
                    type_of_any: TYPE_OF_ANY_FROM_OMITTED_GENERICS,
                    ..
                }
            ) {
                out.push(default);
            }
        }
        // CallableType handled separately. Parameters, leaves: none.
        _ => {}
    }
    out
}

// ---------------------------------------------------------------------------
// has_uninhabited_component
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_uninhabited_component` — whether a type contains
/// an UninhabitedType component.
///
/// Mirrors `HasUninhabitedComponent` (checkexpr.py). Expands TypeAliasType
/// via the alias resolver (Phase C, #594), same shape as `has_any_type`
/// (B3b #593): `BoolTypeQuery.visit_type_alias_type` visits the substituted
/// target and args. Defers only when alias expansion defers (missing
/// snapshot, undecodable target, cycle, or a substitution the kernel
/// cannot perform exactly).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_uninhabited_component(
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_uninhabited_component_inner(
        &typ,
        resolver.alias_resolver(),
    ))
}

pub(crate) fn has_uninhabited_component_inner(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let mut seen: Vec<String> = Vec::new();
    has_uninhabited_component_inner_seen(typ, aliases, &mut seen)
}

/// `seen` holds alias fullnames already visited on this descent; a repeated
/// alias short-circuits to the strategy default (false), which terminates
/// recursive aliases across the recursion boundary. Passed through the whole
/// walk, matching `has_any_type_inner_seen`, so nested aliases inside
/// children (e.g. `A = List[A]`) do not restart the cycle detection.
fn has_uninhabited_component_inner_seen(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Option<bool> {
    if let Type::TypeAliasType { type_ref, .. } = typ {
        // Python's BoolTypeQuery.visit_type_alias_type (type_visitor.py:598)
        // visits the substituted target and (for new-style aliases only)

        // the args. When the substitution defers we defer the whole query
        // (parity-safe).
        if seen.contains(type_ref) {
            return Some(false);
        }
        seen.push(type_ref.clone());
        let (target, args, python_3_12) = expanded_alias_target(typ, aliases)?;
        if has_uninhabited_component_inner_seen(&target, aliases, seen)? {
            return Some(true);
        }
        if python_3_12 {
            for arg in &args {
                if has_uninhabited_component_inner_seen(arg, aliases, seen)? {
                    return Some(true);
                }
            }
        }
        return Some(false);
    }
    if matches!(typ, Type::UninhabitedType { .. }) {
        return Some(true);
    }
    for child in all_children(typ) {
        match has_uninhabited_component_inner_seen(child, aliases, seen) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// has_ambiguous_uninhabited_component
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_ambiguous_uninhabited_component` — whether a type
/// contains an UninhabitedType marked ambiguous.
///
/// Mirrors `HasAmbiguousUninhabitedComponentsQuery` (checkexpr.py:
/// 7007-7018). `ambiguous` is the flag on the UninhabitedType wire
/// variant; False matches the plain has_uninhabited_component case.
/// Expands TypeAliasType via the alias resolver (Phase C, #594), same as
/// `has_uninhabited_component`.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_ambiguous_uninhabited_component(
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_ambiguous_uninhabited_component_inner(
        &typ,
        resolver.alias_resolver(),
    ))
}

pub(crate) fn has_ambiguous_uninhabited_component_inner(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let mut seen: Vec<String> = Vec::new();
    has_ambiguous_uninhabited_component_inner_seen(typ, aliases, &mut seen)
}

/// Same as `has_uninhabited_component_inner_seen` but for the ambiguous
/// flag: True only when an UninhabitedType marked ambiguous is reached.
fn has_ambiguous_uninhabited_component_inner_seen(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Option<bool> {
    if let Type::TypeAliasType { type_ref, .. } = typ {
        if seen.contains(type_ref) {
            return Some(false);
        }
        seen.push(type_ref.clone());
        let (target, args, python_3_12) = expanded_alias_target(typ, aliases)?;
        if has_ambiguous_uninhabited_component_inner_seen(&target, aliases, seen)? {
            return Some(true);
        }
        if python_3_12 {
            for arg in &args {
                if has_ambiguous_uninhabited_component_inner_seen(arg, aliases, seen)? {
                    return Some(true);
                }
            }
        }
        return Some(false);
    }
    if let Type::UninhabitedType { ambiguous } = typ {
        return Some(*ambiguous);
    }
    for child in all_children(typ) {
        match has_ambiguous_uninhabited_component_inner_seen(child, aliases, seen) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// has_erased_component
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_erased_component` — whether a type contains an
/// ErasedType component (checkexpr.py:8060-8071).
///
/// Mirrors `HasErasedComponentsQuery`, a `BoolTypeQuery(ANY_STRATEGY)` whose
/// only override is `visit_erased_type -> True`; every other node returns the
/// default (false), so this is the same ANY walk as
/// `has_uninhabited_component` with a different leaf predicate. Type aliases
/// expand via the alias resolver exactly as the uninhabited query does
/// (`visit_type_alias_type` targets true when the substituted target or a
/// new-style alias arg contains an ErasedType). Defer only when alias
/// expansion defers.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_erased_component(
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_erased_component_inner(&typ, resolver.alias_resolver()))
}

pub(crate) fn has_erased_component_inner(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let mut seen: Vec<String> = Vec::new();
    has_erased_component_inner_seen(typ, aliases, &mut seen)
}

/// ANY walk over `all_children` with `ErasedType` as the terminal predicate,
/// matching `HasErasedComponentsQuery` (checkexpr.py:8064-8071). Alias
/// expansion and cycle handling are identical to the uninhabited query.
fn has_erased_component_inner_seen(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Option<bool> {
    if let Type::TypeAliasType { type_ref, .. } = typ {
        if seen.contains(type_ref) {
            return Some(false);
        }
        seen.push(type_ref.clone());
        let (target, args, python_3_12) = expanded_alias_target(typ, aliases)?;
        if has_erased_component_inner_seen(&target, aliases, seen)? {
            return Some(true);
        }
        if python_3_12 {
            for arg in &args {
                if has_erased_component_inner_seen(arg, aliases, seen)? {
                    return Some(true);
                }
            }
        }
        return Some(false);
    }
    if matches!(typ, Type::ErasedType) {
        return Some(true);
    }
    for child in all_children(typ) {
        match has_erased_component_inner_seen(child, aliases, seen) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// has_abstract_type
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.has_abstract_type`
/// (checkexpr.py:8134-8143): pure boolean conjunction on two live
/// `mypy.types.ProperType` objects.
///
/// Mirrors the Python body:
///   isinstance(caller_type, FunctionLike)
///   and isinstance(callee_type, TypeType)
///   and caller_type.is_type_obj()
///   and (caller_type.type_object().is_abstract
///        or caller_type.type_object().is_protocol)
///   and isinstance(callee_type.item, Instance)
///   and (callee_type.item.type.is_abstract
///        or callee_type.item.type.is_protocol)
///   and not allow_abstract_call
///
/// `is_type_obj()` and `type_object()` are read by calling the live
/// Python methods: `is_type_obj` walks `fallback.type.is_metaclass()`'s
/// MRO and `get_proper_type(ret_type)`, while `type_object` runs the
/// `force_fallback` coercion chain in `get_instance_type`. The wire
/// format cannot reconstruct those without a resolver, so this seam
/// delegates them to the same live methods the pure-Python body calls.
/// Reading `is_abstract` / `is_protocol` is a plain `bool` attribute
/// read on the live `TypeInfo`, so the truth value reduces to scalar
/// bools and the function never defers.
#[pyfunction]
pub(crate) fn rust_has_abstract_type(
    py: Python<'_>,
    caller_type: &PyAny,
    callee_type: &PyAny,
    allow_abstract_call: bool,
) -> PyResult<Option<bool>> {
    Ok(has_abstract_type_inner(
        py,
        caller_type,
        callee_type,
        allow_abstract_call,
    ))
}

fn has_abstract_type_inner(
    py: Python<'_>,
    caller_type: &PyAny,
    callee_type: &PyAny,
    allow_abstract_call: bool,
) -> Option<bool> {
    use crate::typeinfo::read_bool_attr;
    if allow_abstract_call {
        return Some(false);
    }
    let mypy_types = py.import("mypy.types").ok()?;
    let function_like = mypy_types.getattr("FunctionLike").ok()?;
    if !caller_type.is_instance(function_like).ok()? {
        return Some(false);
    }
    let type_type = mypy_types.getattr("TypeType").ok()?;
    if !callee_type.is_instance(type_type).ok()? {
        return Some(false);
    }
    let is_type_obj: bool = caller_type
        .call_method0("is_type_obj")
        .ok()?
        .extract()
        .ok()?;
    if !is_type_obj {
        return Some(false);
    }
    let type_obj = caller_type.call_method0("type_object").ok()?;
    if !read_bool_attr(type_obj, "is_abstract").unwrap_or(false)
        && !read_bool_attr(type_obj, "is_protocol").unwrap_or(false)
    {
        return Some(false);
    }
    let item = callee_type.getattr("item").ok()?;
    let instance_cls = mypy_types.getattr("Instance").ok()?;
    if !item.is_instance(instance_cls).ok()? {
        return Some(false);
    }
    let item_type = item.getattr("type").ok()?;
    if !read_bool_attr(item_type, "is_abstract").unwrap_or(false)
        && !read_bool_attr(item_type, "is_protocol").unwrap_or(false)
    {
        return Some(false);
    }
    Some(true)
}

// ---------------------------------------------------------------------------
// has_bytes_component
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_bytes_component` — is this one of builtin byte
/// types, or a union that contains it?
///
/// Mirrors `has_bytes_component` (checkexpr.py:8762-8774). The Python
/// body `get_proper_type`-expands a top-level TypeAliasType and every
/// union item before the class-name check, so we thread the alias
/// resolver and expand via `get_proper_or_expand`. A missing resolver
/// snapshot defers to the pure-Python fallback.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_bytes_component(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_bytes_component_inner(&typ, resolver.alias_resolver()))
}

pub(crate) fn has_bytes_component_inner(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let proper = get_proper_or_expand(typ, aliases)?;
    match &proper {
        Type::UnionType { items, .. } => {
            for t in items {
                match has_bytes_component_inner(t, aliases) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        Type::Instance { type_ref, .. } => {
            Some(type_ref == "builtins.bytes" || type_ref == "builtins.bytearray")
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// has_bool_item
// ---------------------------------------------------------------------------

/// `mypy.checker.has_bool_item` — return True if type is 'bool' or a
/// union with a 'bool' item.
///
/// Mirrors `has_bool_item` (checker.py:9731-9738). Defers on TypeAliasType.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_bool_item(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_bool_item_inner(&typ))
}

pub(crate) fn has_bool_item_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance { type_ref, .. } => Some(*type_ref == "builtins.bool"),
        Type::UnionType { items, .. } => {
            for t in items {
                match has_bool_item_inner(t) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_non_empty_tuple
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_non_empty_tuple` — whether t is a TupleType with
/// at least one item.
///
/// Mirrors `is_non_empty_tuple` (checkexpr.py:6702-6704). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_non_empty_tuple(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_non_empty_tuple_inner(&typ))
}

pub(crate) fn is_non_empty_tuple_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::TupleType { items, .. } => Some(!items.is_empty()),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// has_coroutine_decorator
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.has_coroutine_decorator` — whether t came from a
/// function decorated with `@coroutine`.
///
/// Mirrors `has_coroutine_decorator` (checkexpr.py:6662-6665). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_coroutine_decorator(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_coroutine_decorator_inner(&typ))
}

pub(crate) fn has_coroutine_decorator_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance { type_ref, .. } => Some(*type_ref == "typing.AwaitableGenerator"),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_async_def
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_async_def` — whether t came from a function defined
/// using `async def`.
///
/// An `async def` decorated with `@typing.coroutine` (or `@asyncio.coroutine`)
/// has return type `typing.AwaitableGenerator[...]`, which preserves the
/// original return type as its 4th type argument. That argument is unwrapped
/// and checked for `typing.Coroutine` (the actual `async def` return type).
///
/// Mirrors `is_async_def` (checkexpr.py:6900-6909). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_async_def(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_async_def_inner(&typ))
}

pub(crate) fn is_async_def_inner(typ: &Type) -> Option<bool> {
    let mut proper = get_proper_or_none(typ)?;
    if let Type::Instance { type_ref, args, .. } = proper {
        if type_ref == "typing.AwaitableGenerator" && args.len() >= 4 {
            proper = get_proper_or_none(&args[3])?;
        }
    }
    match proper {
        Type::Instance { type_ref, .. } => Some(type_ref == "typing.Coroutine"),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_duplicate_mapping
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_duplicate_mapping` — whether multiple actual
/// arguments map to the same formal in a non-star position, i.e. the call
/// has duplicate values for that formal.
///
/// Mirrors `is_duplicate_mapping` (checkexpr.py:6947-6959). The exception
/// cases where duplicates are allowed at runtime:
///   * `f(..., *args, **kwargs)` with exactly two actuals (`*args` and
///     `**kwargs`) mapping to the same formal.
///   * Multiple `**kwargs` that all map to the same formal, provided they
///     are not TypedDicts (a non-TypedDict `**kwargs` cannot be matched
///     with certainty).
///
/// `actual_types` each carry a serialized type; each non-star actual is
/// resolved through `get_proper_or_none` so a `TypeAliasType` actual
/// expands via the alias resolver (mirroring Python's
/// `isinstance(get_proper_type(actual_types[m]), TypedDictType)`). An
/// unexpandable alias defers (None).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_duplicate_mapping(
    mapping: Vec<i64>,
    actual_types: Vec<Vec<u8>>,
    actual_kinds: Vec<i64>,
    resolver: &NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let mut types = Vec::with_capacity(mapping.len());
    for &idx in &mapping {
        match actual_types.get(idx as usize) {
            Some(bytes) => match decode_type(bytes) {
                Some(t) => types.push(t),
                None => return Ok(None),
            },
            None => return Ok(None),
        }
    }
    Ok(is_duplicate_mapping_inner(
        &mapping,
        &types,
        &actual_kinds,
        resolver.alias_resolver(),
    ))
}

fn is_duplicate_mapping_inner(
    mapping: &[i64],
    actual_types: &[Type],
    actual_kinds: &[i64],
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    // `mapping` with one entry (or fewer) cannot be a duplicate.
    if mapping.len() <= 1 {
        return Some(false);
    }
    // `f(..., *args, **kwargs)`: the two actuals can both map to the same
    // formal and no runtime duplicate occurs. Allow this exception.
    if mapping.len() == 2 {
        let first = *actual_kinds.get(mapping[0] as usize)?;
        let second = *actual_kinds.get(mapping[1] as usize)?;
        if first == ARG_STAR && second == ARG_STAR2 {
            return Some(false);
        }
    }
    // Exceptions where duplicates are allowed: every mapped actual is a
    // non-TypedDict `**kwargs` (cannot be matched with certainty), so
    // `all(mapped actual is a non-TypedDict **kwargs)` disables the check.
    let mut all_non_typeddict_star2 = true;
    for (i, &idx) in mapping.iter().enumerate() {
        let kind = *actual_kinds.get(idx as usize)?;
        if kind != ARG_STAR2 {
            all_non_typeddict_star2 = false;
            break;
        }
        let proper = get_proper_or_expand(&actual_types[i], aliases)?;
        if matches!(proper, Type::TypedDictType { .. }) {
            all_non_typeddict_star2 = false;
            break;
        }
    }
    Some(!all_non_typeddict_star2)
}

// ---------------------------------------------------------------------------
// is_typed_callable
// ---------------------------------------------------------------------------

/// `mypy.checker.is_typed_callable` — whether a callable type has at
/// least one non-unannotated-Any type in its args or return.
///
/// Mirrors `is_typed_callable` (checker.py:9613-9621). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_typed_callable(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_typed_callable_inner(&typ))
}

pub(crate) fn is_typed_callable_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::CallableType {
            arg_types,
            ret_type,
            ..
        } => {
            // Returns True if NOT all types are unannotated Any.
            let all_unannotated = arg_types
                .iter()
                .chain(std::iter::once(ret_type.as_ref()))
                .all(is_unannotated_any_type);
            Some(!all_unannotated)
        }
        _ => Some(false),
    }
}

fn is_unannotated_any_type(typ: &Type) -> bool {
    matches!(typ, Type::AnyType { type_of_any, .. } if *type_of_any == TYPE_OF_ANY_UNANNOTATED)
}

// ---------------------------------------------------------------------------
// is_private
// ---------------------------------------------------------------------------

/// `mypy.checker.is_private` — check if node name is private to class.
/// Mirrors `is_private` (checker.py:9721-9723).
#[pyfunction]
pub(crate) fn rust_is_private(node_name: &str) -> PyResult<bool> {
    Ok(node_name.starts_with("__") && !node_name.ends_with("__"))
}

// ---------------------------------------------------------------------------
// is_operator_method
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_operator_method` — check if fullname is an
/// operator method.
/// Mirrors `is_operator_method` (checkexpr.py:7019-7026).
#[pyfunction]
pub(crate) fn rust_is_operator_method(fullname: Option<&str>) -> PyResult<bool> {
    Ok(match fullname {
        Some(f) => {
            let short_name = f.rsplit('.').next().unwrap_or("");
            is_operator_method_name(short_name)
        }
        None => false,
    })
}

// ---------------------------------------------------------------------------
// are_argument_counts_overlapping
// ---------------------------------------------------------------------------

/// `mypy.checker.are_argument_counts_overlapping` — can a single call
/// match both t and s, based just on positional argument counts?
///
/// Mirrors `are_argument_counts_overlapping` (checker.py:9115-9119).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_are_argument_counts_overlapping(
    t_bytes: &[u8],
    s_bytes: &[u8],
) -> PyResult<Option<bool>> {
    let t = match decode_type(t_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let s = match decode_type(s_bytes) {
        Some(s) => s,
        None => return Ok(None),
    };
    Ok(are_argument_counts_overlapping_inner(&t, &s))
}

pub(crate) fn are_argument_counts_overlapping_inner(t: &Type, s: &Type) -> Option<bool> {
    let t_kinds = match get_proper_or_none(t)? {
        Type::CallableType { arg_kinds, .. } => arg_kinds,
        _ => return Some(false),
    };
    let s_kinds = match get_proper_or_none(s)? {
        Type::CallableType { arg_kinds, .. } => arg_kinds,
        _ => return Some(false),
    };
    let min_args_t = count_min_args(t_kinds);
    let min_args_s = count_min_args(s_kinds);
    let min_args = min_args_t.max(min_args_s);
    let max_t = count_max_positional(t_kinds);
    let max_s = count_max_positional(s_kinds);
    let max_args = max_t.min(max_s);
    Some(min_args <= max_args)
}

/// `min_args`: count of ARG_POS only (required positional args).
/// Mirrors `CallableType.min_args` property: `arg_kinds.count(ARG_POS)`.
fn count_min_args(arg_kinds: &[i64]) -> usize {
    arg_kinds.iter().filter(|&&k| k == ARG_POS).count()
}

/// `max_possible_positional_args`: if the callable has *args or **kwargs,
/// returns `usize::MAX` (mirrors `sys.maxsize`). Otherwise counts all
/// positional args (ARG_POS, ARG_OPT, ARG_NAMED, ARG_NAMED_OPT).
fn count_max_positional(arg_kinds: &[i64]) -> usize {
    if arg_kinds.iter().any(|&k| k == ARG_STAR || k == ARG_STAR2) {
        usize::MAX
    } else {
        arg_kinds
            .iter()
            .filter(|&&k| k == ARG_POS || k == ARG_OPT || k == ARG_NAMED || k == ARG_NAMED_OPT)
            .count()
    }
}

// ---------------------------------------------------------------------------
// is_type_type_context
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.is_type_type_context` — whether context is a TypeType
/// or a union containing TypeType.
///
/// Mirrors `is_type_type_context` (checkexpr.py:7031-7036). Expands
/// TypeAliasType via the alias resolver (B3a); defers only when the
/// alias snapshot is missing or its target cannot be decoded.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_type_type_context(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_type_type_context_inner(&typ, resolver.alias_resolver()))
}

pub(crate) fn is_type_type_context_inner(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let expanded = expand_alias_shape(typ, aliases)?;
    match expanded {
        Type::TypeType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            for t in &items {
                match is_type_type_context_inner(t, aliases) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// try_getting_literal
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.try_getting_literal` — if possible, get a more
/// precise literal type for a given type. Unwraps Instance with
/// last_known_value.
///
/// Mirrors `try_getting_literal` (checkexpr.py:6961-6965). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_try_getting_literal(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let result = match try_getting_literal_inner(&typ) {
        Some(r) => r,
        None => return Ok(None),
    };
    Ok(encode_type(&result))
}

pub(crate) fn try_getting_literal_inner(typ: &Type) -> Option<Type> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => Some(lkv.as_ref().clone()),
        _ => Some(proper.clone()),
    }
}

// ---------------------------------------------------------------------------
// enum-callable base + protocol-test callee classifiers
// ---------------------------------------------------------------------------

/// Mirrors checkexpr.py:2586: true when `callable_node` is a RefExpr whose
/// fullname is an Enum base.
#[pyfunction]
pub(crate) fn rust_is_enum_callable_base(
    py: Python<'_>,
    callable_node: &PyAny,
    enum_bases: &PyAny,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if !callable_node.is_instance(ref_expr_cls)? {
        return Ok(false);
    }
    let enum_set = normalize_enum_bases(enum_bases)?;
    let fullname = callable_node.getattr("fullname")?;
    let fullname_str: &str = fullname.downcast::<PyString>()?.to_str()?;
    Ok(enum_set.contains(fullname_str))
}

/// Normalize `enum_bases` (frozenset, tuple, or str) into a `HashSet`.
fn normalize_enum_bases(enum_bases: &PyAny) -> PyResult<HashSet<String>> {
    if let Ok(fs) = enum_bases.downcast::<PyFrozenSet>() {
        let mut result = HashSet::with_capacity(fs.len());
        for item in fs.iter() {
            let s = item.downcast::<PyString>()?;
            result.insert(s.to_str()?.to_string());
        }
        return Ok(result);
    }
    if let Ok(tup) = enum_bases.downcast::<PyTuple>() {
        let mut result = HashSet::with_capacity(tup.len());
        for item in tup.iter() {
            let s = item.downcast::<PyString>()?;
            result.insert(s.to_str()?.to_string());
        }
        return Ok(result);
    }
    let s = enum_bases.downcast::<PyString>()?;
    Ok([s.to_str()?.to_string()].into_iter().collect())
}

/// Mirrors checkexpr.py:1467-1471: which protocol-test branch a call takes.
/// Returns the callee tag when `n_args == 2` and `callee` is a RefExpr with
/// a isinstance/issubclass fullname; otherwise None (defer to Python).
#[pyfunction]
pub(crate) fn rust_classify_protocol_test_callee(
    py: Python<'_>,
    callee: &PyAny,
    n_args: usize,
) -> PyResult<Option<String>> {
    if n_args != 2 {
        return Ok(None);
    }
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if !callee.is_instance(ref_expr_cls)? {
        return Ok(None);
    }
    let fullname = callee.getattr("fullname")?;
    let fullname_str: &str = fullname.downcast::<PyString>()?.to_str()?;
    if fullname_str == "builtins.isinstance" {
        return Ok(Some("builtins.isinstance".to_string()));
    }
    if fullname_str == "builtins.issubclass" {
        return Ok(Some("builtins.issubclass".to_string()));
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// is_string_literal
// ---------------------------------------------------------------------------

/// `mypy.checker.is_string_literal` — check if a type is a single string
/// literal. Uses `try_getting_str_literals_from_type` semantics: checks
/// for LiteralType with str value or Instance with last_known_value that
/// is a str literal.
///
/// Mirrors `is_string_literal` (checker.py:9726-9728). Defers on alias.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_string_literal(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_string_literal_inner(&typ))
}

pub(crate) fn is_string_literal_inner(typ: &Type) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::LiteralType { value, fallback } => {
            // Check if it's a string literal (fallback is builtins.str).
            if let Type::Instance { type_ref, .. } = fallback.as_ref() {
                Some(*type_ref == "builtins.str" && matches!(value, LiteralValue::Str(_)))
            } else {
                Some(false)
            }
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => is_string_literal_inner(lkv),
        Type::UnionType { items, .. } => {
            if items.len() != 1 {
                return Some(false);
            }
            is_string_literal_inner(&items[0])
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// is_untyped_decorator (live-object walk over Instance `__call__`)
// ---------------------------------------------------------------------------

/// Bounded descent depth for the live `is_untyped_decorator` walk. The
/// shapes it recurses into (Overloaded items, an Instance's `__call__`
/// method's func/var types) are effectively leaves, so the cap only guards
/// a pathological recursive decorator structure. Beyond the cap the seam
/// defers and the Python body re-runs (which would recurse identically).
const UNTYPED_DECORATOR_LIVE_DEPTH_CAP: u32 = 50;

/// `mypy.checker.is_untyped_decorator` (checker.py:12067-12097) as a
/// live-PyO3-object seam (the `rust_is_final_enum_value` shape). The wire
/// version deferred on every Instance (the `__call__` lookup needs the live
/// `TypeInfo`), which also dragged the `rust_check_for_untyped_decorator`
/// conjunction to 64% native.
///
/// Mirrors the Python body: `get_proper_type` on a top-level alias (via the
/// real Python function), then the CallableType / Overloaded / Instance
/// dispatch. The Instance arm runs a real `TypeInfo.get_method("__call__")`
/// and recurses on the method's live types. Returns `None` (defer to the
/// Python body) on any unreadable attribute, an undecodable
/// `type_of_any`, or an alias nested inside a callable's arg/ret types
/// (Python's `get_proper_types` would expand it from the live alias node).
#[pyfunction]
#[pyo3(signature = (typ))]
pub(crate) fn rust_is_untyped_decorator(
    py: Python<'_>,
    typ: Option<&PyAny>,
) -> PyResult<Option<bool>> {
    if typ.is_none() {
        // is_untyped_decorator(None) is True (get_proper_type(None) is falsy).
        return Ok(Some(true));
    }
    Ok(is_untyped_decorator_live(py, typ.unwrap()))
}

/// Shared walk body. `typ` is the live `Type` object; the top-level
/// alias/TypeGuardedType normalization mirrors Python's
/// `get_proper_type(typ)` and is also what the Overloaded item recursion
/// re-enters through (each item rides the same normalization).
pub(crate) fn is_untyped_decorator_live(py: Python<'_>, typ: &PyAny) -> Option<bool> {
    let types_mod = py.import("mypy.types").ok()?;
    let alias_cls: &PyType = types_mod.getattr("TypeAliasType").ok()?.downcast().ok()?;
    let guarded_cls: &PyType = types_mod.getattr("TypeGuardedType").ok()?.downcast().ok()?;
    // Python's get_proper_type is identity outside these two kinds.
    let proper: &PyAny = if typ.is_instance(alias_cls).ok()? || typ.is_instance(guarded_cls).ok()? {
        types_mod
            .getattr("get_proper_type")
            .ok()?
            .call1((typ,))
            .ok()?
    } else {
        typ
    };
    walk_untyped_decorator_proper(py, types_mod, proper, 0)
}

/// The post-`get_proper_type` dispatch of `is_untyped_decorator`. `not typ`
/// is True for a falsy type (None from an alias expansion).
fn walk_untyped_decorator_proper(
    py: Python<'_>,
    types_mod: &PyModule,
    proper: &PyAny,
    depth: u32,
) -> Option<bool> {
    if depth > UNTYPED_DECORATOR_LIVE_DEPTH_CAP {
        return None;
    }
    if proper.is_none() {
        return Some(true);
    }
    let callable_cls: &PyType = types_mod.getattr("CallableType").ok()?.downcast().ok()?;
    if proper.is_instance(callable_cls).ok()? {
        return Some(!is_typed_callable_live(types_mod, proper, depth)?);
    }
    let overloaded_cls: &PyType = types_mod.getattr("Overloaded").ok()?.downcast().ok()?;
    if proper.is_instance(overloaded_cls).ok()? {
        return overloaded_untyped_fold(py, proper, depth);
    }
    let instance_cls: &PyType = types_mod.getattr("Instance").ok()?.downcast().ok()?;
    if proper.is_instance(instance_cls).ok()? {
        return instance_untyped_decorator_walk(py, types_mod, proper, depth);
    }
    // AnyType, NoneType, UninhabitedType, ...: the Python tail returns True.
    Some(true)
}

/// `any(is_untyped_decorator(item) for item in typ.items)` over the live
/// item list. An item defer (unreadable fact / nested alias) defers the
/// whole call, mirroring the Python `any` that would re-run the body.
fn overloaded_untyped_fold(py: Python<'_>, typ: &PyAny, depth: u32) -> Option<bool> {
    if depth > UNTYPED_DECORATOR_LIVE_DEPTH_CAP {
        return None;
    }
    let items: &PyList = typ.getattr("items").ok()?.downcast().ok()?;
    for item in items.iter() {
        if is_untyped_decorator_live(py, item)? {
            return Some(true);
        }
    }
    Some(false)
}

/// The Instance arm of `is_untyped_decorator`: the `__call__` lookup and
/// its three sub-arms (Decorator head, Overloaded method type, plain
/// method type). A `get_method` miss is `Some(false)`.
fn instance_untyped_decorator_walk(
    py: Python<'_>,
    types_mod: &PyModule,
    typ: &PyAny,
    depth: u32,
) -> Option<bool> {
    if depth > UNTYPED_DECORATOR_LIVE_DEPTH_CAP {
        return None;
    }
    let info = typ.getattr("type").ok()?;
    let method = info.call_method1("get_method", ("__call__",)).ok()?;
    if method.is_none() {
        return Some(false);
    }
    let nodes_mod = py.import("mypy.nodes").ok()?;
    let decorator_cls: &PyType = nodes_mod.getattr("Decorator").ok()?.downcast().ok()?;
    if method.is_instance(decorator_cls).ok()? {
        // is_untyped_decorator(method.func.type) or is_untyped_decorator(
        //     method.var.type); is_untyped_decorator(None) is True, so a
        // None func.type short-circuits the `or`.
        let func_type = method.getattr("func").ok()?.getattr("type").ok()?;
        if func_type.is_none() {
            return Some(true);
        }
        if is_untyped_decorator_live(py, func_type)? {
            return Some(true);
        }
        let var_type = method.getattr("var").ok()?.getattr("type").ok()?;
        if var_type.is_none() {
            return Some(true);
        }
        return is_untyped_decorator_live(py, var_type);
    }
    let method_type = method.getattr("type").ok()?;
    if method_type.is_none() {
        // not is_typed_callable(None)
        return Some(true);
    }
    let overloaded_cls: &PyType = types_mod.getattr("Overloaded").ok()?.downcast().ok()?;
    if method_type.is_instance(overloaded_cls).ok()? {
        return overloaded_untyped_fold(py, method_type, depth + 1);
    }
    let callable_cls: &PyType = types_mod.getattr("CallableType").ok()?.downcast().ok()?;
    if !method_type.is_instance(callable_cls).ok()? {
        // is_typed_callable is False outside CallableType.
        return Some(true);
    }
    Some(!is_typed_callable_live(types_mod, method_type, depth + 1)?)
}

/// The `is_typed_callable` fold of `is_untyped_decorator`, over a live
/// proper `CallableType` (the isinstance check is the caller's, mirroring
/// the Python order). A non-unannotated-Any arg/ret or a non-Any type
/// answers typed; an alias/guard leaf defers (Python's `get_proper_types`
/// expands it from the live alias node).
fn is_typed_callable_live(types_mod: &PyModule, c: &PyAny, depth: u32) -> Option<bool> {
    if depth > UNTYPED_DECORATOR_LIVE_DEPTH_CAP {
        return None;
    }
    let args: &PyList = c.getattr("arg_types").ok()?.downcast().ok()?;
    let ret = c.getattr("ret_type").ok()?;
    let any_cls: &PyType = types_mod.getattr("AnyType").ok()?.downcast().ok()?;
    let alias_cls: &PyType = types_mod.getattr("TypeAliasType").ok()?.downcast().ok()?;
    let guarded_cls: &PyType = types_mod.getattr("TypeGuardedType").ok()?.downcast().ok()?;
    let unannotated = types_mod
        .getattr("TypeOfAny")
        .ok()?
        .getattr("unannotated")
        .ok()?;
    for t in args.iter().chain(std::iter::once(ret)) {
        if t.is_instance(any_cls).ok()? {
            let toa = t.getattr("type_of_any").ok()?;
            let is_unannotated = toa.eq(unannotated).ok()?;
            if is_unannotated {
                continue;
            }
            return Some(true);
        }
        if t.is_instance(alias_cls).ok()? || t.is_instance(guarded_cls).ok()? {
            return None;
        }
        return Some(true);
    }
    Some(false)
}

// ---------------------------------------------------------------------------
// is_typeddict_type_context
// ---------------------------------------------------------------------------

/// `mypy.checker.is_typeddict_type_context` — whether the type is a
/// TypedDictType (used as a type context for TypedDict construction).
///
/// Mirrors `is_typeddict_type_context` (checker.py:9978-9988). Expands
/// TypeAliasType via the alias resolver like `rust_is_type_type_context`;
/// defers only when the alias snapshot is missing or its chain cycles.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_typeddict_type_context(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_typeddict_type_context_inner(
        &typ,
        resolver.alias_resolver(),
    ))
}

pub(crate) fn is_typeddict_type_context_inner(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let proper = expand_alias_shape(typ, aliases)?;
    match proper {
        Type::TypedDictType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            for t in items {
                match is_typeddict_type_context_inner(&t, aliases) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// allow_fast_container_literal
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.allow_fast_container_literal` — whether a type is a
/// fast-path container literal: an Instance, or a TupleType whose items
/// all qualify.
///
/// Mirrors `allow_fast_container_literal` (checkexpr.py:413-419). The
/// Python body `get_proper_type`-expands a TypeAliasType before the
/// isinstance chain, so we thread the alias resolver and expand via
/// `expand_alias_shape` (the answer depends only on the target's type
/// class, never on Instance args). A recursive alias defers (a shape
/// chain cycle returns None; Python would answer False), as does a
/// missing resolver snapshot.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_allow_fast_container_literal(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(allow_fast_container_literal_inner(
        &typ,
        resolver.alias_resolver(),
    ))
}

pub(crate) fn allow_fast_container_literal_inner(
    typ: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let proper = expand_alias_shape(typ, aliases)?;
    match &proper {
        Type::TupleType { items, .. } => {
            for it in items {
                match allow_fast_container_literal_inner(it, aliases) {
                    Some(true) => {}
                    Some(false) => return Some(false),
                    None => return None,
                }
            }
            Some(true)
        }
        Type::Instance { .. } => Some(true),
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// all_children: include CallableType children (for has_uninhabited)
// ---------------------------------------------------------------------------

/// Like `children` but also yields CallableType children (arg_types,
/// ret_type, variables, instance_type). Used by has_uninhabited_component
/// which needs to recurse into callables.
fn all_children(typ: &Type) -> Vec<&Type> {
    let mut out = children(typ);
    if let Type::CallableType {
        arg_types,
        ret_type,
        variables,
        instance_type,
        ..
    } = typ
    {
        out.extend(arg_types.iter());
        out.push(ret_type);
        out.extend(variables.iter());
        if let Some(it) = instance_type {
            out.push(it);
        }
    }
    if let Type::Parameters(p) = typ {
        out.extend(p.arg_types.iter());
    }
    out
}

// ---------------------------------------------------------------------------
// method_fullname
// ---------------------------------------------------------------------------

/// Mirror of `mypy.checkexpr.ExpressionChecker.method_fullname`
/// (checkexpr.py:861-899). Resolves `method_name` to a fully qualified
/// name (`type_name.method_name`) for the type the method is invoked on.
/// Returns None (defer) when the name cannot be determined.
///
/// Mirrors the Python isinstance chain one level deep: the outer type is
/// `get_proper_type`-expanded (TypeAliasType defers) and unwrapped once
/// for CallableType type objects and TypeType, then the flat chain checks
/// Instance / TypedDictType / LiteralType / TupleType. Anything else
/// (including a CallableType or TypeType nested in a type wrapper that
/// Python doesn't reach) defers.
fn method_fullname_inner(typ: &Type, method_name: &str, resolver: &TypeResolver) -> Option<String> {
    let proper = get_proper_or_none(typ)?; // TypeAliasType defers
    let unwrapped: &Type = match proper {
        Type::CallableType {
            fallback,
            ret_type,
            instance_type,
            from_concatenate,
            ..
        } if is_type_obj(fallback, *from_concatenate) => {
            // `CallableType.is_type_obj` also rejects a callable whose
            // proper return type is UninhabitedType; deferring is safe.
            if matches!(ret_type.as_ref(), Type::UninhabitedType { .. }) {
                return None;
            }
            // `get_instance_type()`: the explicit instance type, else the
            // proper return type.
            match instance_type {
                Some(t) => t.as_ref(),
                None => ret_type.as_ref(),
            }
        }
        Type::TypeType { item, .. } => item.as_ref(),
        other => other,
    };
    match unwrapped {
        Type::Instance { type_ref, .. } => Some(format!("{type_ref}.{method_name}")),
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            // `tuple_fallback()`: named tuples return the partial fallback
            // directly; plain tuples rebuild from the fallback's type info.
            // Only the variadic-unpack path raises, so defer when it exists.
            let Type::Instance { type_ref, .. } = partial_fallback.as_ref() else {
                return None;
            };
            if type_ref == "builtins.tuple"
                && items.iter().any(|it| matches!(it, Type::UnpackType { .. }))
            {
                return None;
            }
            Some(format!("{type_ref}.{method_name}"))
        }
        Type::TypedDictType { fallback, .. } | Type::LiteralType { fallback, .. } => {
            containing_type_info(resolver, fallback, method_name)
        }
        _ => None,
    }
}

/// `TypeInfo.get_containing_type_info` (nodes.py:3953) for the wire
/// format: walk the fallback's MRO and return the first class whose
/// `names` table defines `method_name`, as a fully qualified method name.
/// A missing snapshot for the fallback or any MRO base defers (None) so
/// the Python side re-runs the real TypeInfo graph.
fn containing_type_info(
    resolver: &TypeResolver,
    fallback: &Type,
    method_name: &str,
) -> Option<String> {
    let Type::Instance { type_ref, .. } = fallback else {
        return None;
    };
    let start = resolver.get(type_ref)?;
    for base in &start.mro {
        let snap = resolver.get(base)?;
        if snap.member_info.contains_key(method_name) {
            return Some(format!("{}.{}", snap.fullname, method_name));
        }
    }
    None
}

/// `mypy.checkexpr.ExpressionChecker.method_fullname` (M25). The caller
/// serializes the (already proper) object type and the method name; the
/// kernel resolves the qualified name against the shared type-info
/// snapshot. Deferral (None) is the strangler-fig escape hatch: the
/// Python side recomputes via the real TypeInfo graph.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_method_fullname(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    method_name: &str,
) -> PyResult<Option<String>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(method_fullname_inner(
        &typ,
        method_name,
        resolver.resolver(),
    ))
}

// ---------------------------------------------------------------------------
// visit_star_expr — star expression type echo
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_star_expr` (M8c).
///
/// The Python implementation is simply `return self.accept(e.expr)`,
/// an identity pass-through.  The Rust kernel here accepts a single
/// serialized inner type and returns it verbatim.  This lets the
/// Python side skip the sub-expression traversal when the checker
/// has already computed the inner type.
///
/// Defer (return None) on any input shape we do not handle — the
/// strangler-fig escape hatch.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_star_expr(type_bytes: &[u8]) -> PyResult<Option<Vec<u8>>> {
    // We must round-trip through decode+encode so that an
    // unsupported shape (e.g. TypeAliasType) produces None.
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(encode_type(&typ))
}

// ---------------------------------------------------------------------------
// visit_conditional_expr — ternary join of branch types
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_conditional_expr` (M8c)
/// result-type computation.
///
/// The Python visitor computes `if_type` and `else_type` from the two
/// branches, then computes `make_simplified_union([if_type, else_type])`
/// as the result (line 6467).  In edge cases where that union contains an
/// uninhabited component it falls back to `join_types(if_type, else_type)`
/// (line 6472).
///
/// This Rust function receives both branch types serialized and returns
/// the join.  The caller (Python) still handles `accept` of the sub-expressions
/// and error reporting; this function is a pure type-derivation helper.
///
/// Defer (return None) when any input shape we cannot identify.
///
/// Takes a `NativeTypeResolver` because the full join logic uses
/// `subtypes::is_subtype` which needs the type-info snapshot for
/// Instance-Instance subtype checks.  Without a resolver we fall back
/// to Python, matching the Python gate pattern.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_conditional_expr_join(
    if_bytes: &[u8],
    else_bytes: &[u8],
    resolver: &NativeTypeResolver,
) -> PyResult<Option<Vec<u8>>> {
    let if_type = match decode_type(if_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let else_type = match decode_type(else_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };

    Ok(conditional_join_inner(
        &if_type,
        &else_type,
        resolver.resolver(),
    ))
}

/// Compute the join of two types for the conditional expression.
/// Mirrors `join.join_types` (the full implementation), which delegates
/// to `visit_<type>` on each concrete type variant.
///
/// This uses the resolver's type-info map for Instance-Instance subtype
/// and nominal join.  Returns `None` when the resolver doesn't cover a
/// needed type, so the Python side falls through to the real join.
fn conditional_join_inner(
    if_type: &Type,
    else_type: &Type,
    resolver: &TypeResolver,
) -> Option<Vec<u8>> {
    // Python's join calls get_proper_type first, expanding aliases. The
    // wire TypeAliasType has no resolved target, so a join touching one
    // would fabricate a wrong answer; defer to Python.
    if matches!(
        if_type,
        Type::TypeAliasType { .. } | Type::UnpackType { .. }
    ) || matches!(
        else_type,
        Type::TypeAliasType { .. } | Type::UnpackType { .. }
    ) {
        return None;
    }
    use crate::subtypes::SubtypeContext;

    // Build the subtype context: strict_optional = true (safe default).
    let ctx = SubtypeContext::new(false, false, false, false, false, true);

    // join.py:651: `join(Any, t) = Any` — Python returns the s operand
    // itself (preserving its type_of_any / source). Without this the
    // subtype kernel's `Any <: X = True` fast path returns the other side.
    if matches!(if_type, Type::AnyType { .. }) {
        return encode_type(if_type);
    }

    // trivial_join handles s <: t -> t, t <: s -> s, Instance-right -> object.
    match crate::setops::trivial_join(if_type, else_type, &ctx, resolver) {
        Some(crate::setops::SetOpResult::SameS) => encode_type(if_type),
        Some(crate::setops::SetOpResult::SameT) => encode_type(else_type),
        Some(crate::setops::SetOpResult::Object) => {
            // Return `object` instance as the join.
            encode_type(&Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            })
        }
        Some(crate::setops::SetOpResult::Bottom) => {
            // Bottom in a join usually means a type error;
            // fall back to a union of both branches.
            encode_type(&Type::UnionType {
                items: vec![if_type.clone(), else_type.clone()],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            })
        }
        Some(crate::setops::SetOpResult::Any) => {
            // Any is the universal supertype: the join shim's disc-4
            // arm is AnyType(TypeOfAny.special_form) (join.py:547).
            encode_type(&Type::AnyType {
                type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
                source_any: None,
                missing_import_name: None,
            })
        }
        Some(crate::setops::SetOpResult::Ancestor(fullname)) => {
            // Nominal join found a common ancestor.
            encode_type(&Type::Instance {
                type_ref: fullname,
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            })
        }
        Some(crate::setops::SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        }) => {
            // Same-type Instance-Instance join with per-arg discriminators.
            // Per-arg reconstruction: disc 0 -> if_type.args[i], disc 1
            // -> else_type.args[i] (mirrors join.py:425-441). The

            // operands must be Instances to index args; defer otherwise.
            let (
                Type::Instance { args: if_args, .. },
                Type::Instance {
                    args: else_args, ..
                },
            ) = (if_type, else_type)
            else {
                return None;
            };
            if arg_discs.len() != if_args.len() || arg_discs.len() != else_args.len() {
                return None;
            }
            let final_args: Option<Vec<Type>> = arg_discs
                .iter()
                .enumerate()
                .map(|(i, &d)| match d {
                    0 => Some(if_args[i].clone()),
                    1 => Some(else_args[i].clone()),
                    // Disc 4: joined arg is Any (from_another_any, Any
                    // side); other discs defer. See `joined_arg_any`.
                    4 => Some(joined_arg_any(&if_args[i], &else_args[i])),
                    _ => None,
                })
                .collect();
            let final_args = final_args?;
            encode_type(&Type::Instance {
                type_ref,
                args: final_args,
                last_known_value: None,
                extra_attrs: None,
            })
        }
        Some(crate::setops::SetOpResult::Encoded(bytes)) => Some(bytes),
        None => {
            // trivial_join returned None for an unsupported shape;
            // fall back to a union of both branches (Python's default).
            encode_type(&Type::UnionType {
                items: vec![if_type.clone(), else_type.clone()],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Container literal fast paths (issue #385)
// ---------------------------------------------------------------------------

/// `rust_container_type`: join + node construction for list/set/dict literal types.
///
/// Receives a tag ("list", "set", "dict") + already-serialized item types.
/// Returns the container node serialized, or None to defer.
///
/// For list/set: `elements` is a flat list of serialized item types.
/// For dict: `elements` is a flat list where the first `n_keys` elements
/// are keys and the rest are values (Python passes the true deduped key
/// count; key/value lists can differ in length after dedup).
#[pyfunction]
#[pyo3(signature = (resolver, tag, elements, _ctx, n_keys))]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_container_type<'py>(
    py: Python<'py>,
    resolver: &NativeTypeResolver,
    tag: &str,
    elements: Vec<Vec<u8>>,
    _ctx: Option<Vec<u8>>,
    n_keys: i64,
) -> PyResult<Option<&'py PyBytes>> {
    let original_len = elements.len();
    // Decode all items; if any fail, treat as unsupported → defer.
    let items: Vec<Type> = elements
        .into_iter()
        .filter_map(|b| decode_type(&b))
        .collect();
    if items.len() != original_len {
        return Ok(None); // decode failure → defer to Python
    }

    match tag {
        "list" | "set" => {
            let container_fullname = match tag {
                "list" => "builtins.list",
                "set" => "builtins.set",
                _ => unreachable!(),
            };

            let vt = first_or_join_fast_item_inner(&items, resolver);
            match vt {
                None => Ok(None),
                Some(vt) => {
                    let t = Type::Instance {
                        type_ref: container_fullname.to_string(),
                        // Python's `named_generic_type` strips LKV from
                        // container args: `set(Literal['x']?)` is
                        // `set[str]`, so mirror that strip here.
                        args: vec![strip_lkv(&vt)],
                        last_known_value: None,
                        extra_attrs: None,
                    };
                    match encode_type(&t) {
                        Some(b) => Ok(Some(PyBytes::new(py, &b))),
                        None => Ok(None),
                    }
                }
            }
        }
        "dict" => match build_dict_type(resolver, &items, n_keys) {
            None => Ok(None),
            Some(bytes) => Ok(Some(PyBytes::new(py, &bytes))),
        },
        _ => Ok(None),
    }
}

/// Strip `last_known_value` from a wire `Type`, mirroring
/// Python's `remove_instance_last_known_values`.
fn strip_lkv(t: &Type) -> Type {
    match t {
        Type::Instance {
            type_ref,
            args,
            last_known_value: Some(_),
            extra_attrs,
        } => Type::Instance {
            type_ref: type_ref.clone(),
            args: args.clone(),
            last_known_value: None,
            extra_attrs: extra_attrs.clone(),
        },
        _ => t.clone(),
    }
}

/// Build a dict type from a flat list of key/value types.
/// The first `n_keys` elements are the deduplicated keys, the rest are
/// values. Python (`fast_dict_type`) passes the true key count, not half
/// the element count: dedup can make key/value lists unequal lengths, and
/// splitting at `n/2` mis-feeds keys into the values list (issue #385).
fn build_dict_type(
    resolver: &NativeTypeResolver,
    elements: &[Type],
    n_keys: i64,
) -> Option<Vec<u8>> {
    if elements.is_empty() {
        return None;
    }
    if n_keys < 0 || n_keys as usize > elements.len() || n_keys >= elements.len() as i64 {
        // Must have at least one key and one value.
        return None;
    }
    let keys: Vec<Type> = elements[..n_keys as usize].to_vec();
    let values: Vec<Type> = elements[n_keys as usize..].to_vec();
    if keys.is_empty() || values.is_empty() {
        return None;
    }

    let kt = first_or_join_fast_item_inner(&keys, resolver)?;
    let vt = first_or_join_fast_item_inner(&values, resolver)?;

    encode_type(&Type::Instance {
        type_ref: "builtins.dict".to_string(),
        // Python's `named_generic_type` strips LKV from dict args too;
        // mirror that on both key and value.
        args: vec![strip_lkv(&kt), strip_lkv(&vt)],
        last_known_value: None,
        extra_attrs: None,
    })
}

/// Join a list of types, mirroring `join.join_type_list`.
fn join_type_list_inner(items: &[Type], resolver: &NativeTypeResolver) -> Option<Type> {
    if items.is_empty() {
        return None;
    }
    if items.len() == 1 {
        return Some(items[0].clone());
    }
    let ctx = crate::subtypes::SubtypeContext::new(false, false, false, false, false, true);
    let mut result = items[0].clone();
    for item in &items[1..] {
        result = join_one_pair(&result, item, &ctx, resolver)?;
    }
    Some(result)
}

/// Join one pair of types, mirroring `join.join_type_list` for a
/// two-item list. The fast path is the same Instance-fold that
/// `join_type_list_inner` was already doing; the added prejoin handles
/// the args-less Instance-Instance nominal case exactly like
/// `visit_instance_join` (setops.rs), so a list like
/// `[int, object]` joins to `object` instead of deferring on the
/// non-subtype pair.
fn join_one_pair(
    left: &Type,
    right: &Type,
    ctx: &crate::subtypes::SubtypeContext,
    resolver: &NativeTypeResolver,
) -> Option<Type> {
    // Instance-Instance args-less nominal prejoin (mirrors join_types via
    // visit_instance_join: same-type, subtype -> supertype, else common
    // ancestor). Needs the resolver for the MRO/bases walk.
    if let (
        Type::Instance {
            type_ref: l_ref,
            args: l_args,
            last_known_value: l_lkv,
            ..
        },
        Type::Instance {
            type_ref: r_ref,
            args: r_args,
            last_known_value: r_lkv,
            ..
        },
    ) = (left, right)
    {
        if l_args.is_empty()
            && r_args.is_empty()
            && l_lkv.is_none()
            && r_lkv.is_none()
            && resolver.resolver().get(l_ref).is_some()
            && resolver.resolver().get(r_ref).is_some()
        {
            if l_ref == r_ref {
                return Some(left.clone());
            }
            // Decide via the shared nominal join, mapped back to the
            // concrete Instance node (Python's join_types never returns
            // a bare SetOpResult).
            let result = crate::setops::visit_instance_join(left, right, ctx, resolver.resolver())?;
            return instance_join_result_to_type(&result, left, right);
        }
    }
    let joined = crate::setops::join_types(left, right, ctx, resolver.resolver());
    match joined {
        Some(crate::setops::SetOpResult::SameS) => Some(left.clone()),
        Some(crate::setops::SetOpResult::SameT) => Some(right.clone()),
        Some(crate::setops::SetOpResult::Object) => Some(Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }),
        Some(crate::setops::SetOpResult::Bottom) => Some(Type::UnionType {
            items: vec![left.clone(), right.clone()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }),
        Some(crate::setops::SetOpResult::Any) => Some(Type::AnyType {
            type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
            source_any: None,
            missing_import_name: None,
        }),
        Some(crate::setops::SetOpResult::Ancestor(fullname)) => Some(Type::Instance {
            type_ref: fullname,
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }),
        Some(crate::setops::SetOpResult::SameTypeWithArgs {
            type_ref,
            arg_discs,
        }) => {
            let (Type::Instance { args: l_args, .. }, Type::Instance { args: r_args, .. }) =
                (left, right)
            else {
                return None;
            };
            if arg_discs.len() != l_args.len() || arg_discs.len() != r_args.len() {
                return None;
            }
            let final_args: Option<Vec<Type>> = arg_discs
                .iter()
                .enumerate()
                .map(|(i, &d)| match d {
                    0 => Some(l_args[i].clone()),
                    1 => Some(r_args[i].clone()),
                    // Disc 4: joined arg is Any (from_another_any, Any
                    // side); other discs defer. See `joined_arg_any`.
                    4 => Some(joined_arg_any(&l_args[i], &r_args[i])),
                    _ => None,
                })
                .collect();
            let final_args = final_args?;
            Some(Type::Instance {
                type_ref,
                args: final_args,
                last_known_value: None,
                extra_attrs: None,
            })
        }
        Some(crate::setops::SetOpResult::Encoded(bytes)) => decode_type(&bytes),
        None => None,
    }
}

/// Map a `visit_instance_join` result to the concrete joined `Type`.
/// The join of two args-less `Instance`s never builds a fresh type in
/// the Python source: `SameS`/`SameT` are the operands, `Ancestor`/
/// `Object` are built as `Instance(fullname)` / `object`, exactly what
/// `visit_instance_join` produces for the args-less case.
fn instance_join_result_to_type(
    result: &crate::setops::SetOpResult,
    left: &Type,
    right: &Type,
) -> Option<Type> {
    match result {
        crate::setops::SetOpResult::SameS => Some(left.clone()),
        crate::setops::SetOpResult::SameT => Some(right.clone()),
        crate::setops::SetOpResult::Ancestor(fullname) => Some(Type::Instance {
            type_ref: fullname.clone(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }),
        crate::setops::SetOpResult::Object => Some(Type::Instance {
            type_ref: "builtins.object".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }),
        // Other results (SameTypeWithArgs, Encoded, Bottom, Any) cannot
        // be produced for the args-less no-LKV case; defer if seen.
        _ => None,
    }
}

/// `_first_or_join_fast_item` inner: mirroring the Python version.
fn first_or_join_fast_item_inner(items: &[Type], resolver: &NativeTypeResolver) -> Option<Type> {
    if items.len() == 1 {
        if is_type_obj_callable(&items[0], resolver.resolver()) {
            return None;
        }
        return Some(items[0].clone());
    }
    let typ = join_type_list_inner(items, resolver);
    let joined = typ?;
    if items
        .iter()
        .any(|item| is_type_obj_callable(item, resolver.resolver()))
    {
        return None;
    }
    match allow_fast_container_literal_inner(&joined, resolver.alias_resolver()) {
        Some(true) => Some(joined),
        _ => None,
    }
}

/// `rust_tuple_context_matches`: pure function matching `tuple_context_matches`.
///
/// `elements_tags`: sequence of ints; 0 for non-star items, 1 for star items
/// (there may be several). The kernel counts stars and derives the first
/// star's positional index from these tags.
///
/// `ctx_bytes`: serialized TupleType context.
///
/// Returns Some(true) if the context matches, Some(false) if it doesn't,
/// or None for unsupported shapes (TypeAliasType context).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_tuple_context_matches(
    elements_tags: Vec<i64>,
    ctx_bytes: Vec<u8>,
) -> PyResult<Option<bool>> {
    let ctx = match decode_type(&ctx_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };

    Ok(tuple_context_matches_inner(&elements_tags, &ctx))
}

fn tuple_context_matches_inner(elements_tags: &[i64], ctx: &Type) -> Option<bool> {
    let ctx = match ctx {
        Type::TupleType { items, .. } => items,
        Type::TypeAliasType { .. } => return None,
        _ => return Some(false), // Not a TupleType context
    };

    let has_unpack_in_ctx = find_unpack_in_list_inner(ctx);
    let non_star_count = elements_tags.iter().filter(|&&t| t == 0).count();

    if has_unpack_in_ctx.is_none() {
        // Fixed tuple context: accept if non-star count <= len(ctx.items)
        Some(non_star_count <= ctx.len())
    } else {
        // Variadic context: need exactly one star and exact structure match
        let star_count = elements_tags.iter().filter(|&&t| t == 1).count();
        if star_count != 1 {
            return Some(false);
        }
        let total = elements_tags.len();
        // ctx_unpack_index == expr_star_index (position of the single star)
        let expr_star_index = elements_tags.iter().position(|&t| t == 1).unwrap();
        Some(total == ctx.len() && find_unpack_in_list_inner(ctx) == Some(expr_star_index))
    }
}

/// Find UnpackType in a list, mirroring `find_unpack_in_list`.
fn find_unpack_in_list_inner(items: &[Type]) -> Option<usize> {
    items
        .iter()
        .position(|it| matches!(it, Type::UnpackType { .. }))
}

/// `rust_build_tuple_type`: build the final TupleType node.
///
/// `items_bytes`: serialized list of element types (the already-computed
/// items from `self.accept` calls).
///
/// `seen_unpack`: whether an unpack was encountered (True/False as i64).
///
/// Returns the serialized TupleType, or None for unsupported shapes.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_build_tuple_type<'py>(
    py: Python<'py>,
    items_bytes: Vec<Vec<u8>>,
    seen_unpack: i64,
) -> PyResult<Option<&'py PyBytes>> {
    let original_len = items_bytes.len();
    // Decode all items; if any fail, treat as unsupported → defer.
    let items: Vec<Type> = items_bytes
        .into_iter()
        .filter_map(|b| decode_type(&b))
        .collect();
    if items.len() != original_len {
        return Ok(None); // decode failure → defer to Python
    }

    let fallback_item = Type::AnyType {
        type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
        source_any: None,
        missing_import_name: None,
    };

    let fallback = Type::Instance {
        type_ref: "builtins.tuple".to_string(),
        args: vec![fallback_item],
        last_known_value: None,
        extra_attrs: None,
    };

    let result = Type::TupleType {
        partial_fallback: Box::new(fallback),
        items,
        implicit: false,
    };

    if seen_unpack == 1 {
        // Python: `result = expand_type(result, {})` (checkexpr.py:6033-6035).
        // Identity except a single-item `Tuple[*tuple[X, ...]]` unwraps to the
        // `tuple[X, ...]` Instance itself; lone alias/non-tuple unpack defers.
        let updated = match unpack_expand_updated(&result) {
            Some(u) => u,
            None => return Ok(None), // lone non-tuple/recursive-alias unpack: defer
        };
        let bytes = encode_type(&updated).unwrap_or_default();
        return Ok(Some(PyBytes::new(py, &bytes)));
    }

    let bytes = encode_type(&result).unwrap_or_default();
    Ok(Some(PyBytes::new(py, &bytes)))
}

/// `expand_type(result, {})` for a tuple built by `build_tuple_type`.
/// Mirrors `TypeExpander.visit_tuple_type` (expandtype.py:1004-1033)
/// with an empty substitution map, which makes the walk effectively the
/// identity. The only non-identity step is the single-item
/// normalization at expandtype.py:1009-1033: a lone `UnpackType` whose
/// un-packed type is a `builtins.tuple` Instance unwraps to that
/// Instance (a `TypeVarTuple` star or `Any`/`Uninhabited` star does NOT,
/// since `get_proper_type` on them is not an Instance). Returns `None`
/// (defer) when the lone unpack needs live TypeInfo to decide.
fn unpack_expand_updated(result: &Type) -> Option<Type> {
    let Type::TupleType {
        partial_fallback,
        items,
        implicit,
    } = result
    else {
        return None;
    };
    if items.len() == 1 {
        let item = &items[0];
        if let Type::UnpackType { typ: inner } = item {
            match inner.as_ref() {
                Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                    // Normalize Tuple[*tuple[X, ...]] -> tuple[X, ...].
                    return Some((**inner).clone());
                }
                Type::Instance { .. } => return None, // non-tuple unpack: defer
                Type::TypeAliasType { .. } => return None, // recursive check needs info
                _ => {}
            }
        }
    }
    // Everything else is the identity: same items, same fallback.
    Some(Type::TupleType {
        partial_fallback: partial_fallback.clone(),
        items: items.clone(),
        implicit: *implicit,
    })
}

/// Compute the combined type context for the right operand of a boolean
/// `and`/`or` expression. Mirrors `ExpressionChecker._combined_context`.
///
/// The caller (Python) serializes the branch-derived type (`ty`, the
/// left operand's type after narrowing) and the outer type context
/// (`self.type_context[-1]`). Either may be absent.
///
/// Semantics match the Python original:
/// * branch-derived type containing Any is contagious: return it
///   verbatim (a union would lose the Any). TypeAliasType input defers
///   (wire cannot serialize aliases); Python's `has_any_type` get_proper
///   expansion then applies.
/// * otherwise the combined context is `make_simplified_union` of the
///   present items (branch type and/or outer context).
/// * no items at all -> None (Python returns None too; serializing
///   nothing costs nothing).
///
/// Returns None on any untranslatable shape; the caller falls back to
/// the pure-Python implementation.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_analyze_cond_branch(
    resolver: &NativeTypeResolver,
    branch_bytes: Option<Vec<u8>>,
    known_bytes: Option<Vec<u8>>,
) -> PyResult<Option<Vec<u8>>> {
    let branch = match branch_bytes {
        Some(bytes) => match decode_type(&bytes) {
            Some(t) => Some(t),
            None => return Ok(None),
        },
        None => None,
    };
    let known = match known_bytes {
        Some(bytes) => match decode_type(&bytes) {
            Some(t) => Some(t),
            None => return Ok(None),
        },
        None => None,
    };
    Ok(combined_context_inner(
        branch.as_ref(),
        known.as_ref(),
        resolver.resolver(),
        resolver.alias_resolver(),
    ))
}

fn combined_context_inner(
    branch: Option<&Type>,
    known: Option<&Type>,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Vec<u8>> {
    use crate::subtypes::SubtypeContext;

    // Any is contagious: `dict[str, Any] or <x>` should still infer Any
    // in `x`, so return the branch type directly (no union with it).
    if let Some(b) = branch {
        match has_any_type_inner(b, false, aliases) {
            Some(true) => return encode_type(b),
            Some(false) => {}
            None => return None, // TypeAliasType: defer, Python expands.
        }
    }
    let mut items = Vec::with_capacity(2);
    if let Some(b) = branch {
        items.push(b.clone());
    }
    if let Some(k) = known {
        items.push(k.clone());
    }
    if items.is_empty() {
        return None;
    }
    // proper_subtype=True preserves Any/alias items (Python's
    // _remove_redundant_union_items uses is_proper_subtype). Nested
    // union items flatten (step 1), single items fast-path (step 2),

    // aliases defer (flatten rejects them) -> Python get_proper_type.
    let ctx = SubtypeContext::new(false, false, false, true, true, true);
    let result = crate::setops::make_simplified_union(&items, &ctx, resolver, true, false)?;
    encode_type(&result)
}

// ---------------------------------------------------------------------------
// visit_temp_node — identity pass (e.type)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_temp_node` (Issue #458).
///
/// The Python implementation is a single line: `return e.type`.
/// Pure identity on the wire-format type blob.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_visit_temp_node<'py>(
    py: Python<'py>,
    type_bytes: &[u8],
) -> PyResult<Option<&'py PyBytes>> {
    // Decode to verify the shape is something we can round-trip.
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(encode_type(&typ).map(|b| PyBytes::new(py, &b)))
}

// ---------------------------------------------------------------------------
// visit__promote_expr — identity pass (e.type)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit__promote_expr` (Issue #458).
///
/// The Python implementation is a single line: `return e.type`.
/// Pure identity on the wire-format type blob.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_visit_promote_expr<'py>(
    py: Python<'py>,
    type_bytes: &[u8],
) -> PyResult<Option<&'py PyBytes>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(encode_type(&typ).map(|b| PyBytes::new(py, &b)))
}

// ---------------------------------------------------------------------------
// visit_paramspec_expr — constant AnyType(TypeOfAny.special_form)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_paramspec_expr` (Issue #458).
///
/// Returns `AnyType(TypeOfAny.special_form)` unconditionally.
/// The Python body is a single `return` with no branches or side effects.
#[pyfunction]
pub(crate) fn rust_visit_paramspec_expr<'py>(py: Python<'py>) -> PyResult<&'py PyBytes> {
    let t = any_type(TYPE_OF_ANY_SPECIAL_FORM, None);
    let bytes = encode_type(&t).unwrap_or_default();
    Ok(PyBytes::new(py, &bytes))
}

// ---------------------------------------------------------------------------
// visit_type_var_tuple_expr — constant AnyType(TypeOfAny.special_form)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_type_var_tuple_expr` (Issue #458).
///
/// Returns `AnyType(TypeOfAny.special_form)` unconditionally.
#[pyfunction]
pub(crate) fn rust_visit_type_var_tuple_expr<'py>(py: Python<'py>) -> PyResult<&'py PyBytes> {
    let t = any_type(TYPE_OF_ANY_SPECIAL_FORM, None);
    let bytes = encode_type(&t).unwrap_or_default();
    Ok(PyBytes::new(py, &bytes))
}

// ---------------------------------------------------------------------------
// visit_newtype_expr — constant AnyType(TypeOfAny.special_form)
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.ExpressionChecker.visit_newtype_expr` (Issue #458).
///
/// Returns `AnyType(TypeOfAny.special_form)` unconditionally.
#[pyfunction]
pub(crate) fn rust_visit_newtype_expr<'py>(py: Python<'py>) -> PyResult<&'py PyBytes> {
    let t = any_type(TYPE_OF_ANY_SPECIAL_FORM, None);
    let bytes = encode_type(&t).unwrap_or_default();
    Ok(PyBytes::new(py, &bytes))
}

// ---------------------------------------------------------------------------
// Helpers for constant AnyType construction
// ---------------------------------------------------------------------------

fn any_type(type_of_any: i64, source_any: Option<Box<Type>>) -> Type {
    Type::AnyType {
        type_of_any,
        source_any,
        missing_import_name: None,
    }
}

// ---------------------------------------------------------------------------
// Issue #486: tuple-index / tuple-slice helpers (visit_tuple_index_helper,
// visit_tuple_slice_helper, try_getting_int_literals)

// ---------------------------------------------------------------------------

/// `mypy.checkexpr.try_getting_int_literals` — extract int literal values
/// from a serialized type blob.
///
/// Mirrors the type-level part of `try_getting_int_literals`
/// (checkexpr.py:5799-5835): given a `Type` that is `Literal[int]` or a
/// `UnionType` of `Literal[int]`, returns the list of `int` values.
/// Returns `None` (defer) for non-matching types.
///
/// The Python side resolves AST expressions (`IntExpr`, `UnaryExpr`) to
/// types before calling this; Rust operates purely on the wire format.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_try_getting_int_literals(type_bytes: &[u8]) -> PyResult<Option<Vec<i64>>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(try_getting_int_literals_inner(&typ))
}

fn try_getting_int_literals_inner(typ: &Type) -> Option<Vec<i64>> {
    match typ {
        Type::LiteralType {
            value: LiteralValue::Int(n),
            ..
        } => Some(vec![*n]),
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => try_getting_int_literals_inner(lkv),
        Type::UnionType { items, .. } => {
            let mut out = Vec::new();
            for item in items {
                out.extend(try_getting_int_literals_inner(item)?);
            }
            Some(out)
        }
        _ => None,
    }
}

/// `mypy.checkexpr.visit_tuple_index_helper` — resolve `tuple[n]` to the
/// element type at position `n`.
///
/// Mirrors `visit_tuple_index_helper` (checkexpr.py:5719-5765). Returns
/// `None` when:
///   * The index is out of range for a fixed tuple.
///   * The index exceeds the variadic length bound.
///   * Any type class in the tuple items is unsupported (TypeAliasType).
///
/// `items_bytes`: serialized `TupleType.items` (flattened list).
/// `partial_fallback_bytes`: serialized `TupleType.partial_fallback`.
/// `n`: the integer index (may be negative).
/// `line`, `column`: line/column for UnionType construction (passed through).
/// `min_length`: pre-computed `min_tuple_length` to avoid recomputation.
///
/// Returns `Some(encoded_result_type)` or `None` to defer.
#[pyfunction]
#[pyo3(signature = (items_bytes, partial_fallback_bytes, n, line, column, min_length))]
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
pub(crate) fn rust_visit_tuple_index_helper(
    items_bytes: Vec<Vec<u8>>,
    partial_fallback_bytes: Vec<u8>,
    n: i64,
    line: i64,
    column: i64,
    min_length: i64,
) -> PyResult<Option<Vec<u8>>> {
    let original_len = items_bytes.len();
    let items: Vec<Type> = items_bytes
        .into_iter()
        .filter_map(|b| decode_type(&b))
        .collect();
    if items.len() != original_len {
        return Ok(None);
    }
    let partial_fallback = match decode_type(&partial_fallback_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };

    Ok(visit_tuple_index_helper_inner(
        &items,
        &partial_fallback,
        n,
        line,
        column,
        min_length,
    ))
}

fn visit_tuple_index_helper_inner(
    items: &[Type],
    _partial_fallback: &Type,
    n: i64,
    _line: i64,
    _column: i64,
    min_length: i64,
) -> Option<Vec<u8>> {
    let unpack_index = find_unpack_in_list_inner(items);
    if unpack_index.is_none() {
        // Fixed tuple path
        let mut idx = n;
        if idx < 0 {
            idx += items.len() as i64;
        }
        if 0 <= idx && idx < items.len() as i64 {
            return encode_type(&items[idx as usize]);
        }
        return None;
    }

    let unpack_index = unpack_index.unwrap();
    let unpack = &items[unpack_index];
    let Type::UnpackType { typ } = unpack else {
        return None;
    };
    let unpacked = get_proper_or_none(typ.as_ref())?;

    // Extract `middle` from the unpacked type.
    let middle = match unpacked {
        Type::TypeVarTupleType { upper_bound, .. } => {
            let bound = get_proper_or_none(upper_bound)?;
            match bound {
                Type::Instance { type_ref, args, .. }
                    if type_ref == "builtins.tuple" && !args.is_empty() =>
                {
                    &args[0]
                }
                _ => return None,
            }
        }
        Type::Instance { type_ref, args, .. }
            if type_ref == "builtins.tuple" && !args.is_empty() =>
        {
            &args[0]
        }
        _ => return None,
    };

    let extra_items = min_length - items.len() as i64 + 1;
    if n >= 0 {
        if n >= min_length {
            return None;
        }
        if n < unpack_index as i64 {
            return encode_type(&items[n as usize]);
        }
        // UnionType: [middle] + items[unpack_index+1 .. max(n-extra_items+2, unpack_index+1)]
        let end = std::cmp::max(n - extra_items + 2, unpack_index as i64 + 1);
        let mut union_items = vec![middle.clone()];
        for idx in (unpack_index + 1)..end as usize {
            if idx < items.len() {
                union_items.push(items[idx].clone());
            }
        }
        let result = Type::UnionType {
            items: union_items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        encode_type(&result)
    } else {
        let adjusted = n + min_length;
        if adjusted < 0 {
            return None;
        }
        if adjusted >= unpack_index as i64 + extra_items {
            let real_idx = (adjusted - extra_items + 1) as usize;
            if real_idx < items.len() {
                return encode_type(&items[real_idx]);
            }
            return None;
        }
        let end_idx = std::cmp::min(adjusted as usize, unpack_index);
        let mut prefix: Vec<Type> = items[..end_idx].to_vec();
        prefix.push(middle.clone());
        let result = Type::UnionType {
            items: prefix,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        encode_type(&result)
    }
}

/// `mypy.checkexpr.visit_tuple_slice_helper` — resolve `tuple[begin:end:stride]`.
///
/// Mirrors `visit_tuple_slice_helper` (checkexpr.py:5767-5797). Returns a
/// `UnionType` of all slice results (via `itertools.product`).
///
/// `items_bytes`: serialized `TupleType.items`.
/// `partial_fallback_bytes`: serialized `TupleType.partial_fallback`.
/// `begin`: `Some(n)` or `None` for start.
/// `end`: `Some(n)` or `None` for stop.
/// `stride`: `Some(n)` or `None` for step.
/// `line`, `column`: for UnionType construction.
///
/// Returns `Some(encoded_result_type)` or `None` to defer.
/// `None` indicates the Python side should call `nonliteral_tuple_index_helper`
/// (this function only handles the literal-slice path).
#[pyfunction]
#[pyo3(signature = (items_bytes, partial_fallback_bytes, begin, end, stride, line, column))]
#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
pub(crate) fn rust_visit_tuple_slice_helper(
    items_bytes: Vec<Vec<u8>>,
    partial_fallback_bytes: Vec<u8>,
    begin: Option<i64>,
    end: Option<i64>,
    stride: Option<i64>,
    line: i64,
    column: i64,
) -> PyResult<Option<Vec<u8>>> {
    let original_len = items_bytes.len();
    let items: Vec<Type> = items_bytes
        .into_iter()
        .filter_map(|b| decode_type(&b))
        .collect();
    if items.len() != original_len {
        return Ok(None);
    }
    let partial_fallback = match decode_type(&partial_fallback_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };

    Ok(visit_tuple_slice_helper_inner(
        &items,
        &partial_fallback,
        begin,
        end,
        stride,
        line,
        column,
    ))
}

fn visit_tuple_slice_helper_inner(
    items: &[Type],
    partial_fallback: &Type,
    begin: Option<i64>,
    end: Option<i64>,
    stride: Option<i64>,
    _line: i64,
    _column: i64,
) -> Option<Vec<u8>> {
    let begin_vals: Vec<Option<i64>> = begin.map(|v| vec![Some(v)]).unwrap_or_else(|| vec![None]);
    let end_vals: Vec<Option<i64>> = end.map(|v| vec![Some(v)]).unwrap_or_else(|| vec![None]);
    let stride_vals: Vec<Option<i64>> = stride.map(|v| vec![Some(v)]).unwrap_or_else(|| vec![None]);

    let mut slice_results: Vec<Type> = Vec::new();
    for &b in &begin_vals {
        for &e in &end_vals {
            for &s in &stride_vals {
                let slice_type = tuple_slice(items, b, e, s, partial_fallback);
                match slice_type {
                    Some(t) => slice_results.push(t),
                    None => {
                        // AMBIGUOUS_SLICE_OF_VARIADIC_TUPLE — Python would
                        // return AnyType. We defer; the Python side handles
                        // error reporting.
                        return None;
                    }
                }
            }
        }
    }

    if slice_results.is_empty() {
        return None;
    }
    if slice_results.len() == 1 {
        return encode_type(&slice_results[0]);
    }

    // make_simplified_union of slice results.
    // We use the union constructor directly since the Python caller
    // applies make_simplified_union afterward.
    let result = Type::UnionType {
        items: slice_results,
        uses_pep604_syntax: false,
        can_be_true: true,
        can_be_false: true,
    };
    encode_type(&result)
}

/// `TupleType.slice` for the wire format: return the sliced TupleType or None
/// (for ambiguous variadic slice).
fn tuple_slice(
    items: &[Type],
    begin: Option<i64>,
    end: Option<i64>,
    stride: Option<i64>,
    partial_fallback: &Type,
) -> Option<Type> {
    let stride_val = stride.unwrap_or(1);
    if stride_val == 0 {
        return None;
    }

    if let Some(unpack_idx_u) = find_unpack_in_list_inner(items) {
        // Variadic tuple slicing — complex logic from mypy/types.py:3026-3068
        let total = items.len() as i64;
        let unpack_idx = unpack_idx_u as i64;

        let result = if begin.is_none() && end.is_none() {
            // Special-case: reversing or identity on variadic
            match (stride_val, stride.is_none()) {
                (-1, _) => {
                    let mut rev = items.to_vec();
                    rev.reverse();
                    Some(rev)
                }
                (_, true) | (1, false) => Some(items.to_vec()),
                _ => None,
            }
        } else {
            let b = begin.unwrap_or(0);
            let e = end.unwrap_or(total);
            let prefix_ok = begin.is_none() || (unpack_idx >= b && b >= 0);
            let suffix_ok = end.is_none() || (unpack_idx - total < e && e < 0);
            let start_in_suffix = begin.is_some() && unpack_idx - total < b && b < 0;
            let start_in_prefix = begin.is_none() || (unpack_idx >= b && b >= 0);
            let end_in_prefix = end.is_none() || (unpack_idx >= e && e >= 0);

            if prefix_ok && end_in_prefix {
                // Start and end in prefix
                Some(slice_items(items, begin, end, stride))
            } else if start_in_suffix && suffix_ok {
                // Start and end in suffix
                Some(slice_items(items, begin, end, stride))
            } else if start_in_prefix && suffix_ok {
                // Start in prefix, end in suffix — trivial strides only
                if stride.is_none() || stride_val == 1 {
                    Some(slice_items(items, begin, end, stride))
                } else {
                    None
                }
            } else if start_in_suffix && end_in_prefix {
                // Start in suffix, end in prefix — only -1 stride
                if stride.is_none() || stride_val == -1 {
                    Some(slice_items(items, begin, end, stride))
                } else {
                    None
                }
            } else {
                None
            }
        };

        Some(Type::TupleType {
            partial_fallback: Box::new(partial_fallback.clone()),
            items: result?,
            implicit: false,
        })
    } else {
        // Fixed tuple — simple slicing
        let sliced = slice_items(items, begin, end, stride);
        Some(Type::TupleType {
            partial_fallback: Box::new(partial_fallback.clone()),
            items: sliced,
            implicit: false,
        })
    }
}

/// Slice a type list using begin/end/stride (mirrors Python list slicing).
fn slice_items(
    items: &[Type],
    begin: Option<i64>,
    end: Option<i64>,
    stride: Option<i64>,
) -> Vec<Type> {
    let n = items.len() as i64;
    let stride = stride.unwrap_or(1);
    let b = begin.unwrap_or(if stride > 0 { 0 } else { n - 1 });
    let e = end.unwrap_or(if stride > 0 { n } else { -n - 1 });

    // Python slice semantics: indices are clamped, and negative indices
    // count from the end. Both b and e are resolved against n first.
    let clamp = |i: i64| if i < 0 { (i + n).max(0) } else { i.min(n) };
    let b = clamp(b);
    let e = clamp(e);

    if stride == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();
    if stride > 0 {
        let mut i = b;
        while i < e && i < n {
            result.push(items[i as usize].clone());
            i += stride;
        }
    } else {
        // Negative stride: start at b, step down by |stride| until past e.
        let mut i = b;
        while i > e {
            if i >= 0 && i < n {
                result.push(items[i as usize].clone());
            }
            i += stride;
        }
    }
    result
}

/// Classify `check_typeddict_call`'s dispatch tag from live Python AST
/// objects. `args` is the call argument expression list and `arg_kinds` the
/// parallel list of `ArgKind.value` ints. Returns the branch tag 0..4
/// (kwargs/dict-expr/dict-call/empty/invalid), or None to defer on a shape
/// mismatch or unexpected arg-kind value.
#[pyfunction]
pub(crate) fn rust_classify_typeddict_call(
    py: Python<'_>,
    args: &PyList,
    arg_kinds: &PyList,
) -> PyResult<Option<i64>> {
    if args.len() != arg_kinds.len() {
        return Ok(None);
    }
    let mut kinds = Vec::with_capacity(arg_kinds.len());
    for item in arg_kinds.iter() {
        match item.extract::<i64>() {
            Ok(k) => kinds.push(k),
            Err(_) => return Ok(None),
        }
    }

    // 0: kwargs — every arg is a keyword or ** unpack.
    if !args.is_empty() && kinds.iter().all(|&k| k == ARG_NAMED || k == ARG_STAR2) {
        return Ok(Some(0));
    }

    // 1: single positional DictExpr. 2: single positional dict-literal
    // CallExpr whose `.analyzed` is a DictExpr.
    if args.len() == 1 && kinds[0] == ARG_POS {
        let unique_arg = args.get_item(0)?;
        let nodes_mod = py.import("mypy.nodes")?;
        let dict_expr_cls: &PyType = nodes_mod.getattr("DictExpr")?.downcast()?;
        let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;
        if unique_arg.is_instance(dict_expr_cls)? {
            return Ok(Some(1));
        }
        if unique_arg.is_instance(call_expr_cls)? {
            let analyzed = unique_arg.getattr("analyzed")?;
            if !analyzed.is_none() && analyzed.is_instance(dict_expr_cls)? {
                return Ok(Some(2));
            }
        }
    }

    // 3: no args.
    if args.is_empty() {
        return Ok(Some(3));
    }

    // 4: anything else is invalid.
    Ok(Some(4))
}

/// `ExpressionChecker.refers_to_typeddict` (checkexpr.py:1385-1393).
/// Pure bool predicate over the live callee expression, mirrored after
/// `rust_classify_lvalue_validity`'s PyO3 `is_instance` shape. Rust
/// reads `mypy.nodes.RefExpr` / `TypeInfo` / `TypeAlias` off the live
/// `base`; the TypeAlias target is handed over as wire bytes of its
/// proper type (serialized by the Python shim) and matched against
/// `Type::TypedDictType`. Never defers: every reachable branch returns
/// a bool. A TypeAlias node without decodable target bytes raises, an
/// unreachable-by-construction case the Python shim treats as a
/// fallback to the pure-Python body.
#[pyfunction]
#[pyo3(signature = (base, target_bytes=None))]
pub(crate) fn rust_refers_to_typeddict(
    py: Python<'_>,
    base: &PyAny,
    target_bytes: Option<&[u8]>,
) -> PyResult<bool> {
    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    if !base.is_instance(ref_expr_cls)? {
        return Ok(false);
    }
    let node = base.getattr("node")?;
    if node.is_none() {
        return Ok(false);
    }
    let typeinfo_cls: &PyType = nodes_mod.getattr("TypeInfo")?.downcast()?;
    if node.is_instance(typeinfo_cls)? {
        return Ok(!node.getattr("typeddict_type")?.is_none());
    }
    let typealias_cls: &PyType = nodes_mod.getattr("TypeAlias")?.downcast()?;
    if node.is_instance(typealias_cls)? {
        let bytes = target_bytes
            .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("missing alias target"))?;
        return match decode_type(bytes) {
            Some(Type::TypedDictType { .. }) => Ok(true),
            Some(_) => Ok(false),
            None => Err(pyo3::exceptions::PyValueError::new_err(
                "undecodable alias target",
            )),
        };
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn empty_alias_lookup() -> crate::aliases::TypeAliasResolver {
        crate::aliases::TypeAliasResolver::new()
    }

    fn make_any(type_of_any: i64) -> Type {
        Type::AnyType {
            type_of_any,
            source_any: None,
            missing_import_name: None,
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

    #[test]
    fn test_has_any_type_true() {
        assert_eq!(
            has_any_type_inner(&make_any(2), false, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_any_type_special_form_false() {
        assert_eq!(
            has_any_type_inner(
                &make_any(TYPE_OF_ANY_SPECIAL_FORM),
                false,
                &empty_alias_resolver()
            ),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_in_instance() {
        let inst = make_instance("Foo", vec![make_any(2)]);
        assert_eq!(
            has_any_type_inner(&inst, false, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_any_type_false_simple() {
        assert_eq!(
            has_any_type_inner(
                &make_instance("int", vec![]),
                false,
                &empty_alias_resolver()
            ),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_alias_defers_without_snapshot() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        // No snapshot in the resolver -> expanded_alias_target defers.
        assert_eq!(
            has_any_type_inner(&alias, false, &empty_alias_resolver()),
            None
        );
    }

    #[test]
    fn test_has_any_type_alias_expands_typevar_arg_any_true() {
        // A[T] = List[T], applied as A[Any]: the substitution must
        // replace T with Any so has_any_type answers true (B3b core).

        // The snapshot's alias_tvars declares T with raw_id 7;
        // the wire TypeAliasType node carries args=[Any].
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 7,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        let tvar_type = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 7,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 0,
        };
        let target = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![tvar_type],
            last_known_value: None,
            extra_attrs: None,
        };
        let snap = crate::aliases::TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_type(&target).expect("target must encode"),
            alias_tvars: vec![tv],
            python_3_12_type_alias: true,
            ..Default::default()
        };
        resolver.insert("mod.A".to_string(), snap);
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![make_any(2)],
        };
        assert_eq!(has_any_type_inner(&alias_app, false, &resolver), Some(true));
    }

    #[test]
    fn test_has_any_type_alias_expands_typevar_arg_int_false() {
        // A[T] = List[T], applied A[int]: substitution yields List[int]
        // (no Any) and the old-style alias does not visit args, so the
        // answer is false (Python parity: visit_type_alias_type visits the

        // proper target only).
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 7,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        let tvar_type = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 7,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 0,
        };
        let target = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![tvar_type],
            last_known_value: None,
            extra_attrs: None,
        };
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&target).expect("target must encode"),
                alias_tvars: vec![tv],
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![make_instance("int", vec![])],
        };
        assert_eq!(
            has_any_type_inner(&alias_app, false, &resolver),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_alias_python312_visits_args() {
        // New-style (PEP 695) alias A[T] = Callable[[T], int] applied
        // A[Any]: the target Callable[[Any], int] contains Any, so true
        // regardless of the args-visit; use an alias whose target has NO

        // typevars so only the args can carry Any.
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 9,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        // Target is plain `int` (no T in target): dead typevar.
        let target = make_instance("int", vec![]);
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&target).expect("target must encode"),
                alias_tvars: vec![tv],
                python_3_12_type_alias: true,
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![make_any(2)],
        };
        // New-style alias: `res or query_types(args)` -> args=[Any] -> true.
        assert_eq!(has_any_type_inner(&alias_app, false, &resolver), Some(true));
    }

    #[test]
    fn test_has_any_type_alias_oldstyle_skips_args() {
        // Same dead-typevar alias but old-style (python_3_12 == false):
        // `res or args` -> only the target is visited, args are ignored,
        // so has_any_type is false even though args=[Any].
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 9,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        let target = make_instance("int", vec![]);
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&target).expect("target must encode"),
                alias_tvars: vec![tv],
                python_3_12_type_alias: false,
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![make_any(2)],
        };
        assert_eq!(
            has_any_type_inner(&alias_app, false, &resolver),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_alias_arg_count_mismatch_defers() {
        // A[T] = List[T] applied with zero args: build_alias_env sees a
        // length mismatch? No, zip truncates, leaving T unbound. Python
        // leaves the TypeVar in the target (not Any), so has_any_type is

        // false. The Rust env maps nothing (zip over empty), the TypeVar
        // survives, and Instance[List[T]] has no Any -> false. This
        // matches Python's zip-truncate semantics.
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 7,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        let tvar_type = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 7,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 0,
        };
        let target = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![tvar_type],
            last_known_value: None,
            extra_attrs: None,
        };
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&target).expect("target must encode"),
                alias_tvars: vec![tv],
                python_3_12_type_alias: false,
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(
            has_any_type_inner(&alias_app, false, &resolver),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_alias_no_args_defer() {
        // no_args alias: target returned as-is (parity-safe, production
        // strips typevars). Chain-extension with the top-level args is not
        // needed; verify a no_args alias with a plain target answers

        // correctly.
        let mut resolver = empty_alias_resolver();
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&make_instance("list", vec![])).expect("target must encode"),
                no_args: true,
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(
            has_any_type_inner(&alias_app, false, &resolver),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_alias_closure_cycle_defers() {
        // A = List[A]: expanded_alias_target must detect the cycle and
        // defer, never looping. Python's get_proper_type also cannot
        // terminate on recursive aliases, and BoolTypeQuery's

        // seen_aliases set short-circuits to default, so defer is the
        // parity-safe answer.
        let mut resolver = empty_alias_resolver();
        // Self-referential target A = List[A]: build via hand-crafted
        // wire bytes (write_type refuses TypeAliasType).
        let mut wbuf = WriteBuffer::new();
        crate::wire::write_tag(&mut wbuf, crate::wire::TYPE_ALIAS_TYPE);
        crate::wire::write_type_list(&mut wbuf, &[]).expect("empty args encode");
        crate::wire::write_str(&mut wbuf, "mod.A").expect("ref encodes");
        crate::wire::write_tag(&mut wbuf, crate::wire::END_TAG);
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: wbuf.into_bytes(),
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![],
        };
        assert_eq!(has_any_type_inner(&alias_app, false, &resolver), None);
    }

    #[test]
    fn test_has_any_type_alias_typevar_never_bound_defers() {
        // A[T] = List[T] applied A[int] where the env's TypeVar key does
        // not match (raw_id 7 vs declared 8): expand_type_inner leaves T
        // unbound -> the target keeps a TypeVar node. has_any_type only

        // traverses values/upper_bound/default of TypeVar (never the
        // bare node), so it answers false, matching Python (the T is
        // non-Any). This documents the non-deferring substitution path.
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 8,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        let tvar_type = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 7,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 0,
        };
        let target = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![tvar_type],
            last_known_value: None,
            extra_attrs: None,
        };
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&target).expect("target must encode"),
                alias_tvars: vec![tv],
                python_3_12_type_alias: false,
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![make_instance("int", vec![])],
        };
        assert_eq!(
            has_any_type_inner(&alias_app, false, &resolver),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_alias_union_typevar_target() {
        // A[T] = Union[T, str], A[Any]: substitution must fire inside a
        // union item. Rust's expand_type_inner returns a simplified
        // UnionType; has_any_type then finds the Any -> true.
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 7,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        let tvar_type = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 7,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 0,
        };
        let union = Type::UnionType {
            items: vec![tvar_type, make_instance("str", vec![])],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&union).expect("target must encode"),
                alias_tvars: vec![tv],
                python_3_12_type_alias: false,
                ..Default::default()
            },
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![make_any(2)],
        };
        assert_eq!(has_any_type_inner(&alias_app, false, &resolver), Some(true));
    }

    #[test]
    fn test_has_any_type_alias_chain() {
        // B = A, A = List[Any]: the chain loop must follow B -> A and
        // answer true (no typevars involved, so no args needed).
        let mut resolver = empty_alias_resolver();
        resolver.insert(
            "mod.A".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.A".to_string(),
                target: encode_type(&make_instance("list", vec![make_any(2)]))
                    .expect("target must encode"),
                ..Default::default()
            },
        );
        insert_chain_edges(
            &mut resolver,
            alias_resolver_with_alias_targets(&[("mod.B", "mod.A".to_string())]),
        );
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.B".to_string(),
            args: vec![],
        };
        // B's target is a TypeAliasType to A with no args; the chain
        // loop rewrites current = target until a non-alias target.
        assert_eq!(has_any_type_inner(&alias_app, false, &resolver), Some(true));
    }

    #[test]
    fn test_has_any_type_typevar_no_default_false() {
        // `T = TypeVar('T')`: default is the from_omitted_generics
        // sentinel (type_of_any=4), which is NOT a real Any. Python's
        // HasAnyType.visit_type_var only visits the default when

        // has_default() is true. The sentinel must be skipped (B3b
        // regression: it was treated as a real Any -> spurious true).
        for default in [
            make_any(TYPE_OF_ANY_FROM_OMITTED_GENERICS),
            Type::UninhabitedType { ambiguous: false },
        ] {
            let tv = Type::TypeVarType {
                name: "T".to_string(),
                fullname: "mod.T".to_string(),
                raw_id: 1,
                namespace: Default::default(),
                values: vec![],
                upper_bound: Box::new(make_instance("object", vec![])),
                default: Box::new(default),
                variance: 0,
                meta_level: 0,
            };
            assert_eq!(
                has_any_type_inner(&tv, false, &empty_alias_resolver()),
                Some(false)
            );
        }
    }

    #[test]
    fn test_has_any_type_typevar_real_default_any_true() {
        // `T = TypeVar('T', default=Any)`: a genuine Any default IS a
        // real Any and must make has_any_type true.
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(make_any(TYPE_OF_ANY_UNANNOTATED)),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            has_any_type_inner(&tv, false, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_any_type_typevar_bound_any_true() {
        // `T = TypeVar('T', bound=Any)`: the upper bound being Any must
        // be counted (Python visits the upper_bound unconditionally).
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_any(TYPE_OF_ANY_UNANNOTATED)),
            default: Box::new(make_any(TYPE_OF_ANY_FROM_OMITTED_GENERICS)),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            has_any_type_inner(&tv, false, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_any_type_paramspec_no_default_false() {
        // ParamSpec with sentinel default: no real Any.
        let ps = Type::ParamSpecType {
            name: "P".to_string(),
            fullname: "mod.P".to_string(),
            raw_id: 1,
            prefix: Box::new(crate::wire::Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            flavor: 0,
            upper_bound: Box::new(make_instance("object", vec![])),
            namespace: Default::default(),
            default: Box::new(make_any(TYPE_OF_ANY_FROM_OMITTED_GENERICS)),
        };
        assert_eq!(
            has_any_type_inner(&ps, false, &empty_alias_resolver()),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_typevar_tuple_no_default_false() {
        // TypeVarTuple's tuple_fallback is NOT visited by Python
        // (visit_type_var_tuple visits upper_bound + default only). An
        // Any in tuple_fallback must not trigger a hit, and the sentinel

        // default must be skipped. Both directions of that bug are here.
        let tvt = Type::TypeVarTupleType {
            name: "Ts".to_string(),
            fullname: "mod.Ts".to_string(),
            raw_id: 1,
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(make_any(TYPE_OF_ANY_FROM_OMITTED_GENERICS)),
            tuple_fallback: Box::new(make_any(TYPE_OF_ANY_UNANNOTATED)),
            min_len: 0,
            namespace: Default::default(),
        };
        assert_eq!(
            has_any_type_inner(&tvt, false, &empty_alias_resolver()),
            Some(false)
        );
    }

    #[test]
    fn test_has_any_type_typevar_tuple_bound_any_true() {
        // TypeVarTuple upper_bound Any -> true (Python always visits it).
        let tvt = Type::TypeVarTupleType {
            name: "Ts".to_string(),
            fullname: "mod.Ts".to_string(),
            raw_id: 1,
            upper_bound: Box::new(make_any(TYPE_OF_ANY_UNANNOTATED)),
            default: Box::new(make_any(TYPE_OF_ANY_FROM_OMITTED_GENERICS)),
            tuple_fallback: Box::new(make_instance("tuple", vec![])),
            min_len: 0,
            namespace: Default::default(),
        };
        assert_eq!(
            has_any_type_inner(&tvt, false, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_uninhabited_component_true() {
        assert_eq!(
            has_uninhabited_component_inner(
                &Type::UninhabitedType { ambiguous: false },
                &empty_alias_resolver()
            ),
            Some(true)
        );
    }

    #[test]
    fn test_has_uninhabited_component_false() {
        assert_eq!(
            has_uninhabited_component_inner(&make_instance("int", vec![]), &empty_alias_resolver()),
            Some(false)
        );
    }

    #[test]
    fn test_has_uninhabited_component_typevar_real_default_true() {
        // A TypeVar with a genuine UninhabitedType default must be
        // detected through all_children (children() covers TypeVarType;
        // the sentinel filter only drops the no-default Any, never an

        // UninhabitedType default).
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            has_uninhabited_component_inner(&tv, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_uninhabited_component_typevar_sentinel_default_false() {
        // The from_omitted_generics sentinel (an AnyType) is not an
        // UninhabitedType, so skipping it changes nothing; the answer
        // stays false. Guards all_children against double-visits.
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(make_any(TYPE_OF_ANY_FROM_OMITTED_GENERICS)),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            has_uninhabited_component_inner(&tv, &empty_alias_resolver()),
            Some(false)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_true() {
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(
                &Type::UninhabitedType { ambiguous: true },
                &empty_alias_resolver()
            ),
            Some(true)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_false_flag() {
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(
                &Type::UninhabitedType { ambiguous: false },
                &empty_alias_resolver()
            ),
            Some(false)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_in_union() {
        let u = make_union(vec![
            make_instance("int", vec![]),
            Type::UninhabitedType { ambiguous: true },
        ]);
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(&u, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_clean_instance() {
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(
                &make_instance("int", vec![]),
                &empty_alias_resolver()
            ),
            Some(false)
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(&alias, &empty_alias_resolver()),
            None
        );
    }

    #[test]
    fn test_has_uninhabited_component_alias_target_uninhabited_true() {
        // An alias whose (no-arg) target is UninhabitedType must answer
        // true via the alias resolver (Phase C, #594).
        let aliases =
            alias_resolver_with_targets(&[("mod.A", Type::UninhabitedType { ambiguous: false })]);
        assert_eq!(
            has_uninhabited_component_inner(&make_type_alias("mod.A"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_has_uninhabited_component_alias_target_clean_false() {
        let aliases = alias_resolver_with_targets(&[(
            "mod.A",
            make_instance("list", vec![make_instance("int", vec![])]),
        )]);
        assert_eq!(
            has_uninhabited_component_inner(&make_type_alias("mod.A"), &aliases),
            Some(false)
        );
    }

    #[test]
    fn test_has_uninhabited_component_alias_arg_uninhabited_true() {
        // New-style alias A[T] = List[T] applied as A[Uninhabited]: the
        // substitution must carry the uninhabited arg into the target and
        // the args-visit must find it (Phase C core).
        let mut resolver = empty_alias_resolver();
        let tv = crate::aliases::AliasTvar {
            name: "T".to_string(),
            raw_id: 7,
            meta_level: 0,
            namespace: Default::default(),
            is_type_var_tuple: false,
        };
        let tvar_type = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 7,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            variance: 0,
            meta_level: 0,
        };
        let target = Type::Instance {
            type_ref: "builtins.list".to_string(),
            args: vec![tvar_type],
            last_known_value: None,
            extra_attrs: None,
        };
        let snap = crate::aliases::TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_type(&target).expect("target must encode"),
            alias_tvars: vec![tv],
            python_3_12_type_alias: true,
            ..Default::default()
        };
        resolver.insert("mod.A".to_string(), snap);
        let alias_app = Type::TypeAliasType {
            type_ref: "mod.A".to_string(),
            args: vec![Type::UninhabitedType { ambiguous: false }],
        };
        assert_eq!(
            has_uninhabited_component_inner(&alias_app, &resolver),
            Some(true)
        );
    }

    #[test]
    fn test_has_uninhabited_component_recursive_alias_defers() {
        // A = List[A]. The target cannot encode (alias target cannot cross
        // the wire), so build a self-referential chain by hand: A's
        // snapshot target references A itself. expanded_alias_target's

        // per-call chain detection trips first and defers (None), exactly
        // like has_any_type's recursive-alias test: Python's
        // get_proper_type cannot terminate recursive aliases either, and

        // BoolTypeQuery's seen_aliases short-circuits to default, so defer
        // is the parity-safe answer.
        let mut resolver = empty_alias_resolver();
        insert_chain_edges(
            &mut resolver,
            alias_resolver_with_alias_targets(&[("mod.A", "mod.A".to_string())]),
        );
        assert_eq!(
            has_uninhabited_component_inner(&make_type_alias("mod.A"), &resolver),
            None
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_alias_target_ambiguous_true() {
        let aliases =
            alias_resolver_with_targets(&[("mod.A", Type::UninhabitedType { ambiguous: true })]);
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(&make_type_alias("mod.A"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_has_erased_component_true() {
        assert_eq!(
            has_erased_component_inner(&Type::ErasedType, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_erased_component_in_union() {
        let u = make_union(vec![make_instance("int", vec![]), Type::ErasedType]);
        assert_eq!(
            has_erased_component_inner(&u, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_erased_component_clean_instance_false() {
        assert_eq!(
            has_erased_component_inner(&make_instance("int", vec![]), &empty_alias_resolver()),
            Some(false)
        );
    }

    #[test]
    fn test_has_erased_component_never_false() {
        // ErasedType is a distinct leaf: UninhabitedType does NOT trigger it
        // (the two queries answer independently).
        assert_eq!(
            has_erased_component_inner(
                &Type::UninhabitedType { ambiguous: false },
                &empty_alias_resolver()
            ),
            Some(false)
        );
    }

    #[test]
    fn test_has_erased_component_typevar_default_true() {
        // A TypeVar with a genuine ErasedType default must be found through
        // all_children (children() covers TypeVarType).
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: Default::default(),
            values: vec![],
            upper_bound: Box::new(make_instance("object", vec![])),
            default: Box::new(Type::ErasedType),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            has_erased_component_inner(&tv, &empty_alias_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_has_erased_component_alias_target_erased_true() {
        let aliases = alias_resolver_with_targets(&[("mod.A", Type::ErasedType)]);
        assert_eq!(
            has_erased_component_inner(&make_type_alias("mod.A"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_has_erased_component_alias_target_clean_false() {
        let aliases = alias_resolver_with_targets(&[(
            "mod.A",
            make_instance("list", vec![make_instance("int", vec![])]),
        )]);
        assert_eq!(
            has_erased_component_inner(&make_type_alias("mod.A"), &aliases),
            Some(false)
        );
    }

    #[test]
    fn test_has_erased_component_recursive_alias_defers() {
        let mut resolver = empty_alias_resolver();
        insert_chain_edges(
            &mut resolver,
            alias_resolver_with_alias_targets(&[("mod.A", "mod.A".to_string())]),
        );
        assert_eq!(
            has_erased_component_inner(&make_type_alias("mod.A"), &resolver),
            None
        );
    }

    #[test]
    fn test_has_ambiguous_uninhabited_component_recursive_alias_defers() {
        let mut resolver = empty_alias_resolver();
        insert_chain_edges(
            &mut resolver,
            alias_resolver_with_alias_targets(&[("mod.A", "mod.A".to_string())]),
        );
        assert_eq!(
            has_ambiguous_uninhabited_component_inner(&make_type_alias("mod.A"), &resolver),
            None
        );
    }

    #[test]
    fn test_allow_fast_container_literal_instance() {
        assert_eq!(
            allow_fast_container_literal_inner(
                &make_instance("list", vec![]),
                &crate::aliases::TypeAliasResolver::new()
            ),
            Some(true)
        );
    }

    #[test]
    fn test_allow_fast_container_literal_tuple_all_items() {
        let tup = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("int", vec![]), make_instance("str", vec![])],
            implicit: false,
        };
        assert_eq!(
            allow_fast_container_literal_inner(&tup, &crate::aliases::TypeAliasResolver::new()),
            Some(true)
        );
    }

    #[test]
    fn test_allow_fast_container_literal_tuple_with_non_instance() {
        let tup = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("int", vec![]), make_union(vec![])],
            implicit: false,
        };
        assert_eq!(
            allow_fast_container_literal_inner(&tup, &crate::aliases::TypeAliasResolver::new()),
            Some(false)
        );
    }

    #[test]
    fn test_allow_fast_container_literal_alias_missing_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(
            allow_fast_container_literal_inner(&alias, &crate::aliases::TypeAliasResolver::new()),
            None
        );
    }

    #[test]
    fn test_allow_fast_container_literal_alias_to_tuple_resolves() {
        // A = Tuple[int, str]: Python get_proper_type expands the alias and
        // all items qualify -> true.
        let aliases = alias_resolver_with_targets(&[(
            "mod.A",
            Type::TupleType {
                partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
                items: vec![make_instance("int", vec![]), make_instance("str", vec![])],
                implicit: false,
            },
        )]);
        assert_eq!(
            allow_fast_container_literal_inner(&make_type_alias("mod.A"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_allow_fast_container_literal_alias_to_list_resolves() {
        // A = list[int]: expands to an Instance -> true.
        let aliases = alias_resolver_with_targets(&[("mod.L", make_instance("list", vec![]))]);
        assert_eq!(
            allow_fast_container_literal_inner(&make_type_alias("mod.L"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_has_bytes_component_true() {
        assert_eq!(
            has_bytes_component_inner(
                &make_instance("builtins.bytes", vec![]),
                &crate::aliases::TypeAliasResolver::new()
            ),
            Some(true)
        );
    }

    #[test]
    fn test_has_bytes_component_false() {
        assert_eq!(
            has_bytes_component_inner(
                &make_instance("builtins.int", vec![]),
                &crate::aliases::TypeAliasResolver::new()
            ),
            Some(false)
        );
    }

    #[test]
    fn test_has_bytes_component_in_union() {
        let u = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.bytes", vec![]),
        ]);
        assert_eq!(
            has_bytes_component_inner(&u, &crate::aliases::TypeAliasResolver::new()),
            Some(true)
        );
    }

    #[test]
    fn test_has_bytes_component_alias_to_bytes_resolves() {
        // A = builtins.bytes: Python get_proper_type expands the alias.
        let aliases =
            alias_resolver_with_targets(&[("mod.B", make_instance("builtins.bytes", vec![]))]);
        assert_eq!(
            has_bytes_component_inner(&make_type_alias("mod.B"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_has_bytes_component_union_alias_item_resolves() {
        // Union[int, A] where A = builtins.bytearray.
        let aliases =
            alias_resolver_with_targets(&[("mod.BA", make_instance("builtins.bytearray", vec![]))]);
        let u = Type::UnionType {
            items: vec![
                make_instance("builtins.int", vec![]),
                make_type_alias("mod.BA"),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(has_bytes_component_inner(&u, &aliases), Some(true));
    }

    #[test]
    fn test_has_bytes_component_alias_missing_defers() {
        assert_eq!(
            has_bytes_component_inner(
                &make_type_alias("mod.Missing"),
                &crate::aliases::TypeAliasResolver::new()
            ),
            None
        );
    }

    #[test]
    fn test_has_bool_item_true() {
        assert_eq!(
            has_bool_item_inner(&make_instance("builtins.bool", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_has_bool_item_false() {
        assert_eq!(
            has_bool_item_inner(&make_instance("builtins.int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_has_bool_item_in_union() {
        let u = make_union(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.bool", vec![]),
        ]);
        assert_eq!(has_bool_item_inner(&u), Some(true));
    }

    #[test]
    fn test_is_non_empty_tuple_true() {
        let t = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![make_instance("int", vec![])],
            implicit: false,
        };
        assert_eq!(is_non_empty_tuple_inner(&t), Some(true));
    }

    #[test]
    fn test_is_non_empty_tuple_false_empty() {
        let t = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![],
            implicit: false,
        };
        assert_eq!(is_non_empty_tuple_inner(&t), Some(false));
    }

    #[test]
    fn test_is_non_empty_tuple_false_non_tuple() {
        assert_eq!(
            is_non_empty_tuple_inner(&make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_has_coroutine_decorator_true() {
        assert_eq!(
            has_coroutine_decorator_inner(&make_instance("typing.AwaitableGenerator", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_has_coroutine_decorator_false() {
        assert_eq!(
            has_coroutine_decorator_inner(&make_instance("builtins.int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_is_async_def_plain_coroutine() {
        assert_eq!(
            is_async_def_inner(&make_instance("typing.Coroutine", vec![])),
            Some(true)
        );
    }

    #[test]
    fn test_is_async_def_awaitable_generator() {
        // AwaitableGenerator[Any, Any, Any, Coroutine] unwraps args[3].
        let inner = make_instance("typing.Coroutine", vec![]);
        let gen = make_instance(
            "typing.AwaitableGenerator",
            vec![
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                inner,
            ],
        );
        assert_eq!(is_async_def_inner(&gen), Some(true));
    }

    #[test]
    fn test_is_async_def_awaitable_generator_non_coroutine() {
        let gen = make_instance(
            "typing.AwaitableGenerator",
            vec![
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("builtins.list", vec![]),
            ],
        );
        assert_eq!(is_async_def_inner(&gen), Some(false));
    }

    #[test]
    fn test_is_async_def_awaitable_generator_few_args() {
        let gen = make_instance(
            "typing.AwaitableGenerator",
            vec![make_instance("int", vec![])],
        );
        assert_eq!(is_async_def_inner(&gen), Some(false));
    }

    #[test]
    fn test_is_async_def_false_non_coroutine() {
        assert_eq!(
            is_async_def_inner(&make_instance("builtins.int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_is_async_def_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(is_async_def_inner(&alias), None);
    }

    #[test]
    fn test_is_private_true() {
        assert!(rust_is_private("__foo").unwrap());
    }

    #[test]
    fn test_is_private_false_dunder() {
        assert!(!rust_is_private("__foo__").unwrap());
    }

    #[test]
    fn test_is_private_false_single_underscore() {
        assert!(!rust_is_private("_foo").unwrap());
    }

    #[test]
    fn test_is_operator_method_true() {
        assert!(rust_is_operator_method(Some("builtins.int.__add__")).unwrap());
    }

    #[test]
    fn test_is_operator_method_false() {
        assert!(!rust_is_operator_method(Some("builtins.int.foo")).unwrap());
    }

    #[test]
    fn test_is_operator_method_none() {
        assert!(!rust_is_operator_method(None).unwrap());
    }

    #[test]
    fn test_is_type_type_context_true() {
        let t = Type::TypeType {
            item: Box::new(make_instance("int", vec![])),
            is_type_form: false,
        };
        assert_eq!(
            is_type_type_context_inner(&t, &crate::aliases::TypeAliasResolver::new()),
            Some(true)
        );
    }

    #[test]
    fn test_is_type_type_context_false() {
        assert_eq!(
            is_type_type_context_inner(
                &make_instance("int", vec![]),
                &crate::aliases::TypeAliasResolver::new()
            ),
            Some(false)
        );
    }

    fn alias_resolver_with_targets(targets: &[(&str, Type)]) -> crate::aliases::TypeAliasResolver {
        let mut r = crate::aliases::TypeAliasResolver::new();
        for (fullname, target) in targets {
            let bytes = encode_type(target).expect("target must encode");
            r.insert(
                fullname.to_string(),
                crate::aliases::TypeAliasSnapshot {
                    fullname: fullname.to_string(),
                    target: bytes,
                    ..Default::default()
                },
            );
        }
        r
    }

    /// Build a resolver whose `fullname -> target` mapping has its target
    /// serialized by hand. `write_type` refuses `TypeAliasType`, but
    /// Python's `Type.write` encodes nested aliases (the production snapshot
    /// path), so alias chains only occur in bytes we craft here. Mirrors
    /// `read_type_alias_type` (wire.rs:1101): TYPE_ALIAS_TYPE tag, arg list,
    /// ref string, END_TAG. Returns a Vec so a caller can extend a resolver
    /// built from real targets with the hand-crafted chain edges.
    fn alias_resolver_with_alias_targets(
        targets: &[(&str, String)],
    ) -> Vec<(String, crate::aliases::TypeAliasSnapshot)> {
        let mut out = Vec::new();
        for (fullname, inner_ref) in targets {
            let mut wbuf = WriteBuffer::new();
            crate::wire::write_tag(&mut wbuf, crate::wire::TYPE_ALIAS_TYPE);
            crate::wire::write_type_list(&mut wbuf, &[]).expect("empty args encode");
            crate::wire::write_str(&mut wbuf, inner_ref).expect("ref encodes");
            crate::wire::write_tag(&mut wbuf, crate::wire::END_TAG);
            out.push((
                fullname.to_string(),
                crate::aliases::TypeAliasSnapshot {
                    fullname: fullname.to_string(),
                    target: wbuf.into_bytes(),
                    ..Default::default()
                },
            ));
        }
        out
    }

    fn insert_chain_edges(
        resolver: &mut crate::aliases::TypeAliasResolver,
        edges: Vec<(String, crate::aliases::TypeAliasSnapshot)>,
    ) {
        for (name, snap) in edges {
            resolver.insert(name, snap);
        }
    }

    fn make_type_alias(type_ref: &str) -> Type {
        Type::TypeAliasType {
            type_ref: type_ref.to_string(),
            args: vec![],
        }
    }

    #[test]
    fn test_is_type_type_context_expands_alias() {
        // Alias whose target is `Type[int]` must answer true.
        let aliases = alias_resolver_with_targets(&[(
            "mod.A",
            Type::TypeType {
                item: Box::new(make_instance("int", vec![])),
                is_type_form: false,
            },
        )]);
        assert_eq!(
            is_type_type_context_inner(&make_type_alias("mod.A"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_is_type_type_context_expands_list_alias_false() {
        // Alias whose target is `List[int]` must answer false.
        let aliases = alias_resolver_with_targets(&[("mod.B", make_instance("list", vec![]))]);
        assert_eq!(
            is_type_type_context_inner(&make_type_alias("mod.B"), &aliases),
            Some(false)
        );
    }

    #[test]
    fn test_is_type_type_context_alias_missing_defers() {
        assert_eq!(
            is_type_type_context_inner(
                &make_type_alias("mod.Missing"),
                &crate::aliases::TypeAliasResolver::new()
            ),
            None
        );
    }

    #[test]
    fn test_is_type_type_context_union_expands_alias_item() {
        // `Union[int, Type[int]]` where the TypeType arm is behind an alias.
        let aliases = alias_resolver_with_targets(&[(
            "mod.T",
            Type::TypeType {
                item: Box::new(make_instance("str", vec![])),
                is_type_form: false,
            },
        )]);
        let t = Type::UnionType {
            items: vec![make_instance("int", vec![]), make_type_alias("mod.T")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(is_type_type_context_inner(&t, &aliases), Some(true));
    }

    #[test]
    fn test_is_type_type_context_expands_alias_chain() {
        // B = A, A = Type[int]; the snapshot chain must be followed.
        // Nested-alias targets don't round-trip through Rust's write_type,
        // so craft the B->A edge bytes by hand (Python's Type.write encodes

        // nested aliases, matching the production snapshot path).
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        {
            let target = Type::TypeType {
                item: Box::new(make_instance("int", vec![])),
                is_type_form: false,
            };
            let bytes = encode_type(&target).expect("TypeType target must encode");
            aliases.insert(
                "mod.A".to_string(),
                crate::aliases::TypeAliasSnapshot {
                    fullname: "mod.A".to_string(),
                    target: bytes,
                    ..Default::default()
                },
            );
        }
        let chain = alias_resolver_with_alias_targets(&[("mod.Alias", "mod.A".to_string())]);
        insert_chain_edges(&mut aliases, chain);
        assert_eq!(
            is_type_type_context_inner(&make_type_alias("mod.Alias"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_is_type_type_context_cyclic_alias_defers() {
        // A = A is a degenerate cycle; expansion must defer, not loop.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_chain_edges(
            &mut aliases,
            alias_resolver_with_alias_targets(&[("mod.A", "mod.A".to_string())]),
        );
        assert_eq!(
            is_type_type_context_inner(&make_type_alias("mod.A"), &aliases),
            None
        );
    }

    #[test]
    fn test_is_type_type_context_two_cycle_defers() {
        // A = B, B = A; mutual recursion must defer, not loop.
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        insert_chain_edges(
            &mut aliases,
            alias_resolver_with_alias_targets(&[
                ("mod.A", "mod.B".to_string()),
                ("mod.B", "mod.A".to_string()),
            ]),
        );
        assert_eq!(
            is_type_type_context_inner(&make_type_alias("mod.A"), &aliases),
            None
        );
    }

    #[test]
    fn test_is_typeddict_type_context_true() {
        let t = Type::TypedDictType {
            fallback: Box::new(make_instance("TD", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert_eq!(
            is_typeddict_type_context_inner(&t, &empty_alias_lookup()),
            Some(true)
        );
    }

    #[test]
    fn test_is_typeddict_type_context_false() {
        assert_eq!(
            is_typeddict_type_context_inner(&make_instance("int", vec![]), &empty_alias_lookup()),
            Some(false)
        );
    }

    #[test]
    fn test_is_typeddict_type_context_alias_expands() {
        // Issue #1309: mod.AliasTD = TypedDictType(...) answers true once
        // the alias expands via the resolver snapshot.
        let target = Type::TypedDictType {
            fallback: Box::new(make_instance("TD", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        let aliases = alias_resolver_with_targets(&[("mod.AliasTD", target)]);
        assert_eq!(
            is_typeddict_type_context_inner(&make_type_alias("mod.AliasTD"), &aliases),
            Some(true)
        );
    }

    #[test]
    fn test_is_typeddict_type_context_union_alias_item() {
        // Union[int, AliasTD] with the alias target being a TypedDictType:
        // the union walk recurses through the alias item and answers true.
        let target = Type::TypedDictType {
            fallback: Box::new(make_instance("TD", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        let aliases = alias_resolver_with_targets(&[("mod.AliasTD", target)]);
        let u = make_union(vec![
            make_instance("int", vec![]),
            make_type_alias("mod.AliasTD"),
        ]);
        assert_eq!(is_typeddict_type_context_inner(&u, &aliases), Some(true));
    }

    #[test]
    fn test_is_string_literal_true() {
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.str", vec![])),
            value: LiteralValue::Str("hello".to_string()),
        };
        assert_eq!(is_string_literal_inner(&lit), Some(true));
    }

    #[test]
    fn test_is_string_literal_false_int() {
        let lit = Type::LiteralType {
            fallback: Box::new(make_instance("builtins.int", vec![])),
            value: LiteralValue::Int(42),
        };
        assert_eq!(is_string_literal_inner(&lit), Some(false));
    }

    // -- is_duplicate_mapping --

    fn make_typeddict() -> Type {
        Type::TypedDictType {
            fallback: Box::new(make_instance("TD", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        }
    }

    #[test]
    fn test_is_duplicate_mapping_single_false() {
        let kinds = vec![ARG_POS];
        assert_eq!(
            is_duplicate_mapping_inner(
                &[0],
                &[make_instance("int", vec![])],
                &kinds,
                &Default::default()
            ),
            Some(false)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_empty_false() {
        assert_eq!(
            is_duplicate_mapping_inner(&[], &[], &[], &Default::default()),
            Some(false)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_star_kwargs_exception_false() {
        let kinds = vec![ARG_STAR, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), make_instance("str", vec![])];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds, &Default::default()),
            Some(false)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_two_star2() {
        // Two **kwargs, both non-TypedDict: allowed (no duplicate).
        let kinds = vec![ARG_STAR2, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), make_instance("str", vec![])];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds, &Default::default()),
            Some(false)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_star2_typeddict_true() {
        // Two **kwargs, but one is a TypedDict: duplicate is real.
        let kinds = vec![ARG_STAR2, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), make_typeddict()];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds, &Default::default()),
            Some(true)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_two_pos_true() {
        let kinds = vec![ARG_POS, ARG_POS];
        let types = vec![make_instance("int", vec![]), make_instance("str", vec![])];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds, &Default::default()),
            Some(true)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_pos_does_not_touch_alias() {
        // A non-`**kwargs` actual short-circuits the `all(...)`: the alias
        // at index 1 is never resolved, matching Python's short-circuit.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let kinds = vec![ARG_POS, ARG_POS];
        let types = vec![make_instance("int", vec![]), alias];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds, &Default::default()),
            Some(true)
        );
    }

    #[test]
    fn test_is_duplicate_mapping_alias_defers() {
        // Both actuals are `**kwargs`, so `get_proper_type` runs on each;
        // the wire format has no alias target, so the result defers.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let kinds = vec![ARG_STAR2, ARG_STAR2];
        let types = vec![make_instance("int", vec![]), alias];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 1], &types, &kinds, &Default::default()),
            None
        );
    }

    #[test]
    fn test_is_duplicate_mapping_out_of_range_defers() {
        let kinds = vec![ARG_POS];
        let types = vec![make_instance("int", vec![])];
        assert_eq!(
            is_duplicate_mapping_inner(&[0, 5], &types, &kinds, &Default::default()),
            None
        );
    }

    // -- method_fullname --

    fn make_type_obj(instance: Type) -> Type {
        // A class object callable: fallback is `builtins.type` (metaclass),
        // ret_type is the constructed instance.
        Type::CallableType {
            fallback: Box::new(make_instance("builtins.type", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(instance),
            name: Some("A".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    fn make_typet(item: Type) -> Type {
        Type::TypeType {
            item: Box::new(item),
            is_type_form: false,
        }
    }

    fn make_tuple(partial_fallback: Type, items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(partial_fallback),
            items,
            implicit: false,
        }
    }

    fn empty_resolver() -> TypeResolver {
        TypeResolver::new()
    }

    fn empty_alias_resolver() -> crate::aliases::TypeAliasResolver {
        crate::aliases::TypeAliasResolver::new()
    }

    fn containing_resolver() -> TypeResolver {
        // A -> (B defines "foo"), C -> B; "foo" sits on B.
        let mut r = TypeResolver::new();
        let mut b = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.B".to_string(),
            name: "B".to_string(),
            ..Default::default()
        };
        b.mro.push("mod.B".to_string());
        b.member_info.insert("foo".to_string(), (false, true));
        r.insert("mod.B".to_string(), b);
        let mut c = crate::typeinfo::TypeInfoSnapshot {
            fullname: "mod.C".to_string(),
            name: "C".to_string(),
            ..Default::default()
        };
        c.mro.push("mod.C".to_string());
        c.mro.push("mod.B".to_string());
        r.insert("mod.C".to_string(), c);
        r
    }

    #[test]
    fn test_method_fullname_instance() {
        let t = make_instance("mod.A", vec![]);
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.A.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_type_obj_unwraps_instance() {
        let t = make_type_obj(make_instance("mod.A", vec![]));
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.A.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_type_obj_with_explicit_instance_type() {
        // instance_type takes precedence over ret_type.
        let mut t = make_type_obj(make_instance("mod.Ret", vec![]));
        let Type::CallableType { instance_type, .. } = &mut t else {
            unreachable!();
        };
        *instance_type = Some(Box::new(make_instance("mod.Inst", vec![])));
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.Inst.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_type_obj_uninhabited_ret_defers() {
        let t = make_type_obj(Type::UninhabitedType { ambiguous: false });
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_callable_non_type_obj_defers() {
        // Plain callable (fallback is function, not a metaclass): defers.
        let t = Type::CallableType {
            fallback: Box::new(make_instance("builtins.function", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(make_instance("builtins.int", vec![])),
            name: Some("f".to_string()),
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_type_type_unwraps_item() {
        let t = make_typet(make_instance("mod.A", vec![]));
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("mod.A.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_tuple_plain() {
        let t = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![make_instance("int", vec![])],
        );
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("builtins.tuple.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_tuple_named() {
        // Named tuples return partial_fallback directly (non-tuple info).
        let t = make_tuple(make_instance("collections.NamedTuple", vec![]), vec![]);
        assert_eq!(
            method_fullname_inner(&t, "foo", &empty_resolver()),
            Some("collections.NamedTuple.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_tuple_with_unpack_defers() {
        // tuple_fallback raises NotImplementedError for unpacked items.
        let t = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![Type::UnpackType {
                typ: Box::new(make_instance("builtins.tuple", vec![])),
            }],
        );
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_typeddict_containing() {
        let r = containing_resolver();
        let t = Type::TypedDictType {
            fallback: Box::new(make_instance("mod.C", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert_eq!(
            method_fullname_inner(&t, "foo", &r),
            Some("mod.B.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_literal_fallback_containing() {
        let r = containing_resolver();
        let t = Type::LiteralType {
            fallback: Box::new(make_instance("mod.C", vec![])),
            value: LiteralValue::Int(1),
        };
        assert_eq!(
            method_fullname_inner(&t, "foo", &r),
            Some("mod.B.foo".to_string())
        );
    }

    #[test]
    fn test_method_fullname_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(
            method_fullname_inner(&alias, "foo", &empty_resolver()),
            None
        );
    }

    #[test]
    fn test_method_fullname_missing_containing_defers() {
        // Fallback info not in the resolver: defer to Python.
        let t = Type::TypedDictType {
            fallback: Box::new(make_instance("mod.Nope", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_method_fullname_union_defers() {
        let t = make_union(vec![make_instance("mod.A", vec![])]);
        assert_eq!(method_fullname_inner(&t, "foo", &empty_resolver()), None);
    }

    #[test]
    fn test_combined_context_no_items_defers() {
        assert_eq!(
            combined_context_inner(None, None, &empty_resolver(), &empty_alias_resolver()),
            None
        );
    }

    #[test]
    fn test_combined_context_any_contagion_returns_branch() {
        // `dict[str, Any] or <x>`: the branch type contains Any, so it
        // is returned verbatim (never unioned with the known context).
        let any = make_any(TYPE_OF_ANY_UNANNOTATED);
        let known = make_instance("builtins.str", vec![]);
        let out = combined_context_inner(
            Some(&any),
            Some(&known),
            &empty_resolver(),
            &empty_alias_resolver(),
        );
        let decoded = decode_type(&out.unwrap()).unwrap();
        assert_eq!(decoded, any);
    }

    #[test]
    fn test_combined_context_special_form_any_not_contagious() {
        // AnyType with type_of_any == special form is not "any" here.
        let any = make_any(TYPE_OF_ANY_SPECIAL_FORM);
        let known = make_instance("builtins.str", vec![]);
        let out = combined_context_inner(
            Some(&any),
            Some(&known),
            &empty_resolver(),
            &empty_alias_resolver(),
        )
        .unwrap();
        // make_simplified_union([special-any, str]): proper-subtype dedup
        // decides both directions (str <: Any is False under proper, Any <:

        // str is False), so both items survive as a 2-item union. The
        // special-form Any is not contagious, matching Python's
        // _remove_redundant_union_items with an empty resolver.
        let union: Vec<Type> = match decode_type(&out).unwrap() {
            Type::UnionType { items, .. } => items,
            other => panic!("expected union, got {other:?}"),
        };
        assert_eq!(union.len(), 2);
        assert_eq!(union[0], any);
        assert_eq!(union[1], known);
    }

    #[test]
    fn test_combined_context_single_branch_fast_path() {
        let int = make_instance("builtins.int", vec![]);
        let out =
            combined_context_inner(Some(&int), None, &empty_resolver(), &empty_alias_resolver());
        let decoded = decode_type(&out.unwrap()).unwrap();
        assert_eq!(decoded, int);
    }

    #[test]
    fn test_combined_context_known_only_fast_path() {
        let str_inst = make_instance("builtins.str", vec![]);
        let out = combined_context_inner(
            Some(&str_inst),
            None,
            &empty_resolver(),
            &empty_alias_resolver(),
        );
        let decoded = decode_type(&out.unwrap()).unwrap();
        assert_eq!(decoded, str_inst);
    }

    #[test]
    fn test_combined_context_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        // has_any_type_inner(alias) returns None -> defer to Python.
        assert_eq!(
            combined_context_inner(
                Some(&alias),
                None,
                &empty_resolver(),
                &empty_alias_resolver(),
            ),
            None
        );
    }

    // -- tuple_context_matches_inner --

    #[test]
    fn test_tuple_context_matches_fixed_ok() {
        // Two non-star items, fixed 3-tuple context: 2 <= 3 → match.
        let ctx = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("int", vec![]),
            ],
        );
        assert_eq!(tuple_context_matches_inner(&[0, 0], &ctx), Some(true));
    }

    #[test]
    fn test_tuple_context_matches_fixed_too_many() {
        // Four non-star items, fixed 3-tuple context: 4 > 3 → no match.
        let ctx = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![
                make_instance("int", vec![]),
                make_instance("int", vec![]),
                make_instance("int", vec![]),
            ],
        );
        assert_eq!(
            tuple_context_matches_inner(&[0, 0, 0, 0], &ctx),
            Some(false)
        );
    }

    #[test]
    fn test_tuple_context_matches_variadic_ok() {
        // One star at index 1, variadic context with unpack at index 1, same
        // total length → match.
        let ctx = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![
                make_instance("int", vec![]),
                Type::UnpackType {
                    typ: Box::new(make_instance("builtins.tuple", vec![])),
                },
                make_instance("int", vec![]),
            ],
        );
        assert_eq!(tuple_context_matches_inner(&[0, 1, 0], &ctx), Some(true));
    }

    #[test]
    fn test_tuple_context_matches_variadic_two_stars_false() {
        let ctx = make_tuple(
            make_instance("builtins.tuple", vec![]),
            vec![Type::UnpackType {
                typ: Box::new(make_instance("builtins.tuple", vec![])),
            }],
        );
        assert_eq!(tuple_context_matches_inner(&[1, 1], &ctx), Some(false));
    }

    #[test]
    fn test_tuple_context_matches_non_tuple_context_false() {
        assert_eq!(
            tuple_context_matches_inner(&[0, 0], &make_instance("int", vec![])),
            Some(false)
        );
    }

    #[test]
    fn test_tuple_context_matches_alias_context_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        assert_eq!(tuple_context_matches_inner(&[0], &alias), None);
    }

    // -- find_unpack_in_list_inner --

    #[test]
    fn test_find_unpack_in_list_present() {
        let items = vec![
            make_instance("int", vec![]),
            Type::UnpackType {
                typ: Box::new(make_instance("builtins.tuple", vec![])),
            },
        ];
        assert_eq!(find_unpack_in_list_inner(&items), Some(1));
    }

    #[test]
    fn test_find_unpack_in_list_absent() {
        let items = vec![make_instance("int", vec![])];
        assert_eq!(find_unpack_in_list_inner(&items), None);
    }

    // -- conditional_join_inner --

    fn make_native_resolver() -> NativeTypeResolver {
        NativeTypeResolver::new(
            TypeResolver::new(),
            crate::aliases::TypeAliasResolver::new(),
        )
    }

    #[test]
    fn test_conditional_join_subtype_returns_supertype() {
        // With a populated resolver, int <: object → join is object.
        // Here (empty resolver) nominal subtype cannot be confirmed, so
        // trivial_join defers and the union fallback is returned. This

        // documents the conservative behavior: the kernel only returns
        // a definite join when the resolver can prove the subtype chain.
        let if_t = make_instance("builtins.int", vec![]);
        let else_t = make_instance("builtins.object", vec![]);
        let out = conditional_join_inner(&if_t, &else_t, &empty_resolver()).unwrap();
        match decode_type(&out).unwrap() {
            Type::UnionType { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected union fallback, got {other:?}"),
        }
    }

    #[test]
    fn test_conditional_join_object_right_returns_object() {
        // join.py:651: `join(Any, t) = Any` fires before the Instance-
        // right fast path, so the s operand (Any itself) is returned.
        let any = make_any(TYPE_OF_ANY_SPECIAL_FORM);
        let obj = make_instance("builtins.object", vec![]);
        let out = conditional_join_inner(&any, &obj, &empty_resolver()).unwrap();
        let decoded = decode_type(&out).unwrap();
        match decoded {
            Type::AnyType { type_of_any, .. } => {
                assert_eq!(type_of_any, TYPE_OF_ANY_SPECIAL_FORM)
            }
            other => panic!("expected Any, got {other:?}"),
        }
    }

    #[test]
    fn test_conditional_join_equal_instances_returns_joined() {
        // Equal Instances with an empty resolver: the hoisted same-ref
        // fast path (issue #1096) decides int <: int without a snapshot,
        // so the join is int itself, matching Python's join_types.
        let t = make_instance("builtins.int", vec![]);
        let out = conditional_join_inner(&t, &t, &empty_resolver()).unwrap();
        match decode_type(&out).unwrap() {
            Type::Instance { type_ref, .. } => assert_eq!(type_ref, "builtins.int"),
            other => panic!("expected the joined instance, got {other:?}"),
        }
    }

    #[test]
    fn test_conditional_join_unrelated_returns_union() {
        let i = make_instance("builtins.int", vec![]);
        let s = make_instance("builtins.str", vec![]);
        let out = conditional_join_inner(&i, &s, &empty_resolver()).unwrap();
        match decode_type(&out).unwrap() {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
            }
            other => panic!("expected union, got {other:?}"),
        }
    }

    #[test]
    fn test_conditional_join_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let i = make_instance("builtins.int", vec![]);
        // Python's join expands aliases via get_proper_type; the wire
        // alias has no resolved target, so the join must defer (never
        // fabricate a wrong answer from an unexpanded alias).
        assert_eq!(conditional_join_inner(&alias, &i, &empty_resolver()), None);
    }

    // ---------------------------------------------------------------------------
    // Issue #458: visit_temp_node / visit__promote_expr / constant exprs
    // ---------------------------------------------------------------------------

    #[test]
    fn test_visit_temp_node_identity_int() {
        // visit_temp_node should return the input type unchanged.
        let t = make_instance("builtins.int", vec![]);
        let bytes = encode_type(&t).unwrap();
        let decoded = decode_type(&bytes).unwrap();
        assert_eq!(decoded, t);
    }

    #[test]
    fn test_visit_temp_node_identity_string() {
        let t = make_instance("builtins.str", vec![]);
        let bytes = encode_type(&t).unwrap();
        let decoded = decode_type(&bytes).unwrap();
        assert_eq!(decoded, t);
    }

    #[test]
    fn test_visit_temp_node_alias_defers() {
        // TypeAliasType round-trips the wire now (args + type_ref), but
        // visit_temp_node's proper-type expansion still needs the resolver,
        // so the temp-node decision defers.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.A".to_string(),
        };
        let bytes = encode_type(&alias).unwrap();
        assert!(alias_matches(&decode_type(&bytes).unwrap()));
    }

    fn alias_matches(t: &Type) -> bool {
        matches!(
            t,
            Type::TypeAliasType {
                type_ref: r,
                ..
            } if r == "mod.A"
        )
    }

    // -- join_type_list_inner --

    #[test]
    fn test_join_type_list_empty_defers() {
        assert_eq!(join_type_list_inner(&[], &make_native_resolver()), None);
    }

    #[test]
    fn test_join_type_list_single_passthrough() {
        let i = make_instance("builtins.int", vec![]);
        assert_eq!(
            join_type_list_inner(std::slice::from_ref(&i), &make_native_resolver()),
            Some(i)
        );
    }

    #[test]
    fn test_join_type_list_subtype_chain_defers() {
        // [int, object] with an EMPTY resolver: both the nominal prejoin
        // and is_subtype(int, object) are None (no TypeInfo snapshot), so
        // join_type_list_inner defers. Documents conservative behavior.
        let i = make_instance("builtins.int", vec![]);
        let o = make_instance("builtins.object", vec![]);
        assert_eq!(join_type_list_inner(&[i, o], &make_native_resolver()), None);
    }

    #[test]
    fn test_join_type_list_any_left_returns_any() {
        // join_types: AnyType left (post-swap s) → SameS (return s = Any).
        // join.py:314-315 `isinstance(s, AnyType) -> return s`.
        let any = make_any(TYPE_OF_ANY_SPECIAL_FORM);
        let i = make_instance("builtins.int", vec![]);
        assert_eq!(
            join_type_list_inner(&[any.clone(), i], &make_native_resolver()),
            Some(any)
        );
    }

    #[test]
    fn test_join_one_pair_same_instance_type() {
        // The nominal prejoin: equal args-less Instances with populated
        // resolver produce the type itself (Python join_types
        // `t.type == s.type -> Instance(t.type, [])`).
        let i = make_instance("builtins.int", vec![]);
        assert_eq!(
            join_one_pair(
                &i,
                &i,
                &crate::subtypes::SubtypeContext::new(false, false, false, false, false, true),
                &make_native_resolver()
            ),
            Some(i)
        );
    }

    #[test]
    fn test_join_one_pair_subtype_to_object() {
        // [int, object] with populated resolver: int <: object via
        // visit_instance_join, so the join is object (Python join_types
        // too). The old join_type_list_inner DEFERRED on this pair.
        let mut r = TypeResolver::new();
        let mut int_snap = crate::typeinfo::TypeInfoSnapshot {
            fullname: "builtins.int".to_string(),
            name: "int".to_string(),
            ..Default::default()
        };
        int_snap.mro.push("builtins.int".to_string());
        int_snap.mro.push("builtins.object".to_string());
        int_snap.has_base.insert("builtins.object".to_string());
        int_snap.has_base.insert("builtins.int".to_string());
        r.insert("builtins.int".to_string(), int_snap);
        let mut obj_snap = crate::typeinfo::TypeInfoSnapshot {
            fullname: "builtins.object".to_string(),
            name: "object".to_string(),
            ..Default::default()
        };
        obj_snap.mro.push("builtins.object".to_string());
        obj_snap.has_base.insert("builtins.object".to_string());
        r.insert("builtins.object".to_string(), obj_snap);
        let nres = NativeTypeResolver::new(r, crate::aliases::TypeAliasResolver::new());

        let i = make_instance("builtins.int", vec![]);
        let o = make_instance("builtins.object", vec![]);
        let out = join_one_pair(
            &i,
            &o,
            &crate::subtypes::SubtypeContext::new(false, false, false, false, false, true),
            &nres,
        );
        match out {
            Some(Type::Instance { type_ref, .. }) => assert_eq!(type_ref, "builtins.object"),
            other => panic!("expected object join, got {other:?}"),
        }
    }

    #[test]
    fn test_join_one_pair_instance_args_defers() {
        // An args-carrying Instance (e.g. List[int]) does not take the
        // nominal prejoin; it falls through to join_types, which defers
        // on the args case -> None (Python handles it).
        let vi = make_instance("builtins.list", vec![make_instance("builtins.int", vec![])]);
        let vs = make_instance("builtins.list", vec![make_instance("builtins.str", vec![])]);
        assert_eq!(
            join_one_pair(
                &vi,
                &vs,
                &crate::subtypes::SubtypeContext::new(false, false, false, false, false, true),
                &make_native_resolver()
            ),
            None
        );
    }

    #[test]
    fn test_join_one_pair_lkv_pairs_join_to_instance() {
        // Two LKV-carrying Instances of the same type: the prejoin guard (no
        // LKV) does NOT apply, so join_types -> visit_instance_join same-type
        // builds a FRESH Instance with LKV stripped (Python: plain `int`).
        let li = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(Type::LiteralType {
                fallback: Box::new(make_instance("builtins.int", vec![])),
                value: crate::wire::LiteralValue::Int(1),
            })),
            extra_attrs: None,
        };
        let ls = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(Type::LiteralType {
                fallback: Box::new(make_instance("builtins.int", vec![])),
                value: crate::wire::LiteralValue::Int(2),
            })),
            extra_attrs: None,
        };
        let out = join_one_pair(
            &li,
            &ls,
            &crate::subtypes::SubtypeContext::new(false, false, false, false, false, true),
            &make_native_resolver(),
        );
        match out {
            Some(Type::Instance {
                type_ref,
                last_known_value: None,
                ..
            }) => assert_eq!(type_ref, "builtins.int"),
            other => panic!("expected plain int join, got {other:?}"),
        }
    }

    // -- update_unpack_expand (build_tuple_type seen_unpack reduce) --

    #[test]
    fn test_unpack_expand_single_tuple_instance_normalizes() {
        // Tuple[*tuple[int, ...]] -> tuple[int, ...] (the lone-UnpackType
        // normalization, expandtype.py:1009-1033).
        let inner_items = make_instance("builtins.int", vec![]);
        let star = Type::UnpackType {
            typ: Box::new(make_instance("builtins.tuple", vec![inner_items.clone()])),
        };
        let result = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![star],
            implicit: false,
        };
        let updated = unpack_expand_updated(&result).unwrap();
        match updated {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.tuple");
                assert_eq!(args.len(), 1);
            }
            other => panic!("expected tuple[...] instance, got {other:?}"),
        }
    }

    #[test]
    fn test_unpack_expand_single_non_tuple_instance_defers() {
        // A lone *list[int] unpack (non-tuple Instance star): cannot be decided
        // from the wire alone (Python splices via expand_unpack, needs the
        // live TypeVarTuple fallback). Defer.
        let star = Type::UnpackType {
            typ: Box::new(make_instance(
                "builtins.list",
                vec![make_instance("builtins.int", vec![])],
            )),
        };
        let result = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![star],
            implicit: false,
        };
        assert_eq!(unpack_expand_updated(&result), None);
    }

    #[test]
    fn test_unpack_expand_typevar_tuple_single_passthrough() {
        // Tuple[*Ts] (Ts a TypeVarTuple) carries over unchanged with an
        // empty substitution: get_proper_type(Ts) is Ts, not an Instance,
        // so no splice. Returns the same TupleType.
        let ts = Type::TypeVarTupleType {
            tuple_fallback: Box::new(make_instance(
                "builtins.tuple",
                vec![make_instance("builtins.object", vec![])],
            )),
            name: "Ts".to_string(),
            fullname: "mod.Ts".to_string(),
            raw_id: 5,
            namespace: Default::default(),
            upper_bound: Box::new(make_instance(
                "builtins.tuple",
                vec![make_instance("builtins.object", vec![])],
            )),
            default: Box::new(make_any(TYPE_OF_ANY_FROM_OMITTED_GENERICS)),
            min_len: 0,
        };
        let star = Type::UnpackType { typ: Box::new(ts) };
        let tuple = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![star],
            implicit: false,
        };
        let updated = unpack_expand_updated(&tuple).unwrap();
        match updated {
            Type::TupleType { items, .. } => {
                assert_eq!(items.len(), 1);
                assert!(matches!(items[0], Type::UnpackType { .. }));
            }
            other => panic!("expected unchanged TupleType, got {other:?}"),
        }
    }

    #[test]
    fn test_unpack_expand_multi_item_identical() {
        // Tuple[int, str] with no unpack: expand_type is identity, items
        // carry over unchanged.
        let tuple = Type::TupleType {
            partial_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            items: vec![
                make_instance("builtins.int", vec![]),
                make_instance("builtins.str", vec![]),
            ],
            implicit: false,
        };
        let updated = unpack_expand_updated(&tuple).unwrap();
        match updated {
            Type::TupleType { items, .. } => assert_eq!(items.len(), 2),
            other => panic!("expected TupleType, got {other:?}"),
        }
    }

    // -- first_or_join_fast_item_inner --

    #[test]
    fn test_first_or_join_fast_item_single_instance() {
        let i = make_instance("builtins.int", vec![]);
        assert_eq!(
            first_or_join_fast_item_inner(std::slice::from_ref(&i), &make_native_resolver()),
            Some(i)
        );
    }

    #[test]
    fn test_first_or_join_fast_item_empty_defers() {
        assert_eq!(
            first_or_join_fast_item_inner(&[], &make_native_resolver()),
            None
        );
    }

    // -- build_dict_type --

    #[test]
    fn test_build_dict_type_simple() {
        let kt = make_instance("builtins.str", vec![]);
        let vt = make_instance("builtins.int", vec![]);
        let out = build_dict_type(&make_native_resolver(), &[kt, vt], 1).unwrap();
        match decode_type(&out).unwrap() {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.dict");
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected dict instance, got {other:?}"),
        }
    }

    #[test]
    fn test_visit_promote_expr_identity() {
        // Same identity as visit_temp_node.
        let t = make_instance("builtins.object", vec![]);
        let bytes = encode_type(&t).unwrap();
        let decoded = decode_type(&bytes).unwrap();
        assert_eq!(decoded, t);
    }

    #[test]
    fn test_visit_paramspec_expr_returns_special_any() {
        // visit_paramspec_expr returns AnyType(TypeOfAny.special_form).
        let bytes = encode_type(&any_type(TYPE_OF_ANY_SPECIAL_FORM, None)).unwrap();
        let decoded = decode_type(&bytes).unwrap();
        match decoded {
            Type::AnyType {
                type_of_any,
                source_any,
                missing_import_name,
            } => {
                assert_eq!(type_of_any, TYPE_OF_ANY_SPECIAL_FORM);
                assert!(source_any.is_none());
                assert!(missing_import_name.is_none());
            }
            other => panic!("expected AnyType, got {:?}", other),
        }
    }

    #[test]
    fn test_build_dict_type_empty_defers() {
        assert_eq!(build_dict_type(&make_native_resolver(), &[], 0), None);
    }

    #[test]
    fn test_build_dict_type_bad_n_keys_defers() {
        let kt = make_instance("builtins.str", vec![]);
        let vt = make_instance("builtins.int", vec![]);
        // n_keys == len(elements) leaves no values → defer.
        assert_eq!(build_dict_type(&make_native_resolver(), &[kt, vt], 2), None);
    }

    #[test]
    fn test_visit_type_var_tuple_expr_returns_special_any() {
        // Same return type as paramspec_expr.
        let bytes = encode_type(&any_type(TYPE_OF_ANY_SPECIAL_FORM, None)).unwrap();
        let decoded = decode_type(&bytes).unwrap();
        assert!(matches!(
            decoded,
            Type::AnyType {
                type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
                source_any: None,
                missing_import_name: None,
            }
        ));
    }

    #[test]
    fn test_visit_newtype_expr_returns_special_any() {
        // Same return type as paramspec_expr.
        let bytes = encode_type(&any_type(TYPE_OF_ANY_SPECIAL_FORM, None)).unwrap();
        let decoded = decode_type(&bytes).unwrap();
        assert!(matches!(
            decoded,
            Type::AnyType {
                type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
                source_any: None,
                missing_import_name: None,
            }
        ));
    }

    #[test]
    fn test_any_type_helper_special_form() {
        let t = any_type(TYPE_OF_ANY_SPECIAL_FORM, None);
        assert!(matches!(t, Type::AnyType { type_of_any: 6, .. }));
    }

    #[test]
    fn test_any_type_helper_with_source() {
        let inner = any_type(TYPE_OF_ANY_UNANNOTATED, None);
        let t = any_type(2, Some(Box::new(inner)));
        match t {
            Type::AnyType {
                type_of_any: 2,
                source_any: Some(sa),
                ..
            } => {
                assert!(matches!(*sa, Type::AnyType { type_of_any: 1, .. }));
            }
            other => panic!("expected AnyType(2, Some(...)), got {:?}", other),
        }
    }

    #[test]
    fn conditional_join_any_left_is_passthrough() {
        // join.py:651: `join(Any, t) = Any` returning the s operand
        // itself, preserving its type_of_any (5 here) and source.
        let any = Type::AnyType {
            type_of_any: 5,
            source_any: None,
            missing_import_name: None,
        };
        let int_t = make_instance("builtins.int", vec![]);
        let bytes = conditional_join_inner(&any, &int_t, &TypeResolver::new()).unwrap();
        let decoded = decode_type(&bytes).unwrap();
        assert_eq!(decoded, any);
    }
}

// ---------------------------------------------------------------------------
// classify_visit_op_expr
// ---------------------------------------------------------------------------

/// Decision tags for `visit_op_expr`; must match `NATIVE_VISIT_OP_EXPR_*`
/// in mypy/checkexpr.py.
const VISIT_OP_EXPR_ANALYZED: i64 = 0;
const VISIT_OP_EXPR_BOOLEAN: i64 = 1;
const VISIT_OP_EXPR_LIST_MULTIPLY: i64 = 2;
const VISIT_OP_EXPR_STR_INTERP: i64 = 3;
const VISIT_OP_EXPR_CHECK_OP: i64 = 4;

/// Pure 5-way dispatch of `ExpressionChecker.visit_op_expr`
/// (checkexpr.py:5004-5018). `has_analyzed` mirrors `if e.analyzed:`,
/// the isinstance flags mirror `isinstance(e.left, ListExpr/BytesExpr/
/// StrExpr)`. Order matches the Python body exactly: analyzed-passthrough,
/// then boolean op, then list multiply, then str interpolation, then
/// the trailing check-op fall-through.
fn classify_visit_op_expr(
    has_analyzed: bool,
    op: &str,
    left_is_list: bool,
    left_is_bytes: bool,
    left_is_str: bool,
) -> i64 {
    if has_analyzed {
        return VISIT_OP_EXPR_ANALYZED;
    }
    if op == "and" || op == "or" {
        return VISIT_OP_EXPR_BOOLEAN;
    }
    if op == "*" && left_is_list {
        return VISIT_OP_EXPR_LIST_MULTIPLY;
    }
    if op == "%" && (left_is_bytes || left_is_str) {
        return VISIT_OP_EXPR_STR_INTERP;
    }
    VISIT_OP_EXPR_CHECK_OP
}

/// `#[pyfunction]` entry for `ExpressionChecker.visit_op_expr`
/// (checkexpr.py:5004-5018). Reads `e.analyzed` (truthiness), `e.op`
/// (string), and `e.left` isinstance tags via PyO3, returning a branch
/// tag. Defers (`None`) on any unreadable attribute or isinstance error.
#[pyfunction]
pub(crate) fn rust_classify_visit_op_expr(py: Python<'_>, expr: &PyAny) -> PyResult<Option<i64>> {
    let analyzed = match expr.getattr("analyzed") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let has_analyzed = match analyzed.is_true() {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let op: String = match expr.getattr("op") {
        Ok(v) => match v.extract() {
            Ok(s) => s,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let left = match expr.getattr("left") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let nodes_mod = match py.import("mypy.nodes") {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    let list_cls: &PyType = match nodes_mod.getattr("ListExpr") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let bytes_cls: &PyType = match nodes_mod.getattr("BytesExpr") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let str_cls: &PyType = match nodes_mod.getattr("StrExpr") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let left_is_list = match left.is_instance(list_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let left_is_bytes = match left.is_instance(bytes_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let left_is_str = match left.is_instance(str_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    Ok(Some(classify_visit_op_expr(
        has_analyzed,
        &op,
        left_is_list,
        left_is_bytes,
        left_is_str,
    )))
}

#[cfg(test)]
mod classify_visit_op_expr_tests {
    fn classify(
        has_analyzed: bool,
        op: &str,
        left_is_list: bool,
        left_is_bytes: bool,
        left_is_str: bool,
    ) -> i64 {
        super::classify_visit_op_expr(has_analyzed, op, left_is_list, left_is_bytes, left_is_str)
    }

    #[test]
    fn test_analyzed_passthrough() {
        assert_eq!(classify(true, "|", false, false, false), 0);
        assert_eq!(classify(true, "and", false, false, false), 0);
        assert_eq!(classify(true, "*", true, false, false), 0);
    }

    #[test]
    fn test_no_analyzed_boolean() {
        assert_eq!(classify(false, "and", false, false, false), 1);
        assert_eq!(classify(false, "or", false, false, false), 1);
    }

    #[test]
    fn test_list_multiply() {
        assert_eq!(classify(false, "*", true, false, false), 2);
    }

    #[test]
    fn test_list_multiply_not_list() {
        assert_eq!(classify(false, "*", false, false, false), 4);
    }

    #[test]
    fn test_str_interp_bytes() {
        assert_eq!(classify(false, "%", false, true, false), 3);
    }

    #[test]
    fn test_str_interp_str() {
        assert_eq!(classify(false, "%", false, false, true), 3);
    }

    #[test]
    fn test_str_interp_not_bytes_str() {
        assert_eq!(classify(false, "%", false, false, false), 4);
    }

    #[test]
    fn test_check_op_default() {
        assert_eq!(classify(false, "+", false, false, false), 4);
        assert_eq!(classify(false, "|", false, false, false), 4);
        assert_eq!(classify(false, "-", false, false, false), 4);
    }

    #[test]
    fn test_op_precedence_over_isinstance() {
        // "and" takes precedence over list check even if left is a ListExpr.
        assert_eq!(classify(false, "and", true, false, false), 1);
        // "*" with BytesExpr left is NOT list multiply (goes to check_op).
        assert_eq!(classify(false, "*", false, true, false), 4);
    }
}

// ---------------------------------------------------------------------------
// classify_reveal_imported
// ---------------------------------------------------------------------------

/// `mypy.checkexpr.check_reveal_imported` dispatch head (issue #918).
///
/// Mirrors checkexpr.py:6483-6497. The Python dispatch:
///   1. early return when `UNIMPORTED_REVEAL` is not an enabled error code;
///   2. `name = "reveal_locals"` when `kind == REVEAL_LOCALS`;
///   3. `name = "reveal_type"` when `kind == REVEAL_TYPE and not is_imported`;
///   4. else early return (no name).
///
/// `REVEAL_LOCALS` / `REVEAL_TYPE` are `mypy.semanal` module constants,
/// read via PyO3 the same way `rust_visit_reveal_expr` does. Returns
/// `Ok(None)` for both the disabled case (arm 1) and the else-arm (arm 4):
/// in both, Python does nothing. Returns `Ok(Some(name))` when the
/// fail+note body should run in Python with that name.
#[pyfunction]
#[pyo3(signature = (kind, is_imported, unimported_reveal_enabled))]
pub(crate) fn rust_classify_reveal_imported(
    py: Python<'_>,
    kind: i64,
    is_imported: bool,
    unimported_reveal_enabled: bool,
) -> PyResult<Option<String>> {
    if !unimported_reveal_enabled {
        return Ok(None);
    }
    let semanal_mod = py.import("mypy.semanal")?;
    let reveal_locals: i64 = semanal_mod.getattr("REVEAL_LOCALS")?.extract()?;
    let reveal_type: i64 = semanal_mod.getattr("REVEAL_TYPE")?.extract()?;
    if kind == reveal_locals {
        Ok(Some("reveal_locals".to_string()))
    } else if kind == reveal_type && !is_imported {
        Ok(Some("reveal_type".to_string()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod classify_reveal_imported_tests {
    fn consts() -> (i64, i64) {
        // REVEAL_LOCALS == 1, REVEAL_TYPE == 0 (mypy.nodes, re-exported
        // by mypy.semanal). The unit tests run without a Python
        // interpreter, so use the literal ints.
        (1, 0)
    }

    #[test]
    fn test_classify_reveal_imported_disabled() {
        let (rl, rt) = consts();
        // No Python interpreter in `cargo test`; inline the logic.
        let r = classify_reveal_imported_pure(rl, false, false);
        assert_eq!(r, None);
        let r = classify_reveal_imported_pure(rt, true, false);
        assert_eq!(r, None);
    }

    #[test]
    fn test_classify_reveal_imported_reveal_locals() {
        let (rl, _) = consts();
        let r = classify_reveal_imported_pure(rl, false, true);
        assert_eq!(r.as_deref(), Some("reveal_locals"));
        // is_imported is irrelevant for reveal_locals.
        let r = classify_reveal_imported_pure(rl, true, true);
        assert_eq!(r.as_deref(), Some("reveal_locals"));
    }

    #[test]
    fn test_classify_reveal_imported_reveal_type_not_imported() {
        let (_, rt) = consts();
        let r = classify_reveal_imported_pure(rt, false, true);
        assert_eq!(r.as_deref(), Some("reveal_type"));
    }

    #[test]
    fn test_classify_reveal_imported_reveal_type_imported() {
        let (_, rt) = consts();
        let r = classify_reveal_imported_pure(rt, true, true);
        assert_eq!(r, None);
    }

    #[test]
    fn test_classify_reveal_imported_unknown_kind() {
        let r = classify_reveal_imported_pure(999, false, true);
        assert_eq!(r, None);
    }

    /// Pure-logic twin of `rust_classify_reveal_imported` for unit tests
    /// that run without a Python interpreter (constants inlined).
    fn classify_reveal_imported_pure(
        kind: i64,
        is_imported: bool,
        unimported_reveal_enabled: bool,
    ) -> Option<String> {
        if !unimported_reveal_enabled {
            return None;
        }
        let reveal_locals: i64 = 1;
        let reveal_type: i64 = 0;
        if kind == reveal_locals {
            Some("reveal_locals".to_string())
        } else if kind == reveal_type && !is_imported {
            Some("reveal_type".to_string())
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// classify_super_arg_types
// ---------------------------------------------------------------------------

/// Decision tags for `_super_arg_types` stage-1 dispatch; must match
/// `NATIVE_SUPER_ARG_*` in mypy/checkexpr.py.
const SUPER_ARG_NOT_CHECKED: i64 = 0;
const SUPER_ARG_ZERO_ARG_NO_INFO: i64 = 1;
const SUPER_ARG_ZERO_ARG_OUTSIDE_METHOD: i64 = 2;
const SUPER_ARG_ZERO_ARG_OK: i64 = 3;
const SUPER_ARG_VARARGS: i64 = 4;
const SUPER_ARG_NON_POSITIONAL: i64 = 5;
const SUPER_ARG_SINGLE_ARG: i64 = 6;
const SUPER_ARG_TWO_ARG_OK: i64 = 7;
const SUPER_ARG_TOO_MANY: i64 = 8;

/// `_super_arg_types` stage-1 dispatch head (checkexpr.py:7447-7483).
///
/// Mirrors the arity + scope gate chain that produces the seven
/// early-error returns and the two fall-through bodies computing
/// `type_type`/`instance_type` for stage 2 (proper-type dispatch, which
/// stays in Python). Rust classifies which branch stage 1 hits from live
/// checker/expression facts read via PyO3; the shim applies the
/// `self.fail` / `fill_typevars` / `accept` side effects. Defers (`None`)
/// on any unreadable fact.
#[pyfunction]
#[pyo3(signature = (chk, super_expr))]
pub(crate) fn rust_classify_super_arg_types(
    chk: &PyAny,
    super_expr: &PyAny,
) -> PyResult<Option<i64>> {
    // Read facts lazily in Python branch order so a deferred read never
    // fires for a branch that short-circuits before it.
    let in_checked: bool = match chk.call_method0("in_checked_function") {
        Ok(v) => match v.extract() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    if !in_checked {
        return Ok(Some(SUPER_ARG_NOT_CHECKED));
    }

    let call = match super_expr.getattr("call") {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    let args = match call.getattr("args") {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    let n_args: i64 = match args.len() {
        Ok(n) => n as i64,
        Err(_) => return Ok(None),
    };

    if n_args == 0 {
        let info = match super_expr.getattr("info") {
            Ok(i) => i,
            Err(_) => return Ok(None),
        };
        if info.is_none() {
            return Ok(Some(SUPER_ARG_ZERO_ARG_NO_INFO));
        }
        let scope = match chk.getattr("scope") {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        let active_obj = match scope.call_method0("active_class") {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        if !active_obj.is_none() {
            return Ok(Some(SUPER_ARG_ZERO_ARG_OUTSIDE_METHOD));
        }
        return Ok(Some(SUPER_ARG_ZERO_ARG_OK));
    }

    let kinds_obj = match call.getattr("arg_kinds") {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    let kinds = match arg_kind_values(kinds_obj) {
        Some(v) => v,
        None => return Ok(None),
    };
    if kinds.contains(&ARG_STAR) {
        return Ok(Some(SUPER_ARG_VARARGS));
    }
    // set(arg_kinds) != {ARG_POS}; n_args >= 1 here so no empty case.
    if kinds.iter().any(|&k| k != ARG_POS) {
        return Ok(Some(SUPER_ARG_NON_POSITIONAL));
    }
    if n_args == 1 {
        return Ok(Some(SUPER_ARG_SINGLE_ARG));
    }
    if n_args == 2 {
        return Ok(Some(SUPER_ARG_TWO_ARG_OK));
    }
    Ok(Some(SUPER_ARG_TOO_MANY))
}

/// Read `e.call.arg_kinds` (a list of `ArgKind`) as the int `.value`s.
/// Returns `None` on any read failure so the caller defers.
fn arg_kind_values(list: &PyAny) -> Option<Vec<i64>> {
    let seq = list.downcast::<PyList>().ok()?;
    let mut out = Vec::with_capacity(seq.len());
    for item in seq.iter() {
        let v: i64 = item.getattr("value").ok()?.extract().ok()?;
        out.push(v);
    }
    Some(out)
}

#[cfg(test)]
mod classify_super_arg_types_tests {
    use super::*;

    fn classify(
        in_checked: bool,
        n_args: i64,
        info_present: bool,
        active_class: bool,
        arg_kinds: &[i64],
    ) -> i64 {
        if !in_checked {
            return SUPER_ARG_NOT_CHECKED;
        }
        if n_args == 0 {
            if !info_present {
                return SUPER_ARG_ZERO_ARG_NO_INFO;
            }
            if active_class {
                return SUPER_ARG_ZERO_ARG_OUTSIDE_METHOD;
            }
            return SUPER_ARG_ZERO_ARG_OK;
        }
        if arg_kinds.contains(&ARG_STAR) {
            return SUPER_ARG_VARARGS;
        }
        if arg_kinds.iter().any(|&k| k != ARG_POS) {
            return SUPER_ARG_NON_POSITIONAL;
        }
        if n_args == 1 {
            return SUPER_ARG_SINGLE_ARG;
        }
        if n_args == 2 {
            return SUPER_ARG_TWO_ARG_OK;
        }
        SUPER_ARG_TOO_MANY
    }

    #[test]
    fn test_not_checked() {
        assert_eq!(
            classify(false, 2, true, false, &[0, 0]),
            SUPER_ARG_NOT_CHECKED
        );
    }

    #[test]
    fn test_zero_arg_no_info() {
        assert_eq!(
            classify(true, 0, false, false, &[]),
            SUPER_ARG_ZERO_ARG_NO_INFO
        );
    }

    #[test]
    fn test_zero_arg_outside_method() {
        assert_eq!(
            classify(true, 0, true, true, &[]),
            SUPER_ARG_ZERO_ARG_OUTSIDE_METHOD
        );
    }

    #[test]
    fn test_zero_arg_ok() {
        assert_eq!(classify(true, 0, true, false, &[]), SUPER_ARG_ZERO_ARG_OK);
    }

    #[test]
    fn test_varargs() {
        // ARG_STAR (2) beats the positional-set check.
        assert_eq!(classify(true, 1, true, false, &[2]), SUPER_ARG_VARARGS);
        assert_eq!(classify(true, 2, true, false, &[0, 2]), SUPER_ARG_VARARGS);
    }

    #[test]
    fn test_non_positional() {
        assert_eq!(
            classify(true, 1, true, false, &[3]),
            SUPER_ARG_NON_POSITIONAL
        );
        assert_eq!(
            classify(true, 2, true, false, &[0, 4]),
            SUPER_ARG_NON_POSITIONAL
        );
    }

    #[test]
    fn test_single_arg() {
        assert_eq!(classify(true, 1, true, false, &[0]), SUPER_ARG_SINGLE_ARG);
    }

    #[test]
    fn test_two_arg_ok() {
        assert_eq!(
            classify(true, 2, true, false, &[0, 0]),
            SUPER_ARG_TWO_ARG_OK
        );
    }

    #[test]
    fn test_too_many() {
        assert_eq!(
            classify(true, 3, true, false, &[0, 0, 0]),
            SUPER_ARG_TOO_MANY
        );
    }
}

#[cfg(test)]
mod refers_to_typeddict_tests {
    use super::*;

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    #[test]
    fn test_alias_target_typeddict_wire() {
        // A TypeAlias whose proper-type target is a TypedDictType: the
        // wire round-trip preserves the TypedDictType tag the seam
        // matches on.
        let td = Type::TypedDictType {
            items: vec![("x".to_string(), any_type())],
            required_keys: HashSet::new(),
            readonly_keys: HashSet::new(),
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.dict".to_string(),
                args: vec![any_type(), any_type()],
                last_known_value: None,
                extra_attrs: None,
            }),
            is_closed: false,
        };
        let bytes = encode_type(&td).unwrap();
        assert!(matches!(
            decode_type(&bytes).unwrap(),
            Type::TypedDictType { .. }
        ));
    }

    #[test]
    fn test_alias_target_instance_wire() {
        // A non-TypedDictType alias target decodes to a different
        // variant -> the seam must return false, not true.
        let inst = Type::Instance {
            type_ref: "mod.Cls".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let bytes = encode_type(&inst).unwrap();
        assert!(!matches!(
            decode_type(&bytes).unwrap(),
            Type::TypedDictType { .. }
        ));
    }
}

// ---------------------------------------------------------------------------
// is_valid_var_arg / is_valid_keyword_var_arg (issue #981)
// ---------------------------------------------------------------------------

/// `is_valid_var_arg` (checkexpr.py:8010): is a type valid as `*args`?
/// `iterable_ok` is the resolver-backed is_subtype(Iterable[Any]) verdict.
/// Defers on undecodable wire bytes or a `TypeAliasType` (no alias target).
#[pyfunction]
pub(crate) fn rust_is_valid_var_arg(
    type_bytes: &[u8],
    iterable_ok: bool,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_valid_var_arg_inner(&typ, iterable_ok))
}

pub(crate) fn is_valid_var_arg_inner(typ: &Type, iterable_ok: bool) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    Some(
        matches!(
            proper,
            Type::TupleType { .. }
                | Type::AnyType { .. }
                | Type::ParamSpecType { .. }
                | Type::UnpackType { .. }
        ) || iterable_ok,
    )
}

/// `is_valid_keyword_var_arg` (checkexpr.py:8017): is a type valid as
/// `**kwargs`? The `dict_str_keys_ok` / `skag_*_ok` args carry the
/// resolver-backed is_subtype verdicts; isinstance facts come from wire.
/// Defers on undecodable wire bytes, a `TypeAliasType` (no alias target),
/// and a dict Instance with no args (Python would index `args[0]`).
#[pyfunction]
pub(crate) fn rust_is_valid_keyword_var_arg(
    type_bytes: &[u8],
    dict_str_keys_ok: bool,
    skag_str_ok: bool,
    skag_never_ok: bool,
) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_valid_keyword_var_arg_inner(
        &typ,
        dict_str_keys_ok,
        skag_str_ok,
        skag_never_ok,
    ))
}

pub(crate) fn is_valid_keyword_var_arg_inner(
    typ: &Type,
    dict_str_keys_ok: bool,
    skag_str_ok: bool,
    skag_never_ok: bool,
) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance { type_ref, args, .. } if type_ref == "builtins.dict" => {
            if args.is_empty() {
                return None;
            }
            return Some(dict_str_keys_ok || skag_str_ok || skag_never_ok);
        }
        Type::ParamSpecType { .. } => return Some(true),
        _ => {}
    }
    Some(skag_str_ok || skag_never_ok)
}

#[cfg(test)]
mod is_valid_var_arg_tests {
    use super::*;
    use crate::wire::Parameters;

    fn var_arg(typ: &Type, iterable_ok: bool) -> Option<bool> {
        is_valid_var_arg_inner(typ, iterable_ok)
    }

    fn kwarg(typ: &Type, dict_ok: bool, skag_str: bool, skag_never: bool) -> Option<bool> {
        is_valid_keyword_var_arg_inner(typ, dict_ok, skag_str, skag_never)
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: TYPE_OF_ANY_SPECIAL_FORM,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn instance(fullname: &str) -> Type {
        Type::Instance {
            type_ref: fullname.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn paramspec() -> Type {
        Type::ParamSpecType {
            prefix: Box::new(Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "P".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            flavor: 0,
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(any_type()),
        }
    }

    fn tuple_type() -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple")),
            items: vec![],
            implicit: false,
        }
    }

    fn alias() -> Type {
        Type::TypeAliasType {
            args: vec![],
            type_ref: "m.A".to_string(),
        }
    }

    #[test]
    fn test_var_arg_accepts_tag_types() {
        assert_eq!(var_arg(&tuple_type(), false), Some(true));
        assert_eq!(var_arg(&any_type(), false), Some(true));
        assert_eq!(var_arg(&paramspec(), false), Some(true));
        assert_eq!(
            var_arg(
                &Type::UnpackType {
                    typ: Box::new(instance("builtins.list")),
                },
                false
            ),
            Some(true)
        );
    }

    #[test]
    fn test_var_arg_falls_to_iterable_boolean() {
        assert_eq!(var_arg(&instance("builtins.list"), false), Some(false));
        assert_eq!(var_arg(&instance("builtins.list"), true), Some(true));
        assert_eq!(var_arg(&Type::NoneType, true), Some(true));
    }

    #[test]
    fn test_var_arg_defers_on_alias() {
        assert_eq!(var_arg(&alias(), true), None);
    }

    #[test]
    fn test_kwarg_dict_str_keys() {
        // Dict with str keys: the Python or-chain short-circuits on the
        // dict arm; the OR of the passed booleans gives the same value.
        let d_str = Type::Instance {
            type_ref: "builtins.dict".to_string(),
            args: vec![instance("builtins.str"), any_type()],
            last_known_value: None,
            extra_attrs: None,
        };
        assert_eq!(kwarg(&d_str, true, false, false), Some(true));
        // Dict with non-str keys: A false; the skag booleans complete the
        // Python or-chain (both false here).
        let d_int = Type::Instance {
            type_ref: "builtins.dict".to_string(),
            args: vec![instance("builtins.int"), any_type()],
            last_known_value: None,
            extra_attrs: None,
        };
        assert_eq!(kwarg(&d_int, false, false, false), Some(false));
    }

    #[test]
    fn test_kwarg_dict_empty_args_defers() {
        assert_eq!(kwarg(&instance("builtins.dict"), false, true, true), None);
    }

    #[test]
    fn test_kwarg_paramspec() {
        assert_eq!(kwarg(&paramspec(), false, false, false), Some(true));
    }

    #[test]
    fn test_kwarg_other_instance_uses_skag_booleans() {
        assert_eq!(
            kwarg(&instance("builtins.list"), false, false, false),
            Some(false)
        );
        assert_eq!(
            kwarg(&instance("builtins.list"), false, true, false),
            Some(true)
        );
        // dict_str_keys_ok is only consulted for a dict instance: a
        // non-dict Instance with dict_ok=true stays False (the Python
        // isinstance(Instance) and fullname gate owns that fact).
        assert_eq!(
            kwarg(&instance("typing.Mapping"), true, false, false),
            Some(false)
        );
        assert_eq!(kwarg(&Type::NoneType, false, false, true), Some(true));
    }

    #[test]
    fn test_kwarg_defers_on_alias() {
        assert_eq!(kwarg(&alias(), false, true, true), None);
    }
}

// ---------------------------------------------------------------------------
// classify_index_with_type
// ---------------------------------------------------------------------------

/// Decision tags for `visit_index_with_type`; must match
/// `NATIVE_INDEX_*` in mypy/checkexpr.py.
const INDEX_NORMALIZE: i64 = 0;
const INDEX_UNION: i64 = 1;
const INDEX_TUPLE: i64 = 2;
const INDEX_TYPEDDICT: i64 = 3;
const INDEX_ENUM: i64 = 4;
const INDEX_GENERIC_ALIAS: i64 = 5;
const INDEX_TYPEVAR: i64 = 6;
const INDEX_SPECIAL_FORM: i64 = 7;
const INDEX_GETITEM: i64 = 8;

/// Pure dispatch of `ExpressionChecker.visit_index_with_type`
/// (checkexpr.py:6109-6166). Mirrors the Python branch order exactly:
/// variadic-tuple normalization gate (only when `expand_variadic`, since
/// Python normalizes once, never in a loop), union fan-out, tuple arm
/// gated by `in_checked_function()`, TypedDict, enum / generic-alias
/// type-object arms, then the second if-chain (TypeVar, special-form
/// Instance) and the trailing `__getitem__` tail. `left_type` is the
/// already-proper type; the tuple slice vs int-literal vs nonliteral
/// sub-dispatch stays in Python (the literal body needs the `ns` values
/// from `try_getting_int_literals`, which re-accepts the index).
#[allow(clippy::too_many_arguments)]
fn classify_index_with_type(
    is_tuple: bool,
    is_variadic: bool,
    expand_variadic: bool,
    is_union: bool,
    in_checked: bool,
    is_typeddict: bool,
    is_function_like: bool,
    is_type_obj: bool,
    is_enum: bool,
    to_has_type_vars: bool,
    to_is_builtin_type: bool,
    is_typevar: bool,
    is_instance: bool,
    left_fullname: Option<&str>,
) -> i64 {
    if is_tuple && is_variadic && expand_variadic {
        return INDEX_NORMALIZE;
    }
    if is_union {
        return INDEX_UNION;
    }
    if is_tuple && in_checked {
        return INDEX_TUPLE;
    }
    if is_typeddict {
        return INDEX_TYPEDDICT;
    }
    if is_function_like && is_type_obj {
        if is_enum {
            return INDEX_ENUM;
        }
        if to_has_type_vars || to_is_builtin_type {
            return INDEX_GENERIC_ALIAS;
        }
        // Not enum / generic-alias: falls through to the second if-chain.
    }
    if is_typevar {
        return INDEX_TYPEVAR;
    }
    if is_instance && left_fullname == Some("typing._SpecialForm") {
        return INDEX_SPECIAL_FORM;
    }
    INDEX_GETITEM
}

/// `#[pyfunction]` entry for `ExpressionChecker.visit_index_with_type`
/// (checkexpr.py:6095). Reads the live proper `left_type` isinstance tags
/// (variadic walk over `TupleType.items`, `FunctionLike.is_type_obj()` /
/// `type_object()` facts, `Instance.type.fullname`) and
/// `chk.in_checked_function()` via PyO3, then classifies. Returns a
/// branch tag; `None` defers on any unreadable fact.
#[pyfunction]
#[pyo3(signature = (left_type, chk, expand_variadic))]
pub(crate) fn rust_classify_index_with_type(
    py: Python<'_>,
    left_type: &PyAny,
    chk: &PyAny,
    expand_variadic: bool,
) -> PyResult<Option<i64>> {
    let types_mod = match py.import("mypy.types") {
        Ok(m) => m,
        Err(_) => return Ok(None),
    };
    let tuple_cls: &PyType = match types_mod.getattr("TupleType") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let union_cls: &PyType = match types_mod.getattr("UnionType") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let tdict_cls: &PyType = match types_mod.getattr("TypedDictType") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let func_like_cls: &PyType = match types_mod.getattr("FunctionLike") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let typevar_cls: &PyType = match types_mod.getattr("TypeVarType") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let instance_cls: &PyType = match types_mod.getattr("Instance") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };
    let unpack_cls: &PyType = match types_mod.getattr("UnpackType") {
        Ok(c) => match c.downcast() {
            Ok(t) => t,
            Err(_) => return Ok(None),
        },
        Err(_) => return Ok(None),
    };

    // Read facts lazily in Python branch order so a deferred read never
    // fires for a branch that short-circuits before it.
    let is_tuple = match left_type.is_instance(tuple_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let mut is_variadic = false;
    if is_tuple && expand_variadic {
        let items = match left_type.getattr("items") {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        let seq = match items.downcast::<PyList>() {
            Ok(s) => s,
            Err(_) => return Ok(None),
        };
        for item in seq.iter() {
            match item.is_instance(unpack_cls) {
                Ok(true) => {
                    is_variadic = true;
                    break;
                }
                Ok(false) => {}
                Err(_) => return Ok(None),
            }
        }
    }
    let is_union = match left_type.is_instance(union_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };

    let mut in_checked = false;
    if is_tuple {
        in_checked = match chk.call_method0("in_checked_function") {
            Ok(v) => match v.extract() {
                Ok(b) => b,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };
    }

    let is_typeddict = match left_type.is_instance(tdict_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };

    let mut is_type_obj = false;
    let mut is_enum = false;
    let mut to_has_type_vars = false;
    let mut to_is_builtin_type = false;
    let is_function_like = match left_type.is_instance(func_like_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    if is_function_like {
        is_type_obj = match left_type.call_method0("is_type_obj") {
            Ok(v) => match v.extract() {
                Ok(b) => b,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        };
        if is_type_obj {
            let type_object = match left_type.call_method0("type_object") {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
            is_enum = match type_object.getattr("is_enum") {
                Ok(v) => match v.is_true() {
                    Ok(b) => b,
                    Err(_) => return Ok(None),
                },
                Err(_) => return Ok(None),
            };
            if !is_enum {
                to_has_type_vars = match type_object.getattr("type_vars") {
                    Ok(v) => match v.is_true() {
                        Ok(b) => b,
                        Err(_) => return Ok(None),
                    },
                    Err(_) => return Ok(None),
                };
                let to_fullname: String = match type_object.getattr("fullname") {
                    Ok(v) => match v.extract() {
                        Ok(s) => s,
                        Err(_) => return Ok(None),
                    },
                    Err(_) => return Ok(None),
                };
                to_is_builtin_type = to_fullname == "builtins.type";
            }
        }
    }

    let is_typevar = match left_type.is_instance(typevar_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };

    let mut left_fullname: Option<String> = None;
    let is_instance = match left_type.is_instance(instance_cls) {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    if is_instance {
        let tinfo = match left_type.getattr("type") {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        left_fullname = Some(match tinfo.getattr("fullname") {
            Ok(v) => match v.extract() {
                Ok(s) => s,
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        });
    }

    Ok(Some(classify_index_with_type(
        is_tuple,
        is_variadic,
        expand_variadic,
        is_union,
        in_checked,
        is_typeddict,
        is_function_like,
        is_type_obj,
        is_enum,
        to_has_type_vars,
        to_is_builtin_type,
        is_typevar,
        is_instance,
        left_fullname.as_deref(),
    )))
}

#[cfg(test)]
mod classify_index_with_type_tests {

    #[allow(clippy::too_many_arguments)]
    fn classify(
        is_tuple: bool,
        is_variadic: bool,
        expand_variadic: bool,
        is_union: bool,
        in_checked: bool,
        is_typeddict: bool,
        is_function_like: bool,
        is_type_obj: bool,
        is_enum: bool,
        to_has_type_vars: bool,
        to_is_builtin_type: bool,
        is_typevar: bool,
        is_instance: bool,
        left_fullname: Option<&str>,
    ) -> i64 {
        super::classify_index_with_type(
            is_tuple,
            is_variadic,
            expand_variadic,
            is_union,
            in_checked,
            is_typeddict,
            is_function_like,
            is_type_obj,
            is_enum,
            to_has_type_vars,
            to_is_builtin_type,
            is_typevar,
            is_instance,
            left_fullname,
        )
    }

    #[test]
    fn test_variadic_tuple_normalize() {
        assert_eq!(
            classify(
                true, true, true, false, true, false, false, false, false, false, false, false,
                false, None
            ),
            0
        );
        // Second pass (expand_variadic=false) does not re-normalize.
        assert_eq!(
            classify(
                true, true, false, false, true, false, false, false, false, false, false, false,
                false, None
            ),
            2
        );
    }

    #[test]
    fn test_union_first() {
        assert_eq!(
            classify(
                false, false, true, true, true, false, false, false, false, false, false, false,
                false, None
            ),
            1
        );
    }

    #[test]
    fn test_union_beats_variadic_tuple_when_not_expanding() {
        // A non-variadic flag with expand disabled still hits union only
        // when is_union is set; tuple wins otherwise.
        assert_eq!(
            classify(
                true, false, true, false, true, false, false, false, false, false, false, false,
                false, None
            ),
            2
        );
    }

    #[test]
    fn test_tuple_requires_checked_function() {
        assert_eq!(
            classify(
                true, false, true, false, true, false, false, false, false, false, false, false,
                false, None
            ),
            2
        );
        // Not in a checked function: falls to the second if-chain tail.
        assert_eq!(
            classify(
                true, false, true, false, false, false, false, false, false, false, false, false,
                false, None
            ),
            8
        );
    }

    #[test]
    fn test_typeddict() {
        assert_eq!(
            classify(
                false, false, true, false, true, true, false, false, false, false, false, false,
                false, None
            ),
            3
        );
    }

    #[test]
    fn test_enum_type_obj() {
        assert_eq!(
            classify(
                false, false, true, false, true, false, true, true, true, false, false, false,
                false, None
            ),
            4
        );
    }

    #[test]
    fn test_generic_alias_type_vars() {
        assert_eq!(
            classify(
                false, false, true, false, true, false, true, true, false, true, false, false,
                false, None
            ),
            5
        );
        assert_eq!(
            classify(
                false, false, true, false, true, false, true, true, false, false, true, false,
                false, None
            ),
            5
        );
    }

    #[test]
    fn test_type_obj_falls_through() {
        // A type object that is neither enum nor generic-alias reaches the
        // second if-chain (and lands on __getitem__ for a FunctionLike).
        assert_eq!(
            classify(
                false, false, true, false, true, false, true, true, false, false, false, false,
                false, None
            ),
            8
        );
        // Non-type-object FunctionLike also falls through.
        assert_eq!(
            classify(
                false, false, true, false, true, false, true, false, false, false, false, false,
                false, None
            ),
            8
        );
    }

    #[test]
    fn test_typevar() {
        assert_eq!(
            classify(
                false, false, true, false, true, false, false, false, false, false, false, true,
                false, None
            ),
            6
        );
    }

    #[test]
    fn test_special_form_instance() {
        assert_eq!(
            classify(
                false,
                false,
                true,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                Some("typing._SpecialForm")
            ),
            7
        );
    }

    #[test]
    fn test_getitem_tail() {
        assert_eq!(
            classify(
                false, false, true, false, true, false, false, false, false, false, false, false,
                false, None
            ),
            8
        );
        assert_eq!(
            classify(
                false,
                false,
                true,
                false,
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                true,
                Some("builtins.list")
            ),
            8
        );
    }
}

// ---------------------------------------------------------------------------
// classify_check_arg
// ---------------------------------------------------------------------------

/// Decision tags for `check_arg`; must match `NATIVE_CHECK_ARG_*`
/// in mypy/checkexpr.py.
const CHECK_ARG_DELETED: i64 = 0;
const CHECK_ARG_ABSTRACT_ONLY: i64 = 1;
const CHECK_ARG_INCOMPATIBLE: i64 = 2;
const CHECK_ARG_PASS: i64 = 3;

/// Pure 4-way dispatch of `ExpressionChecker.check_arg`
/// (checkexpr.py:4161-4204). `caller` is the decoded wire form of the
/// proper caller type; `is_subtype` and `has_abstract_type_part` are
/// precomputed by the Python shim from the already-native subtype
/// resolver and `rust_has_abstract_type` (the Tuple-x-Tuple fold stays
/// Python-side). Branch order mirrors the Python body exactly:
/// DeletedType first, then the abstract-part gate, then the subtype
/// check, then the implicit pass. Python's body short-circuits -- it
/// never evaluates `is_subtype` / `has_abstract_type_part` on a
/// DeletedType caller -- but both inputs are pure functions of the
/// types, so eager evaluation is value-preserving; the shim documents
/// this and still falls back if the eager computation raises.
fn classify_check_arg(caller: &Type, is_subtype: bool, has_abstract_type_part: bool) -> i64 {
    if matches!(caller, Type::DeletedType { .. }) {
        return CHECK_ARG_DELETED;
    }
    if has_abstract_type_part {
        return CHECK_ARG_ABSTRACT_ONLY;
    }
    if !is_subtype {
        return CHECK_ARG_INCOMPATIBLE;
    }
    CHECK_ARG_PASS
}

/// `#[pyfunction]` entry for `ExpressionChecker.check_arg`
/// (checkexpr.py:4161-4204). Rust decides ONLY the tag from the wire
/// caller type plus the two Python-computed booleans; the shim applies
/// all side effects (deleted_as_rvalue / concrete_only_call /
/// incompatible_argument + note + check_possible_missing_await).
/// Defers (`None`) on undecodable wire bytes; never otherwise.
#[pyfunction]
#[pyo3(signature = (caller_type_bytes, is_subtype, has_abstract_type_part))]
pub(crate) fn rust_classify_check_arg(
    caller_type_bytes: &[u8],
    is_subtype: bool,
    has_abstract_type_part: bool,
) -> PyResult<Option<i64>> {
    let caller = match crate::checkmember::decode_type(caller_type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(Some(classify_check_arg(
        &caller,
        is_subtype,
        has_abstract_type_part,
    )))
}

#[cfg(test)]
mod classify_check_arg_tests {
    use super::*;

    #[test]
    fn test_deleted_type_wins() {
        // DeletedType is checked first: the two booleans are irrelevant.
        let deleted = Type::DeletedType { source: None };
        assert_eq!(
            classify_check_arg(&deleted, false, false),
            CHECK_ARG_DELETED
        );
        assert_eq!(classify_check_arg(&deleted, false, true), CHECK_ARG_DELETED);
        assert_eq!(classify_check_arg(&deleted, true, true), CHECK_ARG_DELETED);
    }

    #[test]
    fn test_abstract_only() {
        let any = Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(
            classify_check_arg(&any, true, true),
            CHECK_ARG_ABSTRACT_ONLY
        );
    }

    #[test]
    fn test_incompatible() {
        let any = Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(
            classify_check_arg(&any, false, false),
            CHECK_ARG_INCOMPATIBLE
        );
    }

    #[test]
    fn test_pass() {
        let any = Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(classify_check_arg(&any, true, false), CHECK_ARG_PASS);
    }

    #[test]
    fn test_abstract_beats_incompatible() {
        // The Python elif-chain never reaches is_subtype when the
        // abstract part fires.
        let any = Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(
            classify_check_arg(&any, false, true),
            CHECK_ARG_ABSTRACT_ONLY
        );
    }
}
// classify_check_boolean_op
// ---------------------------------------------------------------------------

/// Decision tags for `check_boolean_op`; must match
/// `NATIVE_CHECK_BOOLEAN_OP_*` in mypy/checkexpr.py.
const CHECK_BOOLEAN_OP_RETURN_LEFT: i64 = 0;
const CHECK_BOOLEAN_OP_RETURN_RIGHT: i64 = 1;
const CHECK_BOOLEAN_OP_UNION: i64 = 2;
const CHECK_BOOLEAN_OP_UNINHABITED: i64 = 3;

/// Map-selection tags (which branch built `left_map`/`right_map`);
/// must match `NATIVE_CHECK_BOOLEAN_OP_MAP_*` in mypy/checkexpr.py.
const CHECK_BOOLEAN_OP_MAP_RIGHT_ALWAYS: i64 = 0;
const CHECK_BOOLEAN_OP_MAP_RIGHT_UNREACHABLE: i64 = 1;
const CHECK_BOOLEAN_OP_MAP_AND: i64 = 2;
const CHECK_BOOLEAN_OP_MAP_OR: i64 = 3;

/// Decision core of `ExpressionChecker.check_boolean_op`
/// (checkexpr.py:6002-6077), pure over the decoded map values plus live
/// scalar flags. Returns `(map_tag, left_unreachable, right_unreachable,
/// result_tag)`; `None` defers to the pure-Python body.
///
/// * Map tag: 4-way from `e.op` x `e.right_always` x `e.right_unreachable`
///   (the `if e.right_always / elif e.right_unreachable / elif e.op`
///   arrangement in the Python body).
/// * Reachability: `mypy.checker.is_unreachable_map` — the shim already
///   filtered `None` values (never `UninhabitedType`). A `TypeAliasType`
///   value may unwrap to `UninhabitedType` under `get_proper_type`,
///   which the wire cannot resolve -> defer.
/// * Result tail: `restricted_left_type = false_only/true_only(
///   expanded_left)` from the live `can_be_true`/`can_be_false` flags
///   (steps 1-2) plus the typeops leaf kernels (dunder lookup +
///   final/enum via the live resolver). A `UnionType` expanded-left
///   recursion reads live per-item flags the wire does not carry, so
///   the shim precomputes its Uninhabited verdict and passes it in
///   (`restricted_uninhabited`, issue #1161); a missing verdict defers.
#[allow(clippy::too_many_arguments)]
pub(crate) fn classify_check_boolean_op(
    py: Python<'_>,
    op_is_and: bool,
    right_always: bool,
    right_unreachable: bool,
    left_map_values: &[Type],
    right_map_values: &[Type],
    expanded_left: &Type,
    can_be_true: bool,
    can_be_false: bool,
    strict_optional: bool,
    restricted_uninhabited_arg: Option<bool>,
    resolver: &NativeTypeResolver,
) -> Option<(i64, bool, bool, i64)> {
    let map_tag = if right_always {
        CHECK_BOOLEAN_OP_MAP_RIGHT_ALWAYS
    } else if right_unreachable {
        CHECK_BOOLEAN_OP_MAP_RIGHT_UNREACHABLE
    } else if op_is_and {
        CHECK_BOOLEAN_OP_MAP_AND
    } else {
        CHECK_BOOLEAN_OP_MAP_OR
    };

    let any_unreachable = |values: &[Type]| -> Option<bool> {
        let mut unreachable = false;
        for v in values {
            match v {
                Type::TypeAliasType { .. } => return None,
                Type::UninhabitedType { .. } => unreachable = true,
                _ => {}
            }
        }
        Some(unreachable)
    };
    let left_unreachable = any_unreachable(left_map_values)?;
    let right_unreachable = any_unreachable(right_map_values)?;

    if left_unreachable && right_unreachable {
        return Some((
            map_tag,
            left_unreachable,
            right_unreachable,
            CHECK_BOOLEAN_OP_UNINHABITED,
        ));
    }
    if right_unreachable {
        return Some((
            map_tag,
            left_unreachable,
            right_unreachable,
            CHECK_BOOLEAN_OP_RETURN_LEFT,
        ));
    }
    if left_unreachable {
        return Some((
            map_tag,
            left_unreachable,
            right_unreachable,
            CHECK_BOOLEAN_OP_RETURN_RIGHT,
        ));
    }

    // Tail: restricted_left_type = false_only/true_only(expanded_left);
    // result_is_left = not expanded.can_be_true (and) / not can_be_false (or).
    // Union Uninhabited verdict is precomputed Python-side (issue #1161).
    let restricted_uninhabited: bool = if op_is_and {
        if !can_be_false {
            // false_only: UninhabitedType under strict_optional, else NoneType.
            strict_optional
        } else if !can_be_true {
            matches!(expanded_left, Type::UninhabitedType { .. })
        } else {
            match expanded_left {
                Type::TypeAliasType { .. } => return None,
                Type::UnionType { .. } => {
                    // Python precomputes false_only(union) -> Uninhabited?
                    // and hands the verdict in (issue #1161).
                    restricted_uninhabited_arg?
                }
                _ => {
                    matches!(
                        crate::typeops::false_only(py, expanded_left, strict_optional, resolver)?,
                        crate::typeops::TruthinessResult::Uninhabited
                    )
                }
            }
        }
    } else {
        // true_only: step 1 is always UninhabitedType.
        if !can_be_true {
            true
        } else if !can_be_false {
            matches!(expanded_left, Type::UninhabitedType { .. })
        } else {
            match expanded_left {
                Type::TypeAliasType { .. } => return None,
                Type::UnionType { .. } => {
                    // Python precomputes true_only(union) -> Uninhabited?
                    // and hands the verdict in (issue #1161).
                    restricted_uninhabited_arg?
                }
                _ => {
                    matches!(
                        crate::typeops::true_only(py, expanded_left, resolver)?,
                        crate::typeops::TruthinessResult::Uninhabited
                    )
                }
            }
        }
    };

    let result_tag = if restricted_uninhabited {
        CHECK_BOOLEAN_OP_RETURN_RIGHT
    } else {
        let result_is_left = if op_is_and {
            !can_be_true
        } else {
            !can_be_false
        };
        if result_is_left {
            CHECK_BOOLEAN_OP_RETURN_LEFT
        } else {
            CHECK_BOOLEAN_OP_UNION
        }
    };
    Some((map_tag, left_unreachable, right_unreachable, result_tag))
}

/// `#[pyfunction]` entry for the `check_boolean_op` decision head
/// (issue #1049). `left_map_values`/`right_map_values` are the
/// wire-serialized map values; `expanded_left_bytes` is one
/// serialization of the expanded left operand type.
/// `restricted_uninhabited` is the Python-precomputed
/// false_only/true_only(union) -> Uninhabited verdict for a union
/// expanded-left (issue #1161); `None` defers that bucket. Defers
/// (`None`) on any decode failure.
#[pyfunction]
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
#[pyo3(signature = (
    op_is_and,
    right_always,
    right_unreachable,
    left_map_values,
    right_map_values,
    expanded_left_bytes,
    can_be_true,
    can_be_false,
    strict_optional,
    restricted_uninhabited,
    resolver,
))]
pub(crate) fn rust_classify_check_boolean_op(
    py: Python<'_>,
    op_is_and: bool,
    right_always: bool,
    right_unreachable: bool,
    left_map_values: Vec<Vec<u8>>,
    right_map_values: Vec<Vec<u8>>,
    expanded_left_bytes: &[u8],
    can_be_true: bool,
    can_be_false: bool,
    strict_optional: bool,
    restricted_uninhabited: Option<bool>,
    resolver: &NativeTypeResolver,
) -> Option<(i64, bool, bool, i64)> {
    let expanded_left = decode_type(expanded_left_bytes)?;
    let mut left_values: Vec<Type> = Vec::with_capacity(left_map_values.len());
    for v in &left_map_values {
        left_values.push(decode_type(v)?);
    }
    let mut right_values: Vec<Type> = Vec::with_capacity(right_map_values.len());
    for v in &right_map_values {
        right_values.push(decode_type(v)?);
    }
    classify_check_boolean_op(
        py,
        op_is_and,
        right_always,
        right_unreachable,
        &left_values,
        &right_values,
        &expanded_left,
        can_be_true,
        can_be_false,
        strict_optional,
        restricted_uninhabited,
        resolver,
    )
}

#[cfg(test)]
mod classify_check_boolean_op_tests {
    use super::*;
    use crate::typeinfo::NativeTypeResolver;

    /// Initialize the embedded interpreter, then run with the GIL.
    fn with_py<T>(f: impl FnOnce(Python<'_>) -> T) -> T {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(f)
    }

    fn empty_resolver() -> NativeTypeResolver {
        NativeTypeResolver::new(Default::default(), Default::default())
    }

    fn uninhabited() -> Type {
        Type::UninhabitedType { ambiguous: false }
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn int_instance() -> Type {
        Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn map_right_always_left_unreachable() {
        // e.right_always: left_map = {left: Uninhabited}, right_map = {}.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                true,
                false,
                &[uninhabited()],
                &[],
                &any_type(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.0, CHECK_BOOLEAN_OP_MAP_RIGHT_ALWAYS);
        assert!(r.1 && !r.2);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_RIGHT);
    }

    #[test]
    fn map_right_unreachable_returns_left() {
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                false,
                false,
                true,
                &[],
                &[uninhabited()],
                &any_type(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.0, CHECK_BOOLEAN_OP_MAP_RIGHT_UNREACHABLE);
        assert!(!r.1 && r.2);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_LEFT);
    }

    #[test]
    fn map_and_non_unreachable_unions() {
        // and: ordinary find_isinstance_check maps, expanded int (leaf ->
        // LiteralType(0), never Uninhabited), can_be_true -> UNION.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[int_instance()],
                &[],
                &int_instance(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.0, CHECK_BOOLEAN_OP_MAP_AND);
        assert!(!r.1 && !r.2);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_UNION);
    }

    #[test]
    fn map_or_tag() {
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                false,
                false,
                false,
                &[],
                &[],
                &any_type(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.0, CHECK_BOOLEAN_OP_MAP_OR);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_UNION);
    }

    #[test]
    fn both_unreachable_returns_uninhabited() {
        // Both find_isinstance_check maps carry an UninhabitedType value.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[uninhabited()],
                &[uninhabited()],
                &any_type(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert!(r.1 && r.2);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_UNINHABITED);
    }

    #[test]
    fn left_unreachable_only_returns_right() {
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                false,
                false,
                false,
                &[uninhabited()],
                &[any_type()],
                &any_type(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert!(r.1 && !r.2);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_RIGHT);
    }

    #[test]
    fn restricted_uninhabited_strict_returns_right() {
        // and: can_be_false=False under strict_optional -> false_only
        // yields UninhabitedType -> return right_type.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[any_type()],
                &[],
                &any_type(),
                true,
                false,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_RIGHT);
    }

    #[test]
    fn restricted_none_type_non_strict_returns_left() {
        // Same flags but --no-strict-optional: false_only yields NoneType,
        // not UninhabitedType; result_is_left = not can_be_true -> left.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[any_type()],
                &[],
                &any_type(),
                false,
                false,
                false,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_LEFT);
    }

    #[test]
    fn result_is_left_or_never_false() {
        // or: can_be_false=False -> true_only keeps t (not Uninhabited);
        // result_is_left = not can_be_false -> return left_type.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                false,
                false,
                false,
                &[any_type()],
                &[],
                &any_type(),
                true,
                false,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_LEFT);
    }

    #[test]
    fn true_only_flag_step_returns_uninhabited_tail() {
        // or with can_be_true=False and expanded not UninhabitedType on
        // the wire: true_only step 1 is unconditionally UninhabitedType
        // -> RETURN_RIGHT.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                false,
                false,
                false,
                &[],
                &[],
                &int_instance(),
                false,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_RIGHT);
    }

    #[test]
    fn union_expanded_defers() {
        // Union recursion reads live per-item flags the wire does not
        // carry -> the whole call defers.
        let union = Type::UnionType {
            items: vec![int_instance(), any_type()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[],
                &[],
                &union,
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        });
        assert!(r.is_none());
    }

    #[test]
    fn literal_expanded_defers() {
        // Literals need the live TypeInfo unwrap in the leaf -> defer.
        let lit = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(true),
        };
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[],
                &[],
                &lit,
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        });
        assert!(r.is_none());
    }

    #[test]
    fn alias_map_value_defers() {
        // A TypeAliasType map value may unwrap to UninhabitedType under
        // get_proper_type; the wire cannot resolve it -> defer.
        let alias = Type::TypeAliasType {
            type_ref: "mod.TA".to_string(),
            args: vec![],
        };
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[alias],
                &[],
                &any_type(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        });
        assert!(r.is_none());
    }

    fn union_type() -> Type {
        Type::UnionType {
            items: vec![int_instance(), any_type()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }
    }

    #[test]
    fn union_expanded_left_and_restricted_uninhabited_returns_right() {
        // and, both flags true: Python's false_only(union) verdict is
        // Uninhabited -> RETURN_RIGHT without deferring (issue #1161).
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[],
                &[],
                &union_type(),
                true,
                true,
                true,
                Some(true),
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.0, CHECK_BOOLEAN_OP_MAP_AND);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_RIGHT);
    }

    #[test]
    fn union_expanded_left_and_restricted_inhabited_unions() {
        // and, restricted left inhabited, can_be_true -> UNION tail.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[],
                &[],
                &union_type(),
                true,
                true,
                true,
                Some(false),
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.3, CHECK_BOOLEAN_OP_UNION);
    }

    #[test]
    fn union_expanded_left_or_restricted_uninhabited_returns_right() {
        // or, both flags true, true_only(union) verdict Uninhabited.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                false,
                false,
                false,
                &[],
                &[],
                &union_type(),
                true,
                true,
                true,
                Some(true),
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.0, CHECK_BOOLEAN_OP_MAP_OR);
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_RIGHT);
    }

    #[test]
    fn union_expanded_left_or_restricted_inhabited_unions() {
        // or, restricted left inhabited, can_be_false -> UNION tail.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                false,
                false,
                false,
                &[],
                &[],
                &union_type(),
                true,
                true,
                true,
                Some(false),
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.3, CHECK_BOOLEAN_OP_UNION);
    }

    #[test]
    fn union_expanded_left_missing_verdict_defers() {
        // Without the Python-precomputed verdict the union arm still defers.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[],
                &[],
                &union_type(),
                true,
                true,
                true,
                None,
                &empty_resolver(),
            )
        });
        assert!(r.is_none());
    }

    #[test]
    fn union_expanded_left_and_never_true_returns_left() {
        // and, !can_be_true: restricted step decides by the Uninhabited
        // match, no verdict needed -> result_is_left -> RETURN_LEFT.
        let r = with_py(|py| {
            classify_check_boolean_op(
                py,
                true,
                false,
                false,
                &[],
                &[],
                &union_type(),
                false,
                true,
                true,
                None,
                &empty_resolver(),
            )
        })
        .unwrap();
        assert_eq!(r.3, CHECK_BOOLEAN_OP_RETURN_LEFT);
    }
}

// ---------------------------------------------------------------------------
// compute_arg_context_indices
// ---------------------------------------------------------------------------

/// Pure index-decision core of `infer_arg_types_in_context` (issue #1064):
/// maps each actual-arg index to its formal index (`-1` = no context), skips
/// star args, later formals win. Returns `None` on malformed input.
pub(crate) fn compute_arg_context_indices_inner(
    arg_kinds: &[i64],
    formal_to_actual: &[Vec<i64>],
    args_len: i64,
    callee_arg_types_len: i64,
) -> Option<Vec<i64>> {
    if args_len < 0 || callee_arg_types_len < 0 {
        return None;
    }
    if arg_kinds.len() as i64 != args_len {
        return None;
    }
    let n_args = args_len as usize;
    let formal_len = callee_arg_types_len as usize;
    let mut out = vec![-1i64; n_args];
    for (fi, actuals) in formal_to_actual.iter().enumerate() {
        if fi >= formal_len {
            return None;
        }
        for &ai in actuals {
            if ai < 0 {
                return None;
            }
            let ai = ai as usize;
            if ai >= n_args {
                return None;
            }
            let kind = arg_kinds[ai];
            if kind == ARG_STAR || kind == ARG_STAR2 {
                continue;
            }
            out[ai] = fi as i64;
        }
    }
    Some(out)
}

/// Python seam: see `compute_arg_context_indices_inner`. Args arrive as
/// plain scalars (`arg_kinds` as the integer `ArgKind.value`s), so no wire
/// serializer is involved. Never defers on well-formed input.
#[pyfunction]
#[pyo3(signature = (arg_kinds, formal_to_actual, args_len, callee_arg_types_len))]
pub(crate) fn rust_compute_arg_context_indices(
    arg_kinds: Vec<i64>,
    formal_to_actual: Vec<Vec<i64>>,
    args_len: i64,
    callee_arg_types_len: i64,
) -> PyResult<Option<Vec<i64>>> {
    Ok(compute_arg_context_indices_inner(
        &arg_kinds,
        &formal_to_actual,
        args_len,
        callee_arg_types_len,
    ))
}

#[cfg(test)]
mod compute_arg_context_indices_tests {
    use super::compute_arg_context_indices_inner;

    #[test]
    fn positional_one_to_one() {
        let out = compute_arg_context_indices_inner(&[0, 0], &[vec![0], vec![1]], 2, 2);
        assert_eq!(out, Some(vec![0, 1]));
    }

    #[test]
    fn star_args_skipped() {
        // ARG_STAR (2) and ARG_STAR2 (4) actuals get no context.
        let out = compute_arg_context_indices_inner(&[0, 2, 4, 0], &[vec![0, 1, 2, 3]], 4, 1);
        assert_eq!(out, Some(vec![0, -1, -1, 0]));
    }

    #[test]
    fn empty_formal_to_actual() {
        let out = compute_arg_context_indices_inner(&[0, 0], &[], 2, 3);
        assert_eq!(out, Some(vec![-1, -1]));
    }

    #[test]
    fn no_context_tail() {
        // Actuals not covered by any formal stay -1.
        let out = compute_arg_context_indices_inner(&[0, 0, 0], &[vec![1]], 3, 1);
        assert_eq!(out, Some(vec![-1, 0, -1]));
    }

    #[test]
    fn later_formal_wins() {
        // Python assigns in formal order, so a later formal overwrites.
        let out = compute_arg_context_indices_inner(&[0, 0], &[vec![0], vec![0]], 2, 2);
        assert_eq!(out, Some(vec![1, -1]));
    }

    #[test]
    fn actual_index_out_of_bounds_defers() {
        let out = compute_arg_context_indices_inner(&[0, 0], &[vec![2]], 2, 1);
        assert_eq!(out, None);
        let out = compute_arg_context_indices_inner(&[0, 0], &[vec![-1]], 2, 1);
        assert_eq!(out, None);
    }

    #[test]
    fn formal_index_out_of_bounds_defers() {
        // formal_to_actual longer than callee.arg_types -> malformed.
        let out = compute_arg_context_indices_inner(&[0, 0], &[vec![0], vec![1]], 2, 1);
        assert_eq!(out, None);
    }

    #[test]
    fn kinds_len_mismatch_defers() {
        let out = compute_arg_context_indices_inner(&[0], &[vec![0]], 2, 1);
        assert_eq!(out, None);
    }

    #[test]
    fn negative_lengths_defer() {
        let out = compute_arg_context_indices_inner(&[0], &[vec![0]], -1, 1);
        assert_eq!(out, None);
        let out = compute_arg_context_indices_inner(&[0], &[vec![0]], 1, -1);
        assert_eq!(out, None);
    }
}
