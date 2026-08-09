//! Stage 4 `check_call` dispatch classification (checkcall.rs).
//!
//! Ports the pure dispatch decision at the top of `mypy.checkexpr.check_call`:
//! given an already-proper callee type, classify it into the branch that
//! Python's `isinstance` chain would take. Purely structural; no mutation,
//! no checker state, so the kernel cannot emit/suppress errors here.

use pyo3::prelude::*;

use crate::visitor::find_unpack_in_list_inner;
use crate::wire::{read_type, write_type, ReadBuffer, Type, WireError, WriteBuffer};

/// Dispatch kinds mirroring `check_call`'s isinstance chain.
pub(crate) const CALL_PLAIN: i64 = 0; // CallableType without variables
pub(crate) const CALL_WITH_VARS: i64 = 1; // CallableType with variables
pub(crate) const CALL_OVERLOADED: i64 = 2; // Overloaded
pub(crate) const CALL_ANY: i64 = 3; // AnyType (or not checked function)
pub(crate) const CALL_UNION: i64 = 4; // UnionType
pub(crate) const CALL_INSTANCE: i64 = 5; // Instance -> __call__ member access
pub(crate) const CALL_TYPE_TYPE: i64 = 6; // TypeType (falls through to member access)
pub(crate) const CALL_OTHER: i64 = 7;

/// ArgKind values (mirrors mypy.nodes.ArgKind).
const ARG_POS: i64 = 0;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_NAMED_OPT: i64 = 5;

/// The CallableType fields that `with_unpacked_kwargs` and
/// `with_normalized_var_args` rewrite plus the immutable passes-through.
struct CallableBase {
    fallback: Box<Type>,
    instance_type: Option<Box<Type>>,
    is_ellipsis_args: bool,
    implicit: bool,
    is_bound: bool,
    from_concatenate: bool,
    imprecise_arg_kinds: bool,
    unpack_kwargs: bool,
    arg_types: Vec<Type>,
    arg_kinds: Vec<i64>,
    arg_names: Vec<Option<String>>,
    ret_type: Box<Type>,
    name: Option<String>,
    variables: Vec<Type>,
    type_guard: Option<Box<Type>>,
    type_is: Option<Box<Type>>,
}

/// Classify an already-proper callee type into the `check_call` dispatch
/// branch. Defer (None) on any wire/decode failure.
#[pyfunction]
pub(crate) fn rust_classify_call(callee_bytes: &[u8]) -> Option<i64> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    classify_call(&callee).ok()
}

/// Pure classification mirroring `check_call`'s isinstance chain.
fn classify_call(callee: &Type) -> Result<i64, WireError> {
    Ok(match callee {
        Type::CallableType { variables, .. } => {
            if variables.is_empty() {
                CALL_PLAIN
            } else {
                CALL_WITH_VARS
            }
        }
        Type::Overloaded { .. } => CALL_OVERLOADED,
        Type::AnyType { .. } => CALL_ANY,
        Type::UnionType { .. } => CALL_UNION,
        Type::Instance { .. } => CALL_INSTANCE,
        Type::TypeType { .. } => CALL_TYPE_TYPE,
        _ => CALL_OTHER,
    })
}

/// Apply `CallableType.with_unpacked_kwargs()` then
/// `with_normalized_var_args()` (types.py:2505-2613), the normalization at
/// the head of `check_callable_call`. Defer (None) on any wire/decode
/// failure or non-CallableType callee.
#[pyfunction]
pub(crate) fn rust_normalize_callable(callee_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    let normalized = normalize_callable(&callee).ok()?;
    let mut out = WriteBuffer::new();
    write_type(&mut out, &normalized).ok()?;
    Some(out.into_bytes())
}

fn normalize_callable(callee: &Type) -> Result<Type, WireError> {
    let Type::CallableType {
        fallback,
        instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        arg_types,
        arg_kinds,
        arg_names,
        ret_type,
        name,
        variables,
        type_guard,
        type_is,
    } = callee
    else {
        return Err(WireError::invalid(
            "normalize: callee is not a CallableType",
        ));
    };
    let mut base = CallableBase {
        fallback: fallback.clone(),
        instance_type: instance_type.clone(),
        is_ellipsis_args: *is_ellipsis_args,
        implicit: *implicit,
        is_bound: *is_bound,
        from_concatenate: *from_concatenate,
        imprecise_arg_kinds: *imprecise_arg_kinds,
        unpack_kwargs: *unpack_kwargs,
        arg_types: arg_types.clone(),
        arg_kinds: arg_kinds.clone(),
        arg_names: arg_names.clone(),
        ret_type: ret_type.clone(),
        name: name.clone(),
        variables: variables.clone(),
        type_guard: type_guard.clone(),
        type_is: type_is.clone(),
    };
    with_unpacked_kwargs(&mut base)?;
    with_normalized_var_args(&mut base)?;
    Ok(base.into_type())
}

/// `with_unpacked_kwargs`: expand `**kwargs: TypedDict` into named keys.
fn with_unpacked_kwargs(base: &mut CallableBase) -> Result<(), WireError> {
    if !base.unpack_kwargs {
        return Ok(());
    }
    let Some(Type::TypedDictType {
        items,
        required_keys,
        ..
    }) = base.arg_types.last()
    else {
        // Python asserts isinstance(last_type, TypedDictType).
        return Err(WireError::invalid(
            "with_unpacked_kwargs: last arg type is not a TypedDictType",
        ));
    };
    let required: std::collections::HashSet<&String> = required_keys.iter().collect();
    let mut types = base.arg_types[..base.arg_types.len() - 1].to_vec();
    let mut kinds = base.arg_kinds[..base.arg_kinds.len() - 1].to_vec();
    let mut names = base.arg_names[..base.arg_names.len() - 1].to_vec();
    for (key, typ) in items {
        types.push(typ.clone());
        kinds.push(if required.contains(&key) {
            ARG_NAMED
        } else {
            ARG_NAMED_OPT
        });
        names.push(Some(key.clone()));
    }
    base.arg_types = types;
    base.arg_kinds = kinds;
    base.arg_names = names;
    base.unpack_kwargs = false;
    Ok(())
}

/// `with_normalized_var_args`: expand `*args: *Tuple[...]` into fixed args.
fn with_normalized_var_args(base: &mut CallableBase) -> Result<(), WireError> {
    let var_arg_index = base.arg_kinds.iter().position(|&k| k == ARG_STAR);
    let unpacked_items = match var_arg_index {
        Some(idx) => match &base.arg_types[idx] {
            Type::UnpackType { typ } => match &**typ {
                Type::TupleType { items, .. } => Some(items.clone()),
                _ => None,
            },
            _ => None,
        },
        None => None,
    };
    let Some(unpacked_items) = unpacked_items else {
        return Ok(());
    };
    let unpack_index = find_unpack_in_list_inner(&unpacked_items);
    let ui = var_arg_index.unwrap();
    if unpack_index == 0 && unpacked_items.len() > 1 {
        // Already normalized: return the callable unchanged.
        return Ok(());
    }
    let types_prefix = base.arg_types[..ui].to_vec();
    let kinds_prefix = base.arg_kinds[..ui].to_vec();
    let names_prefix = base.arg_names[..ui].to_vec();
    let types_suffix = base.arg_types[ui + 1..].to_vec();
    let kinds_suffix = base.arg_kinds[ui + 1..].to_vec();
    let names_suffix = base.arg_names[ui + 1..].to_vec();
    let (types_middle, kinds_middle, names_middle) = if unpack_index < 0 {
        // Plain *Tuple[X, Y, Z] -> replace with ARG_POS completely.
        (
            unpacked_items.clone(),
            vec![ARG_POS; unpacked_items.len()],
            vec![None; unpacked_items.len()],
        )
    } else {
        let ui_idx = unpack_index as usize;
        let Type::UnpackType { typ } = &unpacked_items[ui_idx] else {
            unreachable!("unpack_index points at an UnpackType");
        };
        let nested_unpacked = &**typ;
        let mut types_middle = unpacked_items[..ui_idx].to_vec();
        let mut kinds_middle: Vec<i64> = (0..ui_idx).map(|_| ARG_POS).collect();
        let mut names_middle: Vec<Option<String>> = (0..ui_idx).map(|_| None).collect();
        if ui_idx == unpacked_items.len() - 1 {
            // Normalize also single item tuples like
            //   *args: *Tuple[*tuple[X, ...]] -> *args: X
            //   *args: *Tuple[*Ts] -> *args: *Ts
            match nested_unpacked {
                Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
                    types_middle.push(args[0].clone());
                    kinds_middle.push(ARG_STAR);
                    names_middle.push(base.arg_names[ui].clone());
                }
                Type::TypeVarTupleType { .. } => {
                    types_middle.push(nested_unpacked.clone());
                    kinds_middle.push(ARG_STAR);
                    names_middle.push(base.arg_names[ui].clone());
                }
                _ => {
                    // Non-normalized tuple during semanal: return as-is.
                    return Ok(());
                }
            }
        } else {
            // *Tuple[X, *Ts, Y, Z] -> prefix ARG_POS, keep the tail
            // unpacked as a single UnpackType.
            types_middle.push(unpacked_items[ui_idx].clone());
            kinds_middle.push(ARG_STAR);
            names_middle.push(base.arg_names[ui].clone());
        }
        (types_middle, kinds_middle, names_middle)
    };
    base.arg_types = [types_prefix, types_middle, types_suffix].concat();
    base.arg_kinds = [kinds_prefix, kinds_middle, kinds_suffix].concat();
    base.arg_names = [names_prefix, names_middle, names_suffix].concat();
    Ok(())
}

impl CallableBase {
    fn into_type(self) -> Type {
        Type::CallableType {
            fallback: self.fallback,
            instance_type: self.instance_type,
            is_ellipsis_args: self.is_ellipsis_args,
            implicit: self.implicit,
            is_bound: self.is_bound,
            from_concatenate: self.from_concatenate,
            imprecise_arg_kinds: self.imprecise_arg_kinds,
            unpack_kwargs: self.unpack_kwargs,
            arg_types: self.arg_types,
            arg_kinds: self.arg_kinds,
            arg_names: self.arg_names,
            ret_type: self.ret_type,
            name: self.name,
            variables: self.variables,
            type_guard: self.type_guard,
            type_is: self.type_is,
        }
    }
}

/// `mypy.checkexpr.ExpressionChecker.real_union` (checkexpr.py:3488):
/// a "real" union has more than one relevant item. When `strict_optional`
/// is False, NoneType items are stripped from the union before counting
/// (mirrors `UnionType.relevant_items`). Returns None on wire/decode
/// failure or TypeAliasType (unresolved alias target).
#[pyfunction]
pub(crate) fn rust_real_union(type_bytes: &[u8], strict_optional: bool) -> Option<bool> {
    let mut buf = ReadBuffer::new(type_bytes);
    let typ = read_type(&mut buf, None).ok()?;
    real_union(&typ, strict_optional)
}

fn real_union(typ: &Type, strict_optional: bool) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::UnionType { items, .. } => {
            let count = if strict_optional {
                items.len()
            } else {
                items
                    .iter()
                    .filter(|t| !matches!(t, Type::NoneType))
                    .count()
            };
            Some(count > 1)
        }
        _ => Some(false),
    }
}

/// `mypy.checkexpr.ExpressionChecker.possible_none_type_var_overlap`
/// (checkexpr.py:3348): heuristic to decide whether union math should be
/// forced. Returns True when an argument is a union containing NoneType
/// AND some plausible overload target has a NoneType formal while another
/// has a TypeVarType formal at the same position. Returns None on any
/// wire/decode failure or TypeAliasType in the inputs.
///
/// Inputs are serialized arg_types and plausible_targets (CallableType
/// list). The Python caller passes already-proper types.
#[pyfunction]
pub(crate) fn rust_possible_none_type_var_overlap(
    arg_type_bytes: Vec<Vec<u8>>,
    target_bytes: Vec<Vec<u8>>,
) -> Option<bool> {
    let mut arg_types: Vec<Type> = Vec::with_capacity(arg_type_bytes.len());
    for bytes in &arg_type_bytes {
        let mut buf = ReadBuffer::new(bytes);
        arg_types.push(read_type(&mut buf, None).ok()?);
    }
    let mut targets: Vec<Type> = Vec::with_capacity(target_bytes.len());
    for bytes in &target_bytes {
        let mut buf = ReadBuffer::new(bytes);
        targets.push(read_type(&mut buf, None).ok()?);
    }
    possible_none_type_var_overlap(&arg_types, &targets)
}

fn possible_none_type_var_overlap(arg_types: &[Type], targets: &[Type]) -> Option<bool> {
    if targets.is_empty() || arg_types.is_empty() {
        return Some(false);
    }
    // Step 1: check if any arg_type is a union containing NoneType.
    let mut has_optional_arg = false;
    for arg_type in arg_types {
        let proper = get_proper_or_none(arg_type)?;
        if let Type::UnionType { items, .. } = proper {
            for item in items {
                let item_proper = get_proper_or_none(item)?;
                if matches!(item_proper, Type::NoneType) {
                    has_optional_arg = true;
                    break;
                }
            }
        }
        if has_optional_arg {
            break;
        }
    }
    if !has_optional_arg {
        return Some(false);
    }
    // Step 2: find min prefix length across all target arg_types.
    let mut min_prefix = usize::MAX;
    for target in targets {
        let proper = get_proper_or_none(target)?;
        let Type::CallableType {
            arg_types: t_arg_types,
            ..
        } = proper
        else {
            return None;
        };
        if t_arg_types.len() < min_prefix {
            min_prefix = t_arg_types.len();
        }
    }
    if min_prefix == 0 {
        return Some(false);
    }
    // Step 3: for each position, check if some target has NoneType and
    // another has TypeVarType at that position.
    for i in 0..min_prefix {
        let mut has_none = false;
        let mut has_typevar = false;
        for target in targets {
            let proper = get_proper_or_none(target)?;
            let Type::CallableType {
                arg_types: t_arg_types,
                ..
            } = proper
            else {
                return None;
            };
            let formal = get_proper_or_none(&t_arg_types[i])?;
            match formal {
                Type::NoneType => has_none = true,
                Type::TypeVarType { .. } => has_typevar = true,
                _ => {}
            }
        }
        if has_none && has_typevar {
            return Some(true);
        }
    }
    Some(false)
}

/// `get_proper_type` for the wire format. Expands TypeAliasType by
/// returning None (defer) since the wire format has no alias target.
fn get_proper_or_none(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ),
    }
}

/// `rust_solve_generic_call`: normalize + map + infer + solve + apply.
///
/// Takes a generic callable (serialized), actual arg types (serialized),
/// the formal-to-actual mapping, and metadata flags. Returns the
/// fully-resolved (non-generic) callable with type args applied, or `None`
/// to defer to Python.
///
/// The return value is a wire-format blob for the resolved callable if
/// the Rust kernel successfully inferred and solved type arguments. The
/// caller checks the first byte of the resolved callee's `variables`
/// length — if 0 the callable is fully resolved; if >0 it still carries
/// residual (non-inferred) type vars, and the caller may do additional
/// passes or fall through to Python.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub fn rust_solve_generic_call(
    _py: Python<'_>,
    resolver: &crate::typeinfo::NativeTypeResolver,
    callee_bytes: &[u8],
    arg_types_bytes: Vec<Vec<u8>>,
    formal_to_actual: Vec<Vec<i64>>,
    strict: bool,
    infer_unions: bool,
) -> Option<Vec<u8>> {
    // Decode the callee.
    let mut buf = crate::wire::ReadBuffer::new(callee_bytes);
    let callee = crate::wire::read_type(&mut buf, None).ok()?;

    let Type::CallableType { .. } = &callee else {
        return None;
    };

    // Step 1: Normalize (with_unpacked_kwargs + with_normalized_var_args).
    let normalized = normalize_callable(&callee).ok()?;
    let (formal_arg_types, _formal_arg_kinds, _formal_arg_names, variables) = match &normalized {
        Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            variables,
            ..
        } => (arg_types, arg_kinds, arg_names, variables),
        _ => return None,
    };

    // Defer on ParamSpec/TypeVarTuple variables — expand_type defers.
    if variables.iter().any(|v| {
        matches!(
            v,
            Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. }
        )
    }) {
        return None;
    }

    // Step 2: Infer constraints by iterating formal-to-actual.
    let mut all_constraints: Vec<crate::constraints::Constraint> = Vec::new();
    let arg_types_vec: Vec<Type> = arg_types_bytes
        .iter()
        .map(|b| {
            let mut b2 = crate::wire::ReadBuffer::new(b);
            crate::wire::read_type(&mut b2, None).ok()
        })
        .collect::<Option<Vec<_>>>()?;

    for (fi, actual_indices) in formal_to_actual.iter().enumerate() {
        if actual_indices.is_empty() {
            continue;
        }
        let formal_type = formal_arg_types.get(fi)?;

        // Handle UnpackType formals (*args: *Tuple[...], etc.)
        if let Type::UnpackType { typ: unpack_inner } = formal_type {
            if let Type::TypeVarTupleType { tuple_fallback, .. } = unpack_inner.as_ref() {
                // Collect expanded actual types for TupleType constraint.
                let mut expanded: Vec<Type> = Vec::with_capacity(actual_indices.len());
                for &ai in actual_indices {
                    let at = arg_types_vec.get(ai as usize)?;
                    if let Type::UnpackType { typ: inner } = at {
                        if let Type::TupleType { items, .. } = inner.as_ref() {
                            expanded.extend(items.iter().cloned());
                        } else {
                            expanded.push(at.clone());
                        }
                    } else {
                        expanded.push(at.clone());
                    }
                }
                if !expanded.is_empty() {
                    let tuple_target = Type::TupleType {
                        partial_fallback: Box::new(tuple_fallback.as_ref().clone()),
                        items: expanded,
                        implicit: false,
                    };
                    all_constraints.push(crate::constraints::Constraint {
                        origin_type_var: unpack_inner.as_ref().clone(),
                        op: crate::constraints::SUPERTYPE_OF,
                        target: tuple_target,
                    });
                }
            } else if let Type::TupleType { .. } = unpack_inner.as_ref() {
                // *args: *Tuple[...] — not a TypeVarTuple.
                // Each actual gets a constraint against the tuple element type.
                // For simplicity, we defer this complex unpack case to Python.
                return None;
            }
            continue;
        }

        // Standard case: infer constraints for each actual against formal.
        for &ai in actual_indices {
            let actual_type = arg_types_vec.get(ai as usize)?;
            // Skip None actuals (argument was in a deferred pass).
            let actual_proper = get_proper_or_none(actual_type)?;
            if matches!(actual_proper, Type::AnyType { .. }) {
                continue;
            }
            let formal_proper = get_proper_or_none(formal_type)?;
            let constraints = crate::constraints::infer_constraints_full_inner(
                formal_proper,
                actual_proper,
                crate::constraints::SUBTYPE_OF,
                resolver.resolver(),
            )?;
            all_constraints.extend(constraints);
        }
    }

    if all_constraints.is_empty() {
        return None; // Nothing to solve.
    }

    // Step 3: Solve constraints for the callable's type vars.
    let tvar_types: Vec<Type> = variables.to_vec();
    let constraint_blobs: Vec<Vec<u8>> = all_constraints
        .iter()
        .map(|c| {
            let mut b = crate::wire::WriteBuffer::new();
            c.write(&mut b).ok()?;
            Some(b.into_bytes())
        })
        .collect::<Option<Vec<_>>>()?;

    let vars_blobs: Vec<Vec<u8>> = tvar_types
        .iter()
        .map(|t| {
            let mut b = crate::wire::WriteBuffer::new();
            crate::wire::write_type(&mut b, t).ok()?;
            Some(b.into_bytes())
        })
        .collect::<Option<_>>()?;

    let solve_result = crate::solve::rust_solve_constraints(
        vars_blobs.clone(),
        vars_blobs,
        constraint_blobs,
        strict,
        infer_unions,
        true, // strict_optional: assume strict
        true, // skip_unsatisfied: safe for callable checking
        resolver,
    );

    let Some((num_solved, sol_blob, _free_blob)) = solve_result else {
        return None; // Solver deferred.
    };
    if num_solved == 0 {
        return None;
    }

    // Step 4: Decode solutions and apply to callable.
    let sol_bytes = sol_blob?;
    let solutions = decode_solve_solutions(&sol_bytes)?;
    let orig_types: Vec<Option<Type>> = variables
        .iter()
        .map(|tv| {
            let key = solve_typevar_key(tv)?;
            solutions
                .iter()
                .find(|(k, _)| *k == key)
                .and_then(|(_, t)| t.clone())
        })
        .collect();

    // Serialize orig_types in the wire format expected by apply_generic_arguments.
    let orig_types_blob = serialize_optional_types(&orig_types)?;

    let mut wbuf = crate::wire::WriteBuffer::new();
    crate::wire::write_type(&mut wbuf, &normalized).ok()?;
    crate::applytype::rust_apply_generic_arguments(
        resolver,
        wbuf.into_bytes().as_slice(),
        &orig_types_blob,
        true, // skip_unsatisfied
        true, // strict_optional
    )
}

/// A solved type-var entry: `(raw_id, meta_level, namespace)` key plus the
/// substituted type (None when the solver left it unsolved).
type SolveEntry = ((i64, i64, String), Option<Type>);

/// Decode `(raw, meta, ns, has_sol, type?)...` from a solve solutions blob.
fn decode_solve_solutions(blob: &[u8]) -> Option<Vec<SolveEntry>> {
    let mut buf = crate::wire::ReadBuffer::new(blob);
    let count = crate::wire::read_int(&mut buf).ok()?;
    let mut result = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let raw = crate::wire::read_int(&mut buf).ok()?;
        let meta = crate::wire::read_int(&mut buf).ok()?;
        let ns = crate::wire::read_str(&mut buf).ok()?;
        let has_sol = crate::wire::read_int(&mut buf).ok()?;
        let typ = if has_sol == 1 {
            Some(crate::wire::read_type(&mut buf, None).ok()?)
        } else {
            None
        };
        result.push(((raw, meta, ns), typ));
    }
    Some(result)
}

/// Get the TypeVar key `(raw_id, meta_level, namespace)` from a Type.
fn solve_typevar_key(t: &Type) -> Option<(i64, i64, String)> {
    match t {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        Type::ParamSpecType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        _ => None,
    }
}

/// Serialize an optional type list in the wire format expected by
/// `apply_generic_arguments`: count (bare int) + for each entry: a 0/1 byte
/// (0 = None, 1 = present) followed by a Type blob if present.
fn serialize_optional_types(types: &[Option<Type>]) -> Option<Vec<u8>> {
    let mut buf = crate::wire::WriteBuffer::new();
    crate::wire::write_int(&mut buf, types.len() as i64).ok()?;
    for t in types {
        match t {
            None => buf.push(0),
            Some(t) => {
                buf.push(1);
                crate::wire::write_type(&mut buf, t).ok()?;
            }
        }
    }
    Some(buf.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::{write_type, WriteBuffer};

    fn classify_bytes(t: &Type) -> Option<i64> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).ok()?;
        rust_classify_call(&buf.into_bytes())
    }

    fn normalize_bytes(t: &Type) -> Option<Type> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).ok()?;
        let out = rust_normalize_callable(&buf.into_bytes())?;
        let mut rb = ReadBuffer::new(&out);
        read_type(&mut rb, None).ok()
    }

    fn typed_dict(required: &[&str], optional: &[(&str, Type)]) -> Type {
        let mut items: Vec<(String, Type)> = Vec::new();
        let mut required_keys: std::collections::HashSet<String> = Default::default();
        for name in required {
            items.push((name.to_string(), any_type()));
            required_keys.insert(name.to_string());
        }
        for (name, typ) in optional {
            items.push((name.to_string(), typ.clone()));
        }
        Type::TypedDictType {
            fallback: Box::new(instance()),
            items,
            required_keys,
            readonly_keys: Default::default(),
            is_closed: true,
        }
    }

    fn unpack(typ: Type) -> Type {
        Type::UnpackType { typ: Box::new(typ) }
    }

    fn tuple_of(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance()),
            items,
            implicit: false,
        }
    }

    fn callable_with(unpack_kwargs: bool, args: Vec<(Type, i64, Option<String>)>) -> Type {
        let mut arg_types = Vec::with_capacity(args.len());
        let mut arg_kinds = Vec::with_capacity(args.len());
        let mut arg_names = Vec::with_capacity(args.len());
        for (t, k, n) in args {
            arg_types.push(t);
            arg_kinds.push(k);
            arg_names.push(n);
        }
        Type::CallableType {
            fallback: Box::new(instance()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type: Box::new(any_type()),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
        }
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn instance() -> Type {
        Type::Instance {
            type_ref: "mod.C".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn type_var() -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "mod".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn callable(variables: usize) -> Type {
        Type::CallableType {
            fallback: Box::new(instance()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(any_type()),
            name: None,
            variables: vec![type_var(); variables],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn classifies_plain_callable() {
        assert_eq!(classify_bytes(&callable(0)), Some(CALL_PLAIN));
    }

    #[test]
    fn classifies_callable_with_vars() {
        assert_eq!(classify_bytes(&callable(2)), Some(CALL_WITH_VARS));
    }

    #[test]
    fn classifies_any() {
        assert_eq!(classify_bytes(&any_type()), Some(CALL_ANY));
    }

    #[test]
    fn classifies_instance() {
        assert_eq!(classify_bytes(&instance()), Some(CALL_INSTANCE));
    }

    #[test]
    fn classifies_overloaded() {
        let t = Type::Overloaded {
            items: vec![callable(0)],
        };
        assert_eq!(classify_bytes(&t), Some(CALL_OVERLOADED));
    }

    #[test]
    fn classifies_union() {
        let t = Type::UnionType {
            items: vec![any_type(), instance()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(classify_bytes(&t), Some(CALL_UNION));
    }

    #[test]
    fn normalize_noop_for_plain_callable() {
        let t = callable(0);
        assert_eq!(normalize_bytes(&t), Some(t));
    }

    #[test]
    fn normalize_unpacks_kwargs_typeddict() {
        let td = typed_dict(&["x"], &[("y", any_type())]);
        let t = callable_with(
            true,
            vec![
                (any_type(), ARG_POS, None),
                (td, ARG_NAMED, Some("kwargs".into())),
            ],
        );
        let out = normalize_bytes(&t).unwrap();
        let Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            unpack_kwargs,
            ..
        } = out
        else {
            panic!("expected CallableType");
        };
        assert!(!unpack_kwargs);
        assert_eq!(arg_types.len(), 3);
        assert_eq!(arg_kinds, vec![ARG_POS, ARG_NAMED, ARG_NAMED_OPT]);
        assert_eq!(arg_names, vec![None, Some("x".into()), Some("y".into())]);
    }

    #[test]
    fn normalize_var_args_plain_tuple() {
        let t = callable_with(
            false,
            vec![(
                unpack(tuple_of(vec![any_type(), any_type()])),
                ARG_STAR,
                Some("args".into()),
            )],
        );
        let out = normalize_bytes(&t).unwrap();
        let Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            ..
        } = out
        else {
            panic!("expected CallableType");
        };
        assert_eq!(arg_types.len(), 2);
        assert_eq!(arg_kinds, vec![ARG_POS, ARG_POS]);
        assert_eq!(arg_names, vec![None, None]);
    }

    #[test]
    fn normalize_var_args_non_tuple_unpack_unchanged() {
        // *args: *tuple[X, ...] is not a TupleType on the wire; the
        // Python method also leaves it unchanged.
        let star_unpack = unpack(tuple_of(vec![unpack(tuple_of(vec![instance()]))]));
        let t = callable_with(false, vec![(star_unpack, ARG_STAR, Some("args".into()))]);
        let out = normalize_bytes(&t);
        assert_eq!(
            out,
            Some(callable_with(
                false,
                vec![(
                    unpack(tuple_of(vec![unpack(tuple_of(vec![instance()]))])),
                    ARG_STAR,
                    Some("args".into())
                )]
            ))
        );
    }

    fn none_type() -> Type {
        Type::NoneType
    }

    fn union_of(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }
    }

    fn callable_with_args(arg_types: Vec<Type>) -> Type {
        let arg_kinds = vec![ARG_POS; arg_types.len()];
        let arg_names = vec![None; arg_types.len()];
        Type::CallableType {
            fallback: Box::new(instance()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type: Box::new(any_type()),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
        }
    }

    fn encode(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, t).unwrap();
        buf.into_bytes()
    }

    #[test]
    fn real_union_non_union_returns_false() {
        let t = any_type();
        assert_eq!(rust_real_union(&encode(&t), true), Some(false));
    }

    #[test]
    fn real_union_single_item_returns_false() {
        let t = union_of(vec![any_type()]);
        assert_eq!(rust_real_union(&encode(&t), true), Some(false));
    }

    #[test]
    fn real_union_multi_item_returns_true() {
        let t = union_of(vec![any_type(), instance()]);
        assert_eq!(rust_real_union(&encode(&t), true), Some(true));
    }

    #[test]
    fn real_union_strips_none_when_not_strict() {
        // Union[int, None] with strict_optional=False: relevant = [int], count=1 -> false
        let t = union_of(vec![instance(), none_type()]);
        assert_eq!(rust_real_union(&encode(&t), false), Some(false));
    }

    #[test]
    fn real_union_keeps_none_when_strict() {
        // Union[int, None] with strict_optional=True: relevant = [int, None], count=2 -> true
        let t = union_of(vec![instance(), none_type()]);
        assert_eq!(rust_real_union(&encode(&t), true), Some(true));
    }

    #[test]
    fn none_overlap_empty_args_returns_false() {
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![], vec![encode(&callable(0))]),
            Some(false)
        );
    }

    #[test]
    fn none_overlap_no_targets_returns_false() {
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![encode(&any_type())], vec![]),
            Some(false)
        );
    }

    #[test]
    fn none_overlap_no_union_arg_returns_false() {
        let arg = encode(&any_type());
        let target = encode(&callable_with_args(vec![none_type(), type_var()]));
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![arg], vec![target]),
            Some(false)
        );
    }

    #[test]
    fn none_overlap_union_without_none_returns_false() {
        let arg = encode(&union_of(vec![any_type(), instance()]));
        let target = encode(&callable_with_args(vec![none_type(), type_var()]));
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![arg], vec![target]),
            Some(false)
        );
    }

    #[test]
    fn none_overlap_union_with_none_no_typevar_returns_false() {
        let arg = encode(&union_of(vec![instance(), none_type()]));
        let target = encode(&callable_with_args(vec![none_type(), any_type()]));
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![arg], vec![target]),
            Some(false)
        );
    }

    #[test]
    fn none_overlap_union_with_none_and_typevar_returns_true() {
        let arg = encode(&union_of(vec![instance(), none_type()]));
        let target1 = encode(&callable_with_args(vec![none_type()]));
        let target2 = encode(&callable_with_args(vec![type_var()]));
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![arg], vec![target1, target2]),
            Some(true)
        );
    }

    #[test]
    fn none_overlap_single_target_none_and_typevar_different_pos_returns_false() {
        // NoneType at pos 0, TypeVar at pos 1: neither position has both.
        let arg = encode(&union_of(vec![instance(), none_type()]));
        let target = encode(&callable_with_args(vec![none_type(), type_var()]));
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![arg], vec![target]),
            Some(false)
        );
    }

    #[test]
    fn none_overlap_typevar_in_different_position_returns_false() {
        let arg = encode(&union_of(vec![instance(), none_type()]));
        let target1 = encode(&callable_with_args(vec![none_type(), any_type()]));
        let target2 = encode(&callable_with_args(vec![any_type(), type_var()]));
        assert_eq!(
            rust_possible_none_type_var_overlap(vec![arg], vec![target1, target2]),
            Some(false)
        );
    }
}
