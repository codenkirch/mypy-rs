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
pub(crate) struct CallableBase {
    pub(crate) fallback: Box<Type>,
    pub(crate) instance_type: Option<Box<Type>>,
    pub(crate) is_ellipsis_args: bool,
    pub(crate) implicit: bool,
    pub(crate) is_bound: bool,
    pub(crate) from_concatenate: bool,
    pub(crate) imprecise_arg_kinds: bool,
    pub(crate) unpack_kwargs: bool,
    pub(crate) from_type_type: bool,
    pub(crate) arg_types: Vec<Type>,
    pub(crate) arg_kinds: Vec<i64>,
    pub(crate) arg_names: Vec<Option<String>>,
    pub(crate) ret_type: Box<Type>,
    pub(crate) name: Option<String>,
    pub(crate) variables: Vec<Type>,
    pub(crate) type_guard: Option<Box<Type>>,
    pub(crate) type_is: Option<Box<Type>>,
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
    let normalized = match normalize_callable(&callee) {
        Ok(t) => t,
        Err(_) => {
            return None;
        }
    };
    let mut out = WriteBuffer::new();
    write_type(&mut out, &normalized).ok()?;
    Some(out.into_bytes())
}

pub(crate) fn normalize_callable(callee: &Type) -> Result<Type, WireError> {
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
        from_type_type: *from_type_type,
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

/// `TypeType.make_normalized(arg_types[0])` calibration of a type-object
/// callable's return type (checkexpr.py step 14). Defer (None) on wire
/// failure, a non-CallableType callee, or an unresolved TypeAliasType arg
/// (Python's `get_proper_type` would resolve the alias first; the wire has
/// only the alias name, so the native path cannot match).
#[pyfunction]
pub(crate) fn rust_calibrate_type_obj_return(
    callee_bytes: &[u8],
    arg_type_bytes: &[u8],
) -> Option<Vec<u8>> {
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    let mut abuf = ReadBuffer::new(arg_type_bytes);
    let arg_type = read_type(&mut abuf, None).ok()?;
    if matches!(arg_type, Type::TypeAliasType { .. }) {
        // Python resolves the alias target first; defer to keep the union
        // distribution and item-wrapping identical.
        return None;
    }
    let new_ret = crate::expandtype::make_type_normalized(arg_type, false);
    let mut base = callable_base(&callee).ok()?;
    base.ret_type = Box::new(new_ret);
    let out = base.into_type();
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, &out).ok()?;
    Some(wbuf.into_bytes())
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

/// Copy a `Type::CallableType` into a `CallableBase` for field edits.
///
/// Returns `Err(WireError)` (caller defers to Python) when `callee` is not a
/// `CallableType`, mirroring `normalize_callable`.
pub(crate) fn callable_base(callee: &Type) -> Result<CallableBase, WireError> {
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
            "callable_base: callee is not a CallableType",
        ));
    };
    Ok(CallableBase {
        fallback: fallback.clone(),
        instance_type: instance_type.clone(),
        is_ellipsis_args: *is_ellipsis_args,
        implicit: *implicit,
        is_bound: *is_bound,
        from_concatenate: *from_concatenate,
        imprecise_arg_kinds: *imprecise_arg_kinds,
        unpack_kwargs: *unpack_kwargs,
        from_type_type: *from_type_type,
        arg_types: arg_types.clone(),
        arg_kinds: arg_kinds.clone(),
        arg_names: arg_names.clone(),
        ret_type: ret_type.clone(),
        name: name.clone(),
        variables: variables.clone(),
        type_guard: type_guard.clone(),
        type_is: type_is.clone(),
    })
}

impl CallableBase {
    pub(crate) fn into_type(self) -> Type {
        Type::CallableType {
            fallback: self.fallback,
            instance_type: self.instance_type,
            is_ellipsis_args: self.is_ellipsis_args,
            implicit: self.implicit,
            is_bound: self.is_bound,
            from_concatenate: self.from_concatenate,
            imprecise_arg_kinds: self.imprecise_arg_kinds,
            unpack_kwargs: self.unpack_kwargs,
            from_type_type: self.from_type_type,
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
/// failure or on an unresolvable TypeAliasType (missing alias snapshot /
/// substitution the kernel cannot perform); a resolvable alias expands
/// through the alias resolver, mirroring Python's `get_proper_type`.
#[pyfunction]
pub(crate) fn rust_real_union(
    resolver: &crate::typeinfo::NativeTypeResolver,
    type_bytes: &[u8],
    strict_optional: bool,
) -> Option<bool> {
    let mut buf = ReadBuffer::new(type_bytes);
    let typ = read_type(&mut buf, None).ok()?;
    real_union(&typ, strict_optional, resolver.alias_resolver())
}

/// Mirror of `checkexpr.real_union` (checkexpr.py:4541-4550).
/// A top-level TypeAliasType expands through the alias resolver exactly
/// like Python's `typ = get_proper_type(typ)`, so an alias resolving to a
/// union is counted; an unresolvable alias or a non-Union proper type
/// incl. a union-bound TypeVar is Some(false).
fn real_union(
    typ: &Type,
    strict_optional: bool,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let proper = crate::checkexpr_functions::get_proper_or_expand(typ, aliases)?;
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
/// wire/decode failure or an unresolvable TypeAliasType (the Python
/// mirror resolves aliases with `get_proper_types` / `get_proper_type`
/// before the checks; missing snapshot defers).
///
/// Inputs are serialized arg_types and plausible_targets (CallableType
/// list). The Python caller passes already-proper types, but a
/// TypeAliasType may still appear (e.g. a formal typed with an alias);
/// the resolver expands it.
#[pyfunction]
pub(crate) fn rust_possible_none_type_var_overlap(
    resolver: &crate::typeinfo::NativeTypeResolver,
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
    possible_none_type_var_overlap(&arg_types, &targets, resolver.alias_resolver())
}

fn possible_none_type_var_overlap(
    arg_types: &[Type],
    targets: &[Type],
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    if targets.is_empty() || arg_types.is_empty() {
        return Some(false);
    }
    // Step 1: check if any arg_type is a union containing NoneType.
    // Python resolves each arg and each union item with `get_proper_types`.
    let mut has_optional_arg = false;
    for arg_type in arg_types {
        let proper = crate::checkexpr_functions::get_proper_or_expand(arg_type, aliases)?;
        if let Type::UnionType { items, .. } = &proper {
            for item in items {
                let item_proper = crate::checkexpr_functions::get_proper_or_expand(item, aliases)?;
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
    // Step 2: find min prefix length across all target arg_types. Python
    // does not resolve the target itself (it is a CallableType), so only
    // the formal types inside are resolved.
    let mut min_prefix = usize::MAX;
    for target in targets {
        let proper = crate::checkexpr_functions::get_proper_or_expand(target, aliases)?;
        let Type::CallableType {
            arg_types: t_arg_types,
            ..
        } = &proper
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
    // another has TypeVarType at that position (mirrors Python's
    // `get_proper_type(c.arg_types[i])`).
    for i in 0..min_prefix {
        let mut has_none = false;
        let mut has_typevar = false;
        for target in targets {
            let proper = crate::checkexpr_functions::get_proper_or_expand(target, aliases)?;
            let Type::CallableType {
                arg_types: t_arg_types,
                ..
            } = &proper
            else {
                return None;
            };
            let formal =
                crate::checkexpr_functions::get_proper_or_expand(&t_arg_types[i], aliases)?;
            match &formal {
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

/// Whether a CallableType is a type object: fallback is `builtins.type`
/// and the return type is not UninhabitedType. Mirrors
/// `CallableType.is_type_obj()` (types.py:2343-2346) on the wire, whose
/// Instance carries only `type_ref` — so a fallback with a different
/// `type_ref` (custom metaclass, ABCMeta) cannot be proven to be a
/// metaclass and is treated as not-a-type-object (deferral territory for
/// callers that must match Python exactly).
fn is_type_obj_callable(ret_type: &Type, from_concatenate: bool, fallback: &Type) -> bool {
    if from_concatenate {
        return false;
    }
    if !matches!(
        fallback,
        Type::Instance { type_ref, .. } if type_ref == "builtins.type"
    ) {
        return false;
    }
    !matches!(ret_type, Type::UninhabitedType { .. })
}

/// Port of the `check_callable_call` tail that runs after argument
/// binding (checkexpr.py:2548-2627): type-object return calibration plus
/// the plugin-hook presence probe. Returns the serialized final callee
/// when the native path handles the whole tail, or `None` so the caller
/// falls back to the full Python tail (strangler-fig per-call gate).
///
/// Native handles exactly the case where Python's calibration
/// (`copy_modified(ret_type=TypeType.make_normalized(arg_types[0]))`)
/// applies, and where no plugin call hook fires for `callable_name`. Any
/// uncertainty — user plugins, a live hook, a TypeAliasType component the
/// wire cannot resolve, or an `instance_type` whose force-fallback walk
/// could reach `builtins.type` — defers the entire tail to Python.
#[pyfunction]
#[pyo3(signature = (
    _resolver,
    callee_bytes,
    arg_types_bytes,
    callable_name,
    object_type_present,
    registry,
    has_user_plugins,
    plugins,
))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_check_callable_call(
    _py: Python<'_>,
    _resolver: &crate::typeinfo::NativeTypeResolver,
    callee_bytes: &[u8],
    arg_types_bytes: Vec<Vec<u8>>,
    callable_name: Option<String>,
    object_type_present: bool,
    registry: &PyAny,
    has_user_plugins: bool,
    plugins: &PyAny,
) -> Option<Vec<u8>> {
    if has_user_plugins {
        // User-plugin hooks are not enumerable; defer the whole tail.
        return None;
    }
    if let Some(name) = callable_name.as_deref() {
        let hook_method = if object_type_present {
            "get_method_hook"
        } else {
            "get_function_hook"
        };
        match crate::plugin_hooks::rust_resolve_plugin_hook(
            _py,
            registry,
            name,
            plugins,
            hook_method,
        ) {
            // A hook exists (or the FFI probe failed): Python must run the
            // full hook chain plus calibration.
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) => {}
        }
    }
    let mut buf = ReadBuffer::new(callee_bytes);
    let callee = read_type(&mut buf, None).ok()?;
    let mut arg_types = Vec::with_capacity(arg_types_bytes.len());
    for bytes in &arg_types_bytes {
        let mut abuf = ReadBuffer::new(bytes);
        arg_types.push(read_type(&mut abuf, None).ok()?);
    }
    let out = check_callable_call_tail(&callee, &arg_types)?;
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, &out).ok()?;
    Some(wbuf.into_bytes())
}

/// Pure calibration/assemble decision shared by `rust_check_callable_call`
/// (hook probing happens in the pyfunction). Returns the final callee, or
/// `None` to defer the whole tail to Python.
fn check_callable_call_tail(callee: &Type, arg_types: &[Type]) -> Option<Type> {
    let Type::CallableType {
        ret_type,
        instance_type,
        from_concatenate,
        fallback,
        ..
    } = callee
    else {
        return None; // not a CallableType; Python normalizes first
    };
    if !is_type_obj_callable(ret_type, *from_concatenate, fallback) {
        // Python's calibration condition is False here (is_type_obj False),
        // so defer: the pure tail also runs store_type, the plugin hook
        // section, and returns the original callee (definition intact).
        return None;
    }
    if arg_types.len() != 1 {
        // Same defer reason: the single-argument calibration cannot fire.
        return None;
    }
    // A TypeAliasType return could resolve to UninhabitedType (making
    // is_type_obj False) or to a force-fallback source; the wire has no
    // alias target, so defer.
    if matches!(**ret_type, Type::TypeAliasType { .. }) {
        return None;
    }
    // get_instance_type(force_fallback=True): prefer instance_type, else
    // walk ret_type; only Instance builtins.type triggers calibration.
    let target_is_type = match instance_type {
        Some(inst) => {
            matches!(&**inst, Type::Instance { type_ref, .. } if type_ref == "builtins.type")
        }
        None => matches!(
            &**ret_type,
            Type::Instance { type_ref, .. } if type_ref == "builtins.type"
        ),
    };
    if !target_is_type {
        // An Instance with another type_ref, or a TypeVar/Tuple/TypedDict/
        // Literal whose force-fallback walk could reach builtins.type —
        // the wire cannot verify, so defer rather than risk a mismatch

        // with Python's calibrated ret_type.
        let walkable = |t: &Type| {
            matches!(
                t,
                Type::TypeVarType { .. }
                    | Type::TupleType { .. }
                    | Type::TypedDictType { .. }
                    | Type::LiteralType { .. }
            )
        };
        let uncertain = match instance_type {
            Some(inst) => walkable(inst),
            None => walkable(ret_type),
        };
        if uncertain {
            return None;
        }
        // A non-type-object target whose force-fallback walk cannot reach
        // builtins.type means calibration would not fire; defer so the pure
        // tail keeps definition/plugin-hook behavior intact.
        return None;
    }
    // Python resolves TypeAliasType args before make_normalized; the wire
    // cannot, so defer.
    if matches!(arg_types[0], Type::TypeAliasType { .. }) {
        return None;
    }
    let new_ret = crate::expandtype::make_type_normalized(arg_types[0].clone(), false);
    let mut base = callable_base(callee).ok()?;
    base.ret_type = Box::new(new_ret);
    Some(base.into_type())
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
///
/// The solve step uses `solve::rust_solve_constraints`, whose first tuple
/// element is a status sentinel (0 = success), not a "number solved"
/// count. The solver always fills every callable variable with a concrete
/// solution (strict Never / lax Any for unconstrained vars, mirroring
/// `solve_constraints` solve.py:322-331), so a success status does not
/// mean "nothing was solved" — the solutions blob is authoritative.
///
/// `strict_optional` mirrors `state.strict_optional` at the Python call
/// site (checkexpr.py), and `skip_unsatisfied` mirrors the
/// `solve_constraints` / `apply_generic_arguments` defaults used by
/// Python's inference path: both are `False`, so `pre_validate_solutions`
/// runs and unsatisfied type-variable values still get the
/// "cannot be ..." error instead of being silently skipped.
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
    strict_optional: bool,
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
            // Python's `infer_constraints` applies `get_proper_type` to both
            // sides at entry (constraints.py:510-512); expand a resolvable
            // top-level alias instead of deferring it.
            let actual_expanded;
            let actual_proper = match actual_type {
                Type::TypeAliasType { .. } => {
                    actual_expanded = crate::checkexpr_functions::get_proper_or_expand(
                        actual_type,
                        resolver.alias_resolver(),
                    )?;
                    get_proper_or_none(&actual_expanded)?
                }
                t => get_proper_or_none(t)?,
            };
            // Star actuals (TupleType against `*args`) are gated Python-side
            // (checkexpr.py `any(k.is_star() ...)`), so a TupleType actual
            // here is positional; ArgTypeExpander passes it 1:1.
            if matches!(actual_proper, Type::UninhabitedType { .. }) {
                return None;
            }
            let formal_expanded;
            let formal_proper = match formal_type {
                Type::TypeAliasType { .. } => {
                    formal_expanded = crate::checkexpr_functions::get_proper_or_expand(
                        formal_type,
                        resolver.alias_resolver(),
                    )?;
                    get_proper_or_none(&formal_expanded)?
                }
                t => get_proper_or_none(t)?,
            };
            let constraints = match crate::constraints::infer_constraints_full_inner(
                formal_proper,
                actual_proper,
                crate::constraints::SUPERTYPE_OF, // mirrors constraints.py:641
                resolver.resolver(),
                resolver.alias_resolver(),
                strict_optional,
                false,
                false,
                // Python `infer_constraints` wrapper default (constraints.py:802).
                true,
            ) {
                Some(c) => c,
                None => {
                    return None;
                }
            };
            all_constraints.extend(constraints);
        }
    }

    // Empty constraints are still solvable: the solver fills every
    // unconstrained var with strict Never / lax Any (solve.py:277-289),
    // matching Python's empty-cmap fill (#382 path). No deferral needed.
    let tv_key = |t: &Type| -> Option<(i64, String)> {
        match t {
            Type::TypeVarType {
                raw_id, namespace, ..
            } => Some((*raw_id, namespace.clone())),
            Type::ParamSpecType {
                raw_id, namespace, ..
            } => Some((*raw_id, namespace.clone())),
            Type::TypeVarTupleType {
                raw_id, namespace, ..
            } => Some((*raw_id, namespace.clone())),
            _ => None,
        }
    };
    // A var with multiple lowers is joined by the solver. When the joined
    // solution nests a FunctionLike, the nested FuncDef definitions do not
    // survive the wire (pretty_callable needs them): defer those joins.
    let mut lowers_by_var: std::collections::HashMap<(i64, String), usize> =
        std::collections::HashMap::new();
    for c in &all_constraints {
        if c.op == crate::constraints::SUPERTYPE_OF {
            if let Some(key) = tv_key(&c.origin_type_var) {
                *lowers_by_var.entry(key).or_insert(0) += 1;
            }
        }
    }
    let multi_lower_vars: std::collections::HashSet<(i64, String)> = lowers_by_var
        .iter()
        .filter(|(_, &n)| n >= 2)
        .map(|(k, _)| k.clone())
        .collect();

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
        strict_optional,
        false, // skip_unsatisfied: mirror infer_function_type_arguments
        resolver,
    );

    let Some((_status, sol_blob, _free_blob)) = solve_result else {
        return None; // Solver deferred.
    };

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
    // A var the solver could not pin (solve_one returned no solution,
    // e.g. an invariant conflict like T :> str, T <: Never) makes Python
    // emit "Cannot infer value of type parameter" + substitute Any: defer.
    if orig_types.iter().any(|t| t.is_none()) {
        return None;
    }
    // Multi-lower joins whose solution is a FunctionLike (or nests one)
    // lose nested FuncDef definitions over the wire (see comment above
    // where multi_lower_vars is built): defer to Python.
    if !multi_lower_vars.is_empty() {
        let nested_fnlike = orig_types.iter().zip(variables.iter()).any(|(s, tv)| {
            let Some(sol) = s else { return false };
            if !contains_function_like(sol) {
                return false;
            }
            match solve_typevar_key(tv) {
                Some((raw, _meta, ns)) => multi_lower_vars.contains(&(raw, ns)),
                None => false,
            }
        });
        if nested_fnlike {
            return None;
        }
    }

    // Serialize orig_types in the wire format expected by apply_generic_arguments.
    let orig_types_blob = serialize_optional_types(&orig_types)?;

    let mut wbuf = crate::wire::WriteBuffer::new();
    crate::wire::write_type(&mut wbuf, &normalized).ok()?;
    let normalized_bytes = wbuf.into_bytes();
    let applied = crate::applytype::rust_apply_generic_arguments(
        resolver,
        normalized_bytes.as_slice(),
        &orig_types_blob,
        false, // skip_unsatisfied: mirror the Python inference path
        strict_optional,
    )?;
    Some(applied)
}

/// Whether a type (recursively) contains a FunctionLike (CallableType or
/// Overloaded). Conservative recursion over the wire shape; used to decide
/// whether a joined multi-lower solution would lose nested FuncDef
/// definitions in the round-trip.
fn contains_function_like(t: &Type) -> bool {
    match t {
        Type::CallableType { .. } | Type::Overloaded { .. } => true,
        Type::Instance { args, .. } => args.iter().any(contains_function_like),
        Type::UnionType { items, .. } => items.iter().any(contains_function_like),
        Type::TupleType { items, .. } => items.iter().any(contains_function_like),
        Type::TypeAliasType { args, .. } => args.iter().any(contains_function_like),
        Type::UnpackType { typ } => contains_function_like(typ),
        Type::TypeType { item, .. } => contains_function_like(item),
        Type::Parameters(params) => params.arg_types.iter().any(contains_function_like),
        Type::NoneType
        | Type::AnyType { .. }
        | Type::TypeVarType { .. }
        | Type::ParamSpecType { .. }
        | Type::TypeVarTupleType { .. }
        | Type::UnboundType { .. }
        | Type::TypedDictType { .. }
        | Type::LiteralType { .. }
        | Type::UninhabitedType { .. }
        | Type::ErasedType
        | Type::DeletedType { .. } => false,
    }
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
/// (0 = None, 1 = present) followed by a Type blob if present. The count
/// uses a bare int (`write_int_bare`): `decode_optional_type_list`
/// (applytype.rs:540) reads it with `read_int_bare`, and a tagged
/// `write_int` would insert a LITERAL_INT tag byte the reader treats as
/// the int's first byte.
fn serialize_optional_types(types: &[Option<Type>]) -> Option<Vec<u8>> {
    let mut buf = crate::wire::WriteBuffer::new();
    crate::wire::write_int_bare(&mut buf, types.len() as i64).ok()?;
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

// ---------------------------------------------------------------------------
// get_arg_infer_passes (checkexpr.py:3561)
// ---------------------------------------------------------------------------

/// `ArgInferSecondPassQuery` (checkexpr.py:8569): a `BoolTypeQuery` with
/// `ANY_STRATEGY` whose only override is `visit_callable_type`
/// (`query_types(t.arg_types) or has_type_vars(t)`); every other node kind
/// takes the base `BoolTypeQuery` walk. Returns `None` when the walk hits
/// an alias node the wire cannot decide (missing snapshot, cycle, or a
/// substitution the kernel cannot mirror) so the caller defers. `seen`
/// mirrors the visitor's `seen_aliases` cycle guard.
fn arg_infer_second_pass(
    t: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
    seen: &mut Vec<String>,
) -> Option<bool> {
    // ANY_STRATEGY fold with Python's `any()` short-circuit: an earlier
    // True wins before a later deferring item is ever consulted.
    fn any_second_pass(
        types: &[Type],
        aliases: &crate::aliases::TypeAliasResolver,
        seen: &mut Vec<String>,
    ) -> Option<bool> {
        for t in types {
            if arg_infer_second_pass(t, aliases, seen)? {
                return Some(true);
            }
        }
        Some(false)
    }
    match t {
        Type::CallableType { arg_types, .. } => {
            // visit_callable_type override: arg walk or has_type_vars(t).
            // has_type_vars covers ret_type + instance_type recursively.
            if any_second_pass(arg_types, aliases, seen)? {
                return Some(true);
            }
            Some(has_type_vars_arg_query(t))
        }
        Type::TypeAliasType { type_ref, args, .. } => {
            // visit_type_alias_type (type_visitor.py:601): expand via
            // get_proper_type, guard cycles with `seen_aliases`, then
            // `res or (python_3_12_type_alias and query_types(t.args))`.
            if seen.contains(type_ref) {
                return Some(false);
            }
            seen.push(type_ref.clone());
            let (target, _, py312) = crate::checkexpr_functions::expanded_alias_target(t, aliases)?;
            let mut res = arg_infer_second_pass(&target, aliases, seen)?;
            if !res && py312 {
                res = any_second_pass(args, aliases, seen)?;
            }
            Some(res)
        }
        Type::UnboundType { args, .. } => any_second_pass(args, aliases, seen),
        Type::TypeVarType {
            upper_bound,
            default,
            values,
            ..
        } => Some(
            arg_infer_second_pass(upper_bound, aliases, seen)?
                || arg_infer_second_pass(default, aliases, seen)?
                || any_second_pass(values, aliases, seen)?,
        ),
        Type::ParamSpecType {
            prefix,
            upper_bound,
            default,
            ..
        } => Some(
            arg_infer_second_pass(upper_bound, aliases, seen)?
                || arg_infer_second_pass(default, aliases, seen)?
                || any_second_pass(&prefix.arg_types, aliases, seen)?,
        ),
        Type::TypeVarTupleType {
            upper_bound,
            default,
            ..
        } => Some(
            arg_infer_second_pass(upper_bound, aliases, seen)?
                || arg_infer_second_pass(default, aliases, seen)?,
        ),
        Type::UnpackType { typ } => arg_infer_second_pass(typ, aliases, seen),
        Type::Parameters(p) => any_second_pass(&p.arg_types, aliases, seen),
        Type::Instance { args, .. } => any_second_pass(args, aliases, seen),
        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => Some(
            arg_infer_second_pass(partial_fallback, aliases, seen)?
                || any_second_pass(items, aliases, seen)?,
        ),
        // TypedDict: item values only (no fallback in BoolTypeQuery).
        Type::TypedDictType { items, .. } => {
            let values: Vec<Type> = items.iter().map(|(_, t)| t.clone()).collect();
            any_second_pass(&values, aliases, seen)
        }
        Type::UnionType { items, .. } => any_second_pass(items, aliases, seen),
        Type::Overloaded { items } => any_second_pass(items, aliases, seen),
        Type::TypeType { item, .. } => arg_infer_second_pass(item, aliases, seen),
        // AnyType, UninhabitedType, NoneType, ErasedType, DeletedType,
        // LiteralType, RawExpressionType, EllipsisType: BoolTypeQuery
        // defaults, all False under ANY_STRATEGY.
        _ => Some(false),
    }
}

/// `mypy.types.HasTypeVars` semantics exactly (skip_alias_target=True,
/// typevar-like leaves are True, base `BoolTypeQuery` walk otherwise).
/// Unlike `visitor::has_type_vars_inner`, this never walks callable
/// `variables`, Instance `last_known_value`, `AnyType.source_any`, or
/// TypedDict/Tuple/ literal fallbacks, mirroring the Python visitor.
fn has_type_vars_arg_query(t: &Type) -> bool {
    match t {
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::TypeVarTupleType { .. } => {
            true
        }
        Type::UnboundType { args, .. } => args.iter().any(has_type_vars_arg_query),
        Type::UnpackType { typ } => has_type_vars_arg_query(typ),
        Type::Parameters(p) => p.arg_types.iter().any(has_type_vars_arg_query),
        Type::Instance { args, .. } => args.iter().any(has_type_vars_arg_query),
        Type::CallableType {
            arg_types,
            ret_type,
            instance_type,
            ..
        } => {
            arg_types.iter().any(has_type_vars_arg_query)
                || has_type_vars_arg_query(ret_type)
                || instance_type
                    .as_ref()
                    .is_some_and(|t| has_type_vars_arg_query(t))
        }
        Type::Overloaded { items } => items.iter().any(has_type_vars_arg_query),
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => items.iter().any(has_type_vars_arg_query) || has_type_vars_arg_query(partial_fallback),
        Type::TypedDictType { items, .. } => items.iter().any(|(_, t)| has_type_vars_arg_query(t)),
        Type::UnionType { items, .. } => items.iter().any(has_type_vars_arg_query),
        Type::TypeType { item, .. } => has_type_vars_arg_query(item),
        Type::TypeAliasType { args, .. } => {
            // skip_alias_target: wire has no alias target; args only.
            args.iter().any(has_type_vars_arg_query)
        }
        // Leaves: AnyType, UninhabitedType, NoneType, ErasedType,
        // DeletedType, LiteralType. Base BoolTypeQuery returns the default
        // (False under ANY_STRATEGY) without recursion.
        _ => false,
    }
}

/// `ExpressionChecker.get_arg_infer_passes` (checkexpr.py:3561-3608):
/// two-pass argument-inference classification. For each formal of the
/// callee, decide pass 1 vs pass 2 for its actuals:
///   * a ParamSpec-carrying CallableType formal whose actuals are all
///     "concrete" (a non-generic non-lambda CallableType, possibly after
///     `find_member("__call__", ...)` on an Instance actual) suppresses
///     the second pass for that formal;
///   * otherwise `ArgInferSecondPassQuery(formal)` promotes its actuals
///     to pass 2.
///
/// Pure decision: Python keeps the result application and every side
/// effect. Defers (`None`) on any undecodable blob, alias-expansion
/// failure, out-of-range index, or a `find_member` case the kernel
/// cannot decide (see `find_member_call_is_plain_callable`).
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub fn rust_get_arg_infer_passes(
    py: Python<'_>,
    resolver: &crate::typeinfo::NativeTypeResolver,
    formal_bytes: Vec<Vec<u8>>,
    actual_bytes: Vec<Vec<u8>>,
    lambda_flags: Vec<bool>,
    formal_to_actual: Vec<Vec<i64>>,
    num_actuals: usize,
) -> Option<Vec<i64>> {
    let decode_blob = |b: &[u8]| -> Option<Type> {
        let mut buf = ReadBuffer::new(b);
        read_type(&mut buf, None).ok()
    };
    const ARG_STAR: i64 = 2;
    const ARG_STAR2: i64 = 4;
    let formals: Vec<Type> = formal_bytes
        .iter()
        .map(|b| decode_blob(b))
        .collect::<Option<Vec<_>>>()?;
    let actuals: Vec<Type> = actual_bytes
        .iter()
        .map(|b| decode_blob(b))
        .collect::<Option<Vec<_>>>()?;
    let mut res = vec![1i64; num_actuals];
    let aliases = resolver.alias_resolver();
    for (i, formal) in formals.iter().enumerate() {
        // p_formal = get_proper_type(callee.arg_types[i]).
        let (p_formal, _, _) = crate::checkexpr_functions::expanded_alias_target(formal, aliases)?;
        // CallableType.param_spec() gate (types.py:2701-2721): the last two
        // parameters must be *args: P.args, **kwargs: P.kwargs.
        let has_param_spec = match &p_formal {
            Type::CallableType {
                arg_types,
                arg_kinds,
                ..
            } if arg_types.len() >= 2
                && arg_kinds.get(arg_types.len() - 2) == Some(&ARG_STAR)
                && arg_kinds.last() == Some(&ARG_STAR2) =>
            {
                matches!(arg_types[arg_types.len() - 2], Type::ParamSpecType { .. })
            }
            _ => false,
        };
        let mut skip_param_spec = false;
        if has_param_spec {
            for &j in formal_to_actual.get(i)? {
                let j = usize::try_from(j).ok()?;
                let lambda_flag = *lambda_flags.get(j)?;
                let p_actual = &actuals.get(j)?;
                let trigger = match p_actual {
                    Type::Instance { .. } => {
                        crate::checker_helpers::find_member_call_is_plain_callable(
                            py, p_actual, resolver,
                        )? && !lambda_flag
                    }
                    Type::CallableType { variables, .. } => variables.is_empty() && !lambda_flag,
                    _ => false,
                };
                if trigger {
                    skip_param_spec = true;
                    break;
                }
            }
        }
        if !skip_param_spec {
            // Fresh query per formal: Python builds a new
            // ArgInferSecondPassQuery for each `arg.accept(...)` call.
            let mut seen: Vec<String> = Vec::new();
            if arg_infer_second_pass(formal, aliases, &mut seen)? {
                for &j in formal_to_actual.get(i)? {
                    let j = usize::try_from(j).ok()?;
                    *res.get_mut(j)? = 2;
                }
            }
        }
    }
    Some(res)
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

    fn calibrate_bytes(callee: &Type, arg: &Type) -> Option<Type> {
        let mut cb = WriteBuffer::new();
        write_type(&mut cb, callee).ok()?;
        let mut ab = WriteBuffer::new();
        write_type(&mut ab, arg).ok()?;
        let out = rust_calibrate_type_obj_return(&cb.into_bytes(), &ab.into_bytes())?;
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
            from_type_type: false,
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
            from_type_type: false,
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
            from_type_type: false,
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

    fn empty_resolver() -> crate::typeinfo::NativeTypeResolver {
        crate::typeinfo::NativeTypeResolver::new(
            crate::typeinfo::TypeResolver::new(),
            crate::aliases::TypeAliasResolver::new(),
        )
    }

    fn alias_snap(fullname: &str, target: &Type) -> crate::aliases::TypeAliasSnapshot {
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, target).expect("alias target must encode");
        crate::aliases::TypeAliasSnapshot {
            fullname: fullname.to_string(),
            target: buf.into_bytes(),
            ..Default::default()
        }
    }

    fn resolver_with_alias(alias: &str, target: &Type) -> crate::typeinfo::NativeTypeResolver {
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(alias.to_string(), alias_snap(alias, target));
        crate::typeinfo::NativeTypeResolver::new(crate::typeinfo::TypeResolver::new(), aliases)
    }

    fn alias_type(type_ref: &str) -> Type {
        Type::TypeAliasType {
            args: vec![],
            type_ref: type_ref.to_string(),
        }
    }

    fn real_union_bytes(typ: &Type, strict_optional: bool) -> Option<bool> {
        rust_real_union(&empty_resolver(), &encode(typ), strict_optional)
    }

    fn overlap_bytes(args: &[Type], targets: &[Type]) -> Option<bool> {
        rust_possible_none_type_var_overlap(
            &empty_resolver(),
            args.iter().map(encode).collect(),
            targets.iter().map(encode).collect(),
        )
    }

    #[test]
    fn real_union_non_union_returns_false() {
        let t = any_type();
        assert_eq!(real_union_bytes(&t, true), Some(false));
    }

    #[test]
    fn real_union_single_item_returns_false() {
        let t = union_of(vec![any_type()]);
        assert_eq!(real_union_bytes(&t, true), Some(false));
    }

    #[test]
    fn real_union_multi_item_returns_true() {
        let t = union_of(vec![any_type(), instance()]);
        assert_eq!(real_union_bytes(&t, true), Some(true));
    }

    #[test]
    fn real_union_strips_none_when_not_strict() {
        // Union[int, None] with strict_optional=False: relevant = [int], count=1 -> false
        let t = union_of(vec![instance(), none_type()]);
        assert_eq!(real_union_bytes(&t, false), Some(false));
    }

    #[test]
    fn real_union_keeps_none_when_strict() {
        // Union[int, None] with strict_optional=True: relevant = [int, None], count=2 -> true
        let t = union_of(vec![instance(), none_type()]);
        assert_eq!(real_union_bytes(&t, true), Some(true));
    }

    #[test]
    fn real_union_alias_to_union_resolves() {
        // T = Union[A, B]; real_union(T) counts the two items -> true.
        let t = union_of(vec![any_type(), instance()]);
        let res = resolver_with_alias("mod.T", &t);
        assert_eq!(
            rust_real_union(&res, &encode(&alias_type("mod.T")), true),
            Some(true)
        );
    }

    #[test]
    fn real_union_alias_to_single_union_false() {
        // T = Union[A]; real_union(T) -> one relevant item -> false.
        let t = union_of(vec![any_type()]);
        let res = resolver_with_alias("mod.T", &t);
        assert_eq!(
            rust_real_union(&res, &encode(&alias_type("mod.T")), true),
            Some(false)
        );
    }

    #[test]
    fn real_union_alias_missing_snapshot_defers() {
        // No snapshot for mod.T: the expansion cannot resolve -> defer.
        assert_eq!(
            rust_real_union(&empty_resolver(), &encode(&alias_type("mod.T")), true),
            None
        );
    }

    #[test]
    fn real_union_alias_to_non_union_false() {
        // T = A (non-union); after expansion the result is Some(false).
        let res = resolver_with_alias("mod.T", &instance());
        assert_eq!(
            rust_real_union(&res, &encode(&alias_type("mod.T")), true),
            Some(false)
        );
    }

    #[test]
    fn none_overlap_empty_args_returns_false() {
        assert_eq!(overlap_bytes(&[], &[callable(0)]), Some(false));
    }

    #[test]
    fn none_overlap_no_targets_returns_false() {
        assert_eq!(overlap_bytes(&[any_type()], &[]), Some(false));
    }

    #[test]
    fn none_overlap_no_union_arg_returns_false() {
        let arg = any_type();
        let target = callable_with_args(vec![none_type(), type_var()]);
        assert_eq!(overlap_bytes(&[arg], &[target]), Some(false));
    }

    #[test]
    fn none_overlap_union_without_none_returns_false() {
        let arg = union_of(vec![any_type(), instance()]);
        let target = callable_with_args(vec![none_type(), type_var()]);
        assert_eq!(overlap_bytes(&[arg], &[target]), Some(false));
    }

    #[test]
    fn none_overlap_union_with_none_no_typevar_returns_false() {
        let arg = union_of(vec![instance(), none_type()]);
        let target = callable_with_args(vec![none_type(), any_type()]);
        assert_eq!(overlap_bytes(&[arg], &[target]), Some(false));
    }

    #[test]
    fn none_overlap_union_with_none_and_typevar_returns_true() {
        let arg = union_of(vec![instance(), none_type()]);
        let target1 = callable_with_args(vec![none_type()]);
        let target2 = callable_with_args(vec![type_var()]);
        assert_eq!(overlap_bytes(&[arg], &[target1, target2]), Some(true));
    }

    #[test]
    fn none_overlap_single_target_none_and_typevar_different_pos_returns_false() {
        // NoneType at pos 0, TypeVar at pos 1: neither position has both.
        let arg = union_of(vec![instance(), none_type()]);
        let target = callable_with_args(vec![none_type(), type_var()]);
        assert_eq!(overlap_bytes(&[arg], &[target]), Some(false));
    }

    #[test]
    fn none_overlap_typevar_in_different_position_returns_false() {
        let arg = union_of(vec![instance(), none_type()]);
        let target1 = callable_with_args(vec![none_type(), any_type()]);
        let target2 = callable_with_args(vec![any_type(), type_var()]);
        assert_eq!(overlap_bytes(&[arg], &[target1, target2]), Some(false));
    }

    #[test]
    fn none_overlap_alias_arg_to_union_resolves() {
        // T = Union[A, None]; overlap(T, [None], [T'var]) -> NoneType present
        // after expansion, a target has NoneType / another a TypeVar -> true.
        let arg_union = union_of(vec![instance(), none_type()]);
        let arg_alias = alias_type("mod.T");
        let res = resolver_with_alias("mod.T", &arg_union);
        let target1 = callable_with_args(vec![none_type()]);
        let target2 = callable_with_args(vec![type_var()]);
        assert_eq!(
            rust_possible_none_type_var_overlap(
                &res,
                vec![encode(&arg_alias)],
                vec![encode(&target1), encode(&target2)],
            ),
            Some(true)
        );
    }

    #[test]
    fn none_overlap_alias_missing_snapshot_defers() {
        // Alias with no snapshot: the expansion defers.
        let arg = alias_type("mod.T");
        let target1 = callable_with_args(vec![none_type()]);
        let target2 = callable_with_args(vec![type_var()]);
        assert_eq!(overlap_bytes(&[arg], &[target1, target2]), None);
    }

    #[test]
    fn none_overlap_alias_formal_resolves() {
        // A formal typed as an alias expands to NoneType; another formal is
        // a TypeVarType -> the None+TypeVar overlap is found at that pos.
        let res = resolver_with_alias("mod.N", &none_type());
        let arg = union_of(vec![instance(), none_type()]);
        let target1 = callable_with_args(vec![alias_type("mod.N")]);
        let target2 = callable_with_args(vec![type_var()]);
        assert_eq!(
            rust_possible_none_type_var_overlap(
                &res,
                vec![encode(&arg)],
                vec![encode(&target1), encode(&target2)],
            ),
            Some(true)
        );
    }

    fn str_instance() -> Type {
        Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn int_instance() -> Type {
        Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn calibrate_type_obj_wraps_ret_in_typetype() {
        // Type[type(T)] call: callee ret becomes Type[str].
        let callee = callable_with_args(vec![instance()]);
        let out = calibrate_bytes(&callee, &str_instance()).unwrap();
        match out {
            Type::CallableType { ret_type, .. } => {
                assert_eq!(
                    *ret_type,
                    Type::TypeType {
                        item: Box::new(str_instance()),
                        is_type_form: false,
                    }
                );
            }
            other => panic!("expected CallableType, got {other:?}"),
        }
    }

    #[test]
    fn calibrate_type_obj_distributes_union() {
        // Type[Union[int, str]] -> Union[Type[int], Type[str]].
        let callee = callable_with_args(vec![instance()]);
        let arg = union_of(vec![int_instance(), str_instance()]);
        let out = calibrate_bytes(&callee, &arg).unwrap();
        let Type::CallableType { ret_type, .. } = out else {
            panic!("expected CallableType");
        };
        assert_eq!(
            *ret_type,
            union_of(vec![
                Type::TypeType {
                    item: Box::new(int_instance()),
                    is_type_form: false,
                },
                Type::TypeType {
                    item: Box::new(str_instance()),
                    is_type_form: false,
                },
            ])
        );
    }

    #[test]
    fn calibrate_type_obj_keeps_other_fields() {
        // Calibration must only rewrite ret_type; identity of other fields
        // is preserved (mirrors copy_modified in types.py).
        let mut callee = callable_with_args(vec![instance()]);
        if let Type::CallableType { variables, .. } = &mut callee {
            variables.push(type_var());
        }
        let out = calibrate_bytes(&callee, &str_instance()).unwrap();
        let Type::CallableType {
            arg_types,
            variables,
            ret_type,
            ..
        } = out
        else {
            panic!("expected CallableType");
        };
        assert_eq!(arg_types, vec![instance()]);
        assert_eq!(variables.len(), 1);
        assert_eq!(
            *ret_type,
            Type::TypeType {
                item: Box::new(str_instance()),
                is_type_form: false,
            }
        );
    }

    #[test]
    fn calibrate_type_obj_defers_type_alias_arg() {
        // TypeAliasType has no resolved target on the wire; both the
        // caller and Python's get_proper_type would resolve it, so defer.
        let callee = callable_with_args(vec![instance()]);
        let alias = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(calibrate_bytes(&callee, &alias), None);
    }

    #[test]
    fn calibrate_type_obj_defers_non_callable() {
        // Only CallableType callees are calibrated; anything else defers.
        assert_eq!(calibrate_bytes(&instance(), &str_instance()), None);
    }

    fn type_obj_callable(instance_type: bool, ret: Type) -> Type {
        Type::CallableType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.type".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            instance_type: if instance_type {
                Some(Box::new(Type::Instance {
                    type_ref: "builtins.type".to_string(),
                    args: Vec::new(),
                    last_known_value: None,
                    extra_attrs: None,
                }))
            } else {
                None
            },
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![str_instance()],
            arg_kinds: vec![ARG_POS],
            arg_names: vec![None],
            ret_type: Box::new(ret),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn checkcall_tail_type_obj_calibrates() {
        // Type[type] call with one str arg: ret becomes Type[str].
        let callee = type_obj_callable(true, any_type());
        let arg = str_instance();
        let base = check_callable_call_tail(&callee, &[arg]).unwrap();
        match base {
            Type::CallableType { ret_type, .. } => {
                assert_eq!(
                    *ret_type,
                    Type::TypeType {
                        item: Box::new(str_instance()),
                        is_type_form: false,
                    }
                );
            }
            other => panic!("expected CallableType, got {other:?}"),
        }
    }

    #[test]
    fn checkcall_tail_ret_fallback_calibrates() {
        // instance_type None but ret_type is Instance builtins.type;
        // get_instance_type(force_fallback=True) walks to builtins.type.
        let callee = type_obj_callable(false, instance_type());
        let out = check_callable_call_tail(&callee, &[str_instance()]).unwrap();
        let Type::CallableType { ret_type, .. } = out else {
            panic!("expected CallableType");
        };
        assert_eq!(
            *ret_type,
            Type::TypeType {
                item: Box::new(str_instance()),
                is_type_form: false,
            }
        );
    }

    fn instance_type() -> Type {
        Type::Instance {
            type_ref: "builtins.type".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn checkcall_tail_non_type_obj_defers() {
        // Fallback is not builtins.type -> is_type_obj False, Python must
        // keep definition/plugin-hook behavior, so the tail defers.
        let callee = callable_with_args(vec![str_instance()]);
        assert_eq!(check_callable_call_tail(&callee, &[str_instance()]), None);
    }

    #[test]
    fn checkcall_tail_uninhabited_not_type_obj() {
        // UninhabitedType ret -> is_type_obj False, defer to Python.
        let callee = type_obj_callable(true, Type::UninhabitedType { ambiguous: true });
        assert_eq!(check_callable_call_tail(&callee, &[str_instance()]), None);
    }

    #[test]
    fn checkcall_tail_two_args_defers() {
        // arg_types.len() != 1 -> calibration cannot fire, defer.
        let mut callee = type_obj_callable(true, any_type());
        if let Type::CallableType { arg_types, .. } = &mut callee {
            arg_types.push(int_instance());
        }
        assert_eq!(
            check_callable_call_tail(&callee, &[str_instance(), int_instance()]),
            None
        );
    }

    #[test]
    fn checkcall_tail_type_alias_arg_defers() {
        // TypeAliasType arg cannot be resolved on the wire -> defer.
        let callee = type_obj_callable(true, any_type());
        let alias = Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(check_callable_call_tail(&callee, &[alias]), None);
    }

    #[test]
    fn checkcall_tail_walkable_instance_type_defers() {
        // instance_type Some(TypeVar) could force-fallback to builtins.type
        // in Python; wire cannot resolve -> defer.
        let mut callee = type_obj_callable(true, any_type());
        if let Type::CallableType { instance_type, .. } = &mut callee {
            *instance_type = Some(Box::new(type_var()));
        }
        assert_eq!(check_callable_call_tail(&callee, &[str_instance()]), None);
    }

    #[test]
    fn checkcall_tail_non_walkable_instance_defers() {
        // instance_type Some(Instance mod.C) is not walkable to builtins.type
        // and target_is_type False -> defer to Python.
        let mut callee = type_obj_callable(true, any_type());
        if let Type::CallableType { instance_type, .. } = &mut callee {
            *instance_type = Some(Box::new(instance()));
        }
        assert_eq!(check_callable_call_tail(&callee, &[str_instance()]), None);
    }

    // ------------------------------------------------------------------
    // rust_solve_generic_call
    // ------------------------------------------------------------------

    fn test_resolver() -> crate::typeinfo::NativeTypeResolver {
        let mut r = crate::typeinfo::TypeResolver::new();
        for snap in [
            solve_snap("builtins.int"),
            solve_snap("builtins.str"),
            solve_snap("builtins.object"),
        ] {
            r.insert(snap.fullname.clone(), snap);
        }
        crate::typeinfo::NativeTypeResolver::from_resolver(r)
    }

    fn solve_snap(fullname: &str) -> crate::typeinfo::TypeInfoSnapshot {
        let mut s = crate::typeinfo::TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        if fullname != "builtins.object" && !s.has_base.contains("builtins.object") {
            s.mro.push("builtins.object".to_string());
            s.has_base.insert("builtins.object".to_string());
            // The join_instances_via_supertype walk recurses over
            // `bases` (not `mro`), so direct-object subclasses need the
            // base blob populated or unrelated-nominal joins defer.
            s.bases.push(encode(&Type::Instance {
                type_ref: "builtins.object".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }));
        }
        s
    }

    fn generic_identity() -> Type {
        // def [T] (x: T) -> T
        Type::CallableType {
            fallback: Box::new(instance()),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![type_var()],
            arg_kinds: vec![ARG_POS],
            arg_names: vec![None],
            ret_type: Box::new(type_var()),
            name: None,
            variables: vec![type_var()],
            type_guard: None,
            type_is: None,
        }
    }

    fn solve_generic_bytes(
        callee: &Type,
        arg_types: &[Type],
        formal_to_actual: Vec<Vec<i64>>,
    ) -> Option<Vec<u8>> {
        solve_generic_bytes_with(&test_resolver(), callee, arg_types, formal_to_actual)
    }

    fn solve_generic_bytes_with(
        resolver: &crate::typeinfo::NativeTypeResolver,
        callee: &Type,
        arg_types: &[Type],
        formal_to_actual: Vec<Vec<i64>>,
    ) -> Option<Vec<u8>> {
        let mut cb = WriteBuffer::new();
        write_type(&mut cb, callee).ok()?;
        let arg_blobs: Vec<Vec<u8>> = arg_types
            .iter()
            .map(|t| {
                let mut b = WriteBuffer::new();
                write_type(&mut b, t).ok()?;
                Some(b.into_bytes())
            })
            .collect::<Option<_>>()?;
        pyo3::prepare_freethreaded_python();
        pyo3::Python::with_gil(|py| {
            rust_solve_generic_call(
                py,
                &resolver,
                &cb.into_bytes(),
                arg_blobs,
                formal_to_actual,
                true,
                false,
                true,
            )
        })
    }

    fn solved_typevar(t: &Type) -> bool {
        // The returned callable's variables are empty when the single
        // TypeVar was solved and applied (full resolution).
        let Type::CallableType { variables, .. } = t else {
            return false;
        };
        variables.is_empty()
    }

    #[test]
    fn solve_generic_identity_int() {
        // identity(int) should resolve T=int and fully apply it.
        let callee = generic_identity();
        let arg = int_instance();
        let out = solve_generic_bytes(&callee, &[arg], vec![vec![0]]);
        assert!(out.is_some(), "expected successful solve, got deferral");
        let bytes = out.unwrap();
        let mut rb = ReadBuffer::new(&bytes);
        let resolved = read_type(&mut rb, None).unwrap();
        assert!(
            solved_typevar(&resolved),
            "expected fully-resolved callable"
        );
    }

    #[test]
    fn solve_generic_identity_str() {
        let callee = generic_identity();
        let arg = str_instance();
        let out = solve_generic_bytes(&callee, &[arg], vec![vec![0]]);
        assert!(out.is_some(), "expected successful solve, got deferral");
    }

    #[test]
    fn solve_generic_empty_constraints_solves() {
        // No actuals -> no constraints. The solver fills the unconstrained
        // var with strict Never (lax Any for strict=False), and
        // get_target_type expands the var's default over it (solve.py:277).
        let callee = generic_identity();
        let out = solve_generic_bytes(&callee, &[], vec![]);
        assert!(out.is_some(), "expected solve on empty constraints");
        let bytes = out.unwrap();
        let mut rb = ReadBuffer::new(&bytes);
        let resolved = read_type(&mut rb, None).unwrap();
        assert!(
            solved_typevar(&resolved),
            "expected fully-resolved callable with T=default"
        );
    }

    #[test]
    fn solve_generic_multilower_joins() {
        // def [T] (a: T, b: T) -> T with int + str: two lowers are joined
        // by solve_one_inner instead of the old deferral for the
        // diagnostic; a satisfiable join solves natively.
        let mut two = generic_identity();
        if let Type::CallableType {
            arg_types,
            variables,
            ..
        } = &mut two
        {
            let tv = variables[0].clone();
            arg_types.push(tv.clone());
            variables.push(tv);
        }
        let out = solve_generic_bytes(
            &two,
            &[int_instance(), str_instance()],
            vec![vec![0], vec![1]],
        );
        assert!(out.is_some(), "expected multi-lower join to solve");
        let bytes = out.unwrap();
        let mut rb = ReadBuffer::new(&bytes);
        let resolved = read_type(&mut rb, None).unwrap();
        assert!(solved_typevar(&resolved), "expected resolved callable");
    }

    #[test]
    fn solve_generic_multilower_callable_defers() {
        // Two callable lowers join to a FunctionLike; the live FuncDef
        // definitions nested callables carry do not survive the wire and
        // Python's pretty_callable needs them (def NAME(...) rendering).
        let mut two = generic_identity();
        if let Type::CallableType {
            arg_types,
            variables,
            ..
        } = &mut two
        {
            let tv = variables[0].clone();
            arg_types.push(tv.clone());
            variables.push(tv);
        }
        let f = callable(0);
        let g = callable(0);
        let out = solve_generic_bytes(&two, &[f, g], vec![vec![0], vec![1]]);
        assert!(out.is_none(), "expected deferral on callable join");
    }

    #[test]
    fn solve_generic_tuple_actual_solves() {
        // A positional TupleType actual is passed 1:1 by ArgTypeExpander
        // (star actuals are gated Python-side): T :> tuple[int, str].
        let callee = generic_identity();
        let tup = Type::TupleType {
            partial_fallback: Box::new(instance()),
            items: vec![int_instance(), str_instance()],
            implicit: false,
        };
        let out = solve_generic_bytes(&callee, &[tup], vec![vec![0]]);
        assert!(out.is_some(), "expected tuple actual to solve");
    }

    #[test]
    fn solve_generic_any_actuals_solves() {
        // AnyType actuals are NOT skipped: the template yields T :> Any,
        // so the solve succeeds and T resolves to Any. The old Any-skip
        // left the lower bound empty and broke joins (testNativeIntJoins).
        let callee = generic_identity();
        let arg = any_type();
        let out = solve_generic_bytes(&callee, &[arg], vec![vec![0]]);
        assert!(out.is_some(), "expected solve with T :> Any, got deferral");
        let bytes = out.unwrap();
        let mut rb = ReadBuffer::new(&bytes);
        let resolved = read_type(&mut rb, None).unwrap();
        assert!(
            solved_typevar(&resolved),
            "expected fully-resolved callable with T=Any"
        );
    }

    fn param_spec() -> Type {
        Type::ParamSpecType {
            name: "P".to_string(),
            fullname: "mod.P".to_string(),
            raw_id: 2,
            namespace: "mod".to_string(),
            flavor: 0,
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            prefix: Box::new(crate::wire::Parameters {
                arg_types: Vec::new(),
                arg_kinds: Vec::new(),
                arg_names: Vec::new(),
                variables: Vec::new(),
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
        }
    }

    fn callable_ret_with_args(arg_types: Vec<Type>, ret: Type) -> Type {
        let mut t = callable_with_args(arg_types);
        if let Type::CallableType { ret_type, .. } = &mut t {
            **ret_type = ret;
        }
        t
    }

    /// `Callable[P, Any]` in the shape CallableType.param_spec() accepts:
    /// trailing *args: P.args, **kwargs: P.kwargs and variables=[P].
    fn param_spec_callable() -> Type {
        let mut t = callable_with_args(vec![param_spec(), param_spec()]);
        if let Type::CallableType {
            arg_kinds,
            variables,
            ..
        } = &mut t
        {
            *arg_kinds = vec![2, 4]; // ARG_STAR, ARG_STAR2
            *variables = vec![param_spec()];
        }
        t
    }

    fn infer_passes(
        formals: &[Type],
        actuals: &[Type],
        lambda_flags: Vec<bool>,
        formal_to_actual: Vec<Vec<i64>>,
        num_actuals: usize,
        resolver: &crate::typeinfo::NativeTypeResolver,
    ) -> Option<Vec<i64>> {
        pyo3::prepare_freethreaded_python();
        pyo3::Python::with_gil(|py| {
            rust_get_arg_infer_passes(
                py,
                resolver,
                formals.iter().map(encode).collect(),
                actuals.iter().map(encode).collect(),
                lambda_flags,
                formal_to_actual,
                num_actuals,
            )
        })
    }

    #[test]
    fn arg_infer_plain_formal_pass_one() {
        // No typevars anywhere in the formal: every actual stays pass 1.
        let out = infer_passes(
            &[callable(0)],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![1]));
    }

    #[test]
    fn arg_infer_plain_typevar_formal_pass_one() {
        // A bare TypeVarType formal stays pass 1: BoolTypeQuery walks only
        // its upper_bound/default/values (all Any here), not the typevar
        // itself. Promotion comes from typevars in callable returns.
        let out = infer_passes(
            &[type_var()],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![1]));
    }

    #[test]
    fn arg_infer_typevar_in_callable_ret_pass_two() {
        // Callable[[], T] (typevar in a nested callable return) promotes
        // its actual to pass 2.
        let formal = callable_with_args(vec![callable_ret_with_args(Vec::new(), type_var())]);
        let out = infer_passes(
            &[formal],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![2]));
    }

    #[test]
    fn arg_infer_callable_typevar_in_variables_pass_one() {
        // Python's HasTypeVars never walks callable `variables`: a generic
        // callable formal whose variables are only declared (not used in
        // arg/ret types) stays pass 1.
        let formal = callable(1);
        let out = infer_passes(
            &[formal],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![1]));
    }

    #[test]
    fn arg_infer_alias_typevar_target_pass_two() {
        // Alias expanding to a TypeVar-carrying callable promotes pass 2.
        let target = callable_with_args(vec![type_var()]);
        let res = resolver_with_alias("mod.A", &target);
        let out = infer_passes(
            &[alias_type("mod.A")],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &res,
        );
        assert_eq!(out, Some(vec![2]));
    }

    #[test]
    fn arg_infer_recursive_alias_formal_pass_one() {
        // A self-referential alias terminates via the seen guard: the
        // cycle contributes False and the formal stays pass 1.
        let target = callable_with_args(vec![alias_type("mod.R")]);
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert("mod.R".to_string(), alias_snap("mod.R", &target));
        let res =
            crate::typeinfo::NativeTypeResolver::new(crate::typeinfo::TypeResolver::new(), aliases);
        let out = infer_passes(
            &[alias_type("mod.R")],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &res,
        );
        assert_eq!(out, Some(vec![1]));
    }

    #[test]
    fn arg_infer_alias_missing_snapshot_defers() {
        let out = infer_passes(
            &[alias_type("mod.Missing")],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, None);
    }

    #[test]
    fn arg_infer_undecodable_formal_defers() {
        pyo3::prepare_freethreaded_python();
        pyo3::Python::with_gil(|py| {
            let out = rust_get_arg_infer_passes(
                py,
                &empty_resolver(),
                vec![b"\xff\xff".to_vec()],
                vec![encode(&any_type())],
                vec![false],
                vec![vec![0]],
                1,
            );
            assert_eq!(out, None);
        });
    }

    #[test]
    fn arg_infer_param_spec_plain_callable_skips_pass_two() {
        // ParamSpec formal + non-generic CallableType actual: the skip
        // trigger suppresses the second pass (result stays 1).
        let formal = param_spec_callable();
        let actual = callable(0);
        let out = infer_passes(
            &[formal],
            &[actual],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![1]));
    }

    #[test]
    fn arg_infer_param_spec_wrong_shape_pass_one() {
        // ParamSpec only in `variables`, without the trailing
        // *args: P.args, **kwargs: P.kwargs shape: param_spec() returns
        // None, so no skip arm and no promotion (ret is Any).
        let mut formal = callable_with_args(vec![any_type()]);
        if let Type::CallableType { variables: v, .. } = &mut formal {
            *v = vec![param_spec()];
        }
        let actual = callable(0);
        let out = infer_passes(
            &[formal],
            &[actual],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![1]));
    }

    #[test]
    fn arg_infer_param_spec_lambda_pass_two() {
        // A lambda actual never triggers the ParamSpec skip; the formal
        // carries a ParamSpec in its args -> has_type_vars -> pass 2.
        let formal = param_spec_callable();
        let actual = callable(0);
        let out = infer_passes(
            &[formal],
            &[actual],
            vec![true],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![2]));
    }

    #[test]
    fn arg_infer_param_spec_other_actual_pass_two() {
        // A non-callable, non-instance actual (e.g. AnyType) cannot
        // trigger the skip; the ParamSpec formal promotes pass 2.
        let formal = param_spec_callable();
        let out = infer_passes(
            &[formal],
            &[any_type()],
            vec![false],
            vec![vec![0]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![2]));
    }

    #[test]
    fn arg_infer_param_spec_multiple_actuals_skip() {
        // Two actuals for one ParamSpec formal: both promoted, then both
        // suppressed back to 1 by the skip trigger.
        let formal = param_spec_callable();
        let plain = callable(0);
        let out = infer_passes(
            &[formal],
            &[plain.clone(), plain],
            vec![false, false],
            vec![vec![0, 1]],
            2,
            &empty_resolver(),
        );
        assert_eq!(out, Some(vec![1, 1]));
    }

    #[test]
    fn arg_infer_out_of_range_actual_defers() {
        // A promoted actual index out of range cannot happen in Python
        // (list assignment would raise); the seam defers instead.
        let formal = callable_with_args(vec![callable_ret_with_args(Vec::new(), type_var())]);
        let out = infer_passes(
            &[formal],
            &[any_type()],
            vec![false],
            vec![vec![7]],
            1,
            &empty_resolver(),
        );
        assert_eq!(out, None);
    }

    // ------------------------------------------------------------------
    // alias formal/actual expansion (issue #1241)
    // ------------------------------------------------------------------

    fn alias_resolver_with_int() -> crate::typeinfo::NativeTypeResolver {
        let mut r = crate::typeinfo::TypeResolver::new();
        for snap in [
            solve_snap("builtins.int"),
            solve_snap("builtins.str"),
            solve_snap("builtins.object"),
        ] {
            r.insert(snap.fullname.clone(), snap);
        }
        let mut ar = crate::aliases::TypeAliasResolver::new();
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &int_instance()).expect("encode int");
        ar.insert(
            "mod.IntAlias".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.IntAlias".to_string(),
                target: buf.into_bytes(),
                no_args: true,
                ..Default::default()
            },
        );
        crate::typeinfo::NativeTypeResolver::new(r, ar)
    }

    #[test]
    fn solve_generic_alias_actual_resolves() {
        // identity(mod.IntAlias) with mod.IntAlias = int: the alias actual
        // expands before constraint inference, so T solves to int; before
        // the expansion the top-level alias deferred to Python.
        let resolver = alias_resolver_with_int();
        let callee = generic_identity();
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.IntAlias".to_string(),
        };
        let out = solve_generic_bytes_with(&resolver, &callee, &[alias], vec![vec![0]]);
        let bytes = out.expect("expected successful solve, got deferral");
        let mut rb = ReadBuffer::new(&bytes);
        let resolved = read_type(&mut rb, None).unwrap();
        assert!(
            solved_typevar(&resolved),
            "expected fully-resolved callable"
        );
        let Type::CallableType { arg_types, .. } = &resolved else {
            panic!("expected callable");
        };
        assert_eq!(arg_types[0], int_instance());
    }

    #[test]
    fn solve_generic_alias_formal_resolves() {
        // def [T] (x: mod.TAlias) -> None with mod.TAlias = T: the alias
        // formal expands to the bare TypeVar, so the constraint on T is
        // inferred and solved; before the expansion it deferred to Python.
        let mut r = crate::typeinfo::TypeResolver::new();
        for snap in [
            solve_snap("builtins.int"),
            solve_snap("builtins.str"),
            solve_snap("builtins.object"),
        ] {
            r.insert(snap.fullname.clone(), snap);
        }
        let mut ar = crate::aliases::TypeAliasResolver::new();
        let tv = type_var();
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &tv).expect("encode typevar");
        ar.insert(
            "mod.TAlias".to_string(),
            crate::aliases::TypeAliasSnapshot {
                fullname: "mod.TAlias".to_string(),
                target: buf.into_bytes(),
                no_args: true,
                ..Default::default()
            },
        );
        let resolver = crate::typeinfo::NativeTypeResolver::new(r, ar);

        let mut callee = callable_with_args(vec![Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.TAlias".to_string(),
        }]);
        let Type::CallableType { variables, .. } = &mut callee else {
            panic!("expected callable");
        };
        variables.push(tv);

        let out = solve_generic_bytes_with(&resolver, &callee, &[int_instance()], vec![vec![0]]);
        let bytes = out.expect("expected successful solve, got deferral");
        let mut rb = ReadBuffer::new(&bytes);
        let resolved = read_type(&mut rb, None).unwrap();
        assert!(
            solved_typevar(&resolved),
            "expected fully-resolved callable"
        );
    }
}
