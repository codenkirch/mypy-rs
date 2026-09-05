//! Native port of `mypy/applytype.py` `apply_generic_arguments` and
//! `mypy/typevars.py` `has_no_typevars` (Stage 6c).
//!
//! `apply_generic_arguments` applies generic type arguments to a
//! `CallableType`, mirroring applytype.py:88-193. It builds an `id_to_type`
//! map from TypeVar ids to target types (validating each against the TypeVar's
//! constraints/bounds via `is_subtype`), then expands the callable's
//! arg_types, ret_type, type_guard, type_is, and instance_type using
//! `expand_type`.
//!
//! Deferred (return None):
//!   * `get_proper_type(type)` on a `TypeAliasType` — wire format has no
//!     alias target, so we can't expand. Defers via `get_target_type`.
//!   * `is_subtype` returns `None` — the constraint check hits an
//!     unsupported case. Defer so Python handles it.
//!   * `skip_unsatisfied=False` AND constraint fails — Python must report
//!     the error via the `report_incompatible_typevar_value` callback. Rust
//!     has no side-channel for error reporting, so defer.
//!   * `ParamSpec` in `callable.variables` — `expand_type` defers on
//!     ParamSpec, so `apply_generic_arguments` defers too.
//!   * `UnpackType` var_arg — variadic expansion is deferred in
//!     `expand_type`.
//!   * `TypeAliasType` in `orig_types` — `get_target_type` calls
//!     `get_proper_type` which defers.
//!
//! `has_no_typevars` checks if `typ == erase_typevars(typ)`. Mirrors
//! typevars.py:77-84. Uses Type's `PartialEq` derive. Defers when
//! `erase_typevars` defers (TypeAliasType, UnboundType).

use std::collections::HashMap;

use pyo3::prelude::*;

use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::NativeTypeResolver;
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

// TypeOfAny.from_omitted_generics = 4 (types.py:228).
const TYPE_OF_ANY_FROM_OMITTED_GENERICS: i64 = 4;
// ParamSpecFlavor.BARE = 0 (types.py:764).
#[allow(dead_code)]
const PARAM_SPEC_FLAVOR_BARE: i64 = 0;
const ARG_STAR: i64 = 2;
#[allow(dead_code)]
const ARG_STAR2: i64 = 4;

// Key for typevar identity: `(raw_id, meta_level, namespace)`. Mirrors
// `TypeVarId.__eq__` (types.py). meta_level is wire-serialized only for
// TypeVarType; ParamSpec/TypeVarTuple keys use 0.
type EnvKey = (i64, i64, String);

// ---------------------------------------------------------------------------
// apply_generic_arguments
// ---------------------------------------------------------------------------

/// `#[pyfunction]` entry for `apply_generic_arguments`. The Python-side
/// shim (mypy/applytype.py) calls this with the serialized `callable`
/// blob, a list of (None | serialized Type) for `orig_types`, and the
/// `NativeTypeResolver` pyclass.
///
/// Returns `None` (Python `None`) when Rust doesn't handle the case;
/// `Some(bytes)` otherwise, holding a wire-format CallableType blob the
/// shim decodes via `read_type`.
///
/// The `orig_types` wire format: count (bare int) + for each entry: a
/// 0/1 byte (0 = None, 1 = present) followed by a Type blob if present.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_apply_generic_arguments(
    resolver: &NativeTypeResolver,
    callable_bytes: &[u8],
    orig_types_bytes: &[u8],
    skip_unsatisfied: bool,
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let callable = decode_type(callable_bytes)?;
    let orig_types = decode_optional_type_list(orig_types_bytes)?;
    let result = apply_generic_arguments_inner(
        &callable,
        &orig_types,
        skip_unsatisfied,
        strict_optional,
        resolver.resolver(),
        resolver.alias_resolver(),
    )?;
    encode_type(&result)
}

/// Core logic for `apply_generic_arguments`. Mirrors applytype.py:88-193.
///
/// `r` drives the subtype checks; `aliases` expands alias upper bounds
/// (an empty resolver defers on alias bounds, mirroring a missing
/// snapshot).
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_generic_arguments_inner(
    callable: &Type,
    orig_types: &[Option<Type>],
    skip_unsatisfied: bool,
    strict_optional: bool,
    r: &crate::typeinfo::TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<Type> {
    let callable = if let Type::CallableType { .. } = callable {
        callable
    } else {
        return None;
    };
    // Deconstruct the CallableType.
    let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        ref arg_types,
        ref arg_kinds,
        ref arg_names,
        ref ret_type,
        ref name,
        ref variables,
        ref type_guard,
        ref type_is,
        ..
    } = callable
    else {
        unreachable!()
    };

    // applytype.py:105: tvars = callable.variables
    // applytype.py:106: assert len(orig_types) <= len(tvars)
    if orig_types.len() > variables.len() {
        return None;
    }

    // TypeVarTuple substitutions have splice/pack semantics that expand_type
    // env does not model. Defer when a TypeVarTuple variable receives a
    // TypeVarTuple or Tuple target.
    let has_tvartuple_var = variables
        .iter()
        .any(|v| matches!(v, Type::TypeVarTupleType { .. }));
    for t in orig_types.iter().flatten() {
        if matches!(t, Type::TypeVarTupleType { .. }) {
            return None;
        }
        if has_tvartuple_var {
            let is_tuple = matches!(t, Type::TupleType { .. })
                || matches!(t, Type::Instance { type_ref, .. } if type_ref == "builtins.tuple");
            if is_tuple {
                return None;
            }
        }
    }

    // Build id_to_type map. applytype.py:110-127.
    let mut id_to_type: HashMap<EnvKey, Type> = HashMap::new();
    for (tvar, type_opt) in variables.iter().zip(orig_types.iter()) {
        let target = match type_opt {
            None => continue,
            Some(t) => t,
        };
        let target_type = get_target_type(
            tvar,
            target,
            skip_unsatisfied,
            r,
            aliases,
            &id_to_type,
            strict_optional,
        )?;
        if let Some(tt) = target_type {
            let key = typevar_id_key(tvar)?;
            id_to_type.insert(key, tt);
        }
    }

    // applytype.py:131-141: ParamSpec special-casing. If callable has a
    // ParamSpec and it's in id_to_type, expand the whole callable.
    if let Some(ps) = find_param_spec(variables) {
        let key = typevar_id_key(ps)?;
        if id_to_type.contains_key(&key) {
            // ParamSpec expansion: expand_type on the whole callable.
            // expand_type defers on ParamSpec, so this returns None.
            return None;
        }
    }

    // applytype.py:143-149: UnpackType var_arg special-casing. If the
    // var_arg's type is an UnpackType, expand the whole callable.
    if has_unpack_var_arg(arg_types, arg_kinds) {
        let expanded =
            crate::expandtype::expand_type_inner(callable, &id_to_type, strict_optional)?;
        return Some(expanded);
    }

    // applytype.py:150-153: expand arg_types individually.
    let mut new_arg_types = Vec::with_capacity(arg_types.len());
    for at in arg_types {
        new_arg_types.push(crate::expandtype::expand_type_inner(
            at,
            &id_to_type,
            strict_optional,
        )?);
    }

    // applytype.py:155-163: expand type_guard and type_is.
    let new_type_guard = match type_guard {
        Some(tg) => Some(Box::new(crate::expandtype::expand_type_inner(
            tg,
            &id_to_type,
            strict_optional,
        )?)),
        None => None,
    };
    let new_type_is = match type_is {
        Some(ti) => Some(Box::new(crate::expandtype::expand_type_inner(
            ti,
            &id_to_type,
            strict_optional,
        )?)),
        None => None,
    };

    // applytype.py:165-193: remaining_tvars + instance_type + ret_type.
    let mut remaining_tvars = Vec::new();
    for tv in variables {
        let key = typevar_id_key(tv)?;
        if id_to_type.contains_key(&key) {
            continue;
        }
        if !has_default(tv) {
            remaining_tvars.push(tv.clone());
            continue;
        }
        // Expand the TypeVar default. applytype.py:178.
        let expanded_tv = crate::expandtype::expand_type_inner(tv, &id_to_type, strict_optional)?;
        remaining_tvars.push(expanded_tv);
    }

    let new_instance_type = match instance_type {
        Some(it) => Some(Box::new(crate::expandtype::expand_type_inner(
            it,
            &id_to_type,
            strict_optional,
        )?)),
        None => None,
    };
    let new_ret_type = Box::new(crate::expandtype::expand_type_inner(
        ret_type,
        &id_to_type,
        strict_optional,
    )?);

    Some(Type::CallableType {
        fallback: fallback.clone(),
        instance_type: new_instance_type,
        is_ellipsis_args: *is_ellipsis_args,
        implicit: *implicit,
        is_bound: *is_bound,
        from_concatenate: *from_concatenate,
        imprecise_arg_kinds: *imprecise_arg_kinds,
        unpack_kwargs: *unpack_kwargs,
        from_type_type: *from_type_type,
        arg_types: new_arg_types,
        arg_kinds: arg_kinds.clone(),
        arg_names: arg_names.clone(),
        ret_type: new_ret_type,
        name: name.clone(),
        variables: remaining_tvars,
        type_guard: new_type_guard,
        type_is: new_type_is,
        special_sig: None,
    })
}

/// `get_target_type` (applytype.py:33-85). Validates a type against a
/// TypeVar's constraints/bounds, promoting subtype values to allowed
/// values.
///
/// Returns `Some(Some(type))` when the target type is determined.
/// Returns `Some(None)` when the type should be skipped (skip_unsatisfied
/// and constraint not met).
/// Returns `None` when Rust can't handle the case (defers to Python).
fn get_target_type(
    tvar: &Type,
    type_arg: &Type,
    skip_unsatisfied: bool,
    r: &crate::typeinfo::TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
    id_to_type: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Option<Type>> {
    // applytype.py:42: p_type = get_proper_type(type)
    // Defer on TypeAliasType (wire format has no alias target).
    let p_type = get_proper_type(type_arg)?;

    // applytype.py:254-256: an ambiguous UninhabitedType with a real
    // tvar default expands the default gradually through id_to_type.
    // A no-default tvar or non-ambiguous Never falls through below.
    if let Type::UninhabitedType { ambiguous: true } = p_type {
        if let Some(default) = default_of(tvar) {
            return Some(Some(crate::expandtype::expand_type_inner(
                default,
                id_to_type,
                strict_optional,
            )?));
        }
    }

    match tvar {
        Type::TypeVarType {
            name,
            values,
            upper_bound,
            ..
        } => {
            if values.is_empty() {
                // applytype.py:74-85: bound check.
                let bound = if name == "Self" {
                    // applytype.py:76-80: erase typevars in Self upper_bound.
                    crate::erase_typevars::erase_typevars_inner(upper_bound, None, &make_any())?
                } else {
                    upper_bound.as_ref().clone()
                };
                // The bound can be a TypeAliasType (issue #1260); expand it
                // through the alias snapshot. `type_arg` needs no expansion:
                // the prelude's `get_proper_type` deferred on alias args.
                let s_bound = crate::checkexpr_functions::get_proper_or_expand(&bound, aliases)?;
                match is_subtype(type_arg, &s_bound, &subtype_ctx(), r) {
                    Some(true) => Some(Some(type_arg.clone())),
                    Some(false) => {
                        if skip_unsatisfied {
                            Some(None)
                        } else {
                            // Must report error via callback. Defer.
                            None
                        }
                    }
                    None => None,
                }
            } else {
                // applytype.py:52-73: value constraint check.
                get_target_type_with_values(type_arg, &p_type, values, skip_unsatisfied, r)
            }
        }
        Type::ParamSpecType { .. } => {
            // applytype.py:46-47: ParamSpec returns type as-is.
            Some(Some(type_arg.clone()))
        }
        Type::TypeVarTupleType { .. } => {
            // applytype.py:48-49: TypeVarTuple returns type as-is.
            Some(Some(type_arg.clone()))
        }
        _ => None, // Not a TypeVarLike: defer.
    }
}

/// Value-constraint branch of `get_target_type` (applytype.py:52-73).
fn get_target_type_with_values(
    type_arg: &Type,
    p_type: &Type,
    values: &[Type],
    skip_unsatisfied: bool,
    r: &crate::typeinfo::TypeResolver,
) -> Option<Option<Type>> {
    // applytype.py:53-54: AnyType passes through.
    if matches!(p_type, Type::AnyType { .. }) {
        return Some(Some(type_arg.clone()));
    }
    // applytype.py:55-59: TypeVarType with values — check all values of
    // p_type are legal values of tvar.
    if let Type::TypeVarType {
        values: p_values, ..
    } = p_type
    {
        if !p_values.is_empty() {
            let all_legal = p_values
                .iter()
                .all(|v1| values.iter().any(|v| is_same_type(v, v1, r)));
            if all_legal {
                return Some(Some(type_arg.clone()));
            }
        }
    }
    // applytype.py:60-70: find matching values (narrowest).
    let mut matching: Vec<Type> = Vec::new();
    for value in values {
        match is_subtype(type_arg, value, &subtype_ctx(), r) {
            Some(true) => matching.push(value.clone()),
            Some(false) => {}
            None => {
                return None; // can't decide: defer.
            }
        }
    }
    if !matching.is_empty() {
        // applytype.py:65-70: select narrowest match.
        let mut best = matching[0].clone();
        for m in &matching[1..] {
            match is_subtype(m, &best, &subtype_ctx(), r) {
                Some(true) => best = m.clone(),
                Some(false) => {}
                None => return None,
            }
        }
        Some(Some(best))
    } else {
        // applytype.py:71-73: no match.
        if skip_unsatisfied {
            Some(None)
        } else {
            // Must report error. Defer.
            None
        }
    }
}

/// `is_same_type` (subtypes.py:303-336) — bidirectional proper subtype
/// check. Returns true only if both directions succeed and Rust decided
/// both. If either direction returns None, returns false (conservative).
fn is_same_type(a: &Type, b: &Type, resolver: &crate::typeinfo::TypeResolver) -> bool {
    let ctx = SubtypeContext {
        proper_subtype: true,
        ..Default::default()
    };
    matches!(is_subtype(a, b, &ctx, resolver), Some(true))
        && matches!(is_subtype(b, a, &ctx, resolver), Some(true))
}

/// `get_proper_type` — resolves TypeAliasType to its target. The wire
/// format has no alias target, so we return None (defer) for
/// TypeAliasType. For all other types, returns the type as-is (they are
/// already proper types on the wire).
fn get_proper_type(typ: &Type) -> Option<Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ.clone()),
    }
}

/// Extract the `(raw_id, meta_level, namespace)` key from a TypeVar-like type.
fn typevar_id_key(tvar: &Type) -> Option<EnvKey> {
    match tvar {
        Type::TypeVarType {
            raw_id,
            namespace,
            meta_level,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        Type::ParamSpecType {
            raw_id,
            namespace,
            meta_level,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        Type::TypeVarTupleType {
            raw_id,
            namespace,
            meta_level,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        _ => None,
    }
}

/// Find the first ParamSpecType in `variables` (callable.param_spec()).
/// Mirrors types.py:2460-2477.
fn find_param_spec(variables: &[Type]) -> Option<&Type> {
    variables
        .iter()
        .find(|v| matches!(v, Type::ParamSpecType { .. }))
}

/// Check if the callable has a var_arg whose type is an UnpackType.
/// Mirrors applytype.py:144-145 (`var_arg()` + `isinstance(var_arg.typ, UnpackType)`).
fn has_unpack_var_arg(arg_types: &[Type], arg_kinds: &[i64]) -> bool {
    for (typ, kind) in arg_types.iter().zip(arg_kinds.iter()) {
        if *kind == ARG_STAR && matches!(typ, Type::UnpackType { .. }) {
            return true;
        }
    }
    false
}

/// `TypeVarLikeType.has_default()` (types.py:634-636). Returns true when
/// the default is not `AnyType(from_omitted_generics)`.
fn has_default(tvar: &Type) -> bool {
    let default = match tvar {
        Type::TypeVarType { default, .. }
        | Type::ParamSpecType { default, .. }
        | Type::TypeVarTupleType { default, .. } => default.as_ref(),
        _ => return false,
    };
    // get_proper_type on the default: TypeAliasType would defer, but
    // defaults are typically AnyType or concrete types, not aliases.
    let p_default = get_proper_type(default).unwrap_or_else(|| default.clone());
    !matches!(
        p_default,
        Type::AnyType {
            type_of_any,
            ..
        } if type_of_any == TYPE_OF_ANY_FROM_OMITTED_GENERICS
    )
}

/// Borrow the raw `tvar.default` when the tvar has a real default
/// (`has_default`). Used by `get_target_type`'s ambiguous-UninhabitedType
/// branch to expand the default gradually (applytype.py:256). Returns
/// `None` for non-TypeVarLike types or the no-default sentinel.
fn default_of(tvar: &Type) -> Option<&Type> {
    if !has_default(tvar) {
        return None;
    }
    match tvar {
        Type::TypeVarType { default, .. }
        | Type::ParamSpecType { default, .. }
        | Type::TypeVarTupleType { default, .. } => Some(default.as_ref()),
        _ => None,
    }
}

/// Default SubtypeContext for constraint/bound checks in applytype.
/// Mirrors the default `is_subtype(left, bound)` call with no special flags.
fn subtype_ctx() -> SubtypeContext {
    SubtypeContext {
        proper_subtype: false,
        strict_optional: true,
        ..Default::default()
    }
}

/// Construct the AnyType(special_form) replacement used by erase_typevars.
fn make_any() -> Type {
    Type::AnyType {
        type_of_any: 12,
        source_any: None,
        missing_import_name: None,
    }
}

// ---------------------------------------------------------------------------
// get_target_type decision head (standalone seam)
// ---------------------------------------------------------------------------

// Tags returned by `rust_get_target_type`. The Python shim maps each tag
// onto the side effects / results of the pure-Python body
// (applytype.py:244-296).
const TAG_EXPAND_DEFAULT: i64 = 0;
const TAG_PASSTHROUGH: i64 = 1;
const TAG_MATCH: i64 = 2;
const TAG_SKIP: i64 = 3;
const TAG_REPORT: i64 = 4;

/// `#[pyfunction]` entry for the `get_target_type` decision head
/// (applytype.py:244-296). Rust owns the branch selection; the Python shim
/// computes the resolver-backed subtype/same-type booleans (already native)
/// and passes them in, then applies the side effects the chosen tag
/// implies (expand_type, report_incompatible_typevar_value, erase_typevars
/// for the Self upper bound). The result Type never crosses the seam.
///
/// Returns `(tag, match_index)` or `None` (defer to the pure-Python body).
/// `same_type_ok` is the cross-product conjunction for a TypeVarType
/// argument carrying values; `bound_ok` is `is_subtype(type, upper_bound)`
/// after the shim applied the Self erase; `value_subtypes` is
/// `is_subtype(type, value)` per tvar value; `narrow_matrix` is the
/// flattened `value_i <: value_j` matrix consulted only when more than
/// one value matches.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
#[pyo3(signature = (tvar_wire, type_wire, skip_unsatisfied, same_type_ok, bound_ok, value_subtypes, narrow_matrix))]
pub(crate) fn rust_get_target_type(
    tvar_wire: &[u8],
    type_wire: &[u8],
    skip_unsatisfied: bool,
    same_type_ok: Option<bool>,
    bound_ok: Option<bool>,
    value_subtypes: Option<Vec<bool>>,
    narrow_matrix: Option<Vec<bool>>,
) -> Option<(i64, i64)> {
    let tvar = decode_type(tvar_wire)?;
    let type_arg = decode_type(type_wire)?;
    get_target_type_tag(
        &tvar,
        &type_arg,
        skip_unsatisfied,
        same_type_ok,
        bound_ok,
        value_subtypes.as_deref(),
        narrow_matrix.as_deref(),
    )
}

/// Branch selection for `get_target_type`. Mirrors applytype.py:253-296.
fn get_target_type_tag(
    tvar: &Type,
    type_arg: &Type,
    skip_unsatisfied: bool,
    same_type_ok: Option<bool>,
    bound_ok: Option<bool>,
    value_subtypes: Option<&[bool]>,
    narrow_matrix: Option<&[bool]>,
) -> Option<(i64, i64)> {
    // Defer on TypeAliasType: Python's get_proper_type would expand the
    // alias, which the wire format cannot carry.
    if matches!(type_arg, Type::TypeAliasType { .. }) {
        return None;
    }
    // applytype.py:254-256: an ambiguous UninhabitedType with a real tvar
    // default expands the default (Python runs expand_type(tvar.default)).
    if let Type::UninhabitedType { ambiguous: true } = type_arg {
        if default_of(tvar).is_some() {
            return Some((TAG_EXPAND_DEFAULT, -1));
        }
    }
    match tvar {
        // applytype.py:257-260: ParamSpec / TypeVarTuple pass through.
        Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => Some((TAG_PASSTHROUGH, -1)),
        Type::TypeVarType { values, .. } => {
            if values.is_empty() {
                // applytype.py:285-295: bound check. The shim already
                // applied the Self upper-bound erase and computed
                // is_subtype(type, bound); Rust only arbitrates.
                let ok = bound_ok?;
                if ok {
                    Some((TAG_PASSTHROUGH, -1))
                } else if skip_unsatisfied {
                    Some((TAG_SKIP, -1))
                } else {
                    Some((TAG_REPORT, -1))
                }
            } else {
                // applytype.py:262-284: value-constraint check.
                get_target_type_values_tag(
                    type_arg,
                    values,
                    skip_unsatisfied,
                    same_type_ok,
                    value_subtypes,
                    narrow_matrix,
                )
            }
        }
        _ => None, // Not a TypeVarLike: defer (Python asserts otherwise).
    }
}

/// Value-constraint branch of the `get_target_type` decision
/// (applytype.py:262-284).
fn get_target_type_values_tag(
    type_arg: &Type,
    values: &[Type],
    skip_unsatisfied: bool,
    same_type_ok: Option<bool>,
    value_subtypes: Option<&[bool]>,
    narrow_matrix: Option<&[bool]>,
) -> Option<(i64, i64)> {
    // applytype.py:264-265: AnyType passes through.
    if matches!(type_arg, Type::AnyType { .. }) {
        return Some((TAG_PASSTHROUGH, -1));
    }
    // applytype.py:266-270: a TypeVarType argument whose every value is a
    // legal value of the tvar passes through (shim-computed conjunction).
    if let Type::TypeVarType {
        values: p_values, ..
    } = type_arg
    {
        if !p_values.is_empty() && same_type_ok? {
            return Some((TAG_PASSTHROUGH, -1));
        }
    }
    // applytype.py:271-281: value matching + narrowest-match selection.
    let subs = value_subtypes?;
    let matching: Vec<usize> = (0..values.len())
        .filter(|&i| subs.get(i) == Some(&true))
        .collect();
    if matching.is_empty() {
        // applytype.py:282-284: no match.
        if skip_unsatisfied {
            return Some((TAG_SKIP, -1));
        }
        return Some((TAG_REPORT, -1));
    }
    let mut best = matching[0];
    if matching.len() > 1 {
        let matrix = narrow_matrix?;
        let n = values.len();
        for &m in &matching[1..] {
            // Python: `if is_subtype(match, best): best = match` — the
            // matrix is row-major with the left operand as the row index.
            if matrix.get(m * n + best) == Some(&true) {
                best = m;
            }
        }
    }
    Some((TAG_MATCH, best as i64))
}

// ---------------------------------------------------------------------------
// has_no_typevars
// ---------------------------------------------------------------------------

/// `mypy.typevars.has_no_typevars` — check if a type contains no type
/// variables by comparing `typ` to `erase_typevars(typ)`.
///
/// Mirrors typevars.py:77-84. Uses Type's `PartialEq` derive. Returns
/// `None` (defer to Python) when `erase_typevars` defers (TypeAliasType,
/// UnboundType), since the comparison would be meaningless without the
/// erased result.
#[pyfunction]
pub(crate) fn rust_has_no_typevars(type_bytes: &[u8]) -> PyResult<Option<bool>> {
    let typ = match decode_type(type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(has_no_typevars_inner(&typ))
}

pub(crate) fn has_no_typevars_inner(typ: &Type) -> Option<bool> {
    let erased = crate::erase_typevars::erase_typevars_inner(typ, None, &make_any())?;
    Some(typ == &erased)
}

// ---------------------------------------------------------------------------
// Wire format helpers
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

/// Decode the orig_types wire format: count (bare int) + for each entry:
/// a 0/1 byte (0 = None, 1 = present) + Type blob if present.
fn decode_optional_type_list(bytes: &[u8]) -> Option<Vec<Option<Type>>> {
    let mut buf = ReadBuffer::new(bytes);
    let count = crate::wire::read_int_bare(&mut buf).ok()?;
    if count < 0 {
        return None;
    }
    let mut result = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let present = buf.read_u8().ok()?;
        if present == 0 {
            result.push(None);
        } else {
            let t = read_type(&mut buf, None).ok()?;
            result.push(Some(t));
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_typevar(raw_id: i64, ns: &str) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: ns.to_string(),
            values: vec![],
            upper_bound: Box::new(Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            default: Box::new(make_any()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn make_typevar_with_default(raw_id: i64, ns: &str, default: Type) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id,
            namespace: ns.to_string(),
            values: vec![],
            upper_bound: Box::new(Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            default: Box::new(default),
            variance: 0,
            meta_level: 0,
        }
    }

    fn make_any() -> Type {
        Type::AnyType {
            type_of_any: 12,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_omitted_any() -> Type {
        Type::AnyType {
            type_of_any: TYPE_OF_ANY_FROM_OMITTED_GENERICS,
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

    #[test]
    fn test_has_default_true_with_concrete_default() {
        let tv = make_typevar_with_default(1, "ns", make_instance("builtins.int", vec![]));
        assert!(has_default(&tv));
    }

    #[test]
    fn test_has_default_false_with_omitted_any() {
        let tv = make_typevar_with_default(1, "ns", make_omitted_any());
        assert!(!has_default(&tv));
    }

    #[test]
    fn test_has_default_true_with_special_form_any() {
        let tv = make_typevar_with_default(1, "ns", make_any());
        assert!(has_default(&tv));
    }

    #[test]
    fn test_typevar_id_key_typevar() {
        let tv = make_typevar(42, "ns");
        let key = typevar_id_key(&tv).unwrap();
        assert_eq!(key, (42, 0, "ns".to_string()));
    }

    #[test]
    fn test_typevar_id_key_non_typevar() {
        let inst = make_instance("builtins.int", vec![]);
        assert!(typevar_id_key(&inst).is_none());
    }

    #[test]
    fn test_find_param_spec_none() {
        let tv = make_typevar(1, "ns");
        assert!(find_param_spec(&[tv]).is_none());
    }

    #[test]
    fn test_find_param_spec_found() {
        let ps = Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "P".to_string(),
            raw_id: 2,
            namespace: "ns".to_string(),
            flavor: PARAM_SPEC_FLAVOR_BARE,
            upper_bound: Box::new(make_instance("builtins.object", vec![])),
            default: Box::new(make_any()),
            meta_level: 0,
        };
        assert!(find_param_spec(&[ps]).is_some());
    }

    #[test]
    fn test_has_unpack_var_arg_false() {
        let arg_types = vec![make_instance("builtins.int", vec![])];
        let arg_kinds = vec![ARG_STAR];
        assert!(!has_unpack_var_arg(&arg_types, &arg_kinds));
    }

    #[test]
    fn test_has_unpack_var_arg_true() {
        let arg_types = vec![Type::UnpackType {
            typ: Box::new(make_instance("builtins.tuple", vec![])),
            from_star_syntax: false,
        }];
        let arg_kinds = vec![ARG_STAR];
        assert!(has_unpack_var_arg(&arg_types, &arg_kinds));
    }

    #[test]
    fn test_get_proper_type_passthrough() {
        let inst = make_instance("builtins.int", vec![]);
        assert!(get_proper_type(&inst).is_some());
    }

    #[test]
    fn test_get_proper_type_alias_defers() {
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
            is_recursive: false,
        };
        assert!(get_proper_type(&alias).is_none());
    }

    #[test]
    fn test_has_no_typevars_true_simple_instance() {
        let inst = make_instance("builtins.int", vec![]);
        assert_eq!(has_no_typevars_inner(&inst), Some(true));
    }

    #[test]
    fn test_has_no_typevars_false_with_typevar() {
        let tv = make_typevar(1, "ns");
        assert_eq!(has_no_typevars_inner(&tv), Some(false));
    }

    #[test]
    fn test_has_no_typevars_false_with_typevar_in_args() {
        let tv = make_typevar(1, "ns");
        let inst = make_instance("builtins.list", vec![tv]);
        assert_eq!(has_no_typevars_inner(&inst), Some(false));
    }

    #[test]
    fn test_has_no_typevars_alias_with_no_args() {
        // #1298: erase_typevars_inner mirrors TypeVarEraser's
        // visit_type_alias_type (erase written args only), so an empty-args
        // alias erases to itself and `typ == erase_typevars(typ)` is True.
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
            is_recursive: false,
        };
        assert_eq!(has_no_typevars_inner(&alias), Some(true));
    }

    #[test]
    fn test_has_no_typevars_true_on_none_type() {
        assert_eq!(has_no_typevars_inner(&Type::NoneType), Some(true));
    }

    #[test]
    fn test_has_no_typevars_true_on_any() {
        assert_eq!(has_no_typevars_inner(&make_any()), Some(true));
    }

    #[test]
    fn test_decode_optional_type_list_empty() {
        let mut wbuf = WriteBuffer::new();
        crate::wire::write_int_bare(&mut wbuf, 0).unwrap();
        let result = decode_optional_type_list(&wbuf.into_bytes());
        assert!(result.is_some());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_decode_optional_type_list_with_nones() {
        let mut wbuf = WriteBuffer::new();
        crate::wire::write_int_bare(&mut wbuf, 2).unwrap();
        wbuf.push(0);
        wbuf.push(0);
        let result = decode_optional_type_list(&wbuf.into_bytes()).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].is_none());
        assert!(result[1].is_none());
    }

    #[test]
    fn test_decode_optional_type_list_mixed() {
        let mut wbuf = WriteBuffer::new();
        crate::wire::write_int_bare(&mut wbuf, 2).unwrap();
        wbuf.push(0);
        wbuf.push(1);
        let inst = make_instance("builtins.int", vec![]);
        write_type(&mut wbuf, &inst).unwrap();
        let result = decode_optional_type_list(&wbuf.into_bytes()).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].is_none());
        assert!(result[1].is_some());
        match &result[1] {
            Some(Type::Instance { type_ref, .. }) => {
                assert_eq!(type_ref, "builtins.int");
            }
            _ => panic!("expected Instance"),
        }
    }

    // --- get_target_type standalone decision head (issue #1081) ---

    fn make_constrained_typevar(values: Vec<Type>) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "ns".to_string(),
            values,
            upper_bound: Box::new(make_instance("builtins.object", vec![])),
            default: Box::new(make_omitted_any()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn tags(
        tvar: &Type,
        type_arg: &Type,
        skip: bool,
        same_type_ok: Option<bool>,
        bound_ok: Option<bool>,
        subs: Option<Vec<bool>>,
        matrix: Option<Vec<bool>>,
    ) -> Option<(i64, i64)> {
        get_target_type_tag(
            tvar,
            type_arg,
            skip,
            same_type_ok,
            bound_ok,
            subs.as_deref(),
            matrix.as_deref(),
        )
    }

    #[test]
    fn test_tag_ambiguous_default_expands() {
        let tvar = make_typevar_with_default(1, "ns", make_instance("builtins.int", vec![]));
        assert_eq!(
            tags(&tvar, &make_amb_uninhabited(), true, None, None, None, None),
            Some((TAG_EXPAND_DEFAULT, -1))
        );
    }

    #[test]
    fn test_tag_ambiguous_no_default_falls_through() {
        let tvar = make_typevar_with_default(1, "ns", make_omitted_any());
        // Falls through to the unconstrained bound check; bound decided.
        assert_eq!(
            tags(
                &tvar,
                &make_amb_uninhabited(),
                true,
                None,
                Some(true),
                None,
                None
            ),
            Some((TAG_PASSTHROUGH, -1))
        );
    }

    #[test]
    fn test_tag_param_spec_passthrough() {
        let ps = Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "mod.P".to_string(),
            raw_id: 2,
            namespace: "ns".to_string(),
            flavor: PARAM_SPEC_FLAVOR_BARE,
            upper_bound: Box::new(make_instance("builtins.object", vec![])),
            default: Box::new(make_omitted_any()),
            meta_level: 0,
        };
        assert_eq!(
            tags(
                &ps,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                None,
                None,
                None
            ),
            Some((TAG_PASSTHROUGH, -1))
        );
    }

    #[test]
    fn test_tag_tvt_passthrough() {
        let tvt = Type::TypeVarTupleType {
            tuple_fallback: Box::new(make_instance("builtins.tuple", vec![])),
            name: "Ts".to_string(),
            fullname: "mod.Ts".to_string(),
            raw_id: 3,
            namespace: "ns".to_string(),
            upper_bound: Box::new(make_instance("builtins.object", vec![])),
            default: Box::new(make_omitted_any()),
            min_len: 0,
            meta_level: 0,
        };
        assert_eq!(
            tags(
                &tvt,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                None,
                None,
                None
            ),
            Some((TAG_PASSTHROUGH, -1))
        );
    }

    #[test]
    fn test_tag_constrained_any_passthrough() {
        let tvar = make_constrained_typevar(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.str", vec![]),
        ]);
        assert_eq!(
            tags(&tvar, &make_any(), true, None, None, None, None),
            Some((TAG_PASSTHROUGH, -1))
        );
    }

    #[test]
    fn test_tag_constrained_same_type_ok() {
        let tv_arg = Type::TypeVarType {
            name: "T1".to_string(),
            fullname: "mod.T1".to_string(),
            raw_id: 2,
            namespace: "ns".to_string(),
            values: vec![make_instance("builtins.int", vec![])],
            upper_bound: Box::new(make_instance("builtins.object", vec![])),
            default: Box::new(make_omitted_any()),
            variance: 0,
            meta_level: 0,
        };
        let tvar = make_constrained_typevar(vec![make_instance("builtins.int", vec![])]);
        assert_eq!(
            tags(&tvar, &tv_arg, true, Some(true), None, None, None),
            Some((TAG_PASSTHROUGH, -1))
        );
    }

    #[test]
    fn test_tag_constrained_same_type_false_falls_to_matching() {
        let tv_arg = Type::TypeVarType {
            name: "T1".to_string(),
            fullname: "mod.T1".to_string(),
            raw_id: 2,
            namespace: "ns".to_string(),
            values: vec![make_instance("builtins.str", vec![])],
            upper_bound: Box::new(make_instance("builtins.object", vec![])),
            default: Box::new(make_omitted_any()),
            variance: 0,
            meta_level: 0,
        };
        let tvar = make_constrained_typevar(vec![make_instance("builtins.int", vec![])]);
        assert_eq!(
            tags(
                &tvar,
                &tv_arg,
                true,
                Some(false),
                None,
                Some(vec![true]),
                None
            ),
            Some((TAG_MATCH, 0))
        );
    }

    #[test]
    fn test_tag_single_match() {
        let tvar = make_constrained_typevar(vec![
            make_instance("builtins.int", vec![]),
            make_instance("builtins.str", vec![]),
        ]);
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.str", vec![]),
                true,
                None,
                None,
                Some(vec![false, true]),
                None
            ),
            Some((TAG_MATCH, 1))
        );
    }

    #[test]
    fn test_tag_narrowest_match_selection() {
        // Both values match; value 1 (str) is narrower than value 0
        // (object), so the narrowing matrix picks index 1.
        let tvar = make_constrained_typevar(vec![
            make_instance("builtins.object", vec![]),
            make_instance("builtins.str", vec![]),
        ]);
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.str", vec![]),
                true,
                None,
                None,
                Some(vec![true, true]),
                Some(vec![false, false, true, false])
            ),
            Some((TAG_MATCH, 1))
        );
    }

    #[test]
    fn test_tag_no_match_skip_vs_report() {
        let tvar = make_constrained_typevar(vec![make_instance("builtins.str", vec![])]);
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                None,
                Some(vec![false]),
                None
            ),
            Some((TAG_SKIP, -1))
        );
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.int", vec![]),
                false,
                None,
                None,
                Some(vec![false]),
                None
            ),
            Some((TAG_REPORT, -1))
        );
    }

    #[test]
    fn test_tag_unconstrained_bound_outcomes() {
        let tvar = make_typevar(1, "ns");
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                Some(true),
                None,
                None
            ),
            Some((TAG_PASSTHROUGH, -1))
        );
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                Some(false),
                None,
                None
            ),
            Some((TAG_SKIP, -1))
        );
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.int", vec![]),
                false,
                None,
                Some(false),
                None,
                None
            ),
            Some((TAG_REPORT, -1))
        );
    }

    #[test]
    fn test_tag_unconstrained_missing_bound_defers() {
        let tvar = make_typevar(1, "ns");
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                None,
                None,
                None
            ),
            None
        );
    }

    #[test]
    fn test_tag_alias_defers() {
        let tvar = make_typevar(1, "ns");
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
            is_recursive: false,
        };
        assert_eq!(
            tags(&tvar, &alias, true, None, Some(true), None, None),
            None
        );
    }

    #[test]
    fn test_tag_non_typevar_defers() {
        let inst = make_instance("builtins.int", vec![]);
        assert_eq!(
            tags(
                &inst,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                Some(true),
                None,
                None
            ),
            None
        );
    }

    #[test]
    fn test_tag_matching_missing_facts_defer() {
        let tvar = make_constrained_typevar(vec![make_instance("builtins.int", vec![])]);
        assert_eq!(
            tags(
                &tvar,
                &make_instance("builtins.int", vec![]),
                true,
                None,
                None,
                None,
                None
            ),
            None
        );
    }

    // --- get_target_type ambiguous-UninhabitedType (issue #913) ---

    fn make_resolver() -> NativeTypeResolver {
        crate::typeinfo::NativeTypeResolver::from_resolver(crate::typeinfo::TypeResolver::new())
    }

    fn make_amb_uninhabited() -> Type {
        Type::UninhabitedType { ambiguous: true }
    }

    fn make_plain_uninhabited() -> Type {
        Type::UninhabitedType { ambiguous: false }
    }

    #[test]
    fn test_get_target_type_ambiguous_with_default_expands() {
        // applytype.py:254-256: ambiguous UninhabitedType + a real tvar
        // default expands via the partial env. T2 defaults to T1, which
        // is already mapped to builtins.int, so the result must be int.
        let resolver = make_resolver();
        let t1 = make_typevar(1, "ns");
        let t2 = make_typevar_with_default(2, "ns", t1.clone());
        let mut env: HashMap<EnvKey, Type> = HashMap::new();
        env.insert(
            (1, 0, "ns".to_string()),
            make_instance("builtins.int", vec![]),
        );
        let result = get_target_type(
            &t2,
            &make_amb_uninhabited(),
            true,
            resolver.resolver(),
            &crate::aliases::TypeAliasResolver::new(),
            &env,
            true,
        );
        let got = result.expect("should decide").expect("should not skip");
        assert_eq!(got, make_instance("builtins.int", vec![]));
    }

    #[test]
    fn test_get_target_type_ambiguous_no_default_falls_through() {
        // No default: ambiguous branch is skipped; the UninhabitedType
        // falls through to the bound check. UninhabitedType <: object is
        // Some(true), so the type_arg (the UninhabitedType) is returned.
        let resolver = make_resolver();
        let tvar = make_typevar_with_default(1, "ns", make_omitted_any());
        let env: HashMap<EnvKey, Type> = HashMap::new();
        let result = get_target_type(
            &tvar,
            &make_amb_uninhabited(),
            true,
            resolver.resolver(),
            &crate::aliases::TypeAliasResolver::new(),
            &env,
            true,
        );
        let got = result.expect("should decide").expect("should not skip");
        assert!(matches!(got, Type::UninhabitedType { ambiguous: true }));
    }

    #[test]
    fn test_get_target_type_non_ambiguous_falls_through() {
        // non-ambiguous UninhabitedType: the ambiguous branch does not
        // fire even though the tvar has a real default; it falls through
        // to the bound check and returns the type_arg (not the default).
        let resolver = make_resolver();
        let tvar = make_typevar_with_default(1, "ns", make_instance("builtins.int", vec![]));
        let env: HashMap<EnvKey, Type> = HashMap::new();
        let result = get_target_type(
            &tvar,
            &make_plain_uninhabited(),
            true,
            resolver.resolver(),
            &crate::aliases::TypeAliasResolver::new(),
            &env,
            true,
        );
        let got = result.expect("should decide").expect("should not skip");
        assert!(matches!(got, Type::UninhabitedType { ambiguous: false }));
    }
}
