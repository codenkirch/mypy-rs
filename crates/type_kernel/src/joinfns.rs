//! `object_or_any_from_type` (join.py:1260-1281) and
//! `combine_similar_callables` (join.py:1193-1219), Rust ports.
//!
//! Both run behind the `_native_join_active` gate; the Python shims in
//! `mypy/join.py` serialize the operands and decode the result. `None`
//! means "Rust doesn't handle this, let Python decide" (the strangler-fig
//! per-call contract).
//!
//! `object_or_any_from_type` recurses through the type tree to find a
//! suitable `builtins.object` Instance, falling back to
//! `AnyType(TypeOfAny.implementation_artifact)`. Instance results use
//! `mro[-1]` (`TypeInfoSnapshot.mro` is the full C3 linearization), read
//! from the resolver so the decoded Instance fixes up against the live
//! TypeInfo graph.
//!
//! `combine_similar_callables` mirrors join.py:1193-1219: per-arg
//! `safe_join`, `join_types` on ret/instance_type, `combine_arg_names`,
//! and the fallback pick (builtins.function wins). `match_generic_callables`
//! (join.py:1292-1317) renumbers type variables so both callables share an
//! id space; both-generic renumbering runs on the native `TypeVarId`
//! registry (`freshen.rs::renumber_generic_pair`, sentinel namespace,
//! Python's global counter untouched). The result is a fresh wire
//! CallableType encoded via `write_type`; the Python shim restores
//! `definition` from the live right operand after fixup,
//! mirroring the existing rust_join_types disc==7 path.

use pyo3::prelude::*;

use crate::freshen::renumber_generic_pair;
use crate::setops::{
    combine_arg_names, extract_callable_invariants, join_types, pick_fallback, safe_join,
    setop_result_to_type,
};
use crate::subtypes::SubtypeContext;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

/// AnyType with `type_of_any=8` (`TypeOfAny.implementation_artifact`),
/// the fallback of `object_or_any_from_type` (join.py:1276).
fn any_implementation_artifact() -> Type {
    Type::AnyType {
        type_of_any: 8,
        source_any: None,
        missing_import_name: None,
    }
}

/// Encode via `write_type`. `None` if the variant is not writable
/// (the caller defers to Python).
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, typ).ok()?;
    Some(buf.into_bytes())
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// `object_or_any_from_type` (join.py:1260-1281), wire subset.
/// Returns an Instance (the derived object) or
/// `AnyType(implementation_artifact)` when no object can be derived.
/// Defer (None) only when a required resolver snapshot is missing or an
/// unfixed alias is encountered.
pub(crate) fn object_or_any_from_type(typ: &Type, resolver: &TypeResolver) -> Option<Type> {
    match typ {
        // Instance: mro[-1] is always 'builtins.object' in a sane graph;
        // use the snapshot's last mro entry (join.py:1262-1263,
        // object_from_instance).
        Type::Instance { type_ref, .. } => {
            let snap = resolver.get(type_ref)?;
            let last = snap.mro.last()?;
            Some(Type::Instance {
                type_ref: last.clone(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            })
        }
        // CallableType / TypedDictType / LiteralType: object from the
        // fallback Instance (join.py:1264-1265).
        Type::CallableType { fallback, .. }
        | Type::TypedDictType { fallback, .. }
        | Type::LiteralType { fallback, .. } => object_or_any_from_type(fallback, resolver),
        // TupleType: object from the partial fallback (join.py:1266).
        Type::TupleType {
            partial_fallback, ..
        } => object_or_any_from_type(partial_fallback, resolver),
        // TypeType: recurse on the item (join.py:1267).
        Type::TypeType { item, .. } => object_or_any_from_type(item, resolver),
        // TypeVarLikeType: recurse on the upper_bound (join.py:1268-1269).
        Type::TypeVarType { upper_bound, .. }
        | Type::ParamSpecType { upper_bound, .. }
        | Type::TypeVarTupleType { upper_bound, .. } => {
            object_or_any_from_type(upper_bound, resolver)
        }
        // UnionType: first item that itself yields an Instance
        // (join.py:1270-1273).
        Type::UnionType { items, .. } => {
            for item in items {
                if let Some(candidate) = object_or_any_from_type(item, resolver) {
                    if matches!(candidate, Type::Instance { .. }) {
                        return Some(candidate);
                    }
                }
            }
            Some(any_implementation_artifact())
        }
        // UnpackType: join.py:1274-1275 mirrors this branch by discarding
        // the recursive `object_or_any_from_type` result and falling
        // through to the Any fallback. Match: never return recursion.
        Type::UnpackType { .. } => Some(any_implementation_artifact()),
        // The final fallback: AnyType(implementation_artifact)
        // (join.py:1276).
        _ => Some(any_implementation_artifact()),
    }
}

/// `#[pyfunction]` entry for `object_or_any_from_type`.
#[pyfunction]
pub(crate) fn rust_object_or_any_from_type(
    typ_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let typ = decode_type(typ_bytes)?;
    let result = object_or_any_from_type(&typ, resolver.resolver())?;
    encode_type(&result)
}

/// `object_from_instance` (join.py:1303-1307): construct the type
/// `builtins.object` from an instance type. Uses the fact that `object`
/// is always the last class in the MRO; the resolver snapshot's `mro`
/// is the full C3 linearization, so `mro[-1]` is `builtins.object` in a
/// sane graph (mirrors `IndexSet` in the Python mro).
///
/// Returns the `builtins.object` TypeInfo fullname; `None` (defer to
/// Python) when the instance's TypeInfo snapshot is missing from the
/// resolver or the MRO is empty. The Python shim builds the `Instance`
/// from the decoded fullname via the live TypeInfo map, so identity and
/// fixup semantics stay identical to the pure-Python construction.
#[pyfunction]
pub(crate) fn rust_object_from_instance(
    instance_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<String> {
    let typ = decode_type(instance_bytes)?;
    let Type::Instance { type_ref, .. } = typ else {
        return None;
    };
    let snap = resolver.resolver().get(&type_ref)?;
    Some(snap.mro.last()?.clone())
}

/// `match_generic_callables` (join.py:1292-1317): when both callables are
/// generic, renumber the type variables so both share the same id space.
/// Delegates to the native registry (`freshen.rs::renumber_generic_pair`);
/// Python's global `TypeVarId.new` counter is never advanced, and the
/// fresh ids carry the sentinel `NATIVE_TVAR_NAMESPACE` so they can never
/// collide with a Python id. `min_len == 0` is a no-op (Python returns the
/// inputs unchanged).
fn match_generic_callables(t: &Type, s: &Type, resolver: &TypeResolver) -> Option<(Type, Type)> {
    renumber_generic_pair(t, s, resolver)
}

/// `combine_similar_callables` (join.py:1193-1219): per-arg safe_join,
/// ret join, instance_type join, arg_names combine, fallback pick.
/// Returns the encoded fresh CallableType, or None (defer).
pub(crate) fn combine_similar_callables_core(
    t: &Type,
    s: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<u8>> {
    // match_generic_callables first (join.py:1194). Both-generic
    // renumbers on the native registry (Python re-allocates from the
    // global counter); min_len == 0 returns the untouched operands.
    let (t_c, s_c) = match_generic_callables(t, s, resolver)?;
    let Type::CallableType {
        fallback: t_fallback,
        arg_types: t_arg_types,
        arg_kinds: t_arg_kinds,
        arg_names: t_arg_names,
        ret_type: t_ret_type,
        instance_type: t_instance_type,
        variables: t_variables,
        ..
    } = &t_c
    else {
        return None;
    };
    let Type::CallableType {
        fallback: s_fallback,
        arg_types: s_arg_types,
        arg_kinds: s_arg_kinds,
        arg_names: s_arg_names,
        ret_type: s_ret_type,
        instance_type: s_instance_type,
        ..
    } = &s_c
    else {
        return None;
    };
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    // Per-arg safe_join (join.py:1196-1198).
    let mut new_arg_types = Vec::with_capacity(t_arg_types.len());
    for (ta, sa) in t_arg_types.iter().zip(s_arg_types.iter()) {
        new_arg_types.push(safe_join(ta, sa, &ctx, resolver)?);
    }
    // ret_type join (join.py:1207).
    let new_ret = setop_result_to_type(
        join_types(t_ret_type, s_ret_type, &ctx, resolver),
        t_ret_type,
        s_ret_type,
    )?;
    // instance_type join: only when both are Some (join.py:1205-1206).
    let new_instance_type = match (s_instance_type, t_instance_type) {
        (Some(si), Some(ti)) => Some(Box::new(setop_result_to_type(
            join_types(ti.as_ref(), si.as_ref(), &ctx, resolver),
            ti.as_ref(),
            si.as_ref(),
        )?)),
        _ => None,
    };
    // combine_arg_names (join.py:1206).
    let new_arg_names = combine_arg_names(t_arg_names, s_arg_names, t_arg_kinds, s_arg_kinds);
    // Fallback pick: builtins.function wins (join.py:1200-1203).
    let new_fallback = pick_fallback(s_fallback, t_fallback);
    let (
        arg_kinds,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        type_guard,
        type_is,
    ) = extract_callable_invariants(&t_c);
    // The result callable keeps t's invariants (copy_modified on t), with
    // the (unchanged, min_len==0) variables from t_c.
    let new_callable = Type::CallableType {
        fallback: Box::new(new_fallback),
        instance_type: new_instance_type,
        is_ellipsis_args,
        implicit,
        is_bound,
        from_concatenate,
        imprecise_arg_kinds,
        unpack_kwargs,
        from_type_type,
        arg_types: new_arg_types,
        arg_kinds,
        arg_names: new_arg_names,
        ret_type: Box::new(new_ret),
        name: None,
        variables: t_variables.clone(),
        type_guard,
        type_is,
        special_sig: None,
    };
    encode_type(&new_callable)
}

/// `#[pyfunction]` entry for `combine_similar_callables`.
#[pyfunction]
pub(crate) fn rust_combine_similar_callables(
    t_bytes: &[u8],
    s_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let s = decode_type(s_bytes)?;
    combine_similar_callables_core(&t, &s, strict_optional, resolver.resolver())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use crate::wire::Type;

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    /// Snapshot whose mro is `fullname` followed by `builtins.object`
    /// (mirrors the Python TypeFixture where object is in every mro).
    fn snap(fullname: &str) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        if fullname != "builtins.object" {
            s.mro.push("builtins.object".to_string());
        }
        s
    }

    /// Core of `rust_object_from_instance`: last mro entry, or None.
    fn object_from_instance_core(typ: &Type, resolver: &TypeResolver) -> Option<String> {
        let Type::Instance { type_ref, .. } = typ else {
            return None;
        };
        let snap = resolver.get(type_ref)?;
        snap.mro.last().cloned()
    }

    #[test]
    fn object_from_instance_returns_object_for_simple_class() {
        // A (mro=[A, object]) -> object.
        let r = make_resolver(vec![snap("a.A"), snap("builtins.object")]);
        assert_eq!(
            object_from_instance_core(&instance("a.A"), &r).as_deref(),
            Some("builtins.object")
        );
    }

    #[test]
    fn object_from_instance_returns_object_for_object() {
        // object (mro=[object]) -> object.
        let r = make_resolver(vec![snap("builtins.object")]);
        assert_eq!(
            object_from_instance_core(&instance("builtins.object"), &r).as_deref(),
            Some("builtins.object")
        );
    }

    #[test]
    fn object_from_instance_missing_snapshot_defers() {
        // Snapshot absent from resolver -> None (defer to Python).
        let r = make_resolver(vec![]);
        assert_eq!(object_from_instance_core(&instance("a.A"), &r), None);
    }

    #[test]
    fn object_from_instance_non_instance_defers() {
        // Non-Instance -> None (Python handles via default()).
        let r = make_resolver(vec![snap("builtins.object")]);
        assert_eq!(object_from_instance_core(&Type::NoneType, &r), None);
    }

    #[test]
    fn object_from_instance_empty_mro_defers() {
        // Snapshot with empty mro -> None (sane graphs always have
        // object at mro[-1], so this is defensive only).
        let r = make_resolver(vec![TypeInfoSnapshot {
            fullname: "a.Empty".to_string(),
            ..Default::default()
        }]);
        assert_eq!(object_from_instance_core(&instance("a.Empty"), &r), None);
    }
}
