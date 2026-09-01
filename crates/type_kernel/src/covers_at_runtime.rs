//! Port of `mypy.subtypes.covers_at_runtime` (subtypes.py:2519-2548):
//! will `isinstance(item, supertype)` always return True at runtime?
//! Erases both operands (unless supertype is a type-object `FunctionLike`),
//! then runs the proper-subtype / protocol / TypedDict / TypeVar / native-int
//! checks. Tuple operands ride the normal erase flow
//! (`argapprox::erase_type` maps a `TupleType` to its `partial_fallback`,
//! mirroring `erasetype.py visit_tuple_type`), so they need no special
//! defer. Defers (`None`) on wire-unrepresentable forms; subtype checks go
//! through the Stage-3c kernel and erasures through the wire `erase_type`.
//! The pyfunction seam expands top-level `TypeAliasType` operands via the
//! alias resolver before entering the inner; the inner still defers on
//! nested alias items (mirroring the Python `get_proper_type` calls).

use pyo3::prelude::*;

use crate::argapprox;
use crate::checker_helpers::{get_proper_or_none, is_type_obj};
use crate::subtypes::{self, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type};

/// MYPYC_NATIVE_INT_NAMES (types.py:252-257): fixed-width native int
/// types that are compatible with `builtins.int` at runtime.
const MYPYC_NATIVE_INT_NAMES: &[&str] = &[
    "mypy_extensions.i64",
    "mypy_extensions.i32",
    "mypy_extensions.i16",
    "mypy_extensions.u8",
];

/// Mirror `mypy.subtypes.covers_at_runtime` on wire-decoded types.
/// Returns `Some(bool)` when Rust decided; `None` (defer to the pure-Python
/// body) on wire-unrepresentable forms or a recursive check Rust cannot
/// decide (TypeAliasType operands, erase needing a live TypeInfo rebuild).
pub(crate) fn covers_at_runtime_inner(
    item: &Type,
    supertype: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    let item = get_proper_or_none(item)?;
    let supertype = get_proper_or_none(supertype)?;

    // subtypes.py:2524-2526: runtime type checks ignore type arguments,
    // so erase the supertype unless it is a type-object (a `Type[cls]`
    // check is exact on `cls`, not on its erased form).
    let supertype_erased: Type;
    let supertype_to_use: &Type = if !is_function_like_type_obj(supertype, resolver) {
        supertype_erased = argapprox::erase_type(supertype, strict_optional, resolver)?;
        &supertype_erased
    } else {
        supertype
    };

    // subtypes.py:2527-2530: `is_proper_subtype(erase_type(item), supertype,
    // ignore_promotions=True, erase_instances=True)`; the zero-arg
    // `erase_type(item)` neutralizes the covers-level erase, but the
    // `erase_instances` flag itself erases the instance Python maps to the
    // supertype inside visit_instance (subtypes.py:1151-1155).
    let item_erased = argapprox::erase_type(item, strict_optional, resolver)?;
    let mut ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    ctx.erase_instances = true;
    let decision = subtypes::is_subtype(&item_erased, supertype_to_use, &ctx, resolver);
    match decision {
        Some(true) => return Some(true),
        None => return None,
        Some(false) => {}
    }

    if let Type::Instance { type_ref, .. } = supertype_to_use {
        // subtypes.py:2532-2535: protocol supertype — the plain erased
        // subtype check above is not enough, so check the un-erased item.
        if resolver.get(type_ref).is_some_and(|snap| snap.is_protocol) {
            let pctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            match subtypes::is_subtype(item, supertype_to_use, &pctx, resolver) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
        }
        // subtypes.py:2536-2539: `isinstance(x, dict)` selects TypedDicts
        // from unions.
        if let Type::TypedDictType { .. } = item {
            if type_ref == "builtins.dict" {
                return Some(true);
            }
        }
        // subtypes.py:2554-2556: a TypeVar covers the supertype when its
        // upper_bound does.
        if let Type::TypeVarType { upper_bound, .. } = item {
            let pctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            match subtypes::is_subtype(upper_bound.as_ref(), supertype_to_use, &pctx, resolver) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
        }
        // subtypes.py:2543-2546: `builtins.int` covers the native int types.
        if let Type::Instance {
            type_ref: item_ref, ..
        } = item
        {
            if type_ref == "builtins.int" && MYPYC_NATIVE_INT_NAMES.contains(&item_ref.as_str()) {
                return Some(true);
            }
        }
    }

    Some(false)
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// Whether a type is a `FunctionLike` whose `is_type_obj()` is True.
/// The wire format lacks a unified `FunctionLike`; check `CallableType` and
/// `Overloaded` via the `is_type_obj` helper in `checker_helpers`.
fn is_function_like_type_obj(typ: &Type, resolver: &TypeResolver) -> bool {
    match typ {
        Type::CallableType {
            fallback,
            ret_type,
            from_concatenate,
            ..
        } => is_type_obj(fallback, ret_type, *from_concatenate, resolver),
        Type::Overloaded { items } => items
            .first()
            .map(|first| {
                if let Type::CallableType {
                    fallback,
                    ret_type,
                    from_concatenate,
                    ..
                } = first
                {
                    is_type_obj(fallback, ret_type, *from_concatenate, resolver)
                } else {
                    false
                }
            })
            .unwrap_or(false),
        _ => false,
    }
}

/// `#[pyfunction]` entry for `mypy.subtypes.covers_at_runtime`.
/// Returns `Some(bool)` or `None` (defer to Python).
///
/// Top-level `TypeAliasType` operands expand through the alias resolver,
/// mirroring `_is_subtype`'s `get_proper_type` on both sides
/// (subtypes.py:531); nested alias items still defer in the inner.
#[pyfunction]
#[pyo3(signature = (item_bytes, supertype_bytes, strict_optional, resolver))]
pub(crate) fn rust_covers_at_runtime(
    item_bytes: &[u8],
    supertype_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let item = decode_type(item_bytes)?;
    let supertype = decode_type(supertype_bytes)?;
    let item = crate::checkexpr_functions::get_proper_or_expand(&item, resolver.alias_resolver())?;
    let supertype =
        crate::checkexpr_functions::get_proper_or_expand(&supertype, resolver.alias_resolver())?;
    covers_at_runtime_inner(&item, &supertype, strict_optional, resolver.resolver())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use crate::wire::WriteBuffer;

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

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

    fn encode(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        crate::wire::write_type(&mut buf, t).expect("type must encode");
        buf.into_bytes()
    }

    #[test]
    fn alias_item_and_supertype_expand_at_seam() {
        let res = make_resolver(vec![
            snap("builtins.str", "str"),
            snap("builtins.object", "object"),
        ]);
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Alias".to_string(),
            alias_snap("mod.Alias", &instance("builtins.str")),
        );
        let mut native = NativeTypeResolver::new(res, aliases);
        // Both operands are the alias; the seam expands them to str so the
        // inner proper-subtype check decides Some(true).
        let item = alias_type("mod.Alias");
        let supertype = alias_type("mod.Alias");
        assert_eq!(
            rust_covers_at_runtime(&encode(&item), &encode(&supertype), true, &mut native),
            Some(true)
        );
    }

    #[test]
    fn alias_without_snapshot_defers_at_seam() {
        let res = make_resolver(vec![]);
        let mut native = NativeTypeResolver::new(res, crate::aliases::TypeAliasResolver::new());
        let item = alias_type("mod.Alias");
        let supertype = instance("builtins.str");
        // The alias resolver has no snapshot for mod.Alias: the expansion
        // defers, preserving the pre-change defer behavior.
        assert_eq!(
            rust_covers_at_runtime(&encode(&item), &encode(&supertype), true, &mut native),
            None
        );
    }

    fn snap(fullname: &str, name: &str) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            mro: vec![fullname.to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        s.has_base.insert(fullname.to_string());
        s.has_base.insert("builtins.object".to_string());
        s
    }

    fn instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 2,
            source_any: None,
            missing_import_name: None,
        }
    }

    #[test]
    fn erased_proper_subtype_covers() {
        // List[int] erased -> list, whose is_subtype over object (ordinal
        // path): an Instance whose erase is a proper subtype covers.
        let res = make_resolver(vec![
            snap("builtins.list", "list"),
            snap("builtins.object", "object"),
        ]);
        let item = instance("builtins.list");
        let sup = instance("builtins.object");
        assert_eq!(covers_at_runtime_inner(&item, &sup, true, &res), Some(true));
    }

    #[test]
    fn type_obj_supertype_not_erased() {
        // A type-object supertype (Type[list]) skips the erasure; the
        // erased item (list) then goes proper-subtype against the type-object
        // CallableType, which defers here, so this defers to Python.
        let res = make_resolver(vec![snap("builtins.object", "object")]);
        let item = instance("builtins.list");
        let callable = Type::CallableType {
            fallback: Box::new(instance("builtins.type")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![instance("builtins.object")],
            arg_kinds: vec![1],
            arg_names: vec![None],
            ret_type: Box::new(instance("builtins.type")),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
            special_sig: None,
        };
        assert_eq!(covers_at_runtime_inner(&item, &callable, true, &res), None);
    }

    #[test]
    fn native_int_covers_builtins_int() {
        let res = make_resolver(vec![
            snap("builtins.int", "int"),
            snap("builtins.object", "object"),
            snap("mypy_extensions.i64", "i64"),
        ]);
        let item = instance("mypy_extensions.i64");
        let sup = instance("builtins.int");
        assert_eq!(covers_at_runtime_inner(&item, &sup, true, &res), Some(true));
    }

    #[test]
    fn unrelated_instances_do_not_cover() {
        let res = make_resolver(vec![
            snap("builtins.dict", "dict"),
            snap("builtins.int", "int"),
            snap("builtins.object", "object"),
        ]);
        let item = instance("builtins.dict");
        let sup = instance("builtins.int");
        assert_eq!(
            covers_at_runtime_inner(&item, &sup, true, &res),
            Some(false)
        );
    }

    #[test]
    fn any_item_covers_any_supertype() {
        let res = make_resolver(vec![snap("builtins.int", "int")]);
        let item = any_type();
        let sup = instance("builtins.int");
        // erase_type(Any) = Any; the proper subtype check is
        // is_proper_subtype(Any, int) -> False (visit_any proper).
        assert_eq!(
            covers_at_runtime_inner(&item, &sup, true, &res),
            Some(false)
        );
    }

    #[test]
    fn missing_snapshot_defers() {
        let res = make_resolver(vec![]);
        let item = instance("builtins.list");
        let sup = instance("builtins.object");
        // is_subtype needs the item snapshot; defer rather than decide.
        assert_eq!(covers_at_runtime_inner(&item, &sup, true, &res), None);
    }

    #[test]
    fn typed_dict_covers_dict() {
        let res = make_resolver(vec![
            snap("builtins.dict", "dict"),
            snap("typing._TypedDict", "_TypedDict"),
        ]);
        let dict_fallback = instance("builtins.dict");
        let item = Type::TypedDictType {
            fallback: Box::new(dict_fallback.clone()),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        let sup = instance("builtins.dict");
        assert_eq!(covers_at_runtime_inner(&item, &sup, true, &res), Some(true));
    }

    #[test]
    fn type_var_upper_bound_covers() {
        let res = make_resolver(vec![snap("builtins.object", "object")]);
        let item = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "__main__".to_string(),
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            values: vec![],
            variance: 0,
            meta_level: 0,
        };
        let sup = instance("builtins.object");
        assert_eq!(covers_at_runtime_inner(&item, &sup, true, &res), Some(true));
    }

    #[test]
    fn protocol_supertype_uses_unerased_item() {
        let mut p = snap("typing.Protocol", "Protocol");
        p.is_protocol = true;
        let res = make_resolver(vec![p]);
        let item = instance("builtins.list");
        let sup = instance("typing.Protocol");
        // The un-erased subtype check over a missing list snapshot defers.
        assert_eq!(covers_at_runtime_inner(&item, &sup, true, &res), None);
    }
}
