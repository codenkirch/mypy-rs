//! Issue #477: pure narrowing/validation helpers from `mypy.checker` and
//! `mypy.typeops` / `mypy.subtypes` / `mypy.join`.
//!
//! Ports:
//!   * `custom_special_method` (typeops.py:1555) — does a type have a
//!     custom special method (e.g. `__eq__`) not inherited from
//!     `builtins.` / `typing.`? Uses the `member_definers` snapshot
//!     field and walks the snapshot MRO to match `TypeInfo.get`.
//!   * `has_custom_eq_checks` (checker.py:9493) — thin wrapper calling
//!     `custom_special_method` for `__eq__` and `__ne__`.
//!   * `restrict_subtype_away` (subtypes.py:2363) — `t minus s` for
//!     runtime type assertions. Handles the union-literal restriction
//!     and the non-union `consider_runtime_isinstance` / proper-subtype
//!     branches. Defers (returns `None`) when `covers_at_runtime` needs
//!     a path Rust cannot decide.
//!   * `join_type_list` (join.py:1508) — fold-join over a list of types
//!     (empty -> UninhabitedType, then pairwise via the setops join
//!     kernel: Instance nominal join, union flattening, CallableType
//!     similarity, TypeType, Literal, TypeVar default). Defers the
//!     whole call on any LKV / fallback_to_any item or any pair the
//!     kernel cannot decide. Returns encoded bytes so the Python shim
//!     can decode to a live `Type`.
//!   * `get_protocol_member` (subtypes.py:1832) — `get_protocol_member`
//!     minus `find_member`'s attribute-hook / descriptor / error paths and
//!     the `__call__`-on-class-object `type_object_type` computation. Rust
//!     decides the `__call__` special cases and the common protocol-member
//!     cases of `find_member` (subtypes.py:1874-1948) by live PyO3 reads:
//!     a plain `FuncDef` / `OverloadedFuncDef` method defined directly on
//!     the receiver class binds + expands via the checkmember.rs
//!     `member_method_inner` machinery, and a plain annotated `Var`
//!     (not a property / classmethod / staticmethod / classvar / inferred
//!     var, no descriptor, no plugin attribute hook, no Self type) expands
//!     via `expand_type_by_instance` preserving type-var ids. Defers
//!     (returns `None`) on anything needing plugins, descriptors, class
//!     attributes, error emission, `type_object_type`, or a non-direct
//!     receiver.
//!
//! All functions take wire-format `Type` bytes and a `NativeTypeResolver`,
//! mirroring the established `subtypes::rust_is_subtype` pattern. `None`
//! means "Rust defers, Python runs the pure-Python path".

use pyo3::prelude::*;
use pyo3::types::PyList;

use crate::subtypes::SubtypeContext;
use crate::typeinfo::{serialize_type_to_bytes, NativeTypeResolver, TypeResolver};
use crate::visitor::has_type_vars_inner;
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

// Live-attribute helpers (mirror member_flags.rs:42-52). `None` on any
// read failure means "defer to Python", never a guessed value.
fn get_bool_flag(py: Python<'_>, node: &PyAny, name: &str) -> Option<bool> {
    let v = node.getattr(name).ok()?;
    if v.is_none() {
        return Some(false);
    }
    if let Ok(b) = v.extract::<bool>() {
        return Some(b);
    }
    if let Ok(b) = v.downcast::<pyo3::types::PyBool>() {
        return Some(b.is_true());
    }
    if let Ok(i) = v.extract::<i64>() {
        return Some(i != 0);
    }
    // A non-bool, non-int object: Python truthiness is not decidable here.
    let _ = py;
    None
}

fn get_opt_str_attr(node: &PyAny, name: &str) -> Option<String> {
    let v = node.getattr(name).ok()?;
    if v.is_none() {
        None
    } else {
        v.extract::<String>().ok()
    }
}

/// `is_descriptor(typ)` (subtypes.py:2091-2097) for the wire format:
/// true iff `typ` is an Instance whose class has a readable `__get__`, or
/// a Union all of whose relevant items do. Defer (None) when any component
/// cannot be decided from the resolver snapshots.
fn is_descriptor_wire(typ: &Type, resolver: &TypeResolver) -> Option<bool> {
    let proper = get_proper_or_none(typ)?;
    match proper {
        Type::Instance { type_ref, .. } => {
            crate::checkmember::has_readable_member_by_ref(resolver, type_ref, "__get__")
        }
        Type::UnionType { items, .. } => {
            let mut any_none = false;
            for t in items {
                match is_descriptor_wire(t, resolver) {
                    Some(true) => {
                        // all(...) — first true item: whole union is a
                        // descriptor.
                        return Some(true);
                    }
                    Some(false) => {}
                    None => any_none = true,
                }
            }
            // relevant_items filters NoneType under strict_optional; treat
            // a None item as non-descriptor (mirrors is_descriptor).
            if any_none {
                None
            } else {
                Some(false)
            }
        }
        _ => Some(false),
    }
}

/// `TypeInfo.get(name)` (nodes.py:4062-4066) walking the live MRO: return
/// the first `(TypeInfo, SymbolTableNode)` whose own `names` dict contains
/// `name`. Python looks up with `names.get(name)` (None-safe, never
/// raises), so a missing key on one base must continue the walk, not
/// abort it. `None` when absent or when any MRO class lacks `.names`.
fn mro_get(py: Python<'_>, info: &PyAny, name: &str) -> Option<(PyObject, PyObject)> {
    let mro = info.getattr("mro").ok()?;
    let mro_list = mro.downcast::<PyList>().ok()?;
    for base in mro_list.iter() {
        let names = base.getattr("names").ok()?;
        let value = names.getattr("get").ok()?.call1((name,)).ok()?;
        if !value.is_none() {
            return Some((base.to_object(py), value.to_object(py)));
        }
    }
    None
}

/// Safe read of the plugin-hook absence flag from `mypy.checkexpr`.
fn plugin_get_attribute_hook_absent(py: Python<'_>) -> bool {
    py.import("mypy.checkexpr")
        .and_then(|m| m.getattr("plugin_hook_known_absent"))
        .and_then(|f| f.call1(("get_attribute_hook", "protocol-member-dummy")))
        .and_then(|r| r.extract::<bool>())
        .unwrap_or(false)
}

/// Read `fullname -> TypeInfo` map presence for the plugin-hook registry.
fn live_plugin_registry_absent(py: Python<'_>) -> bool {
    py.import("mypy.checkexpr")
        .and_then(|m| m.getattr("_native_plugin_hook_has_user_plugins"))
        .and_then(|v| v.extract::<bool>())
        .unwrap_or(false)
}

/// Does a live plugin hook `fullname`? Mirrors `ChainedPlugin._find_hook`
/// over `_native_plugin_hook_plugins`; a hit defers to Python, an
/// all-None miss is the parity answer (None when the snapshot is missing).
fn plugin_get_attribute_hook_hits(py: Python<'_>, fullname: &str) -> Option<bool> {
    let plugins = py
        .import("mypy.checkexpr")
        .ok()?
        .getattr("_native_plugin_hook_plugins")
        .ok()?;
    if plugins.is_none() {
        return None;
    }
    for item in plugins.iter().ok()? {
        let plugin = item.ok()?;
        let hook_fn = plugin.getattr("get_attribute_hook").ok()?;
        let hook = hook_fn.call1((fullname,)).ok()?;
        if !hook.is_none() {
            return Some(true);
        }
    }
    Some(false)
}

/// `get_proper_type` for the wire format. Expands `TypeAliasType` by
/// returning `None` (defer) since the wire format has no alias target.
/// For all other types, returns the type as-is (they are already proper).
pub(crate) fn get_proper_or_none(typ: &Type) -> Option<&Type> {
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
        Type::Instance { type_ref, .. } => instance_custom_special_method(type_ref, name, resolver),
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
            // (the fallback). `is_type_obj`: fallback.type.is_metaclass()
            // and ret_type is not UninhabitedType; recurse on the fallback.
            custom_special_method_inner(fallback, name, check_all, resolver)
        }
        Type::Overloaded { items } => {
            // FunctionLike.is_type_obj(): all items agree, so check
            // items[0]. If it's a type obj, recurse on its fallback;
            // otherwise Python falls through to return False.
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
            Some(false)
        }
        Type::TypeType { item, .. } => {
            if let Type::Instance { type_ref, .. } = item.as_ref() {
                if let Some(metaclass_fullname) = &resolver.get(type_ref)?.metaclass_fullname {
                    // Look up __method__ on the metaclass for class objects
                    // (typeops.py:1584-1586), recursing on the metaclass
                    // Instance through the usual MRO walk.
                    return instance_custom_special_method(metaclass_fullname, name, resolver);
                }
            }
            Some(false)
        }
        Type::AnyType { .. } => Some(true),
        _ => Some(false),
    }
}

/// Instance branch of `custom_special_method` (typeops.py:1951-1956):
/// `typ.type.get(name)` walks the MRO and the first class with a
/// truthy `SymbolTableNode` decides (nodes.py:4063-4068). A method
/// counts as custom iff its defining class (the snapshot's
/// `member_definers` value, `node.info.fullname`) is not under
/// `builtins.` / `typing.`.
///
/// Walk the snapshot MRO (same pattern as
/// `dangerous_comparison.rs::instance_has_custom_eq_mro`, parameterized
/// by `name`). `None` when the type's own snapshot or any ancestor's is
/// missing; that safely defers to the Python fallback.
pub(crate) fn instance_custom_special_method(
    type_ref: &str,
    name: &str,
    resolver: &TypeResolver,
) -> Option<bool> {
    let snap = resolver.get(type_ref)?;
    for ancestor in &snap.mro {
        let ancestor_snap = resolver.get(ancestor)?;
        let Some((_kind, definer)) = ancestor_snap.member_definers.get(name) else {
            continue;
        };
        if definer.starts_with("builtins.") || definer.starts_with("typing.") {
            return Some(false);
        }
        return Some(true);
    }
    // MRO exhausted: Python's tail also returns False.
    Some(false)
}

/// Whether a `CallableType` is a type object. Mirrors
/// `CallableType.is_type_obj()` (types.py:2358) =
/// `fallback.type.is_metaclass() and not isinstance(get_proper_type(ret_type),
/// UninhabitedType)`. `is_metaclass` checks MRO for `builtins.type`.
pub(crate) fn is_type_obj(
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
    // Mirror typeops.py:1981 `typ = get_proper_type(typ)`: a top-level
    // alias expands through the resolver (nested alias items defer).
    let typ = crate::checkexpr_functions::get_proper_or_expand(&typ, resolver.alias_resolver())?;
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
    match custom_special_method_inner(typ, "__eq__", false, resolver) {
        Some(true) => Some(true),
        // Python `or` short-circuits: `True or anything` is True (above),
        // `False or x` evaluates x, and `None or x` evaluates x.
        Some(false) | None => custom_special_method_inner(typ, "__ne__", false, resolver),
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
    // The Python mirror routes through `custom_special_method`, whose
    // `get_proper_type(typ)` expands a top-level alias.
    let typ = crate::checkexpr_functions::get_proper_or_expand(&typ, resolver.alias_resolver())?;
    has_custom_eq_checks_inner(&typ, resolver.resolver())
}

// ---------------------------------------------------------------------------
// restrict_subtype_away (subtypes.py:2363)
// ---------------------------------------------------------------------------

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
/// Returns `Some(Type)` (result type) or `None` (defer to Python).
pub(crate) fn restrict_subtype_away_inner(
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
                // Route through the newer covers_at_runtime module: it adds
                // tuple-operand deferrals that fix a latent parity bug
                // (old Rust answered Some(false) where Python answers True).
                match crate::covers_at_runtime::covers_at_runtime_inner(
                    t,
                    s,
                    strict_optional,
                    resolver,
                ) {
                    Some(true) => Some(Type::UninhabitedType { ambiguous: false }),
                    // covers_at_runtime returns Some(false) only when every
                    // modeled check is confident (subtypes.py covers steps
                    // 1-6), so the Python result is `t`.
                    Some(false) => Some(t.clone()),
                    None => None,
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
                // erase_instances=True: the erased check is provably identical
                // to the plain check when `s` is non-generic/non-protocol, so
                // answer `t` natively (see should_restrict_to_t_no_erase).
                if should_restrict_to_t_no_erase(s, resolver)? {
                    return Some(t.clone());
                }
                None
            }
        }
    }
}

/// Decides `erase_instances` parity for `restrict_subtype_away`'s
/// ``consider_runtime_isinstance=False`` branch. Python's second check
/// `is_proper_subtype(t, s, ignore_promotions=True, erase_instances=True)`
/// erases the left Instance *only* inside `visit_instance`'s nominal
/// branch (subtypes.py:1069), and then only against a right whose type
/// parameters drive an argument recursion. When `s` is a non-generic
/// (empty `type_vars_with_variance`), non-protocol Instance, the whole
/// comparison has no parameter recursion, so erasing is a no-op for every
/// reachable subtype check: `Some(false)` from check 1 implies check 2 is
/// also false and Python returns `t` unchanged. Returns `Some(true)` to
/// answer `t` natively, `Some(false)` to keep deferring, or `None` (a
/// missing right snapshot) to propagate the existing deferral.
fn should_restrict_to_t_no_erase(s: &Type, resolver: &TypeResolver) -> Option<bool> {
    let Type::Instance { type_ref, .. } = s else {
        return Some(false);
    };
    let snap = resolver.get(type_ref)?;
    if snap.is_protocol || !snap.type_vars_with_variance.is_empty() {
        return Some(false);
    }
    Some(true)
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
    // Mirror subtypes.py's `_is_subtype` (subtypes.py:531), which does
    // `get_proper_type` on both operands: top-level aliases expand via
    // the resolver (nested alias items still defer, parity-safe).
    let t = crate::checkexpr_functions::get_proper_or_expand(&t, resolver.alias_resolver())?;
    let s = crate::checkexpr_functions::get_proper_or_expand(&s, resolver.alias_resolver())?;
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
// join_type_list (join.py:1508-1529)
// ---------------------------------------------------------------------------

/// `TypeOfAny.special_form` (mypy/type_visitor.py / types.py:2682).
const TYPE_OF_ANY_SPECIAL_FORM: i64 = 6;

/// `mypy.join.join_type_list` (join.py:1508-1529): fold-join over a
/// list, pairing each item with the accumulator through the setops join
/// kernel (`join_one_pair`, mirroring `join_types`, join.py:1508-1510).
/// The kernel is parity-tested for every pair shape it decides:
/// Instance-Instance (same-type passthrough, subtype domination, common
/// ancestor), union flattening, Any / NoneType / UninhabitedType
/// absorption, CallableType similarity + combine, TypeType joins,
/// Literal handling, TypedDict, and TypeVar (same-id bound join, and
/// `default(s)` -> object for an Instance `s`).
///
/// The whole call defers to Python when:
///   - any item carries a `last_known_value` (Python's join erases LKVs
///     by rebuilding instances; the wire kernel would keep them);
///   - any item is a class with `fallback_to_any` (a stub / unknown
///     base; Python's join prefers those pairs' Any fallback);
///   - any single item is not identity-safe (TypeVar/ParamSpec anywhere,
///     or a TypeInfo snapshot is missing);
///   - any pair the setops kernel cannot decide returns None and the
///     whole call falls back to the pure-Python fold. Deferral is always
///     safe: Python re-runs the identical algorithm.
fn join_type_list_inner(
    items: &[Type],
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    // join.py:1519-1520: empty list -> UninhabitedType().
    if items.is_empty() {
        return Some(Type::UninhabitedType { ambiguous: false });
    }
    // Single-item passthrough: Python's fold starts `joined = types[0]`,
    // so one item has no join decision. Pass through only identity-safe
    // items (resolver-resolvable, no TypeVar/ParamSpec anywhere).
    if items.len() == 1 && is_join_safe(&items[0], resolver) && !has_type_vars_inner(&items[0]) {
        return Some(items[0].clone());
    }
    if items.len() == 1 {
        return None;
    }
    // Whole-list cheap guards: Python's join erases last-known values
    // and lets `fallback_to_any` classes absorb into Any; neither is
    // reproducible through the wire kernel, so defer the whole call.
    if items.iter().any(has_lkv) {
        return None;
    }
    if items
        .iter()
        .any(|t| instance_has_fallback_to_any(t, resolver))
    {
        return None;
    }
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    let mut acc = items[0].clone();
    for item in &items[1..] {
        acc = join_one_pair(&acc, item, &ctx, resolver)?;
    }
    Some(acc)
}

/// Join one pair of types, mirroring the `join.join_type_list` fold
/// step (`join_types`, join.py:1508-1510). The args-less Instance-
/// Instance nominal case goes through `visit_instance_join` directly
/// (same-type, subtype -> supertype, else common ancestor), mapped back
/// to a concrete Instance node; every other pair shape goes through the
/// full `setops::join_types` kernel and its SetOpResult mapping.
fn join_one_pair(
    left: &Type,
    right: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<Type> {
    // Instance-Instance args-less nominal prejoin. Python's join_types
    // on such a pair never builds a fresh type: same-ref -> the left
    // operand, subtype -> the supertype, else the common ancestor.
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
            && resolver.get(l_ref).is_some()
            && resolver.get(r_ref).is_some()
        {
            if l_ref == r_ref {
                return Some(left.clone());
            }
            let result = crate::setops::visit_instance_join(left, right, ctx, resolver)?;
            return instance_join_result_to_type(&result, left, right);
        }
    }
    let joined = crate::setops::join_types(left, right, ctx, resolver);
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
            let final_args: Vec<Type> = arg_discs
                .iter()
                .enumerate()
                .map(|(i, &d)| match d {
                    0 => l_args[i].clone(),
                    1 => r_args[i].clone(),
                    _ => Type::AnyType {
                        type_of_any: 0,
                        source_any: None,
                        missing_import_name: None,
                    },
                })
                .collect();
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

/// Map a `visit_instance_join` result to the concrete joined `Type`
/// for the args-less no-LKV case. The join of two args-less Instances
/// never builds a fresh type in Python: `SameS`/`SameT` are the
/// operands, `Ancestor`/`Object` become `Instance(fullname)` /
/// `object`. Other results cannot arise here, so defer if seen.
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
        _ => None,
    }
}

/// Does `t` resolve to a class whose `fallback_to_any` is set (a stub or
/// unknown base)? Such classes are not decision-safe for the Rust fold
/// (see the whole-list guard in `join_type_list_inner`).
fn instance_has_fallback_to_any(t: &Type, resolver: &TypeResolver) -> bool {
    if let Type::Instance { type_ref, .. } = t {
        if let Some(snap) = resolver.get(type_ref) {
            return snap.fallback_to_any;
        }
    }
    false
}

/// Is this single item safe to pass through when the list has length
/// one? Only used by the one-item branch of `join_type_list_inner`,
/// where there is no join decision and the item becomes the answer
/// unchanged. Identity-safe: Instance, non-generic CallableType, and
/// the leaf short-circuits (AnyType / NoneType / UninhabitedType /
/// DeletedType). A generic CallableType is NOT safe: Python joins
/// callables via `combine_similar_callables` /
/// `join_similar_callables`, which can bind or erase type variables;
/// a bare passthrough would leak an unresolved TypeVar or an
/// over-eager Any into inference. A TupleType is safe only with no
/// UnpackType items. TypeVarLikes / TypeAliasType / unions defer.
fn is_join_safe(t: &Type, resolver: &TypeResolver) -> bool {
    if matches!(
        t,
        Type::AnyType { .. }
            | Type::NoneType
            | Type::UninhabitedType { .. }
            | Type::DeletedType { .. }
    ) {
        return true;
    }
    if let Type::CallableType { variables, .. } = t {
        return variables.is_empty();
    }
    if let Type::TupleType { items, .. } = t {
        return !items.iter().any(|i| matches!(i, Type::UnpackType { .. }));
    }
    if let Type::Instance { type_ref, .. } = t {
        return resolver.get(type_ref).is_some();
    }
    false
}

/// Does `t` (recursively) carry a `last_known_value` on any `Instance`?
/// Python's join erases LKVs (`Literal[2]?` joins `int` to `int`); the
/// wire kernel would keep the literal-typed form, so defer any list
/// containing one (the whole-list guard in `join_type_list_inner`).
fn has_lkv(t: &Type) -> bool {
    match t {
        Type::Instance {
            last_known_value,
            args,
            ..
        } => last_known_value.is_some() || args.iter().any(has_lkv),
        Type::TupleType { items, .. } => items.iter().any(has_lkv),
        Type::UnionType { items, .. } => items.iter().any(has_lkv),
        Type::CallableType {
            arg_types,
            ret_type,
            ..
        } => arg_types.iter().any(has_lkv) || has_lkv(ret_type),
        _ => false,
    }
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
// get_protocol_member (subtypes.py:1832)
// ---------------------------------------------------------------------------

/// `mypy.subtypes.get_protocol_member` (subtypes.py:1832-1871): look up
/// a member on a protocol instance.
///
/// Handles, in Python order:
///   1. `member == "__call__" and class_obj` -> defer (needs
///      `type_object_type(left.type)`, the full TypeInfo -> constructor
///      computation, which is complex).
///   2. `member == "__call__" and left.type.is_metaclass(precise=True)`
///      -> return `None` (avoid falling back to metaclass `__call__`).
///   3. Otherwise, `find_member` (subtypes.py:1874-1948) restricted to a
///      member defined directly on the receiver class (`left.type`) whose
///      node is a plain `FuncDef` / `OverloadedFuncDef` / `Var` and whose
///      resolution needs no checker state, plugins, or error emission:
///      * a plain method -> `analyze_instance_member_access` via the
///        `checkmember.rs` machinery (`member_method_inner`): `function_type`
///        passthrough gives the live signature, then check-self-arg +
///        map + expand + bind; the defining class is the receiver class so
///        `map_instance_to_supertype` is checked for identity first.
///      * a plain `Var` (not a property, classmethod, staticmethod,
///        classvar, inferred var, no PartialType) -> expand `var.type` by
///        the receiver preserving type-var ids, matching
///        `find_node_type`'s non-callable tail.
///   4. The find_member miss path (member absent from the MRO, or the
///      found symbol's `node` still None): the `__getattribute__` /
///      `__getattr__` accessor scan runs via `get_method_definer`; with no
///      non-object accessor, `fallback_to_any` -> `AnyType(special_form)`,
///      otherwise a plain miss -> `None` (`member_miss_decision`).
///
/// Defers (returns `None`) whenever anything needs plugins, descriptors,
/// class-attribute access, error emission, live checker state, or a
/// decision the live/wire data cannot carry exactly. A wrong answer is
/// worse than a deferral.
///
/// Returns `Some(Vec<u8>)` (encoded type) for a found member,
/// `Some(empty_vec)` for "Rust decided None", or `None` (defer). The Python
/// shim interprets `Some(empty)` as "return None" and `None` as "defer".
#[allow(clippy::too_many_arguments)]
pub(crate) fn get_protocol_member_inner(
    py: Python<'_>,
    left: &Type,
    member: &str,
    class_obj: bool,
    is_lvalue: bool,
    resolver: &TypeResolver,
) -> Option<GetProtocolMemberResult> {
    let Type::Instance {
        type_ref,
        args: left_args,
        extra_attrs,
        ..
    } = left
    else {
        return None;
    };
    if extra_attrs.is_some() {
        // `find_member` consults `itype.extra_attrs` for module attrs; a
        // present extra_attrs cannot be decided here.
        return Some(GetProtocolMemberResult::Defer);
    }
    let snap = match resolver.get(type_ref) {
        Some(s) => s,
        None => {
            return None;
        }
    };

    if member == "__call__" && class_obj {
        // type_object_type(left.type): the constructor type. This needs
        // the full TypeInfo -> type_object_type computation which is
        // complex; defer to Python.
        return Some(GetProtocolMemberResult::Defer);
    }

    if member == "__call__" && is_metaclass_precise(snap, resolver) {
        // Avoid falling back to metaclass __call__; return None.
        return Some(GetProtocolMemberResult::NoneVal);
    }
    if member == "__init__" {
        // analyze_instance_member_access (checkmember.py:616-621) filters
        // `__init__` access to final methods / super; anything else issues
        // CANNOT_ACCESS_INIT. Rust must not emit errors -> defer.
        return Some(GetProtocolMemberResult::Defer);
    }
    if class_obj {
        // analyze_vars / class-attribute path needs class-var substitution,
        // TypeType wrapping, and metaclass fallbacks. Defer.
        return Some(GetProtocolMemberResult::Defer);
    }
    if is_lvalue {
        // setter / assignment code paths need error emission.
        return Some(GetProtocolMemberResult::Defer);
    }

    // find_member (subtypes.py:1894-1915): `info.get(name)` walks the MRO.
    let info = match resolver.live_typeinfo(py, type_ref) {
        Some(i) => i,
        None => {
            return Some(GetProtocolMemberResult::Defer);
        }
    };
    // `live_typeinfo` returns a dict value which may be `None` when the
    // fullname key maps to nothing; treat that as a deferral.
    if info.is_none() {
        return Some(GetProtocolMemberResult::Defer);
    }
    let (sym_info, sym_node) = match mro_get(py, info, member) {
        Some(pair) => pair,
        None => {
            // find_member miss path: `info.get(name)` found nothing. The
            // decidable arms (accessor scan + fallback_to_any -> AnyType,
            // plain miss -> None) run in Rust; anything else defers.
            return member_miss_decision(py, info, member, snap);
        }
    };
    let sym_info: &PyAny = sym_info.as_ref(py);
    let node = match sym_node.as_ref(py).getattr("node") {
        Ok(n) => n.to_object(py),
        Err(_) => {
            return Some(GetProtocolMemberResult::Defer);
        }
    };
    let node_ref: &PyAny = node.as_ref(py);
    if node_ref.is_none() {
        // A present symbol with an unfilled `node` takes the same miss
        // path in Python (`if not node:` in find_member).
        return member_miss_decision(py, info, member, snap);
    }
    let class_name = node_ref.get_type().name().unwrap_or("").to_string();
    match class_name.as_str() {
        "FuncDef" | "OverloadedFuncDef" => {
            // A property is an OverloadedFuncDef with is_property=True; its
            // member access is analyze_var's property path. Defer so Python
            // returns the getter value, not the bound getter callable.
            if get_bool_flag(py, node_ref, "is_property") == Some(true) {
                return Some(GetProtocolMemberResult::Defer);
            }
            // Plain method: analyze_instance_member_access
            // (checkmember.py:634-776), signature via function_type
            // with preserve_type_var_ids=True; check_self_arg + bind tail.
            let node_type_obj = match node_ref.getattr("type") {
                Ok(t) => t,
                Err(_) => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            // Same-class guard: check_self_arg + bind tail (checkmember.py:769-772).
            if node_type_obj.is_none() {
                // `function_type` falls back to building the signature
                // from the FuncItem (Defn); defer so Python handles it.
                return Some(GetProtocolMemberResult::Defer);
            }
            let signature = match serialize_type_to_bytes(py, node_type_obj) {
                Some(bytes) => match decode_type(&bytes) {
                    Some(t) => t,
                    None => {
                        return Some(GetProtocolMemberResult::Defer);
                    }
                },
                None => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            // Member may be inherited (subclass receiver); Python maps the
            // receiver to `method.info` before expanding, so pass
            // allow_subclass_receiver (issue #1121).
            let method_fullname = match get_opt_str_attr(sym_info, "fullname") {
                Some(f) => f,
                None => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            let strict_optional = live_strict_optional(py);
            let result = crate::checkmember::member_method_inner(
                left,
                &signature,
                &method_fullname,
                left,
                member,
                resolver,
                strict_optional,
                false, // is_class
                true,  // allow_subclass_receiver
            );
            match result {
                Some(t) => Some(GetProtocolMemberResult::Found(t)),
                None => Some(GetProtocolMemberResult::Defer),
            }
        }
        "Decorator" => {
            // Decorators unwrap to `.var` (checkmember.py:1204-1211) and
            // the callable binds/maps/expands like a method; properties
            // bind too, static methods defer (no self bind, issue #1121).
            let var = match node_ref.getattr("var") {
                Ok(v) => v,
                Err(_) => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            // is_staticmethod: analyze_var binds only when
            // `not var.is_staticmethod`; member_method_inner's strip
            // would wrongly drop a static signature's first real arg.
            if get_bool_flag(py, var, "is_staticmethod") == Some(true) {
                return Some(GetProtocolMemberResult::Defer);
            }
            // is_ready: a not-ready var type defers (Python's
            // not_ready_callback).
            if get_bool_flag(py, var, "is_ready") != Some(true) {
                return Some(GetProtocolMemberResult::Defer);
            }
            // is_initialized_in_class: analyze_var computes call_type (and
            // binds) only when the var is initialized in class scope; a
            // False defers to the pure-Python no-bind path.
            if get_bool_flag(py, var, "is_initialized_in_class") != Some(true) {
                return Some(GetProtocolMemberResult::Defer);
            }
            // var.info.self_type: expand_self_type (checkmember.py:1737)
            // needs the Var; defer on a Self-typed member (same guard as
            // live_var_plain).
            let var_info = match var.getattr("info") {
                Ok(i) => i,
                Err(_) => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            let self_type_none = match var_info.getattr("self_type") {
                Ok(s) => s.is_none(),
                Err(_) => false,
            };
            if !self_type_none {
                return Some(GetProtocolMemberResult::Defer);
            }
            let var_type_obj = match var.getattr("type") {
                Ok(t) => t,
                Err(_) => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            if var_type_obj.is_none() {
                return Some(GetProtocolMemberResult::Defer);
            }
            let signature = match serialize_type_to_bytes(py, var_type_obj) {
                Some(bytes) => match decode_type(&bytes) {
                    Some(t) => t,
                    None => {
                        return Some(GetProtocolMemberResult::Defer);
                    }
                },
                None => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            let method_fullname = match get_opt_str_attr(var_info, "fullname") {
                Some(f) => f,
                None => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            // Attribute-hook resolution (checkmember.py:1821-1830 +
            // ChainedPlugin._find_hook): a hit would transform the
            // member result, so Rust defers; all-None is the answer.

            // `_native_plugin_hook_plugins` is None when not installed.
            let hook_fullname = format!("{method_fullname}.{member}");
            match plugin_get_attribute_hook_hits(py, &hook_fullname) {
                Some(true) => {
                    return Some(GetProtocolMemberResult::Defer);
                }
                Some(false) => {}
                None => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            }
            let is_classmethod = get_bool_flag(py, var, "is_classmethod") == Some(true);
            let is_property = get_bool_flag(py, var, "is_property") == Some(true);
            let strict_optional = live_strict_optional(py);
            match crate::checkmember::member_method_inner(
                left,
                &signature,
                &method_fullname,
                left,
                member,
                resolver,
                strict_optional,
                is_classmethod,
                true, // allow_subclass_receiver
            ) {
                Some(t) => {
                    // A property getter yields the getter return type,
                    // not the bound callable (checkmember.py:1966-1982);
                    // callable member types stay as-is.
                    let result = if is_property {
                        let item = match t {
                            Type::Overloaded { items } => match items.first() {
                                Some(i) => (*i).clone(),
                                None => {
                                    return Some(GetProtocolMemberResult::Defer);
                                }
                            },
                            t => t,
                        };
                        let Type::CallableType { ret_type, .. } = item else {
                            return Some(GetProtocolMemberResult::Defer);
                        };
                        (*ret_type).clone()
                    } else {
                        t
                    };
                    Some(GetProtocolMemberResult::Found(result))
                }
                None => Some(GetProtocolMemberResult::Defer),
            }
        }
        "Var" => {
            // find_node_type (subtypes.py:2117-2160) Var path ->
            // analyze_var's non-callable tail (checkmember.py:1377-1422).
            if !live_var_plain(py, node_ref, sym_info, type_ref, resolver) {
                return Some(GetProtocolMemberResult::Defer);
            }
            let var_type_obj = match node_ref.getattr("type") {
                Ok(t) => t,
                Err(_) => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            if var_type_obj.is_none() {
                return Some(GetProtocolMemberResult::Defer);
            }
            let typ = match serialize_type_to_bytes(py, var_type_obj) {
                Some(bytes) => match decode_type(&bytes) {
                    Some(t) => t,
                    None => {
                        return Some(GetProtocolMemberResult::Defer);
                    }
                },
                None => {
                    return Some(GetProtocolMemberResult::Defer);
                }
            };
            // expand_without_binding with preserve_type_var_ids=True
            // (checkmember.py:1498-1503): no freshen / Self; wire fast-path
            // + identity map == plain expand_type_by_instance.
            let mapped = Type::Instance {
                type_ref: type_ref.to_string(),
                args: left_args.to_vec(),
                last_known_value: None,
                extra_attrs: None,
            };
            let expanded = crate::expandtype::expand_type_by_instance_core(
                &typ,
                &mapped,
                resolver,
                live_strict_optional(py),
            );
            match expanded {
                Some(t) => {
                    if crate::expandtype::result_has_typevar(&t) {
                        return Some(GetProtocolMemberResult::Defer);
                    }
                    Some(GetProtocolMemberResult::Found(t))
                }
                None => Some(GetProtocolMemberResult::Defer),
            }
        }
        _ => {
            // Decorator, MypyFile, TypeInfo, TypeAlias, TypeVarLikeExpr,
            // PlaceholderNode: defer.

            Some(GetProtocolMemberResult::Defer)
        }
    }
}

/// The find_member miss path (subtypes.py:2072-2089) for a protocol
/// member absent from the receiver's MRO: the `__getattribute__` /
/// `__getattr__` accessor scan, then the `fallback_to_any` AnyType arm,
/// then the plain miss.
///
/// Python runs this when `info.get(name)` finds nothing or the found
/// symbol's `node` is None. The `extra_attrs` attr-hit arm is unreachable
/// here (the whole Instance-left path defers when `extra_attrs` is
/// present) and the `meta_fallback_to_any` arm needs `class_obj` (also
/// deferred upstream), so only the two decidable arms remain.
///
/// Returns `None` (defer) when any MRO accessor lookup is unreadable —
/// a wrong "no accessor" answer would skip the getattr member type.
fn member_miss_decision(
    py: Python<'_>,
    info: &PyAny,
    member: &str,
    snap: &crate::typeinfo::TypeInfoSnapshot,
) -> Option<GetProtocolMemberResult> {
    if !matches!(member, "__getattr__" | "__setattr__" | "__getattribute__") {
        for method_name in ["__getattribute__", "__getattr__"] {
            if let Some(definer) = get_method_definer(py, info, method_name)? {
                if definer != "builtins.object" {
                    // An accessor is defined on a non-object class: the
                    // member type comes from find_node_type on the
                    // accessor (its ret_type) -> defer.
                    return Some(GetProtocolMemberResult::Defer);
                }
            }
        }
    }
    if snap.fallback_to_any {
        return Some(GetProtocolMemberResult::Found(Type::AnyType {
            type_of_any: 6, // TypeOfAny.special_form
            source_any: None,
            missing_import_name: None,
        }));
    }
    Some(GetProtocolMemberResult::NoneVal)
}

/// Mirror `TypeInfo.get_method` (nodes.py:4167) returning the defining
/// class's fullname: `Some(Some(fullname))` when a FuncBase / Decorator
/// method is found on class `fullname`, `Some(None)` when the walk found
/// a non-function node (get_method stops and returns None) or nothing,
/// `None` on any read failure (defer).
fn get_method_definer(_py: Python<'_>, info: &PyAny, name: &str) -> Option<Option<String>> {
    let mro = info.getattr("mro").ok()?;
    let mro_list = mro.downcast::<PyList>().ok()?;
    for cls in mro_list.iter() {
        let names = cls.getattr("names").ok()?;
        let sym = match names.get_item(name) {
            Ok(s) if !s.is_none() => s,
            _ => {
                // No exact entry: check the `{name}-redefinition` entries,
                // taking the last in sorted order (nodes.py:4170-4174).
                let keys = names.call_method0("keys").ok()?;
                let mut redef: Vec<String> = Vec::new();
                for key in keys.iter().ok()? {
                    let key: String = key.ok()?.extract().ok()?;
                    if key.starts_with(&format!("{name}-redefinition")) {
                        redef.push(key);
                    }
                }
                redef.sort();
                match redef.pop() {
                    Some(k) => match names.get_item(k) {
                        Ok(s) if !s.is_none() => s,
                        _ => continue,
                    },
                    None => continue,
                }
            }
        };
        let node = match sym.getattr("node") {
            Ok(n) => n,
            Err(_) => return None,
        };
        let class_name = match node.get_type().name() {
            Ok(n) => n.to_string(),
            Err(_) => return None,
        };
        if class_name == "FuncDef" || class_name == "OverloadedFuncDef" || class_name == "Decorator"
        {
            let fullname = get_opt_str_attr(cls, "fullname")?;
            return Some(Some(fullname));
        }
        // Found-but-non-function node stops the walk with None.
        return Some(None);
    }
    Some(None)
}

/// The `live_strict_optional` read from `mypy.state` (checkmember.py uses
/// `state.state.strict_optional`); defaults to `true` (the production
/// default) on any read failure. Conservative: a wrong strict_optional
/// could change an expand decision.
fn live_strict_optional(py: Python<'_>) -> bool {
    py.import("mypy.state")
        .and_then(|m| m.getattr("state"))
        .and_then(|s| s.getattr("strict_optional"))
        .and_then(|v| v.extract::<bool>())
        .unwrap_or(true)
}

/// The Var gate of the protocol-member var path: `true` iff the live Var
/// can be answered by plain expand_type_by_instance.
///
/// Mirrors find_node_type's Var tail (subtypes.py:2117-2124) +
/// analyze_var's non-callable decision (checkmember.py:1377-1422) + the
/// descriptor / plugin hooks that Rust cannot run (defer on those instead
/// of guessing). All must hold else the whole member lookup defers.
#[allow(clippy::too_many_arguments)]
fn live_var_plain(
    py: Python<'_>,
    var: &PyAny,
    sym_info: &PyAny,
    type_ref: &str,
    resolver: &TypeResolver,
) -> bool {
    // var type ready + not a property / classmethod / staticmethod /
    // classvar / setter. `get_bool_flag` returns None when the attribute
    // is missing; only proceed when every decision is readable.
    let is_ready = match get_bool_flag(py, var, "is_ready") {
        Some(b) => b,
        None => return false,
    };
    if !is_ready {
        return false;
    }
    let is_property = get_bool_flag(py, var, "is_property");
    let is_classmethod = get_bool_flag(py, var, "is_classmethod");
    let is_staticmethod = get_bool_flag(py, var, "is_staticmethod");
    let is_classvar = get_bool_flag(py, var, "is_classvar");
    let is_settable_property = get_bool_flag(py, var, "is_settable_property");
    let is_property = match is_property {
        Some(b) => b,
        None => return false,
    };
    let is_classmethod = match is_classmethod {
        Some(b) => b,
        None => return false,
    };
    let is_staticmethod = match is_staticmethod {
        Some(b) => b,
        None => return false,
    };
    let is_classvar = match is_classvar {
        Some(b) => b,
        None => return false,
    };
    let is_settable_property = match is_settable_property {
        Some(b) => b,
        None => return false,
    };
    if is_property || is_classmethod || is_staticmethod || is_classvar || is_settable_property {
        return false;
    }
    // is_instance_var (checkmember.py:1346-1355): name in var.info.names,
    // that node is the var, not classvar, not inferred.
    let is_inferred = match get_bool_flag(py, var, "is_inferred") {
        Some(b) => b,
        None => return false,
    };
    if !is_inferred {
        return false;
    }
    // var.info.self_type must be None (expand_self_type would need the Var).
    let info = match var.getattr("info").ok() {
        Some(i) => i,
        None => return false,
    };
    let self_type = match info.getattr("self_type") {
        Ok(t) => t,
        Err(_) => return false,
    };
    if !self_type.is_none() {
        return false;
    }
    // Attribute hook absence: a hook would need the AttributeContext +
    // live checker in Python.
    if !plugin_get_attribute_hook_absent(py) {
        return false;
    }
    if live_plugin_registry_absent(py) {
        // user plugins present -> the Python chain is the source of truth.
        return false;
    }
    // Descriptor gate (checkmember.py:1451-1452): descriptor access runs
    // when result is non-None and not (implicit or protocol-instance-var).
    // Read the `implicit` flag of the defining symbol.
    let name = match var.getattr("name").and_then(|n| n.extract::<String>()) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let implicit = match sym_info
        .getattr("names")
        .and_then(|names| names.get_item(&name))
        .and_then(|sym| sym.getattr("implicit"))
        .and_then(|v| v.extract::<bool>())
    {
        Ok(b) => b,
        Err(_) => return false,
    };
    // `var.info.is_protocol and is_instance_var(var)` skips the descriptor
    // gate. The defining class is `sym_info`; its is_protocol flag decides.
    let proto_skip = match get_bool_flag(py, sym_info, "is_protocol") {
        Some(b) => {
            if b {
                // is_instance_var(var) already guaranteed `not is_inferred`.
                !is_inferred
            } else {
                false
            }
        }
        None => return false,
    };
    if !implicit && !proto_skip {
        // The var type is expanded below; detect descriptor-ness on the
        // *pre-expansion* var.type via has_readable_member_by_ref, deferring
        // whenever the snapshot cannot decide.
        let t = match var.getattr("type") {
            Ok(vt) => match serialize_type_to_bytes(py, vt) {
                Some(b) => match decode_type(&b) {
                    Some(t) => t,
                    None => return false,
                },
                None => return false,
            },
            Err(_) => return false,
        };
        if is_descriptor_wire(&t, resolver) == Some(true) {
            return false;
        }
    }
    let _ = type_ref;
    true
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

/// Distinguish "Rust decided None" from "Rust defers" from "Rust found".
#[derive(Debug, PartialEq)]
pub(crate) enum GetProtocolMemberResult {
    /// The member is None (e.g. __call__ on a precise metaclass).
    NoneVal,
    /// A member type was found and fully computed in Rust.
    Found(Type),
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
    py: Python<'_>,
    left_bytes: &[u8],
    original_left_bytes: &[u8],
    member: &str,
    class_obj: bool,
    is_lvalue: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let _original = decode_type(original_left_bytes)?;
    let left = decode_type(left_bytes)?;
    match get_protocol_member_inner(py, &left, member, class_obj, is_lvalue, resolver.resolver())? {
        GetProtocolMemberResult::NoneVal => Some(Vec::new()),
        GetProtocolMemberResult::Found(t) => encode_type(&t),
        GetProtocolMemberResult::Defer => None,
    }
}

// ---------------------------------------------------------------------------
// find_member("__call__") skip-trigger decision (checkexpr.py:3594-3604)
// ---------------------------------------------------------------------------

/// Decision core of the ParamSpec arm of `get_arg_infer_passes`
/// (checkexpr.py:3594-3604): for an Instance actual, mirror
/// `find_member("__call__", ity, ity, is_operator=True)` (subtypes.py:1960)
/// restricted to the decidable subset, and return whether the resulting
/// member type is a plain non-generic `CallableType` (the skip trigger).
///
/// Decidable subset: the member is missing (find_member returns None, or
/// AnyType via fallback_to_any; both leave the actual non-callable), or it
/// is a plain non-property `FuncDef` / `OverloadedFuncDef` signature read
/// from the live node and bound + expanded via `member_method_inner`.
/// Defers (`None`) on extra_attrs, property / Decorator / Var members,
/// inherited methods (the same-class guard), and any unreadable fact.
pub(crate) fn find_member_call_is_plain_callable(
    py: Python<'_>,
    instance: &Type,
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    let Type::Instance {
        type_ref,
        extra_attrs,
        ..
    } = instance
    else {
        return None;
    };
    if extra_attrs.is_some() {
        // find_member consults itype.extra_attrs; defer instead of guessing.
        return None;
    }
    let info = resolver.live_typeinfo(py, type_ref)?;
    if info.is_none() {
        return None;
    }
    let (sym_info, sym_node) = match mro_get(py, info, "__call__") {
        Some(pair) => pair,
        // Member absent: is_operator=True skips the __getattribute__ /
        // __getattr__ loop, so find_member returns None (or AnyType on
        // fallback_to_any); either way the actual is not a CallableType.
        None => return Some(false),
    };
    let sym_info_ref = sym_info.as_ref(py);
    let node = sym_node.as_ref(py).getattr("node").ok()?;
    if node.is_none() {
        // Unfilled cross-ref behaves like a missing member in find_member.
        return Some(false);
    }
    let class_name = node.get_type().name().unwrap_or("").to_string();
    match class_name.as_str() {
        "FuncDef" | "OverloadedFuncDef" => {
            // A property is an OverloadedFuncDef with is_property=True;
            // its member type is the getter value -> defer.
            if get_bool_flag(py, node, "is_property") == Some(true) {
                return None;
            }
            let sig_obj = node.getattr("type").ok()?;
            if sig_obj.is_none() {
                // function_type would fall back to the FuncItem body.
                return None;
            }
            let sig_bytes = serialize_type_to_bytes(py, sig_obj)?;
            let mut buf = ReadBuffer::new(&sig_bytes);
            let signature = wire::read_type(&mut buf, None).ok()?;
            let method_fullname = get_opt_str_attr(sym_info_ref, "fullname")?;
            // is_operator=True, is_lvalue=False, class_obj=False; errors
            // are suppressed on the Python side, which member_method_inner
            // never emits anyway.
            let result = crate::checkmember::member_method_inner(
                instance,
                &signature,
                &method_fullname,
                instance,
                "__call__",
                resolver.resolver(),
                live_strict_optional(py),
                false, // is_class
                false, // allow_subclass_receiver
            )?;
            Some(matches!(
                result,
                Type::CallableType { variables, .. } if variables.is_empty()
            ))
        }
        _ => {
            // Decorator, Var, TypeInfo, MypyFile, ... need analyze_var /
            // find_node_type paths Rust does not run here -> defer.
            None
        }
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

    /// Wrap a bare `TypeResolver` in the native wrapper for seam calls
    /// (tests only; no live map is installed, so only the snapshot-decided
    /// paths engage).
    fn make_native(r: TypeResolver) -> NativeTypeResolver {
        NativeTypeResolver::from_resolver(r)
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
            // Real snapshots always include the class itself in the MRO
            // (TypeInfo.get walks it, nodes.py:4063-4068).
            mro: vec![type_ref.to_string()],
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
            mro: vec![type_ref.to_string()],
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
            mro: vec![metaclass_fullname.to_string()],
            ..Default::default()
        };
        r.insert(metaclass_fullname.to_string(), meta_snap);
        r
    }

    fn make_callable(fallback_ref: &str) -> Type {
        Type::CallableType {
            fallback: Box::new(make_instance(fallback_ref, vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: true,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
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
        // __eq__ not in member_definers, MRO exhausted -> Some(false),
        // matching Python's `return False` tail (typeops.py:1956).
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(false)
        );
    }

    #[test]
    fn test_custom_special_method_inherited_from_object() {
        // Class without a custom __eq__: the MRO walk finds it on
        // builtins.object and reports it as not-custom.
        let mut r = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: "mymod.Foo".to_string(),
            name: "Foo".to_string(),
            mro: vec!["mymod.Foo".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        r.insert("mymod.Foo".to_string(), snap);
        let mut obj_snap = TypeInfoSnapshot {
            fullname: "builtins.object".to_string(),
            name: "object".to_string(),
            mro: vec!["builtins.object".to_string()],
            ..Default::default()
        };
        obj_snap
            .member_definers
            .insert("__eq__".to_string(), (0, "builtins.object".to_string()));
        r.insert("builtins.object".to_string(), obj_snap);
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(false)
        );
    }

    #[test]
    fn test_custom_special_method_inherited_custom() {
        // Class inherits a custom __eq__ from an ancestor: the MRO walk
        // finds the ancestor's defining class and reports it as custom.
        let mut r = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: "mymod.Child".to_string(),
            name: "Child".to_string(),
            mro: vec![
                "mymod.Child".to_string(),
                "mymod.Base".to_string(),
                "builtins.object".to_string(),
            ],
            ..Default::default()
        };
        r.insert("mymod.Child".to_string(), snap);
        let base_snap = TypeInfoSnapshot {
            fullname: "mymod.Base".to_string(),
            name: "Base".to_string(),
            mro: vec!["mymod.Base".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        r.insert("mymod.Base".to_string(), base_snap);
        let mut obj_snap = TypeInfoSnapshot {
            fullname: "builtins.object".to_string(),
            name: "object".to_string(),
            mro: vec!["builtins.object".to_string()],
            ..Default::default()
        };
        obj_snap
            .member_definers
            .insert("__eq__".to_string(), (0, "builtins.object".to_string()));
        r.insert("builtins.object".to_string(), obj_snap);
        let mut base = TypeInfoSnapshot {
            fullname: "mymod.Base".to_string(),
            name: "Base".to_string(),
            mro: vec!["mymod.Base".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        base.member_definers
            .insert("__eq__".to_string(), (0, "mymod.Base".to_string()));
        r.insert("mymod.Base".to_string(), base);
        let t = make_instance("mymod.Child", vec![]);
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(true)
        );
    }

    #[test]
    fn test_custom_special_method_missing_ancestor_snapshot_defers() {
        // MRO names an ancestor with no snapshot -> None (defer to
        // Python), never a guessed bool.
        let mut r = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: "mymod.Foo".to_string(),
            name: "Foo".to_string(),
            mro: vec!["mymod.Foo".to_string(), "mymod.Missing".to_string()],
            ..Default::default()
        };
        r.insert("mymod.Foo".to_string(), snap);
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(custom_special_method_inner(&t, "__eq__", false, &r), None);
    }

    #[test]
    fn test_custom_special_method_non_type_obj_overloaded_false() {
        // Overloaded whose first item is not a type obj (plain function
        // fallback, not a metaclass): FunctionLike.is_type_obj() False,
        // so Python falls through to the `return False` tail.
        let r = TypeResolver::new();
        let callable = make_callable("builtins.function");
        let overloaded = Type::Overloaded {
            items: vec![callable],
        };
        assert_eq!(
            custom_special_method_inner(&overloaded, "__eq__", false, &r),
            Some(false)
        );
    }

    #[test]
    fn test_custom_special_method_type_obj_overloaded_via_metaclass() {
        // Overloaded whose first item IS a type obj (fallback is
        // builtins.type): recurse on the fallback Instance MRO.
        let r = make_resolver_with_definer("builtins.type", "__eq__", 0, "builtins.type");
        let callable = make_callable("builtins.type");
        let overloaded = Type::Overloaded {
            items: vec![callable],
        };
        assert_eq!(
            custom_special_method_inner(&overloaded, "__eq__", false, &r),
            Some(false)
        );
        // Same, with a custom __eq__ on builtins.type.
        let r2 = make_resolver_with_definer("builtins.type", "__eq__", 0, "mymod.CustomDef");
        assert_eq!(
            custom_special_method_inner(&overloaded, "__eq__", false, &r2),
            Some(true)
        );
    }

    #[test]
    fn test_custom_special_method_callable_type_obj_negative() {
        // A bare CallableType (not Overloaded) whose fallback is a
        // metaclass recurses the same way.
        let r = make_resolver_with_definer("builtins.type", "__eq__", 0, "builtins.type");
        let callable = make_callable("builtins.type");
        assert_eq!(
            custom_special_method_inner(&callable, "__eq__", false, &r),
            Some(false)
        );
    }

    #[test]
    fn test_custom_special_method_typetype_metaclass() {
        // TypeType(Instance(C)): look __eq__ up via C's metaclass MRO.
        let mut r = TypeResolver::new();
        let c_snap = TypeInfoSnapshot {
            fullname: "mymod.C".to_string(),
            name: "C".to_string(),
            metaclass_fullname: Some("mymod.Meta".to_string()),
            mro: vec!["mymod.C".to_string()],
            ..Default::default()
        };
        r.insert("mymod.C".to_string(), c_snap);
        let mut meta_snap = TypeInfoSnapshot {
            fullname: "mymod.Meta".to_string(),
            name: "Meta".to_string(),
            mro: vec!["mymod.Meta".to_string()],
            ..Default::default()
        };
        meta_snap
            .member_definers
            .insert("__eq__".to_string(), (0, "mymod.Meta".to_string()));
        r.insert("mymod.Meta".to_string(), meta_snap);
        let t = Type::TypeType {
            item: Box::new(make_instance("mymod.C", vec![])),
            is_type_form: false,
        };
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(true)
        );
    }

    #[test]
    fn test_custom_special_method_typetype_item_not_instance() {
        // TypeType(Any): Python only enters the metaclass branch when
        // item is an Instance, so it falls through to False.
        let r = TypeResolver::new();
        let t = Type::TypeType {
            item: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            is_type_form: false,
        };
        assert_eq!(
            custom_special_method_inner(&t, "__eq__", false, &r),
            Some(false)
        );
    }

    #[test]
    fn test_custom_special_method_missing_snapshot() {
        let r = TypeResolver::new();
        let t = make_instance("mymod.NotFound", vec![]);
        assert_eq!(custom_special_method_inner(&t, "__eq__", false, &r), None);
    }

    #[test]
    fn test_custom_special_method_type_alias_defers() {
        let r = TypeResolver::new();
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "mymod.Alias".to_string(),
        };
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
            mro: vec!["mymod.Foo".to_string()],
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
            mro: vec!["builtins.int".to_string()],
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

    #[test]
    fn test_has_custom_eq_checks_mro_defers_but_ne_custom() {
        // __eq__ defers (missing snapshot) but Python's `or` still
        // evaluates __ne__, which is custom -> Some(true), not None.
        let mut r = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "mymod.Foo".to_string(),
            name: "Foo".to_string(),
            mro: vec!["mymod.Foo".to_string(), "mymod.Unknown".to_string()],
            ..Default::default()
        };
        snap.member_definers
            .insert("__ne__".to_string(), (0, "mymod.Foo".to_string()));
        r.insert("mymod.Foo".to_string(), snap);
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(has_custom_eq_checks_inner(&t, &r), Some(true));
    }

    #[test]
    fn test_has_custom_eq_checks_ne_builtins_when_eq_defers() {
        // __eq__ defers, __ne__ resolves to builtins -> Some(false).
        let mut r = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "mymod.Foo".to_string(),
            name: "Foo".to_string(),
            mro: vec!["mymod.Foo".to_string(), "builtins.object".to_string()],
            ..Default::default()
        };
        snap.member_definers
            .insert("__ne__".to_string(), (0, "builtins.object".to_string()));
        r.insert("mymod.Foo".to_string(), snap);
        let mut obj_snap = TypeInfoSnapshot {
            fullname: "builtins.object".to_string(),
            name: "object".to_string(),
            mro: vec!["builtins.object".to_string()],
            ..Default::default()
        };
        obj_snap
            .member_definers
            .insert("__eq__".to_string(), (0, "builtins.object".to_string()));
        obj_snap
            .member_definers
            .insert("__ne__".to_string(), (0, "builtins.object".to_string()));
        r.insert("builtins.object".to_string(), obj_snap);
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(has_custom_eq_checks_inner(&t, &r), Some(false));
    }

    #[test]
    fn test_has_custom_eq_checks_mro_defers_both() {
        // __eq__ defers and __ne__ defers -> None, falling back to Python.
        let mut r = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: "mymod.Foo".to_string(),
            name: "Foo".to_string(),
            mro: vec!["mymod.Foo".to_string(), "mymod.Unknown".to_string()],
            ..Default::default()
        };
        r.insert("mymod.Foo".to_string(), snap);
        let t = make_instance("mymod.Foo", vec![]);
        assert_eq!(has_custom_eq_checks_inner(&t, &r), None);
    }

    // -- get_protocol_member --

    // The inner decides `__call__` on a precise metaclass (None) and
    // `__call__` on a class object (defer) before touching live state, so
    // those two paths are testable without a live map.

    #[test]
    fn test_get_protocol_member_call_on_metaclass_precise() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let r = make_native(make_resolver_with_metaclass(
                "builtins.type",
                "builtins.type",
            ));
            let left = make_instance("builtins.type", vec![]);
            let res = get_protocol_member_inner(py, &left, "__call__", false, false, r.resolver());
            assert!(matches!(res, Some(GetProtocolMemberResult::NoneVal)));
        });
    }

    #[test]
    fn test_get_protocol_member_call_on_non_metaclass_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let r = make_native(make_resolver_with_metaclass("mymod.Foo", "builtins.type"));
            let left = make_instance("mymod.Foo", vec![]);
            let res = get_protocol_member_inner(py, &left, "__call__", false, false, r.resolver());
            assert!(matches!(res, Some(GetProtocolMemberResult::Defer)));
        });
    }

    #[test]
    fn test_get_protocol_member_class_obj_call_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let r = make_native(make_resolver_with_metaclass("mymod.Foo", "builtins.type"));
            let left = make_instance("mymod.Foo", vec![]);
            let res = get_protocol_member_inner(py, &left, "__call__", true, false, r.resolver());
            assert!(matches!(res, Some(GetProtocolMemberResult::Defer)));
        });
    }

    #[test]
    fn test_get_protocol_member_missing_snapshot_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let r = make_native(TypeResolver::new());
            let left = make_instance("mymod.NotFound", vec![]);
            let res = get_protocol_member_inner(py, &left, "__call__", false, false, r.resolver());
            assert!(res.is_none());
        });
    }

    #[test]
    fn test_get_protocol_member_non_instance_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let r = make_native(TypeResolver::new());
            let left = Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            };
            let res = get_protocol_member_inner(py, &left, "__call__", false, false, r.resolver());
            assert!(res.is_none());
        });
    }

    #[test]
    fn test_get_protocol_member_init_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let r = make_native(make_resolver_with_metaclass("mymod.Foo", "builtins.type"));
            let left = make_instance("mymod.Foo", vec![]);
            // __init__ access filters to final/super; Rust defers so Python
            // decides CANNOT_ACCESS_INIT.
            let res = get_protocol_member_inner(py, &left, "__init__", false, false, r.resolver());
            assert!(matches!(res, Some(GetProtocolMemberResult::Defer)));
        });
    }

    #[test]
    fn test_get_protocol_member_is_lvalue_defers() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let r = make_native(make_resolver_with_metaclass("mymod.Foo", "builtins.type"));
            let left = make_instance("mymod.Foo", vec![]);
            // lvalue needs setter / assignment paths -> defer.
            let res = get_protocol_member_inner(py, &left, "foo", false, true, r.resolver());
            assert!(matches!(res, Some(GetProtocolMemberResult::Defer)));
        });
    }

    #[test]
    fn test_is_descriptor_wire_missing_snapshot_defers() {
        let r = TypeResolver::new();
        let inst = make_instance("mymod.NoSnapshot", vec![]);
        // has_readable_member_by_ref defers on a missing snapshot.
        assert!(is_descriptor_wire(&inst, &r).is_none());
    }

    #[test]
    fn test_is_descriptor_wire_union_any() {
        let r = TypeResolver::new();
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let union = Type::UnionType {
            items: vec![any.clone()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        // Non-Instance, non-Union item -> Some(false).
        assert_eq!(is_descriptor_wire(&any, &r), Some(false));
        // Union whose items are all non-descriptors -> Some(false).
        assert_eq!(is_descriptor_wire(&union, &r), Some(false));
    }

    // -- join_type_list --

    #[test]
    fn test_join_type_list_empty_returns_uninhabited() {
        let r = TypeResolver::new();
        assert_eq!(
            join_type_list_inner(&[], true, &r),
            Some(Type::UninhabitedType { ambiguous: false })
        );
    }

    #[test]
    fn test_join_type_list_single_returns_item() {
        // Single-item passthrough: no join decision (Python's fold starts
        // `joined = types[0]`). Resolver-resolvable Instance passes
        // through; identity-sensitive classes (TypeVar) still defer.
        let mut r = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: "builtins.int".to_string(),
            name: "int".to_string(),
            ..Default::default()
        };
        r.insert("builtins.int".to_string(), snap);
        let inst = make_instance("builtins.int", vec![]);
        assert_eq!(
            join_type_list_inner(std::slice::from_ref(&inst), true, &r),
            Some(inst)
        );
        let none_t = Type::NoneType;
        assert_eq!(
            join_type_list_inner(std::slice::from_ref(&none_t), true, &r),
            Some(none_t)
        );
    }

    #[test]
    fn test_join_type_list_single_typevar_defers() {
        // TypeVar single item: wire round-trip breaks identity that join
        // inference depends on; Python must run.
        let r = TypeResolver::new();
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "ns".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(join_type_list_inner(&[tvar], true, &r), None);
    }

    #[test]
    fn test_join_type_list_same_items_keeps_item() {
        let mut r = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: "builtins.int".to_string(),
            name: "int".to_string(),
            ..Default::default()
        };
        r.insert("builtins.int".to_string(), snap);
        let inst = make_instance("builtins.int", vec![]);
        let ans = join_type_list_inner(&[inst.clone(), inst.clone()], true, &r);
        assert_eq!(ans, Some(inst));
    }

    #[test]
    fn test_join_type_list_subtype_dominated_returns_dominant() {
        // int <: object (both snapshots with mro + has_base, and object
        // with no type vars): join is object. Mirrors the subtypes.rs
        // fixture setup (snap + instance).
        let mut r = TypeResolver::new();
        for (fullname, name) in [("builtins.int", "int"), ("builtins.object", "object")] {
            let mut s = TypeInfoSnapshot {
                fullname: fullname.to_string(),
                name: name.to_string(),
                ..Default::default()
            };
            s.mro.push(fullname.to_string());
            s.has_base.insert(fullname.to_string());
            r.insert(fullname.to_string(), s);
        }
        let int_t = make_instance("builtins.int", vec![]);
        let obj_t = make_instance("builtins.object", vec![]);
        let ans = join_type_list_inner(&[int_t, obj_t.clone()], true, &r);
        assert_eq!(ans, Some(obj_t));
    }

    #[test]
    fn test_join_type_list_incomparable_defers() {
        // int/str are incomparable; no decisive fold answer, so defer.
        let mut r = TypeResolver::new();
        let mut int_t = TypeInfoSnapshot {
            fullname: "builtins.int".to_string(),
            name: "int".to_string(),
            ..Default::default()
        };
        int_t.mro.push("builtins.int".to_string());
        int_t.mro.push("builtins.object".to_string());
        r.insert("builtins.int".to_string(), int_t);
        let a = make_instance("builtins.int", vec![]);
        let b = make_instance("builtins.str", vec![]);
        assert_eq!(join_type_list_inner(&[a, b], true, &r), None);
    }

    #[test]
    fn test_join_type_list_union_flattens() {
        // Union item: join(int, union[int, None]) -> union (int is a
        // proper subtype of the union, so the union wins, join.py:432).
        let mut r = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "builtins.int".to_string(),
            name: "int".to_string(),
            ..Default::default()
        };
        snap.mro.push("builtins.int".to_string());
        snap.mro.push("builtins.object".to_string());
        snap.has_base.insert("builtins.int".to_string());
        snap.has_base.insert("builtins.object".to_string());
        r.insert("builtins.int".to_string(), snap);
        let union = Type::UnionType {
            items: vec![make_instance("builtins.int", vec![]), Type::NoneType],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let a = make_instance("builtins.int", vec![]);
        assert_eq!(
            join_type_list_inner(&[a, union.clone()], true, &r),
            Some(union)
        );
    }

    #[test]
    fn test_join_type_list_typevar_joins_to_object() {
        // TypeVarType item with an Instance accumulator: Python's
        // visit_type_var falls to default(s) = object (join.py:546).
        let r = TypeResolver::new();
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "ns".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            default: Box::new(Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        let a = make_instance("builtins.int", vec![]);
        assert_eq!(
            join_type_list_inner(&[a, tvar], true, &r),
            Some(make_instance("builtins.object", vec![]))
        );
    }

    #[test]
    fn test_join_type_list_incomparable_joins_to_object() {
        // int and str share only object: the nominal join returns the
        // common ancestor (join_instances_via_supertype -> object).
        let mut r = TypeResolver::new();
        // Each class's `bases` is one encoded `Instance(object)` blob,
        // mirroring how typeinfo.rs serializes TypeInfo.bases.
        let mut wbuf = WriteBuffer::new();
        wire::write_type(&mut wbuf, &make_instance("builtins.object", vec![])).unwrap();
        let object_blob = wbuf.into_bytes();
        for fullname in ["builtins.int", "builtins.str", "builtins.object"] {
            let name = fullname.rsplit('.').next().unwrap();
            let mut s = TypeInfoSnapshot {
                fullname: fullname.to_string(),
                name: name.to_string(),
                ..Default::default()
            };
            s.mro.push(fullname.to_string());
            s.mro.push("builtins.object".to_string());
            s.has_base.insert(fullname.to_string());
            s.has_base.insert("builtins.object".to_string());
            if fullname != "builtins.object" {
                s.bases = vec![object_blob.clone()];
            }
            r.insert(fullname.to_string(), s);
        }
        let int_t = make_instance("builtins.int", vec![]);
        let str_t = make_instance("builtins.str", vec![]);
        assert_eq!(
            join_type_list_inner(&[int_t, str_t], true, &r),
            Some(make_instance("builtins.object", vec![]))
        );
    }

    #[test]
    fn test_join_type_list_identical_callables() {
        // Wire-identical CallableType pair: join_types returns the left
        // operand (visit_join SameS short-circuit).
        let r = TypeResolver::new();
        let c = make_callable("builtins.function");
        assert_eq!(
            join_type_list_inner(&[c.clone(), c.clone()], true, &r),
            Some(c)
        );
    }

    #[test]
    fn test_join_type_list_missing_snapshot_defers() {
        // Instance whose TypeInfo snapshot is absent from the resolver:
        // is_subtype cannot decide, so the whole call defers.
        let r = TypeResolver::new();
        let a = make_instance("builtins.int", vec![]);
        let b = make_instance("mymod.NotFound", vec![]);
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

    // -- restrict_subtype_away: consider_runtime_isinstance=False, non-generic right --

    fn insert_plain_class(r: &mut TypeResolver, fullname: &str) {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.rsplit('.').next().unwrap_or(fullname).to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        r.insert(fullname.to_string(), s);
    }

    #[test]
    fn test_restrict_subtype_away_consider_false_non_generic_right_keeps_t() {
        // int restricted by str (non-generic, non-protocol): check 1 fails and
        // check 2 (erase_instances) is identical, so Python returns t; the
        // previous code deferred the whole call on the erase gap.
        let mut r = TypeResolver::new();
        insert_plain_class(&mut r, "builtins.int");
        insert_plain_class(&mut r, "builtins.str");
        let t = make_instance("builtins.int", vec![]);
        let s = make_instance("builtins.str", vec![]);
        let result = restrict_subtype_away_inner(&t, &s, false, true, &r);
        assert_eq!(result, Some(t));
    }

    #[test]
    fn test_restrict_subtype_away_consider_false_subtype_still_uninhabited() {
        // int proper-subtype of object: check 1 Some(true) -> UninhabitedType.
        let mut r = TypeResolver::new();
        insert_plain_class(&mut r, "builtins.int");
        insert_plain_class(&mut r, "builtins.object");
        let r_snap = r.get("builtins.object").cloned().unwrap();
        // builtins.object must be a base of int for the nominal check.
        let mut int_snap = r.get("builtins.int").cloned().unwrap();
        int_snap.has_base.insert("builtins.object".to_string());
        int_snap.mro.push("builtins.object".to_string());
        r.insert("builtins.int".to_string(), int_snap);
        r.insert("builtins.object".to_string(), r_snap);
        let t = make_instance("builtins.int", vec![]);
        let s = make_instance("builtins.object", vec![]);
        let result = restrict_subtype_away_inner(&t, &s, false, true, &r);
        assert_eq!(result, Some(Type::UninhabitedType { ambiguous: false }));
    }

    #[test]
    fn test_restrict_subtype_away_consider_false_missing_right_snapshot_defers() {
        // s Instance with no snapshot: should_restrict_to_t_no_erase defers
        // (returns None) instead of guessing an erase parity.
        let mut r = TypeResolver::new();
        insert_plain_class(&mut r, "builtins.int");
        let t = make_instance("builtins.int", vec![]);
        let s = make_instance("mymod.NotFound", vec![]);
        let result = restrict_subtype_away_inner(&t, &s, false, true, &r);
        assert_eq!(result, None);
    }

    #[test]
    fn test_restrict_subtype_away_consider_false_generic_right_defers() {
        // s is a generic Instance (List with a type var): erase could differ
        // (the nominal arg recursion would re-compare erased args), so defer.
        let mut r = TypeResolver::new();
        insert_plain_class(&mut r, "builtins.int");
        // builtins.list[list[int]] snapshot marked generic.
        let mut list_snap = TypeInfoSnapshot {
            fullname: "builtins.list".to_string(),
            name: "list".to_string(),
            type_vars_with_variance: vec![("T".to_string(), 0, 0)],
            ..Default::default()
        };
        list_snap.mro.push("builtins.list".to_string());
        r.insert("builtins.list".to_string(), list_snap);
        let t = make_instance("builtins.int", vec![]);
        let s = make_instance(
            "builtins.list",
            vec![Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }],
        );
        let result = restrict_subtype_away_inner(&t, &s, false, true, &r);
        assert_eq!(result, None);
    }

    #[test]
    fn test_restrict_subtype_away_union_with_non_generic_right_restricts() {
        // Union[int, str] minus str under consider=False: int survives (not a
        // proper subtype) while str is dropped, folding to int. Previously the
        // per-item erase defer deferred the whole call.
        let mut r = TypeResolver::new();
        insert_plain_class(&mut r, "builtins.int");
        insert_plain_class(&mut r, "builtins.str");
        let int_t = make_instance("builtins.int", vec![]);
        let str_t = make_instance("builtins.str", vec![]);
        let union = Type::UnionType {
            items: vec![int_t.clone(), str_t.clone()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = restrict_subtype_away_inner(&union, &str_t, false, true, &r);
        // int survives (not a proper subtype of str), str is restricted
        // away (it is equal to s, the "left is right" proper subtype fast
        // path returns True), so the union folds to a single int.
        assert_eq!(result, Some(int_t));
    }

    #[test]
    fn test_restrict_subtype_away_union_protocol_right_defers() {
        // s is a protocol Instance: erase could route through the protocol
        // path, so the whole union falls back to Python.
        let mut r = TypeResolver::new();
        insert_plain_class(&mut r, "builtins.int");
        insert_plain_class(&mut r, "builtins.str");
        let mut proto = TypeInfoSnapshot {
            fullname: "mymod.Proto".to_string(),
            name: "Proto".to_string(),
            is_protocol: true,
            ..Default::default()
        };
        proto.mro.push("mymod.Proto".to_string());
        r.insert("mymod.Proto".to_string(), proto);
        let int_t = make_instance("builtins.int", vec![]);
        let str_t = make_instance("builtins.str", vec![]);
        let union = Type::UnionType {
            items: vec![int_t, str_t],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let s = make_instance("mymod.Proto", vec![]);
        let result = restrict_subtype_away_inner(&union, &s, false, true, &r);
        assert_eq!(result, None);
    }

    #[test]
    fn test_should_restrict_to_t_no_erase_non_generic_instance() {
        let mut r = TypeResolver::new();
        insert_plain_class(&mut r, "builtins.str");
        let s = make_instance("builtins.str", vec![]);
        assert_eq!(should_restrict_to_t_no_erase(&s, &r), Some(true));
    }

    #[test]
    fn test_should_restrict_to_t_no_erase_generic_instance() {
        let mut r = TypeResolver::new();
        let mut list_snap = TypeInfoSnapshot {
            fullname: "builtins.list".to_string(),
            name: "list".to_string(),
            type_vars_with_variance: vec![("T".to_string(), 0, 0)],
            ..Default::default()
        };
        list_snap.mro.push("builtins.list".to_string());
        r.insert("builtins.list".to_string(), list_snap);
        let s = make_instance(
            "builtins.list",
            vec![Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }],
        );
        assert_eq!(should_restrict_to_t_no_erase(&s, &r), Some(false));
    }

    #[test]
    fn test_should_restrict_to_t_no_erase_protocol_instance() {
        let mut r = TypeResolver::new();
        let mut proto = TypeInfoSnapshot {
            fullname: "mymod.Proto".to_string(),
            name: "Proto".to_string(),
            is_protocol: true,
            ..Default::default()
        };
        proto.mro.push("mymod.Proto".to_string());
        r.insert("mymod.Proto".to_string(), proto);
        let s = make_instance("mymod.Proto", vec![]);
        assert_eq!(should_restrict_to_t_no_erase(&s, &r), Some(false));
    }

    #[test]
    fn test_should_restrict_to_t_no_erase_non_instance() {
        let r = TypeResolver::new();
        let s = Type::NoneType;
        assert_eq!(should_restrict_to_t_no_erase(&s, &r), Some(false));
        let s = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(should_restrict_to_t_no_erase(&s, &r), Some(false));
    }

    #[test]
    fn test_should_restrict_to_t_no_erase_missing_snapshot() {
        let r = TypeResolver::new();
        let s = make_instance("mymod.NotFound", vec![]);
        assert_eq!(should_restrict_to_t_no_erase(&s, &r), None);
    }

    fn alias_snap(fullname: &str, target: &Type) -> crate::aliases::TypeAliasSnapshot {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, target).expect("alias target must encode");
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

    fn make_native_with_aliases(
        resolver: TypeResolver,
        aliases: crate::aliases::TypeAliasResolver,
    ) -> NativeTypeResolver {
        NativeTypeResolver::new(resolver, aliases)
    }

    #[test]
    fn test_seam_custom_special_method_alias_expands() {
        let r = make_resolver_with_definer("mymod.Foo", "__eq__", 0, "mymod.Foo");
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Alias".to_string(),
            alias_snap("mod.Alias", &make_instance("mymod.Foo", vec![])),
        );
        let mut native = make_native_with_aliases(r, aliases);
        let alias = alias_type("mod.Alias");
        let bytes = encode_type(&alias).expect("alias must encode");
        // Python mirrors call get_proper_type(typ) before the definer walk.
        assert_eq!(
            rust_custom_special_method(&bytes, "__eq__", false, &mut native),
            Some(true)
        );
        assert_eq!(rust_has_custom_eq_checks(&bytes, &mut native), Some(true));
    }

    #[test]
    fn test_seam_custom_special_method_alias_missing_snapshot_defers() {
        let r = make_resolver_with_definer("mymod.Foo", "__eq__", 0, "mymod.Foo");
        let mut native = make_native_with_aliases(r, crate::aliases::TypeAliasResolver::new());
        let alias = alias_type("mod.Alias");
        let bytes = encode_type(&alias).expect("alias must encode");
        // No alias snapshot: the expansion defers, preserving the old
        // behavior where a TypeAliasType reached the inner as-is.
        assert_eq!(
            rust_custom_special_method(&bytes, "__eq__", false, &mut native),
            None
        );
    }

    #[test]
    fn test_seam_restrict_subtype_away_alias_expands_to_covered_t() {
        let mut r = TypeResolver::new();
        for (fullname, name) in [("builtins.str", "str"), ("builtins.object", "object")] {
            let mut s = TypeInfoSnapshot {
                fullname: fullname.to_string(),
                name: name.to_string(),
                ..Default::default()
            };
            s.mro = vec![fullname.to_string(), "builtins.object".to_string()];
            s.has_base.insert(fullname.to_string());
            s.has_base.insert("builtins.object".to_string());
            r.insert(fullname.to_string(), s);
        }
        let mut aliases = crate::aliases::TypeAliasResolver::new();
        aliases.insert(
            "mod.Alias".to_string(),
            alias_snap("mod.Alias", &make_instance("builtins.str", vec![])),
        );
        let mut native = make_native_with_aliases(r, aliases);
        // t = Alias -> str, s = str: covers_at_runtime(str, str) is True,
        // so restrict_subtype_away returns UninhabitedType.
        let t = encode_type(&alias_type("mod.Alias")).expect("t must encode");
        let s = encode_type(&make_instance("builtins.str", vec![])).expect("s must encode");
        let result =
            rust_restrict_subtype_away(&t, &s, true, true, &mut native).expect("seam must decide");
        assert_eq!(
            decode_type(&result),
            Some(Type::UninhabitedType { ambiguous: false })
        );
    }

    #[test]
    fn test_seam_restrict_subtype_away_alias_missing_snapshot_defers() {
        let mut native = make_native_with_aliases(
            TypeResolver::new(),
            crate::aliases::TypeAliasResolver::new(),
        );
        let t = encode_type(&alias_type("mod.Alias")).expect("t must encode");
        let s = encode_type(&make_instance("builtins.str", vec![])).expect("s must encode");
        assert_eq!(
            rust_restrict_subtype_away(&t, &s, true, true, &mut native),
            None
        );
    }
}
