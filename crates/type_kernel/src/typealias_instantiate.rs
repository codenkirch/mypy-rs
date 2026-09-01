#![allow(non_local_definitions)]

//! Native port of the hot-path decision in
//! `mypy.typeanal.instantiate_type_alias` (typeanal.py:2217-2388).
//!
//! `instantiate_type_alias` normalizes a `TypeAlias` node plus type
//! arguments into the instantiated result type, applying the rules from
//! the `TypeAlias` docstring:
//!
//! - `A = List` (bare generic, `no_args`) eagerly expands to
//!   `Instance(list, args)` — this is the only eager-expansion case.
//! - A non-generic alias (`max_tv_count == 0`) with zero args becomes a
//!   bare `TypeAliasType` (normalization deferred) or, when `no_args`,
//!   `Instance(target.type, [])`.
//! - A generic alias instantiation with a correct argument count returns
//!   `TypeAliasType(node, args)`.
//! - Every error/deferral path (`set_any_tvars`, `fail` emission,
//!   `unknown_unpack`, split TypeVarTuples, `from_error` fallbacks)
//!   returns `None` so the Python shim runs the full pure-Python body.
//!
//! The result crossing the wire is a single branch tag (0/1/2); the
//! Python shim rebuilds the live result object from its own `node`,
//! `args` and `ctx`, so no location data or argument blobs need to
//! round-trip (this mirrors the established idiom where Python rebuilds
//! live objects after a Rust decision — e.g. `rust_fill_typevars`).

use pyo3::prelude::*;
use pyo3::types::PyString;

use crate::refs::{is_instance, TypeRefs};
use crate::wire::Type;

/// Result handed to the Python shim: a branch tag identifying which
/// success path the pure-Python body must take. Python rebuilds the live
/// object (`Instance` / `TypeAliasType`) from its own `node`, `args` and
/// `ctx`, so no location data crosses the wire.
///
/// - `0` — step 6 eager expansion, empty args:
///   `Instance(node.target.type, [], line, column)`, `False`
/// - `1` — step 7 eager expansion, args present:
///   `Instance(node.target.type, args)` + 4 location fields, `False`
/// - `2` — step 12 plain success: `TypeAliasType(node, args, line,
///   column)` (both the step-6 non-eager empty case and the generic
///   correct-count case), then the `FlexibleAlias` unwrap.
///
/// `None` defers to the full pure-Python body (every error /
/// `set_any_tvars` path).
const TAG_EAGER_EMPTY: i64 = 0;
const TAG_EAGER_ARGS: i64 = 1;
const TAG_ALIAS: i64 = 2;

/// `mypy.typeanal.instantiate_type_alias` — mirror of typeanal.py:2217-2388.
///
/// Mirrors the pure decision logic of the Python body: the arity checks,
/// the `no_args` bare-tuple rewrite, and the three non-error success
/// paths. Returns `None` (defer) when any path would call `fail` or
/// `set_any_tvars` (error emission, bad argument count, unknown unpack,
/// split TypeVarTuple, `from_error`), so the Python shim falls through to
/// the pure-Python body and message side effects stay single-sourced.
///
/// `node` is the live `TypeAlias` Python object (needed for `target`,
/// `alias_tvars`, `tvar_tuple_index`); `arg_blobs` are the type-argument
/// wire blobs (already flattened by `flatten_nested_tuples` in the shim),
/// used only for the internal validation checks.
#[pyfunction]
pub(crate) fn rust_instantiate_type_alias(
    py: Python<'_>,
    node: &PyAny,
    arg_blobs: Vec<Vec<u8>>,
    no_args: bool,
    empty_tuple_index: bool,
) -> PyResult<Option<i64>> {
    match instantiate_type_alias_inner(py, node, arg_blobs, no_args, empty_tuple_index) {
        Ok(result) => Ok(result),
        Err(DeferError) => Ok(None),
    }
}

fn instantiate_type_alias_inner(
    py: Python<'_>,
    node: &PyAny,
    arg_blobs: Vec<Vec<u8>>,
    no_args: bool,
    empty_tuple_index: bool,
) -> Result<Option<i64>, DeferError> {
    let refs = TypeRefs::try_new(py).map_err(|_| DeferError)?;
    let args = match decode_arg_list(&arg_blobs) {
        Some(args) => args,
        None => return Ok(None),
    };

    // Step 2 (Python): any(unknown_unpack(a) for a in args) — the type
    // is not ready to be validated. Defer (Python rewrites to Any).
    if args.iter().any(unknown_unpack_inner) {
        return Ok(None);
    }

    // Step 3 (Python): `A = tuple` with provided args is not bare:
    // rewrite no_args to False.
    let mut no_args = no_args;
    if no_args && !args.is_empty() && target_is_bare_tuple(node, &refs)? {
        no_args = false;
    }

    let max_tv_count = alias_tvars_count(node)?;
    let act_len = args.len();

    // Step 5 (Python): missing args on a generic alias -> Any fill
    // (set_any_tvars). Defer.
    if max_tv_count > 0 && act_len == 0 && !(empty_tuple_index && tvar_tuple_index(node)?.is_some())
    {
        return Ok(None);
    }

    // Step 6 (Python): non-generic alias, no args.
    if max_tv_count == 0 && act_len == 0 {
        return if no_args {
            // The only eager-expansion case: Instance(target.type, []).
            Ok(Some(TAG_EAGER_EMPTY))
        } else {
            // Normalization deferred: TypeAliasType(node, []).
            Ok(Some(TAG_ALIAS))
        };
    }

    // Step 7 (Python): non-generic alias with args targeting a bare
    // generic (no_args). Eager expansion carrying ctx location.
    if max_tv_count == 0 && act_len > 0 && no_args && target_is_instance(node, &refs)? {
        return Ok(Some(TAG_EAGER_ARGS));
    }

    // Steps 8-10 (Python): fill TypeVars on count mismatch; every branch
    // calls fail / set_any_tvars. Defer.
    if fill_typevars_required(node, &args, &refs)? {
        return Ok(None);
    }

    // Step 11 (Python): TypeVarTuple split check; the split error path
    // calls fail + set_any_tvars. Defer.
    if tvar_tuple_split_too_small(node, &args)? {
        return Ok(None);
    }

    // Step 12 (Python): TypeAliasType(node, args, line, column).
    // FlexibleAlias unwrap (step 13) stays Python-side: it needs
    // get_proper_type(typ), i.e. alias expansion.
    Ok(Some(TAG_ALIAS))
}

// ---------------------------------------------------------------------------
// Wire arg decoding
// ---------------------------------------------------------------------------

fn decode_arg_list(arg_blobs: &[Vec<u8>]) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(arg_blobs.len());
    for blob in arg_blobs {
        let mut buf = crate::wire::ReadBuffer::new(blob);
        match crate::wire::read_type(&mut buf, None) {
            Ok(t) => out.push(t),
            Err(_) => return None,
        }
    }
    Some(out)
}

/// `mypy.typeanal.unknown_unpack` on a wire type — true if `t` is an
/// `UnpackType` of an `AnyType(TypeOfAny.special_form)` (typeanal.py:2856).
/// An `UnpackType` of a `TypeAliasType` is indeterminate on the wire
/// (`get_proper_type` needs the live alias target): `None` -> treated as
/// unknown, so the whole call defers.
fn unknown_unpack_inner(t: &Type) -> bool {
    let Type::UnpackType { typ, .. } = t else {
        return false;
    };
    match typ.as_ref() {
        // get_proper_type of the alias could be special-form Any; the
        // caller cannot distinguish, so defer the whole instantiation.
        Type::TypeAliasType { .. } => true,
        Type::AnyType { type_of_any, .. } => *type_of_any == 6, // TypeOfAny.special_form
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Live-node readers (mirror typeanal.py attribute reads)
// ---------------------------------------------------------------------------

/// len(node.alias_tvars).
fn alias_tvars_count(node: &PyAny) -> Result<usize, DeferError> {
    let tvars = get_attr_or_defer(node, "alias_tvars")?;
    sequence_len(tvars)
}

/// node.tvar_tuple_index (int or None).
fn tvar_tuple_index(node: &PyAny) -> Result<Option<i64>, DeferError> {
    let idx = get_attr_or_defer(node, "tvar_tuple_index")?;
    if idx.is_none() {
        return Ok(None);
    }
    Ok(Some(idx.extract::<i64>().map_err(|_| DeferError)?))
}

/// isinstance(node.target, ProperType) and isinstance(node.target, Instance)
/// and node.target.type.fullname == "builtins.tuple".
fn target_is_bare_tuple(node: &PyAny, refs: &TypeRefs<'_>) -> Result<bool, DeferError> {
    let target = get_attr_or_defer(node, "target")?;
    if !is_instance(target, refs.instance) {
        return Ok(false);
    }
    Ok(type_fullname(target)? == "builtins.tuple")
}

/// isinstance(node.target, Instance).
fn target_is_instance(node: &PyAny, refs: &TypeRefs<'_>) -> Result<bool, DeferError> {
    let target = get_attr_or_defer(node, "target")?;
    Ok(is_instance(target, refs.instance))
}

/// The fullname of `obj.type` (an Instance's TypeInfo).
fn type_fullname(obj: &PyAny) -> Result<String, DeferError> {
    let typ = get_attr_or_defer(obj, "type")?;
    let fullname = get_attr_or_defer(typ, "fullname")?;
    let s = fullname.downcast::<PyString>().map_err(|_| DeferError)?;
    Ok(s.to_str().map_err(|_| DeferError)?.to_string())
}

// ---------------------------------------------------------------------------
// Arity / fill logic (mirrors steps 8-10)
// ---------------------------------------------------------------------------

/// True when Python would take the `fill_typevars` branch (step 10),
/// which always calls fail/set_any_tvars. Rust defers.
fn fill_typevars_required(
    node: &PyAny,
    args: &[Type],
    refs: &TypeRefs<'_>,
) -> Result<bool, DeferError> {
    let tvars = get_attr_or_defer(node, "alias_tvars")?;
    let tv_list = sequence_to_vec(tvars)?;
    let max_tv_count = tv_list.len();
    let act_len = args.len();

    let tvt_index = tvar_tuple_index(node)?;
    match tvt_index {
        None => {
            // Fixed-size alias (Python step 8). Python sets
            // `fill_typevars = act_len != max_tv_count` unconditionally
            // (not `!correct`): even when defaults cover a short arg list

            // (`min_tv_count <= act_len < max_tv_count`), the body still
            // runs `set_any_tvars`, which substitutes the default and may
            // record default recursion. That substitution is Python-side

            // (expand_type on a gradually-built env), so defer on ANY
            // arity mismatch, not just `!correct`.
            if args.iter().any(|a| matches!(a, Type::UnpackType { .. })) {
                // A variadic unpack in a fixed-size alias is invalid;
                // Python fails + rewrites to Any. Defer.
                return Ok(true);
            }
            Ok(act_len != max_tv_count)
        }
        Some(_) => {
            // Variadic alias (Python step 9).
            let min_tv_count = tv_list
                .iter()
                .filter(|tv| !tv_has_default(tv) && !is_instance(tv, refs.type_var_tuple_type))
                .count();
            let mut correct = act_len >= min_tv_count;
            for a in args {
                if !matches!(a, Type::UnpackType { .. }) {
                    continue;
                }
                // Python: get_proper_type(a.type) is an Instance with
                // fullname "builtins.tuple" -> always correct.
                let some_tuple = match a {
                    Type::UnpackType { typ, .. } => match typ.as_ref() {
                        Type::TypeAliasType { .. } => false,
                        Type::Instance { type_ref, .. } => type_ref == "builtins.tuple",
                        _ => false,
                    },
                    _ => false,
                };
                if some_tuple {
                    correct = true;
                }
            }
            Ok(!correct)
        }
    }
}

/// tv.has_default() on a live TypeVarLikeType.
fn tv_has_default(tv: &PyAny) -> bool {
    tv.call_method0("has_default")
        .map(|v| v.is_true().unwrap_or(true))
        .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// TypeVarTuple split check (mirrors step 11)
// ---------------------------------------------------------------------------

/// True when Python would fail with "TypeVarTuple cannot be split"
/// (step 11) — act_prefix/suffix too small for the declared prefix/suffix.
fn tvar_tuple_split_too_small(node: &PyAny, args: &[Type]) -> Result<bool, DeferError> {
    let index = match tvar_tuple_index(node)? {
        Some(i) => i,
        None => return Ok(false),
    };
    // find_unpack_in_list(args): the first UnpackType position.
    let unpack = args
        .iter()
        .position(|a| matches!(a, Type::UnpackType { .. }));
    let unpack = match unpack {
        Some(u) => u,
        None => return Ok(false),
    };
    // Python: isinstance(unpack_arg.type, TypeVarTupleType).
    let is_tvt = match &args[unpack] {
        Type::UnpackType { typ, .. } => matches!(typ.as_ref(), Type::TypeVarTupleType { .. }),
        _ => false,
    };
    if !is_tvt {
        return Ok(false);
    }
    let tvars = get_attr_or_defer(node, "alias_tvars")?;
    let tv_list = sequence_to_vec(tvars)?;
    let exp_prefix = index;
    let act_prefix = unpack as i64;
    let exp_suffix = tv_list.len() as i64 - index - 1;
    let act_suffix = (args.len() - unpack - 1) as i64;
    Ok(act_prefix < exp_prefix || act_suffix < exp_suffix)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

use crate::typeanal_queries::DeferError;

fn get_attr_or_defer<'a>(obj: &'a PyAny, name: &str) -> Result<&'a PyAny, DeferError> {
    obj.getattr(name).map_err(|_| DeferError)
}

fn sequence_to_vec(obj: &PyAny) -> Result<Vec<&PyAny>, DeferError> {
    crate::typeanal_queries::iter_seq(obj)
}

fn sequence_len(obj: &PyAny) -> Result<usize, DeferError> {
    Ok(sequence_to_vec(obj)?.len())
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

    fn make_unpack(typ: Type) -> Type {
        Type::UnpackType {
            typ: Box::new(typ),
            from_star_syntax: false,
        }
    }

    #[test]
    fn unknown_unpack_matches_special_form_any() {
        assert!(unknown_unpack_inner(&make_unpack(make_any(6))));
        assert!(!unknown_unpack_inner(&make_unpack(make_any(2))));
        assert!(!unknown_unpack_inner(&make_instance(
            "builtins.tuple",
            vec![]
        )));
    }

    #[test]
    fn unknown_unpack_defers_on_alias_target() {
        assert!(unknown_unpack_inner(&make_unpack(Type::TypeAliasType {
            args: vec![],
            type_ref: "m.A".to_string(),
        })));
    }
}
