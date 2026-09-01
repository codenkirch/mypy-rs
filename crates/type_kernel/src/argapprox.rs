//! #432: `arg_approximate_similarity` (mypy/checkexpr.py:7856-7916), Rust port.
//!
//! Decides whether a caller argument type is "roughly" compatible with a
//! signature parameter type for overload/disambiguation error reporting. The
//! function is deliberately loose: two types are similar when their shapes
//! plausibly match.
//!
//! The wire `Type` enum cannot express `TypeAliasType` targets,
//! `ErasedType`, `PartialType`, or `TypeGuardedType`, so every variant that
//! would need `get_proper_type` expansion or a live `TypeInfo` reconstruction
//! defers by returning `None`; the Python gate then falls through to the
//! pure-Python implementation unchanged. This is the strangler-fig per-call
//! gate: Rust decides only the cases it can decide with certainty.

use pyo3::prelude::*;

use crate::setops::{make_simplified_union, union_make_union};
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, ReadBuffer, Type};

/// `ArgKind.ARG_STAR` = 2 (checkexpr_functions.rs:34).
const ARG_STAR: i64 = 2;
/// `ArgKind.ARG_STAR2` = 4 (checkexpr_functions.rs:36).
const ARG_STAR2: i64 = 4;
/// `TypeOfAny.special_form` == 6 (types.py:233).
const ANY_SPECIAL_FORM: i64 = 6;
/// `TypeOfAny.from_error` == 5 (types.py:231).
const ANY_FROM_ERROR: i64 = 5;

fn decode(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// `get_proper_type` for the wire format (checkexpr.py:7867-7868). The wire
/// cannot expand `TypeAliasType` (no resolved target), so aliases defer.
fn get_proper_or_defer(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        t => Some(t),
    }
}

fn any_type(type_of_any: i64) -> Type {
    Type::AnyType {
        type_of_any,
        source_any: None,
        missing_import_name: None,
    }
}

/// `Generic` recursion: `... any(approx(item, other) for item in relevant_items)`
/// (checkexpr.py:7892) for an actual-union, and
/// `... any(approx(other, item) for item in relevant_items)` (checkexpr.py:7894)
/// for a formal-union. The union item is the actual on the left, the formal on
/// the right. Lazily defers: Some(true) on first hit, None if any item defers
/// and no hit, Some(false) only when every item is decided false.
fn approx_union_any(
    item_is_actual: bool,
    items: &[Type],
    other: &Type,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<bool> {
    let mut saw_none = false;
    for item in items {
        if !strict_optional {
            // relevant_items (types.py:3517-3522): when strict_optional is
            // off, drop items whose proper type is NoneType. get_proper_type
            // on a TypeAliasType is unexpandable -> defer.
            let proper = get_proper_or_defer(item)?;
            if matches!(proper, Type::NoneType) {
                continue;
            }
        }
        let (item_side, other_side) = if item_is_actual {
            (item, other)
        } else {
            (other, item)
        };
        match approx(item_side, other_side, strict_optional, res) {
            Some(true) => return Some(true),
            None => saw_none = true,
            Some(false) => {}
        }
    }
    if saw_none {
        None
    } else {
        Some(false)
    }
}

/// `is_typetype_like` (checkexpr.py:7877-7882): TypeType, or a
/// FunctionLike whose `is_type_obj()` holds, or `builtins.type` instance.
///
/// `is_type_obj` (types.py:2343-2346) = `fallback.type.is_metaclass() and
/// ret_type is not UninhabitedType`, with `Overloaded.is_type_obj` proxying
/// to `items[0]` (types.py:2773-2776). `is_metaclass`
/// (nodes.py:4128-4133) = `has_base("builtins.type")` or
/// `fullname == "abc.ABCMeta"` or `fallback_to_any`.
///
/// Returns None when the fallback's TypeInfo snapshot is missing from the
/// resolver (the metaclass query cannot be answered).
fn is_typetype_like(typ: &Type, res: &TypeResolver) -> Option<bool> {
    match typ {
        Type::TypeType { .. } => Some(true),
        Type::CallableType {
            fallback, ret_type, ..
        } => {
            if matches!(ret_type.as_ref(), Type::UninhabitedType { .. }) {
                return Some(false);
            }
            let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                return Some(false);
            };
            let snap = res.get(type_ref)?;
            Some(
                snap.has_base("builtins.type")
                    || snap.fullname == "abc.ABCMeta"
                    || snap.fallback_to_any,
            )
        }
        Type::Overloaded { items } => match items.first() {
            Some(item) => is_typetype_like(item, res),
            None => Some(false),
        },
        Type::Instance { type_ref, .. } => Some(type_ref == "builtins.type"),
        _ => Some(false),
    }
}

/// `erase_to_union_or_bound` (typeops.py:1111-1115): TypeVarType with
/// values -> simplified union of the values; otherwise the (proper)
/// upper bound unchanged (a TypeVarType upper bound passes through, exactly
/// as Python's `get_proper_type(typ.upper_bound)` leaves it).
fn erase_typevar(t: &Type, strict_optional: bool, res: &TypeResolver) -> Option<Type> {
    match t {
        Type::TypeVarType {
            values,
            upper_bound,
            ..
        } => {
            if !values.is_empty() {
                make_simplified_union(
                    values,
                    &SubtypeContext::new(false, false, false, true, true, strict_optional),
                    res,
                    true,
                    false,
                )
            } else {
                let ub = get_proper_or_defer(upper_bound.as_ref())?;
                Some(ub.clone())
            }
        }
        _ => Some(t.clone()),
    }
}

/// `tuple_fallback` (typeops.py:194-220). Returns the fallback Instance
/// for a TupleType: either the non-`builtins.tuple` partial fallback, or a
/// fresh `builtins.tuple[Union[items]]` with the partial fallback's
/// `extra_attrs` preserved. Defers (None) when an Unpack item does not
/// resolve to a `builtins.tuple` instance (Python raises NotImplementedError).
fn tuple_fallback(t: &Type, strict_optional: bool, res: &TypeResolver) -> Option<Type> {
    let Type::TupleType {
        partial_fallback,
        items,
        ..
    } = t
    else {
        return None;
    };
    let Type::Instance {
        type_ref,
        last_known_value,
        extra_attrs,
        ..
    } = partial_fallback.as_ref()
    else {
        // Python reads `typ.partial_fallback.type` (AttributeError for a
        // non-Instance fallback); the wire cannot, so defer.
        return None;
    };
    if type_ref != "builtins.tuple" {
        return Some((**partial_fallback).clone());
    }
    let mut new_items = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Type::UnpackType { typ: inner, .. } => {
                // get_proper_type(item.type) (typeops.py:201).
                let unpacked = get_proper_or_defer(inner)?;
                let unpacked = match unpacked {
                    Type::TypeVarTupleType { upper_bound, .. } => {
                        // get_proper_type(unpacked_type.upper_bound).
                        get_proper_or_defer(upper_bound.as_ref())?
                    }
                    t => t,
                };
                let Type::Instance { type_ref, args, .. } = unpacked else {
                    return None; // raise NotImplementedError
                };
                if type_ref != "builtins.tuple" {
                    return None; // raise NotImplementedError
                }
                // Python `args[0]`; missing arg -> defer (IndexError path).
                let arg0 = args.first()?;
                new_items.push(arg0.clone());
            }
            _ => new_items.push(item.clone()),
        }
    }
    // Note `handle_recursive=False` in Python (typeops.py:218); the Rust
    // `make_simplified_union` does not model the recursion guard but the
    // port is safe because item lists here come from a live tuple, not a

    // recursive alias.
    let union = make_simplified_union(
        &new_items,
        &SubtypeContext::new(false, false, false, true, true, strict_optional),
        res,
        true,
        false,
    )?;
    Some(Type::Instance {
        type_ref: type_ref.clone(),
        args: vec![union],
        last_known_value: last_known_value.clone(),
        extra_attrs: extra_attrs.clone(),
    })
}

/// `EraseTypeVisitor` (erasetype.py:160-245), wire-level port. Returns
/// None (defer) for variants that need a live `TypeInfo` reconstruction:
/// TypeVarTupleType (needs `copy_modified` on a live Instance),
/// TypeAliasType (Python raises), Overloaded (the wire has no `.fallback`),
/// Instance erasure when the snapshot or a TypeVarTuple defn var is present.
pub(crate) fn erase_type(t: &Type, strict_optional: bool, res: &TypeResolver) -> Option<Type> {
    match t {
        Type::TypeAliasType { .. } => None, // Python visit raises RuntimeError
        Type::UnboundType { .. } => Some(any_type(ANY_FROM_ERROR)),
        Type::AnyType { .. }
        | Type::NoneType
        | Type::ErasedType
        | Type::UninhabitedType { .. }
        | Type::DeletedType { .. }
        | Type::LiteralType { .. } => Some(t.clone()),
        // visit_type_var / visit_param_spec / visit_unpack_type
        // (erasetype.py:188-203): AnyType(special_form).
        Type::TypeVarType { .. } | Type::ParamSpecType { .. } | Type::UnpackType { .. } => {
            Some(any_type(ANY_SPECIAL_FORM))
        }
        // visit_type_var_tuple (erasetype.py:197-200): needs a live
        // `tuple_fallback.copy_modified(args=[Any])`; defer.
        Type::TypeVarTupleType { .. } => None,
        Type::Instance { .. } => erase_instance(t, res),
        // visit_callable_type (erasetype.py:205-216): `Callable[..., Any]`
        // preserving the fallback.
        Type::CallableType { fallback, .. } => Some(Type::CallableType {
            fallback: fallback.clone(),
            instance_type: None,
            is_ellipsis_args: true,
            implicit: true,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![any_type(ANY_SPECIAL_FORM), any_type(ANY_SPECIAL_FORM)],
            arg_kinds: vec![ARG_STAR, ARG_STAR2],
            arg_names: vec![None, None],
            ret_type: Box::new(any_type(ANY_SPECIAL_FORM)),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
            special_sig: None,
        }),
        // visit_overloaded (erasetype.py:218-219): `t.fallback.accept(self)`.
        // The wire `Overloaded` carries no fallback, so defer.
        Type::Overloaded { .. } => None,
        Type::TupleType {
            partial_fallback, ..
        } => erase_type(partial_fallback, strict_optional, res),
        Type::TypedDictType { fallback, .. } => erase_type(fallback, strict_optional, res),
        Type::UnionType { items, .. } => {
            let mut erased = Vec::with_capacity(items.len());
            for item in items {
                erased.push(erase_type(item, strict_optional, res)?);
            }
            make_simplified_union(
                &erased,
                &SubtypeContext::new(false, false, false, true, true, strict_optional),
                res,
                true,
                false,
            )
        }
        // visit_type_type (erasetype.py:239-242): TypeType.make_normalized.
        Type::TypeType {
            item, is_type_form, ..
        } => {
            let erased_item = erase_type(item, strict_optional, res)?;
            make_normalized_type_type(erased_item, *is_type_form)
        }
        Type::Parameters(_) => None,
    }
}

/// `Instance` erasure (erasetype.py:184-186 + typevartuples.erased_vars):
/// rebuild with one AnyType(special_form) per `defn.type_vars` entry. A
/// TypeVarTuple defn var would need the live `tuple_fallback` to emit
/// `Unpack(*tuple[Any, ...])`; defer on that rare kind or a missing snapshot.
fn erase_instance(t: &Type, res: &TypeResolver) -> Option<Type> {
    let Type::Instance {
        type_ref,
        extra_attrs,
        ..
    } = t
    else {
        return None;
    };
    let snap = res.get(type_ref)?;
    let mut args = Vec::with_capacity(snap.type_vars_with_variance.len());
    for (_name, _variance, kind) in &snap.type_vars_with_variance {
        match *kind {
            0 | 1 => args.push(any_type(ANY_SPECIAL_FORM)),
            _ => return None, // TypeVarTuple defn var: needs live tuple_fallback
        }
    }
    Some(Type::Instance {
        type_ref: type_ref.clone(),
        args,
        last_known_value: None,
        extra_attrs: extra_attrs.clone(),
    })
}

/// `TypeType.make_normalized` (types.py:3676-3691): with `is_type_form`
/// keep the wrapper; otherwise split a UnionType item into a union of
/// `TypeType`s.
pub(crate) fn make_normalized_type_type(item: Type, is_type_form: bool) -> Option<Type> {
    if is_type_form {
        return Some(Type::TypeType {
            item: Box::new(item),
            is_type_form: true,
        });
    }
    if let Type::UnionType { items, .. } = item {
        // UnionType.make_union over the wrapped items (types.py:3686-3690);
        // make_union recomputes can_be_true/can_be_false from the items.
        let mut wrapped = Vec::with_capacity(items.len());
        for ui in items {
            wrapped.push(make_normalized_type_type(ui, false)?);
        }
        Some(union_make_union(wrapped))
    } else {
        Some(Type::TypeType {
            item: Box::new(item),
            is_type_form: false,
        })
    }
}

/// The core `arg_approximate_similarity` decision, after both operands are
/// proper and type-var-erased. Mirrors checkexpr.py:7876-7916.
fn approx_proper(a: &Type, f: &Type, strict_optional: bool, res: &TypeResolver) -> Option<bool> {
    // Callable or Type[...]-ish (checkexpr.py:7884-7886).
    if matches!(f, Type::CallableType { .. })
        && matches!(
            a,
            Type::CallableType { .. } | Type::Overloaded { .. } | Type::TypeType { .. }
        )
    {
        return Some(true);
    }
    // is_typetype_like on both (checkexpr.py:7887-7888).
    let a_like = is_typetype_like(a, res)?;
    let f_like = is_typetype_like(f, res)?;
    if a_like && f_like {
        return Some(true);
    }
    // Unions (checkexpr.py:7891-7894).
    if let Type::UnionType { items, .. } = a {
        return approx_union_any(true, items, f, strict_optional, res);
    }
    if let Type::UnionType { items, .. } = f {
        return approx_union_any(false, items, a, strict_optional, res);
    }
    // TypedDicts (checkexpr.py:7897-7900).
    if let Type::TypedDictType { fallback, .. } = a {
        if matches!(f, Type::TypedDictType { .. }) {
            return Some(true);
        }
        return approx_proper(fallback.as_ref(), f, strict_optional, res);
    }
    // Instances (checkexpr.py:7904-7913). Python reassigns `actual` through
    // a single unwrap step, then an MRO optimization check; a non-Instance
    // final value falls through to the erased is_subtype below.
    let mut transformed_actual: Option<Type> = None;
    if let Type::Instance {
        type_ref: f_ref, ..
    } = f
    {
        let transformed: Type = match a {
            Type::CallableType { fallback, .. } => (**fallback).clone(),
            Type::Overloaded { items } => {
                let first = items.first()?;
                let Type::CallableType { fallback, .. } = first else {
                    // Python reads `items[0].fallback` (CallableType always);
                    // defer on an unexpected non-Callable item.
                    return None;
                };
                (**fallback).clone()
            }
            Type::TupleType { .. } => tuple_fallback(a, strict_optional, res)?,
            _ => a.clone(),
        };
        if let Type::Instance {
            type_ref: a_ref, ..
        } = &transformed
        {
            if let Some(snap) = res.get(a_ref) {
                if snap.mro.contains(f_ref) {
                    return Some(true);
                }
            }
            // Snapshot missing: Python would have answered from the live
            // TypeInfo. Cannot decide here; fall through to the final
            // is_subtype, which defers if it also cannot decide.
        }
        transformed_actual = Some(transformed);
    }
    let actual: &Type = transformed_actual.as_ref().unwrap_or(a);
    // Fall back to a standard subtype check of the erased types
    // (checkexpr.py:7916). The whole check stays rust-internal: erase on
    // the wire, then `subtypes::is_subtype` with the default context

    // (subtypes.py:170-203: all flags False, options=None ->
    // state.strict_optional).
    let erased_a = erase_type(actual, strict_optional, res)?;
    let erased_f = erase_type(f, strict_optional, res)?;
    is_subtype(
        &erased_a,
        &erased_f,
        &SubtypeContext::new(false, false, false, false, false, strict_optional),
        res,
    )
}

/// `arg_approximate_similarity` (checkexpr.py:7856-7916), full wire port.
fn approx(a: &Type, f: &Type, strict_optional: bool, res: &TypeResolver) -> Option<bool> {
    // get_proper_type (checkexpr.py:7867-7868): aliases defer.
    let a = get_proper_or_defer(a)?;
    let f = get_proper_or_defer(f)?;
    // Erase typevars (checkexpr.py:7871-7874).
    let a = erase_typevar(a, strict_optional, res)?;
    let f = erase_typevar(f, strict_optional, res)?;
    approx_proper(&a, &f, strict_optional, res)
}

/// Entry point mirroring `mypy.checkexpr.arg_approximate_similarity` for
/// the cases Rust can decide. Returns `None` to defer to Python.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_arg_approximate_similarity(
    actual_bytes: &[u8],
    formal_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let actual = decode(actual_bytes)?;
    let formal = decode(formal_bytes)?;
    approx(&actual, &formal, strict_optional, resolver.resolver())
}
