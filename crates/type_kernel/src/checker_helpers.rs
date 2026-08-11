//! Issue #477: pure narrowing/validation helpers from `mypy.checker` and
//! `mypy.typeops` / `mypy.subtypes` / `mypy.join`.
//!
//! Ports:
//!   * `custom_special_method` (typeops.py:1555) — does a type have a
//!     custom special method (e.g. `__eq__`) not inherited from
//!     `builtins.` / `typing.`? Uses the `member_definers` snapshot field.
//!   * `has_custom_eq_checks` (checker.py:9493) — thin wrapper calling
//!     `custom_special_method` for `__eq__` and `__ne__`.
//!   * `restrict_subtype_away` (subtypes.py:2363) — `t minus s` for
//!     runtime type assertions. Handles the union-literal restriction
//!     and the non-union `consider_runtime_isinstance` / proper-subtype
//!     branches. Defers (returns `None`) when `covers_at_runtime` needs
//!     a path Rust cannot decide.
//!   * `join_type_list` (join.py:1142) — fold-join over a list of types.
//!     Reuses the existing `setops::join_types` wire kernel. Returns
//!     encoded bytes so the Python shim can decode to a live `Type`.
//!   * `get_protocol_member` (subtypes.py:1513) — the pure
//!     `__call__`-special-case prefix of `get_protocol_member` that
//!     does not need `find_member`. Defers (returns `None`) for the
//!     general `find_member` path.
//!
//! All functions take wire-format `Type` bytes and a `NativeTypeResolver`,
//! mirroring the established `subtypes::rust_is_subtype` pattern. `None`
//! means "Rust defers, Python runs the pure-Python path".

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

// ---------------------------------------------------------------------------
// Wire helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// `get_proper_type` for the wire format. Expands `TypeAliasType` by
/// returning `None` (defer) since the wire format has no alias target.
/// For all other types, returns the type as-is (they are already proper).
fn get_proper_or_none(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        _ => Some(typ),
    }
}

// ---------------------------------------------------------------------------
// custom_special_method (typeops.py:1555)
// ---------------------------------------------------------------------------

/// Does this type have a custom special method such as `__eq__` or
/// `__ne__`? Mirrors `mypy.typeops.custom_special_method`.
///
/// Returns `Some(bool)` when Rust decided; `None` to defer. The
/// `check_all` flag controls union semantics (any vs all).
pub(crate) fn custom_special_method_inner(
    typ: &Type,
    name: &str,
    check_all: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance { type_ref, .. } => {
            let snap = resolver.get(type_ref)?;
            let (kind, definer) = snap.member_definers.get(name)?;
            // Node must be FuncBase(0) / Decorator(1) / Var(2); the
            // snapshot only stores those kinds, so any entry qualifies.
            if *kind < 0 {
                return Some(false);
            }
            // method.node.info.fullname.startswith(("builtins.", "typing."))
            // -> NOT custom (returns False).
            if definer.starts_with("builtins.") || definer.starts_with("typing.") {
                Some(false)
            } else {
                Some(true)
            }
        }
        Type::UnionType { items, .. } => {
            if check_all {
                // all(...) — short-circuit on first None or false.
                let mut all_true = true;
                for t in items {
                    match custom_special_method_inner(t, name, check_all, resolver) {
                        Some(true) => {}
                        Some(false) => return Some(false),
                        None => {
                            all_true = false;
                        }
                    }
                }
                if all_true {
                    Some(true)
                } else {
                    None
                }
            } else {
                // any(...) — short-circuit on first true; defer on None.
                let mut any_true = false;
                let mut any_none = false;
                for t in items {
                    match custom_special_method_inner(t, name, false, resolver) {
                        Some(true) => return Some(true),
                        Some(false) => {}
                        None => any_none = true,
                    }
                    let _ = &mut any_true;
                }
                if any_none {
                    None
                } else {
                    Some(false)
                }
            }
        }
        Type::TupleType { .. } => {
            let fallback = crate::typeops::tuple_fallback(proper, resolver)?;
            custom_special_method_inner(&fallback, name, check_all, resolver)
        }
        Type::CallableType {
            fallback,
            ret_type,
            from_concatenate,
            ..
        } if is_type_obj(fallback, ret_type, *from_concatenate, resolver) => {
            // FunctionLike.is_type_obj(): look up on the metaclass
            // (the fallback). For CallableType, `is_type_obj` means
            // fallback.type.is_metaclass() and ret_type is not
            // UninhabitedType. We recurse on the fallback Instance.
            custom_special_method_inner(fallback, name, check_all, resolver)
        }
        Type::Overloaded { items } => {
            // FunctionLike.is_type_obj(): all items agree, so check
            // items[0]. If it's a type obj, recurse on its fallback;
            // otherwise defer (the Python path checks the fallback
            // via `typ.fallback` which the wire Overloaded lacks).
            if let Some(Type::CallableType {
                fallback,
                ret_type,
                from_concatenate,
                ..
            }) = items.first()
            {
                if is_type_obj(fallback, ret_type, *from_concatenate, resolver) {
                    return custom_special_method_inner(fallback, name, check_all, resolver);
                }
            }
            None
        }
        Type::TypeType { item, .. } => {
            if let Type::Instance { type_ref, .. } = item.as_ref() {
                if let Some(snap) = resolver.get(type_ref) {
                    if let Some(metaclass_fullname) = &snap.metaclass_fullname {
                        // Look up __method__ on the metaclass for class
                        // objects (typeops.py:1584-1586).
                        if let Some(meta_snap) = resolver.get(metaclass_fullname) {
                            return custom_special_method_on_snap(meta_snap, name, check_all);
                        }
                    }
                }
            }
            None
        }
        Type::AnyType { .. } => Some(true),
        _ => Some(false),
    }
}

/// Lookup `name` directly on a `TypeInfoSnapshot`'s `member_definers`.
/// Used by the `TypeType` branch where we recurse on the metaclass.
fn custom_special_method_on_snap(
    snap: &crate::typeinfo::TypeInfoSnapshot,
    name: &str,
    _check_all: bool,
) -> Option<bool> {
    let (kind, definer) = snap.member_definers.get(name)?;
    if *kind < 0 {
        return Some(false);
    }
    if definer.starts_with("builtins.") || definer.starts_with("typing.") {
        Some(false)
    } else {
        Some(true)
    }
}

/// Whether a `CallableType` is a type object. Mirrors
/// `CallableType.is_type_obj()` (types.py:2358) =
/// `fallback.type.is_metaclass() and not isinstance(get_proper_type(ret_type),
/// UninhabitedType)`. `is_metaclass` checks MRO for `builtins.type`.
fn is_type_obj(
    fallback: &Type,
    ret_type: &Type,
    from_concatenate: bool,
    resolver: &TypeResolver,
) -> bool {
    if from_concatenate {
        return false;
    }
    if matches!(ret_type, Type::UninhabitedType { .. }) {
        return false;
    }
    let type_ref = match fallback {
        Type::Instance { type_ref, .. } => type_ref.as_str(),
        _ => return false,
    };
    if type_ref == "builtins.type" {
        return true;
    }
    if let Some(snap) = resolver.get(type_ref) {
        snap.mro.iter().any(|m| m == "builtins.type")
    } else {
        false
    }
}

/// `#[pyfunction]` entry for `custom_special_method`.
///
/// Takes serialized type bytes, a method name, and the `check_all` flag.
/// Returns `Some(bool)` or `None` (defer to Python).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_custom_special_method(
    type_bytes: &[u8],
    name: &str,
    check_all: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let typ = decode_type(type_bytes)?;
    custom_special_method_inner(&typ, name, check_all, resolver.resolver())
}

// ---------------------------------------------------------------------------
// has_custom_eq_checks (checker.py:9493)
// ---------------------------------------------------------------------------

/// `mypy.checker.has_custom_eq_checks` — whether the type has a custom
/// `__eq__` or `__ne__` method.
///
/// Mirrors `has_custom_eq_checks` (checker.py:9493-9496):
/// `custom_special_method(t, "__eq__", check_all=False) or
///  custom_special_method(t, "__ne__", check_all=False)`.
///
/// Returns `Some(bool)` or `None` (defer to Python).
fn has_custom_eq_checks_inner(typ: &Type, resolver: &TypeResolver) -> Option<bool> {
    // custom_special_method(t, "__eq__", check_all=False) or
    // custom_special_method(t, "__ne__", check_all=False)
    // Defer (return None) when either call returns None, matching the
    // Python `or` short-circuit: if __eq__ is None, the result is None
    // (the `or` of None and bool is None in the parity contract).
    match custom_special_method_inner(typ, "__eq__", false, resolver) {
        Some(true) => Some(true),
        None => None,
        Some(false) => match custom_special_method_inner(typ, "__ne__", false, resolver) {
            Some(true) => Some(true),
            Some(false) => Some(false),
            None => None,
        },
    }
}

/// `#[pyfunction]` entry for `has_custom_eq_checks`.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_has_custom_eq_checks(
    type_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let typ = decode_type(type_bytes)?;
    has_custom_eq_checks_inner(&typ, resolver.resolver())
}

// ---------------------------------------------------------------------------
// restrict_subtype_away (subtypes.py:2363)
// ---------------------------------------------------------------------------

/// MYPYC_NATIVE_INT_NAMES (types.py:190-195).
const MYPYC_NATIVE_INT_NAMES: &[&str] = &[
    "mypy_extensions.i64",
    "mypy_extensions.i32",
    "mypy_extensions.i16",
    "mypy_extensions.u8",
];

/// `mypy.subtypes.covers_at_runtime` (subtypes.py:2402-2435):
/// will `isinstance(item, supertype)` always return True at runtime?
///
/// Defers (returns `None`) when a path Rust cannot decide is reached.
fn covers_at_runtime(
    item: &Type,
    supertype: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    let item = get_proper_or_none(item)?;
    let supertype_proper = get_proper_or_none(supertype)?;

    // If supertype is not a type-obj FunctionLike, erase it.
    let supertype_erased: Type;
    let supertype_to_use: &Type = if !is_function_like_type_obj(supertype_proper, resolver) {
        supertype_erased =
            crate::argapprox::erase_type(supertype_proper, strict_optional, resolver)?;
        &supertype_erased
    } else {
        supertype_proper
    };

    // erase_type(item) <: supertype, ignore_promotions, erase_instances.
    let item_erased = crate::argapprox::erase_type(item, strict_optional, resolver)?;
    let ctx =
        crate::subtypes::SubtypeContext::new(false, false, false, true, true, strict_optional);
    match crate::subtypes::is_subtype(&item_erased, supertype_to_use, &ctx, resolver) {
        Some(true) => return Some(true),
        // Defer: Rust's is_subtype could not decide; Python's
        // is_proper_subtype would, so fall through to Python.
        None => return None,
        Some(false) => {}
    }

    if let Type::Instance { type_ref, .. } = supertype_to_use {
        if let Some(snap) = resolver.get(type_ref) {
            if snap.is_protocol {
                // is_proper_subtype(item, supertype, ignore_promotions=True).
                let pctx = crate::subtypes::SubtypeContext::new(
                    false,
                    false,
                    false,
                    true,
                    true,
                    strict_optional,
                );
                match crate::subtypes::is_subtype(item, supertype_to_use, &pctx, resolver) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
        }
    }

    // isinstance(item, TypedDictType) and supertype is builtins.dict -> True.
    if let Type::TypedDictType { .. } = item {
        if let Type::Instance { type_ref, .. } = supertype_to_use {
            if type_ref == "builtins.dict" {
                return Some(true);
            }
        }
    }

    // isinstance(item, TypeVarType): upper_bound <: supertype.
    if let Type::TypeVarType { upper_bound, .. } = item {
        let pctx =
            crate::subtypes::SubtypeContext::new(false, false, false, true, false, strict_optional);
        match crate::subtypes::is_subtype(upper_bound, supertype_to_use, &pctx, resolver) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }

    // isinstance(item, Instance) and supertype is builtins.int ->
    // covers mypyc native int types.
    if let Type::Instance {
        type_ref: item_ref, ..
    } = item
    {
        if let Type::Instance { type_ref, .. } = supertype_to_use {
            if type_ref == "builtins.int" && MYPYC_NATIVE_INT_NAMES.contains(&item_ref.as_str()) {
                return Some(true);
            }
        }
    }

    Some(false)
}

/// Whether a type is a `FunctionLike` whose `is_type_obj()` is True.
/// The wire format lacks a unified `FunctionLike`; check both
/// `CallableType` and `Overloaded`.
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

/// `mypy.subtypes.restrict_subtype_away` (subtypes.py:2363-2391):
/// return `t minus s` for runtime type assertions.
///
/// Mirrors the Python: for `UnionType` left, restrict each item and
/// drop `UninhabitedType` results; for `TypeVarType` left, restrict the
/// upper_bound; otherwise, if `consider_runtime_isinstance`, return
/// `UninhabitedType()` when `covers_at_runtime(t, s)`, else `t`; if not
/// `consider_runtime_isinstance`, return `UninhabitedType()` when
/// `is_proper_subtype(t, s, ignore_promotions=True)` or
/// `is_proper_subtype(t, s, ignore_promotions=True, erase_instances=True)`,
/// else `t`.
///
/// Returns `Some(Vec<u8>)` (encoded result type) or `None` (defer).
fn restrict_subtype_away_inner(
    t: &Type,
    s: &Type,
    consider_runtime_isinstance: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    let p_t = get_proper_or_none(t)?;

    match p_t {
        Type::UnionType { items, .. } => {
            // try_restrict_literal_union first.
            let restricted = try_restrict_literal_union(t, s, strict_optional, resolver);
            let new_items: Vec<Type> = if let Some(remaining_blobs) = restricted {
                let mut decoded = Vec::with_capacity(remaining_blobs.len());
                for blob in &remaining_blobs {
                    decoded.push(decode_type(blob)?);
                }
                decoded
            } else {
                // relevant_items(): skip NoneType when strict_optional off.
                let mut result = Vec::with_capacity(items.len());
                for item in items {
                    if !strict_optional && matches!(item, Type::NoneType) {
                        continue;
                    }
                    let restricted_item = restrict_subtype_away_inner(
                        item,
                        s,
                        consider_runtime_isinstance,
                        strict_optional,
                        resolver,
                    )?;
                    result.push(restricted_item);
                }
                result
            };
            // Drop UninhabitedType items.
            let filtered: Vec<Type> = new_items
                .into_iter()
                .filter(|t| !matches!(t, Type::UninhabitedType { .. }))
                .collect();
            Some(crate::setops::union_make_union(filtered))
        }
        Type::TypeVarType { upper_bound, .. } => {
            let restricted = restrict_subtype_away_inner(
                upper_bound,
                s,
                consider_runtime_isinstance,
                strict_optional,
                resolver,
            )?;
            // copy_modified(upper_bound=restricted). The wire format
            // stores all TypeVarType fields; we clone and swap the
            // upper_bound.
            if let Type::TypeVarType {
                name,
                fullname,
                raw_id,
                namespace,
                values,
                default,
                variance,
                meta_level,
                ..
            } = p_t
            {
                Some(Type::TypeVarType {
                    name: name.clone(),
                    fullname: fullname.clone(),
                    raw_id: *raw_id,
                    namespace: namespace.clone(),
                    values: values.clone(),
                    upper_bound: Box::new(restricted),
                    default: default.clone(),
                    variance: *variance,
                    meta_level: *meta_level,
                })
            } else {
                None
            }
        }
        _ => {
            if consider_runtime_isinstance {
                match covers_at_runtime(t, s, strict_optional, resolver) {
                    Some(true) => Some(Type::UninhabitedType { ambiguous: false }),
                    // Defer on Some(false): Rust's covers_at_runtime lacks
                    // erase_instances parity, so a false may be wrong. Let
                    // Python's covers_at_runtime decide.
                    Some(false) | None => None,
                }
            } else {
                let ctx = crate::subtypes::SubtypeContext::new(
                    false,
                    false,
                    false,
                    true,
                    true,
                    strict_optional,
                );
                match crate::subtypes::is_subtype(t, s, &ctx, resolver) {
                    Some(true) => return Some(Type::UninhabitedType { ambiguous: false }),
                    None => return None,
                    Some(false) => {}
                }
                // erase_instances=True — Python checks is_proper_subtype
                // again with erase_instances. Rust's SubtypeContext lacks
                // an erase_instances field, so this second check is
                // incomplete. Defer to Python.
                None
            }
        }
    }
}

/// `try_restrict_literal_union` (subtypes.py:2264-2282). Reuses the
/// existing `subtypes::try_restrict_literal_union` wire helper.
fn try_restrict_literal_union(
    t: &Type,
    s: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let Type::UnionType { items, .. } = t else {
        return None;
    };
    let s_is_simple = crate::typeops::is_simple_literal(s, resolver)?;
    if !s_is_simple {
        return None;
    }
    let mut remaining: Vec<Vec<u8>> = Vec::new();
    for item in items {
        if !strict_optional && matches!(item, Type::NoneType) {
            continue;
        }
        let is_simple = crate::typeops::is_simple_literal(item, resolver)?;
        if !is_simple {
            return None;
        }
        if item != s {
            let mut buf = WriteBuffer::new();
            wire::write_type(&mut buf, item).ok()?;
            remaining.push(buf.into_bytes());
        }
    }
    Some(remaining)
}

/// `#[pyfunction]` entry for `restrict_subtype_away`.
///
/// Returns `Some(Vec<u8>)` (encoded result type) or `None` (defer).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_restrict_subtype_away(
    t_bytes: &[u8],
    s_bytes: &[u8],
    consider_runtime_isinstance: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let s = decode_type(s_bytes)?;
    let result = restrict_subtype_away_inner(
        &t,
        &s,
        consider_runtime_isinstance,
        strict_optional,
        resolver.resolver(),
    )?;
    encode_type(&result)
}

// ---------------------------------------------------------------------------
// join_type_list (join.py:1142)
// ---------------------------------------------------------------------------

/// `mypy.join.join_type_list` (join.py:1142-1148): fold-join over a list.
///
/// Defers entirely to Python. The single-item passthrough and empty-list
/// cases look safe, but the wire round-trip breaks TypeVar object identity
/// (Python's inference relies on fresh TypeVars being the same object
/// across occurrences), causing regressions on ParamSpec/variadic tests.
/// Returning None lets Python handle all cases natively.
fn join_type_list_inner(
    _items: &[Type],
    _strict_optional: bool,
    _resolver: &TypeResolver,
) -> Option<Type> {
    None
}

/// `#[pyfunction]` entry for `join_type_list`.
///
/// Takes a list of serialized type blobs and returns the encoded join
/// result, or `None` (defer).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_join_type_list(
    type_blobs: Vec<Vec<u8>>,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let mut items = Vec::with_capacity(type_blobs.len());
    for blob in &type_blobs {
        items.push(decode_type(blob)?);
    }
    let result = join_type_list_inner(&items, strict_optional, resolver.resolver())?;
    encode_type(&result)
}

// ---------------------------------------------------------------------------
// get_protocol_member (subtypes.py:1513)
// ---------------------------------------------------------------------------

/// `mypy.subtypes.get_protocol_member` (subtypes.py:1513-1533): look up
/// a member on a protocol instance.
///
/// Handles the two pure special cases that do not need `find_member`:
///   1. `member == "__call__" and class_obj` -> return `type_object_type(left.type)`.
///   2. `member == "__call__" and left.type.is_metaclass(precise=True)` -> return `None`.
///
/// For all other cases, defers to Python (returns `None`), since the
/// general path needs `find_member` + `checker_state`.
///
/// Returns `Some(Vec<u8>)` (encoded type) or `None` (defer). For case 2,
/// returns `Some(empty_vec)` to distinguish "Rust decided None" from
/// "Rust defers". The Python shim interprets `Some(empty)` as
/// "return None" and `None` as "defer".
fn get_protocol_member_inner(
    left: &Type,
    member: &str,
    class_obj: bool,
    resolver: &TypeResolver,
) -> GetProtocolMemberResult {
    let Type::Instance { type_ref, .. } = left else {
        return GetProtocolMemberResult::Defer;
    };
    let snap = match resolver.get(type_ref) {
        Some(s) => s,
        None => return GetProtocolMemberResult::Defer,
    };

    if member == "__call__" && class_obj {
        // type_object_type(left.type): the constructor type. This needs
        // the full TypeInfo -> type_object_type computation which is
        // complex; defer to Python.
        return GetProtocolMemberResult::Defer;
    }

    if member == "__call__" && is_metaclass_precise(snap, resolver) {
        // Avoid falling back to metaclass __call__; return None.
        return GetProtocolMemberResult::None;
    }

    GetProtocolMemberResult::Defer
}

/// `TypeInfo.is_metaclass(precise=True)` (nodes.py:4128-4133): true iff
/// the TypeInfo itself (not its MRO) is `builtins.type` or `abc.ABCMeta`.
fn is_metaclass_precise(
    snap: &crate::typeinfo::TypeInfoSnapshot,
    _resolver: &TypeResolver,
) -> bool {
    // is_metaclass checks if fullname is builtins.type or abc.ABCMeta.
    snap.fullname == "builtins.type" || snap.fullname == "abc.ABCMeta"
}

/// Distinguish "Rust decided None" from "Rust defers".
enum GetProtocolMemberResult {
    /// The member is None (e.g. __call__ on a precise metaclass).
    None,
    /// Rust defers to Python.
    Defer,
}

/// `#[pyfunction]` entry for `get_protocol_member`.
///
/// Returns `Some(Vec<u8>)` (encoded type) for a found member,
/// `Some(empty Vec)` for "Rust decided None", or `None` (defer).
/// The Python shim checks `result is None` for defer, `len(result) == 0`
/// for "return None", else deserializes the bytes.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_get_protocol_member(
    left_bytes: &[u8],
    original_left_bytes: &[u8],
    member: &str,
    class_obj: bool,
    is_lvalue: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let _original = decode_type(original_left_bytes)?;
    let left = decode_type(left_bytes)?;
    let _ = is_lvalue;
    match get_protocol_member_inner(&left, member, class_obj, resolver.resolver()) {
        GetProtocolMemberResult::None => Some(Vec::new()),
        GetProtocolMemberResult::Defer => None,
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::{TypeInfoSnapshot, TypeResolver};

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn make_resolver_with_definer(
        type_ref: &str,
        member: &str,
        kind: i64,
        definer: &str,
    ) -> TypeResolver {
        let mut r = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: type_ref.to_string(),
            name: type_ref.rsplit('.').next().unwrap_or(type_ref).to_string(),
            ..Default::default()
        };
        snap.member_definers
            .insert(member.to_string(), (kind, definer.to_string()));
        r.insert(type_ref.to_string(), snap);
        r
    }

    fn make_resolver_with_metaclass(type_ref: &str, metaclass_fullname: &str) -> TypeResolver {
        let mut r = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: type_ref.to_string(),
            name: type_ref.rsplit('.').next().unwrap_or(type_ref).to_string(),
            metaclass_fullname: Some(metaclass_fullname.to_string()),
            ..Default::default()
        };
        r.insert(type_ref.to_string(), snap);
        let meta_snap = TypeInfoSnapshot {
            fullname: metaclass_fullname.to_string(),
            name: metaclass_fullname
                .rsplit('.')
                .next()
                .unwrap_or(metaclass_fullname)
                .to_string(),
            ..Default::default()
        };
        r.insert(metaclass_fullname.to_string(), meta_snap);
        r
    }

    // -- custom_special_method --

    #[test]
    fn test_custom_special_method_custom_definer() {
        let r = make_resolver_with_definer("mymod.Foo", "__eq__", 0, "mymod.Foo");
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(true)
        );
    }

    #[test]
    fn test_custom_special_method_builtins_definer() {
        let r = make_resolver_with_definer("builtins.int", "__eq__", 0, "builtins.int");
        let t = make_instance("builtins.int", vec![]);
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(false)
        );
    }

    #[test]
    fn test_custom_special_method_typing_definer() {
        let r = make_resolver_with_definer("typing.Dict", "__ne__", 2, "typing.Dict");
        let t = make_instance("typing.Dict", vec![]);
        assert_eq!(
            custom_special_method_inner(&t, "__ne__", false, &r),
            Some(false)
        );
    }

    #[test]
    fn test_custom_special_method_missing_member() {
        let r = make_resolver_with_definer("mymod.Foo", "__ne__", 0, "mymod.Foo");
        let t = make_instance("mymod.Foo", vec![]);
        // __eq__ not in member_definers -> None (defer).
        assert_eq!(custom_special_method_inner(&t, "__eq__", false, &r), None);
    }

    #[test]
    fn test_custom_special_method_missing_snapshot() {
        let r = TypeResolver::new();
        let t = make_instance("mymod.NotFound", vec![]);
        assert_eq!(custom_special_method_inner(&t, "__eq__", false, &r), None);
    }

    #[test]
    fn test_custom_special_method_any_returns_true() {
        let r = TypeResolver::new();
        let t = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(true)
        );
    }

    #[test]
    fn test_custom_special_method_union_any_short_circuits() {
        let r = make_resolver_with_definer("mymod.Foo", "__eq__", 0, "mymod.Foo");
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let inst = make_instance("mymod.Foo", vec![]);
        let t = Type::UnionType {
            items: vec![inst, any],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(true)
        );
    }

    #[test]
    fn test_custom_special_method_union_check_all_false() {
        let r = make_resolver_with_definer("mymod.Foo", "__eq__", 0, "builtins.object");
        let inst = make_instance("mymod.Foo", vec![]);
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let t = Type::UnionType {
            items: vec![inst, any],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        // check_all=True: first item returns false -> Some(false).
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", true, &r),
            Some(false)
        );
    }

    // -- has_custom_eq_checks --

    #[test]
    fn test_has_custom_eq_checks_eq_custom() {
        let r = make_resolver_with_definer("mymod.Foo", "__eq__", 0, "mymod.Foo");
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(has_custom_eq_checks_inner(&t, &r), Some(true));
    }

    #[test]
    fn test_has_custom_eq_checks_ne_custom_only() {
        // __eq__ is builtins (returns false), __ne__ is custom (returns true).
        let mut r = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "mymod.Foo".to_string(),
            name: "Foo".to_string(),
            ..Default::default()
        };
        snap.member_definers
            .insert("__eq__".to_string(), (0, "builtins.object".to_string()));
        snap.member_definers
            .insert("__ne__".to_string(), (0, "mymod.Foo".to_string()));
        r.insert("mymod.Foo".to_string(), snap);
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(has_custom_eq_checks_inner(&t, &r), Some(true));
    }

    #[test]
    fn test_has_custom_eq_checks_both_builtins() {
        let mut r = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "builtins.int".to_string(),
            name: "int".to_string(),
            ..Default::default()
        };
        snap.member_definers
            .insert("__eq__".to_string(), (0, "builtins.int".to_string()));
        snap.member_definers
            .insert("__ne__".to_string(), (0, "builtins.int".to_string()));
        r.insert("builtins.int".to_string(), snap);
        let t = make_instance("builtins.int", vec![]);
        assert_eq!(has_custom_eq_checks_inner(&t, &r), Some(false));
    }

    #[test]
    fn test_has_custom_eq_checks_defers_on_missing() {
        let r = TypeResolver::new();
        let t = make_instance("mymod.NotFound", vec![]);
        assert_eq!(has_custom_eq_checks_inner(&t, &r), None);
    }

    // -- get_protocol_member --

    #[test]
    fn test_get_protocol_member_call_on_metaclass_precise() {
        let r = make_resolver_with_metaclass("builtins.type", "builtins.type");
        let left = make_instance("builtins.type", vec![]);
        let res = get_protocol_member_inner(&left, "__call__", false, &r);
        assert!(matches!(res, GetProtocolMemberResult::None));
    }

    #[test]
    fn test_get_protocol_member_call_on_non_metaclass_defers() {
        let r = make_resolver_with_metaclass("mymod.Foo", "builtins.type");
        let left = make_instance("mymod.Foo", vec![]);
        let res = get_protocol_member_inner(&left, "__call__", false, &r);
        assert!(matches!(res, GetProtocolMemberResult::Defer));
    }

    #[test]
    fn test_get_protocol_member_non_call_defers() {
        let r = make_resolver_with_metaclass("mymod.Foo", "builtins.type");
        let left = make_instance("mymod.Foo", vec![]);
        let res = get_protocol_member_inner(&left, "foo", false, &r);
        assert!(matches!(res, GetProtocolMemberResult::Defer));
    }

    #[test]
    fn test_get_protocol_member_missing_snapshot_defers() {
        let r = TypeResolver::new();
        let left = make_instance("mymod.NotFound", vec![]);
        let res = get_protocol_member_inner(&left, "__call__", false, &r);
        assert!(matches!(res, GetProtocolMemberResult::Defer));
    }

    #[test]
    fn test_get_protocol_member_non_instance_defers() {
        let r = TypeResolver::new();
        let left = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let res = get_protocol_member_inner(&left, "__call__", false, &r);
        assert!(matches!(res, GetProtocolMemberResult::Defer));
    }

    // -- join_type_list --

    #[test]
    fn test_join_type_list_always_defers() {
        let r = TypeResolver::new();
        // Empty list: defer (Python handles UninhabitedType).
        assert_eq!(join_type_list_inner(&[], true, &r), None);
        // Single element: defer (Python returns it without round-trip).
        let inst = make_instance("builtins.int", vec![]);
        assert_eq!(join_type_list_inner(&[inst], true, &r), None);
        // Multi element: defer.
        let a = make_instance("builtins.int", vec![]);
        let b = make_instance("builtins.str", vec![]);
        assert_eq!(join_type_list_inner(&[a, b], true, &r), None);
    }

    // -- restrict_subtype_away --

    #[test]
    fn test_restrict_subtype_away_simple_instance_no_covers() {
        let r = TypeResolver::new();
        let t = make_instance("builtins.int", vec![]);
        let s = make_instance("builtins.str", vec![]);
        // No snapshot for either; covers_at_runtime defers on missing
        // snapshot -> restrict_subtype_away returns None.
        let result = restrict_subtype_away_inner(&t, &s, true, true, &r);
        assert_eq!(result, None);
    }

    #[test]
    fn test_restrict_subtype_away_any_supertype_defers() {
        let r = TypeResolver::new();
        let t = make_instance("builtins.int", vec![]);
        let s = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        // covers_at_runtime calls is_proper_subtype(erase(int), Any).
        // Rust's is_subtype with proper_subtype=true and right=Any defers
        // (the Any-right short-circuit only fires for non-proper). So the
        // whole restrict_subtype_away defers to Python.
        let result = restrict_subtype_away_inner(&t, &s, true, true, &r);
        assert_eq!(result, None);
    }
}
