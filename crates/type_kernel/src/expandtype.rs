//! Native port of `mypy/expandtype.py` `expand_type` (the TypeVar
//! substitution engine), Stage 3c.
//!
//! Takes a serialized `Type` and an `env: Mapping[TypeVarId, Type]` and
//! substitutes TypeVar references with their values, mirroring
//! `ExpandTypeVisitor` (expandtype.py:180-617). Returns `None` for cases
//! the Rust subset does not handle so the Python caller falls through to
//! the pure-Python visitor (the strangler-fig per-call contract).
//!
//! Deferred (return None):
//!   * ParamSpec (`visit_param_spec`, expandtype.py:252-285) — prefix
//!     merging and flavor handling are too complex for this stage.
//!   * TypeVarTuple substitution requiring `split_with_prefix_and_suffix`
//!     (the variadic middle of a generic instance).
//!   * `TypeAliasType` (unfixed) — defer.
//!   * `Overloaded`, `PartialType`, `Parameters` — defer.
//!   * `visit_callable_type` ParamSpec branch (expandtype.py:436-480).
//!   * `visit_type_var_tuple` (expandtype.py:355-368) raises
//!     `NotImplementedError` in Python for non-trivial replacements; we
//!     defer those to Python rather than raise over FFI.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use pyo3::prelude::*;

use crate::setops::{
    flatten_nested_unions, union_item_can_be_false, union_item_can_be_true, union_make_union,
};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::split_with_prefix_and_suffix_inner;
use crate::wire::{
    read_int_bare, read_str_bare, read_type, read_type_list, write_type, write_type_list,
    ReadBuffer, Type, WriteBuffer,
};

// Alias-expanding union flatten (`flatten_nested_unions`, types.py:5057,
// with handle_type_alias_type=True). Seams that reach `expand_type_inner`
// without the expand entry keep the previous behavior (defer on aliases).
type AliasMap = std::sync::Arc<HashMap<String, crate::aliases::TypeAliasSnapshot>>;

thread_local! {
    /// Alias snapshots for `flatten_union_expanding_aliases`, installed
    /// by `rust_expand_type` for the duration of one call. `None` keeps
    /// the pre-1203 contract (defer on any alias item) for seams that
    /// reach `expand_type_inner` without the expand entry.
    static FLAT_ALIASES: RefCell<Option<AliasMap>> = const { RefCell::new(None) };
    /// InstantiateAliasVisitor mode (types.py:5513-5525): during alias
    /// substitution, `expand_type_inner`'s union arm rebuilds a plain
    /// translated union instead of flatten+simplify.
    static INSTANTIATE: Cell<bool> = const { Cell::new(false) };
}

/// RAII: installs the alias map for one `rust_expand_type` call, clears
/// it on drop (panic-safe).
struct FlatAliasGuard;

impl FlatAliasGuard {
    fn install(resolver: &NativeTypeResolver) -> Self {
        let map = resolver.alias_resolver().shared();
        FLAT_ALIASES.with(|c| *c.borrow_mut() = Some(map));
        FlatAliasGuard
    }
}

impl Drop for FlatAliasGuard {
    fn drop(&mut self) {
        FLAT_ALIASES.with(|c| *c.borrow_mut() = None);
    }
}

/// RAII for `INSTANTIATE`: sets the flag around alias substitution and
/// restores the previous value on drop (panic-safe, nesting-aware).
struct InstantiateGuard(bool);

impl InstantiateGuard {
    fn new() -> Self {
        let prev = INSTANTIATE.with(Cell::get);
        INSTANTIATE.with(|c| c.set(true));
        InstantiateGuard(prev)
    }
}

impl Drop for InstantiateGuard {
    fn drop(&mut self) {
        INSTANTIATE.with(|c| c.set(self.0));
    }
}

/// Walks an item's top-level alias chain and returns `Some(true)` when
/// the chain must defer: any snapshot with a `tvar_tuple_index` (the
/// kernel's arg substitution zips `alias_tvars`/`args` directly, while
/// Python splits the middle via `split_with_prefix_and_suffix`,
/// types.py:452-466). `Some(false)` = safe to substitute; `None` = defer
/// (missing snapshot or alias cycle, mirroring the seen-walk in
/// `chain_resolve_alias_target`). A `no_args` snapshot ends the walk:
/// Python's `_expand_once` asserts its target is an Instance
/// (types.py:445-449), never a union, and the chain cannot continue.
fn alias_chain_needs_defer(type_ref: &str, map: &AliasMap) -> Option<bool> {
    let mut seen: Vec<String> = Vec::new();
    let mut current_ref = type_ref.to_owned();
    loop {
        if seen.contains(&current_ref) {
            return None;
        }
        seen.push(current_ref.clone());
        let snap = map.get(&current_ref)?;
        if snap.tvar_tuple_index.is_some() {
            return Some(true);
        }
        if snap.no_args {
            return Some(false);
        }
        let target = read_type(&mut ReadBuffer::new(&snap.target), None).ok()?;
        match &target {
            Type::TypeAliasType { type_ref: next, .. } => current_ref = next.clone(),
            _ => return Some(false),
        }
    }
}

/// `flatten_nested_unions` (types.py:5057-5100) with
/// `handle_type_alias_type=True`: top-level alias items are chain-expanded
/// via the installed alias map (mirroring `get_proper_type`,
/// types.py:4047-4064), recursing through union positions only; a
/// non-union expansion appends the ORIGINAL alias node ("Must preserve
/// original aliases when possible", types.py:5098-5099). Returns `None`
/// to defer: no alias map installed, missing snapshot, alias cycle, or a
/// `tvar_tuple_index` alias in the chain (see `alias_chain_needs_defer`).
fn flatten_union_expanding_aliases(items: &[Type]) -> Option<Vec<Type>> {
    let map = FLAT_ALIASES.with(|c| c.borrow().clone())?;
    let mut flat = Vec::with_capacity(items.len());
    for t in items {
        match t {
            Type::TypeAliasType { type_ref, .. } => {
                if alias_chain_needs_defer(type_ref, &map)? {
                    return None;
                }
                let tp = {
                    // InstantiateAliasVisitor, not the simplifying union
                    // arm (types.py:5513-5525).
                    let _guard = InstantiateGuard::new();
                    crate::types_impl::chain_resolve_alias_target(t, &map)?
                };
                if let Type::UnionType { items: inner, .. } = tp {
                    flat.extend(flatten_union_expanding_aliases(&inner)?);
                } else {
                    flat.push(t.clone());
                }
            }
            Type::UnionType { items: inner, .. } => {
                flat.extend(flatten_union_expanding_aliases(inner)?);
            }
            _ => flat.push(t.clone()),
        }
    }
    Some(flat)
}
/// Key for the env: `(raw_id, meta_level, namespace)`. Mirrors
/// `TypeVarId.__eq__` (types.py:574-576), which compares `raw_id`,
/// `meta_level`, and `namespace`.
pub(crate) type EnvKey = (i64, i64, String);

/// `#[pyfunction]` entry for `expand_type`. The Python-side shim
/// (mypy/expandtype.py) calls this with the serialized `typ` blob, the
/// serialized `env`, and the `NativeTypeResolver` pyclass. Returns `None`
/// (Python `None`) when Rust doesn't handle the case; `Some(bytes)`
/// otherwise, holding a wire-format type blob the shim decodes via
/// `read_type`.
///
/// The env wire format is: count (bare int) + pairs of
/// (TypeVarId raw_id bare int + TypeVarId meta_level bare int +
/// TypeVarId namespace bare str + Type). Mirrors the Python-side
/// `_serialize_env` in mypy/expandtype.py.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_expand_type(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    env_bytes: &[u8],
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let _ = resolver; // reserved for future Instance.has_type_var_tuple lookups
    let _flat_alias_guard = FlatAliasGuard::install(resolver);
    let typ = decode_type(type_bytes)?;
    let env = decode_env(env_bytes)?;
    // Empty env is fully wire-portable; alias-bearing inputs are fine too:
    // alias args expand natively (mirroring visit_type_alias_type) and the
    // Python shim re-links decoded alias nodes via resolve_aliases fixup.
    expand_with_env(&typ, &env, strict_optional, true)
}

/// Shared tail of the expand FFI entries: run the substitution and ship
/// the result bytes.
fn expand_with_env(
    typ: &Type,
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
    alias_ok: bool,
) -> Option<Vec<u8>> {
    encode_type(&expand_type_with_env_inner(
        typ,
        env,
        strict_optional,
        false,
        alias_ok,
        true,
    )?)
}

/// Inner expansion returning the raw `Type`: leaves that carry no TypeVars
/// defer (Python returns the original object by identity), and any leftover
/// TypeVar after substitution defers for the same reason.
pub(crate) fn expand_type_with_env(
    typ: &Type,
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Type> {
    expand_type_with_env_inner(typ, env, strict_optional, false, false, false)
}

/// Free-result variant: leftover TypeVars are returned instead of deferring
/// (the IAMA member tail freezes them, mirroring `freeze_all_type_vars`,
/// typeops.py:2102). The surviving-alias check still applies: a
/// wire-decoded alias node carries `alias=None` and Python asserts against
/// it (types.py:397).
fn expand_type_with_env_inner(
    typ: &Type,
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
    allow_free: bool,
    alias_ok: bool,
    relink_ok: bool,
) -> Option<Type> {
    // Leaf types carry no TypeVars; Python returns the original object
    // (identity). We return a cloned copy — structurally identical, wire-safe.
    if is_leaf_type(typ) {
        return Some(typ.clone());
    }
    let expanded = expand_type_inner(typ, env, strict_optional)?;
    if !allow_free && !relink_ok && result_has_typevar(&expanded) {
        return None;
    }
    // A surviving alias decodes with alias=None (types.py:397 asserts), so
    // unfixed aliases crash the caller. With alias_ok=true the shim
    // re-links survivors; other callers keep the defer.
    if !alias_ok && result_contains_typealias(&expanded) {
        return None;
    }
    Some(expanded)
}

/// `#[pyfunction]` entry for `expand_type_by_instance`
/// (mypy/expandtype.py:295-325). Serializable subset: plain class type
/// var binding (no TypeVarTuple), every arg readable, args length equal
/// to the class's `defn.type_vars`. Mirroring the Python zip-truncate, a
/// length mismatch leaves extra typevars unbound, so this defers.
///
/// Mirrors the non-TVT branch:
///   tvars = tuple(instance.type.defn.type_vars)
///   variables = {binder.id: arg for binder, arg in zip(tvars, instance.args)}
///   return expand_type(typ, variables)
///
/// The env keys use `(raw_id, 0, "")`: class typevars bind
/// `TypeVarId(raw_id)` (types.py:554 defaults meta_level=0, namespace="").
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_expand_type_by_instance(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    instance_bytes: &[u8],
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let _flat_alias_guard = FlatAliasGuard::install(resolver);
    let typ = decode_type(type_bytes)?;
    let instance = decode_type(instance_bytes)?;
    let expanded =
        expand_type_by_instance_relink(&typ, &instance, resolver.resolver(), strict_optional)?;
    encode_type(&expanded)
}

/// Core `expand_type_by_instance` (mypy/expandtype.py:295-325): bind the
/// class typevars of `instance` in `typ`, then substitute them into `typ`.
/// Serializable subset: plain class type var binding (no TypeVarTuple),
/// every arg readable, args length equal to the class's `defn.type_vars`.
/// Mirroring the Python zip-truncate, a length mismatch leaves extra
/// typevars unbound, so this defers.
///
/// Mirrors the non-TVT branch:
///   tvars = tuple(instance.type.defn.type_vars)
///   variables = {binder.id: arg for binder, arg in zip(tvars, instance.args)}
///   return expand_type(typ, variables)
///
/// The env keys use `(raw_id, 0, type_ref)`: class typevars declare
/// `TypeVarId(raw_id, namespace=<class fullname>)` (types.py:554), and
/// the resolved `type_ref` of the instance is that fullname, so the keys
/// match the namespaces wire-decoded TypeVars carry.
pub(crate) fn expand_type_by_instance_core(
    typ: &Type,
    instance: &Type,
    resolver: &TypeResolver,
    strict_optional: bool,
) -> Option<Type> {
    expand_type_by_instance_inner(
        typ,
        instance,
        resolver,
        strict_optional,
        false,
        false,
        false,
    )
}

/// FFI-only variant with the identity-repair contract: leftover TypeVars
/// are returned and the Python shim relinks them. Aliases ride through
/// (#1289, the #1224 pattern): survivors re-link via resolve_aliases fixup.
pub(crate) fn expand_type_by_instance_relink(
    typ: &Type,
    instance: &Type,
    resolver: &TypeResolver,
    strict_optional: bool,
) -> Option<Type> {
    expand_type_by_instance_inner(typ, instance, resolver, strict_optional, false, true, true)
}

/// Free-result variant of `expand_type_by_instance_core`: leftover TypeVars
/// in the expansion are returned instead of deferring, mirroring Python's
/// `expand_type_by_instance` (which never defers on leftover method type
/// vars — `freeze_all_type_vars` reifies them afterwards, typeops.py:2102).
/// Used by the IAMA member tail, which freezes before returning. Aliases
/// are allowed through in both directions (issue #1224): Python's
/// `visit_type_alias_type` expands alias args and keeps the alias node, and
/// the shim re-links the decoded alias through the wire alias map
/// (`fixup_wire_type(resolve_aliases=True)`); an alias absent from the map
/// defers via `fixer.missing`.
pub(crate) fn expand_type_by_instance_free(
    typ: &Type,
    instance: &Type,
    resolver: &TypeResolver,
    strict_optional: bool,
) -> Option<Type> {
    expand_type_by_instance_inner(typ, instance, resolver, strict_optional, true, false, true)
}

fn expand_type_by_instance_inner(
    typ: &Type,
    instance: &Type,
    resolver: &TypeResolver,
    strict_optional: bool,
    allow_free: bool,
    relink_ok: bool,
    alias_ok: bool,
) -> Option<Type> {
    let Type::Instance { type_ref, args, .. } = instance else {
        return None;
    };
    // Wire-decoded TypeAliasType carries alias=None, which the Python
    // graph asserts against (is_recursive/_expand_once, types.py:362/397)
    // unless the shim re-links it. Callers with alias_ok=true run behind
    // `fixup_wire_type(resolve_aliases=True)`; others defer alias-bearing
    // input to Python.
    if !alias_ok && result_contains_typealias(typ) {
        return None;
    }
    let snap = resolver.get(type_ref)?;
    // Python fast path (expandtype.py:298-299) returns `typ` unchanged
    // when the instance has no args and no TVT.
    if args.is_empty() && !snap.has_type_var_tuple_type {
        return Some(typ.clone());
    }
    // Instance typevars bind TypeVarId(raw_id, namespace=type_ref) so the
    // env keys match the namespaces wire-decoded TypeVars carry.
    let env_ns = type_ref.clone();
    let mut env = HashMap::with_capacity(args.len());
    if snap.has_type_var_tuple_type {
        // TypeVarTuple branch (expandtype.py:389-406): middle args bind
        // the single variadic tvar as a TupleType; prefix/suffix bind the
        // ordinary tvars pairwise (tvars_* vs args_*).
        let tvars = &snap.type_vars_with_variance;
        let raw_ids = &snap.type_var_raw_ids;
        if tvars.len() != raw_ids.len() {
            // Parallel arrays out of sync -> cannot key the env.
            return None;
        }
        let prefix = snap.type_var_tuple_prefix.unwrap_or(0);
        let suffix = snap.type_var_tuple_suffix.unwrap_or(0);
        // Python split_with_prefix_and_suffix on defn.type_vars never
        // pads (extend finds no UnpackType in a tvar list), so the
        // middle must be non-empty or Python raises IndexError.
        if prefix + suffix > tvars.len() {
            return None;
        }
        if args.len() < prefix + suffix {
            // After split's extend, args still has fewer than
            // prefix+suffix items -> slice below would underflow.
            return None;
        }
        let (_args_prefix, args_middle, _args_suffix) =
            split_with_prefix_and_suffix_inner(args, prefix, suffix);
        // tvars_middle[0] must be the single TypeVarTuple
        // (expandtype.py:389-390).
        let (_, _, tvt_kind) = tvars[prefix];
        if tvt_kind != 2 {
            return None;
        }
        let tvt_raw_id = raw_ids[prefix];
        let tvt_fallback = decode_type(snap.type_var_tuple_fallback.as_ref()?)?;
        // Bind tvar.id: TupleType(args_middle, tvar.tuple_fallback)
        // (expandtype.py:390-391).
        env.insert(
            (tvt_raw_id, 0, env_ns.clone()),
            Type::TupleType {
                partial_fallback: Box::new(tvt_fallback),
                items: args_middle,
                implicit: false,
            },
        );
        // Bind prefix/suffix ordinary tvars
        // (expandtype.py:392-405): zip(tvars_prefix, args_prefix) and
        // zip(tvars_suffix, args_suffix).
        if args.len() < prefix || args.len() < suffix {
            return None;
        }
        for i in 0..prefix {
            let raw_id = raw_ids[i];
            if raw_id >= 0 {
                env.insert((raw_id, 0, env_ns.clone()), args[i].clone());
            }
        }
        for j in 0..suffix {
            let raw_id = raw_ids[tvars.len() - suffix + j];
            let arg = &args[args.len() - suffix + j];
            if raw_id >= 0 {
                env.insert((raw_id, 0, env_ns.clone()), arg.clone());
            }
        }
        return expand_type_with_env_inner(
            typ,
            &env,
            strict_optional,
            allow_free,
            alias_ok,
            relink_ok,
        );
    }
    // Non-variadic: fast path + binding (expandtype.py:407-409).
    let raw_ids = &snap.type_var_raw_ids;
    for (raw_id, arg) in raw_ids.iter().zip(args) {
        env.insert((*raw_id, 0, env_ns.clone()), arg.clone());
    }
    expand_type_with_env_inner(typ, &env, strict_optional, allow_free, alias_ok, relink_ok)
}

/// Decode a wire-format `Type` blob. Returns `None` on any read failure.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Decode the env wire format into a `HashMap<EnvKey, Type>`. Returns
/// `None` on any read failure (truncated input, bad tag).
fn decode_env(bytes: &[u8]) -> Option<HashMap<EnvKey, Type>> {
    let mut buf = ReadBuffer::new(bytes);
    let count = read_int_bare(&mut buf).ok()?;
    if count < 0 {
        return None;
    }
    let mut env = HashMap::with_capacity(count as usize);
    for _ in 0..count {
        let raw_id = read_int_bare(&mut buf).ok()?;
        let meta_level = read_int_bare(&mut buf).ok()?;
        // namespace is a fullname string written via `librt write_str`
        // (bare short-int size + utf8, no tag). Must use the bare reader.
        let namespace = read_str_bare(&mut buf).ok()?;
        let typ = read_type(&mut buf, None).ok()?;
        env.insert((raw_id, meta_level, namespace), typ);
    }
    Some(env)
}

/// Encode a `Type` via `write_type`. Returns `None` if the variant is not
/// writable (the caller defers to Python).
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// Substitute TypeVar references in `typ` using `env`, mirroring
/// `ExpandTypeVisitor`. Returns `None` for deferred cases (ParamSpec,
/// TypeAliasType, Overloaded, etc.) so the caller falls through to Python.
pub(crate) fn expand_type_inner(
    typ: &Type,
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Type> {
    match typ {
        // Leaf types that carry no TypeVars: returned as-is.
        // (expandtype.py:189-211)
        Type::AnyType { .. }
        | Type::NoneType
        | Type::ErasedType
        | Type::UninhabitedType { .. }
        | Type::DeletedType { .. }
        | Type::UnboundType { .. } => Some(typ.clone()),

        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let new_args = expand_type_tuple_with_unpack(args, env, strict_optional)?;
            // Tuple[*Tuple[X, ...], ...] -> Tuple[X, ...].
            // When single arg is UnpackType wrapping builtins.tuple,
            // unwrap to that Instance's args.
            let final_args = if type_ref == "builtins.tuple" && new_args.len() == 1 {
                normalize_tuple_unpack(&new_args[0])
            } else {
                new_args
            };
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: final_args,
                last_known_value: last_known_value.clone(),
                extra_attrs: extra_attrs.clone(),
            })
        }

        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default,
            variance,
            meta_level,
        } => {
            // Self type (raw_id == 0): expand upper_bound first
            // (expandtype.py:243-244), since Self`0 <: C[T, S] may reference
            // other TypeVars in the bound.
            let upper_bound = if *raw_id == 0 {
                Box::new(expand_type_inner(upper_bound, env, strict_optional)?)
            } else {
                upper_bound.clone()
            };
            let key = (*raw_id, *meta_level, namespace.clone());
            let repl = env.get(&key);
            match repl {
                Some(Type::Instance {
                    type_ref,
                    args,
                    last_known_value: _,
                    extra_attrs,
                }) => {
                    // Python strips last_known_value on Instance replacements
                    // (expandtype.py:246-249).
                    Some(Type::Instance {
                        type_ref: type_ref.clone(),
                        args: args.clone(),
                        last_known_value: None,
                        extra_attrs: extra_attrs.clone(),
                    })
                }
                Some(other) => Some(other.clone()),
                None => {
                    // Unmatched TypeVar: return a copy with the (possibly
                    // expanded) upper_bound.
                    Some(Type::TypeVarType {
                        name: name.clone(),
                        fullname: fullname.clone(),
                        raw_id: *raw_id,
                        namespace: namespace.clone(),
                        values: values.clone(),
                        upper_bound,
                        default: default.clone(),
                        variance: *variance,
                        meta_level: *meta_level,
                    })
                }
            }
        }

        // UnionType: Python calls
        // make_union(remove_trivial(flatten_nested_unions(expanded))) then
        // get_proper_type, which collapses and deduplicates items.
        Type::UnionType {
            items,
            uses_pep604_syntax,
            ..
        } => {
            let mut expanded = Vec::with_capacity(items.len());
            for item in items {
                expanded.push(expand_type_inner(item, env, strict_optional)?);
            }
            if INSTANTIATE.get() {
                // InstantiateAliasVisitor.visit_union_type
                // (types.py:5513-5525): plain rebuild, truthiness flags
                // recomputed from items.
                let can_t = expanded.iter().any(union_item_can_be_true);
                let can_f = expanded.iter().any(union_item_can_be_false);
                return Some(Type::UnionType {
                    items: expanded,
                    uses_pep604_syntax: *uses_pep604_syntax,
                    can_be_true: can_t,
                    can_be_false: can_f,
                });
            }
            let flat = match flatten_union_expanding_aliases(&expanded) {
                Some(flat) => flat,
                None => flatten_nested_unions(&expanded)?,
            };
            let simplified = union_make_union(remove_trivial(&flat, strict_optional));
            Some(simplified)
        }

        // TypeType: Python expands the item then calls
        // TypeType.make_normalized(item, is_type_form), which distributes
        // Type[Union[A, B]] into Union[Type[A], Type[B]].
        Type::TypeType { item, is_type_form } => {
            let new_item = expand_type_inner(item, env, strict_optional)?;
            Some(make_type_normalized(new_item, *is_type_form))
        }

        // LiteralType: Python's visit_literal_type returns t as-is
        // (expandtype.py:751-753). Do not expand the fallback.
        Type::LiteralType { .. } => Some(typ.clone()),

        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            // (expandtype.py:720-740)
            let new_items = expand_type_list_with_unpack(items, env, strict_optional)?;
            // Normalize Tuple[*Tuple[X, ...]] -> Tuple[X, ...].
            if new_items.len() == 1 {
                if let Type::UnpackType { typ: inner } = &new_items[0] {
                    // Python checks: not (TypeAliasType and is_recursive).
                    // Rust defers TypeAliasType entirely, so inner is never
                    // a TypeAliasType here.
                    let unpacked = inner.as_ref();
                    if let Type::Instance { type_ref, .. } = unpacked {
                        if type_ref == "builtins.tuple" {
                            // If partial_fallback is NOT builtins.tuple
                            // (named tuple), preserve the fallback.
                            let fb_is_tuple = matches!(
                                partial_fallback.as_ref(),
                                Type::Instance { type_ref: fb_ref, .. } if fb_ref == "builtins.tuple"
                            );
                            if fb_is_tuple {
                                return Some(unpacked.clone());
                            }
                            // Named tuple: return expanded fallback.
                            return expand_type_inner(partial_fallback, env, strict_optional);
                        }
                        // unpacked is not builtins.tuple: return fallback.
                        return expand_type_inner(partial_fallback, env, strict_optional);
                    }
                }
            }
            let new_fallback = expand_type_inner(partial_fallback, env, strict_optional)?;
            Some(Type::TupleType {
                partial_fallback: Box::new(new_fallback),
                items: new_items,
                implicit: *implicit,
            })
        }

        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            // (expandtype.py:556-563)
            let new_fallback = expand_type_inner(fallback, env, strict_optional)?;
            let mut new_items = Vec::with_capacity(items.len());
            for (name, typ) in items {
                new_items.push((name.clone(), expand_type_inner(typ, env, strict_optional)?));
            }
            Some(Type::TypedDictType {
                fallback: Box::new(new_fallback),
                items: new_items,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }

        Type::CallableType {
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
        } => {
            // (expandtype.py:435-502). The ParamSpec branch
            // (expandtype.py:436-480) is deferred to Python: if any
            // variable is a ParamSpecType, return None.
            for v in variables {
                if matches!(v, Type::ParamSpecType { .. }) {
                    return None;
                }
            }
            // `is_bound` needs no special handling here: it survives
            // copy_modified unchanged and expansion never branches on it.

            // The Unpack interpolation branch
            // (expandtype.py:482-488, interpolate_args_for_unpack) is
            // deferred: if a var_arg is an UnpackType, defer to Python.
            for at in arg_types {
                if matches!(at, Type::UnpackType { .. }) {
                    return None;
                }
            }
            // ExpandTypeVisitor (expandtype.py:676) expands arg_types, ret_type,
            // type_guard, type_is, instance_type. Does NOT expand fallback or
            // variables (declared type vars are definitions).
            let new_instance_type = match instance_type {
                Some(it) => Some(Box::new(expand_type_inner(it, env, strict_optional)?)),
                None => None,
            };
            let mut new_arg_types = Vec::with_capacity(arg_types.len());
            for at in arg_types {
                new_arg_types.push(expand_type_inner(at, env, strict_optional)?);
            }
            let new_ret_type = Box::new(expand_type_inner(ret_type, env, strict_optional)?);
            let new_type_guard = match type_guard {
                Some(tg) => Some(Box::new(expand_type_inner(tg, env, strict_optional)?)),
                None => None,
            };
            let new_type_is = match type_is {
                Some(ti) => Some(Box::new(expand_type_inner(ti, env, strict_optional)?)),
                None => None,
            };
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
                variables: variables.clone(),
                type_guard: new_type_guard,
                type_is: new_type_is,
            })
        }

        Type::UnpackType { typ } => {
            // (expandtype.py:370-380). visit_unpack_type carries a variadic
            // tuple over. We expand the inner type. The expand_unpack
            // list-expansion path is handled at the tuple/instance level.
            let new_typ = expand_type_inner(typ, env, strict_optional)?;
            Some(Type::UnpackType {
                typ: Box::new(new_typ),
            })
        }

        // TypeAliasType: Python's visit_type_alias_type expands the
        // arguments (expandtype.py:911-918). Target cannot contain typevars
        // (not bound by the alias itself), so we just expand the args.
        Type::TypeAliasType { args, type_ref } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let new_args = expand_type_list_with_unpack(args, env, strict_optional)?;
            Some(Type::TypeAliasType {
                args: new_args,
                type_ref: type_ref.clone(),
            })
        }

        // Overloaded: Python's visit_overloaded expands each item
        // (expandtype.py:811-818). Each item is a CallableType.
        Type::Overloaded { items } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(expand_type_inner(item, env, strict_optional)?);
            }
            Some(Type::Overloaded { items: new_items })
        }

        // Parameters: Python's visit_parameters expands arg_types
        // (expandtype.py:709-710).
        Type::Parameters(params) => {
            let new_arg_types =
                expand_type_list_with_unpack(&params.arg_types, env, strict_optional)?;
            Some(Type::Parameters(crate::wire::Parameters {
                arg_types: new_arg_types,
                arg_kinds: params.arg_kinds.clone(),
                arg_names: params.arg_names.clone(),
                variables: params.variables.clone(),
                imprecise_arg_kinds: params.imprecise_arg_kinds,
                is_ellipsis_args: params.is_ellipsis_args,
            }))
        }

        // Deferred variants: ParamSpecType (prefix merging too complex for
        // this stage).
        Type::ParamSpecType { .. } => None,

        // TypeVarTupleType (expandtype.py:703-715): expand the replacement
        // if bound; Any/Uninhabited build tuple_fallback[args=[repl]];
        // other bindings defer (Python raises NotImplementedError).
        Type::TypeVarTupleType {
            tuple_fallback,
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound,
            default,
            min_len,
        } => {
            let key = (*raw_id, 0, namespace.clone());
            match env.get(&key) {
                Some(repl @ (Type::AnyType { .. } | Type::UninhabitedType { .. })) => {
                    let fallback = tuple_fallback.as_ref();
                    let new_fallback = if let Type::Instance {
                        type_ref,
                        last_known_value,
                        extra_attrs,
                        ..
                    } = fallback
                    {
                        Type::Instance {
                            type_ref: type_ref.clone(),
                            args: vec![repl.clone()],
                            last_known_value: last_known_value.clone(),
                            extra_attrs: extra_attrs.clone(),
                        }
                    } else {
                        return None;
                    };
                    Some(new_fallback)
                }
                Some(_) => None,
                None => Some(Type::TypeVarTupleType {
                    tuple_fallback: tuple_fallback.clone(),
                    name: name.clone(),
                    fullname: fullname.clone(),
                    raw_id: *raw_id,
                    namespace: namespace.clone(),
                    upper_bound: upper_bound.clone(),
                    default: default.clone(),
                    min_len: *min_len,
                }),
            }
        }
    }
}

/// `expand_type_tuple_with_unpack` (expandtype.py:523-532). Expands a
/// tuple of arg types, splicing in the items of any UnpackType wrapping
/// a TypeVarTupleType via `expand_unpack`. Non-Unpack args are expanded
/// normally.
fn expand_type_tuple_with_unpack(
    typs: &[Type],
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Vec<Type>> {
    let mut items = Vec::with_capacity(typs.len());
    for item in typs {
        if let Type::UnpackType { typ: inner } = item {
            if let Type::TypeVarTupleType { .. } = inner.as_ref() {
                // expand_unpack (expandtype.py:382-400).
                let spliced = expand_unpack(inner, env)?;
                items.extend(spliced);
                continue;
            }
        }
        items.push(expand_type_inner(item, env, strict_optional)?);
    }
    Some(items)
}

/// `expand_type_list_with_unpack` (expandtype.py:513-521). Same as
/// `expand_type_tuple_with_unpack` but over a Vec.
fn expand_type_list_with_unpack(
    typs: &[Type],
    env: &HashMap<EnvKey, Type>,
    strict_optional: bool,
) -> Option<Vec<Type>> {
    expand_type_tuple_with_unpack(typs, env, strict_optional)
}

/// `TypeType.make_normalized` (types.py:3677-3691): distributes
/// `Type[Union[A, B]]` into `Union[Type[A], Type[B]]` unless
/// `is_type_form`. The item comes from a wire round-trip so it is already
/// proper (`get_proper_type` is a no-op). The resulting union may be a
/// single TypeType (collapsed by `make_union`).
pub(crate) fn make_type_normalized(item: Type, is_type_form: bool) -> Type {
    if !is_type_form {
        if let Type::UnionType { items, .. } = &item {
            let mut tt_items = Vec::with_capacity(items.len());
            for u in items {
                tt_items.push(make_type_normalized(u.clone(), false));
            }
            return union_make_union(tt_items);
        }
    }
    Type::TypeType {
        item: Box::new(item),
        is_type_form,
    }
}

/// `remove_trivial` (expandtype.py:984-1011). Makes trivial simplifications
/// on a list of types without `is_subtype`: drop bottom types (honoring
/// `strict_optional` for NoneType), short-circuit to a lone
/// `builtins.object`, and drop strict duplicates (push-first-wins).
/// The input comes from the wire format so every type is already proper.
pub(crate) fn remove_trivial(types: &[Type], strict_optional: bool) -> Vec<Type> {
    let mut removed_none = false;
    let mut new_types: Vec<Type> = Vec::new();
    for t in types {
        match t {
            Type::UninhabitedType { .. } => continue,
            Type::NoneType if !strict_optional => {
                removed_none = true;
                continue;
            }
            _ => {}
        }
        if let Type::Instance { type_ref, .. } = t {
            if type_ref == "builtins.object" {
                return vec![t.clone()];
            }
        }
        if !new_types.contains(t) {
            new_types.push(t.clone());
        }
    }
    if !new_types.is_empty() {
        return new_types;
    }
    if removed_none {
        return vec![Type::NoneType];
    }
    vec![Type::UninhabitedType { ambiguous: false }]
}

/// `#[pyfunction]` entry for `remove_trivial` (expandtype.py:984-1011).
/// Takes a wire-format type list (LIST_GEN tag) plus `strict_optional`;
/// returns the simplified list as wire bytes. Returns `None` (Python
/// `None`) only for read/write failure, in which case the caller defers
/// to the pure-Python loop. Every input type is wire-proper, so the
/// Python `get_proper_type` and the set-dedup on proper types map
/// directly onto the structural `PartialEq` of the Rust `Type` enum.
#[pyfunction]
pub(crate) fn rust_remove_trivial(
    types_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<Option<Vec<u8>>> {
    let mut buf = ReadBuffer::new(types_bytes);
    let types = match read_type_list(&mut buf) {
        Ok(types) => types,
        Err(_) => return Ok(None),
    };
    let simplified = remove_trivial(&types, strict_optional);
    let mut wbuf = WriteBuffer::new();
    match write_type_list(&mut wbuf, &simplified) {
        Ok(()) => Ok(Some(wbuf.into_bytes())),
        Err(_) => Ok(None),
    }
}

/// `expand_unpack` (expandtype.py:382-400). Expands an UnpackType whose
/// inner type is a TypeVarTupleType. Looks up the TypeVarTuple in env:
///   * TupleType -> its items (spliced in).
///   * builtins.tuple Instance or TypeVarTupleType -> [UnpackType(repl)].
///   * AnyType / UninhabitedType -> [UnpackType(tuple_fallback[args=[repl]])].
///   * else (UnpackType wrapping a TupleType) -> splice the inner items.
///
/// Returns None for any other replacement (defer to Python, which would
/// raise RuntimeError).
fn expand_unpack(tvt: &Type, env: &HashMap<EnvKey, Type>) -> Option<Vec<Type>> {
    let tvt = if let Type::TypeVarTupleType {
        raw_id, namespace, ..
    } = tvt
    {
        // TypeVarTupleType wire has no meta_level yet; env meta is 0.
        let key = (*raw_id, 0, namespace.clone());
        // Unmatched TypeVarTuple: defer to Python.
        env.get(&key)?
    } else {
        return None;
    };
    // If the replacement is itself an UnpackType, unwrap once
    // (expandtype.py:385-386).
    let repl = if let Type::UnpackType { typ: inner } = tvt {
        inner.as_ref()
    } else {
        tvt
    };
    match repl {
        Type::TupleType { items, .. } => Some(items.clone()),
        Type::Instance { type_ref, .. } if type_ref == "builtins.tuple" => {
            Some(vec![Type::UnpackType {
                typ: Box::new(repl.clone()),
            }])
        }
        Type::TypeVarTupleType { .. } => Some(vec![Type::UnpackType {
            typ: Box::new(repl.clone()),
        }]),
        Type::AnyType { .. } | Type::UninhabitedType { .. } => {
            // (expandtype.py:395-398) Replace *Ts = Any/Never with
            // *tuple[Any, ...] using the TypeVarTuple's tuple_fallback.
            let fallback = match tvt {
                Type::TypeVarTupleType { tuple_fallback, .. } => tuple_fallback.as_ref(),
                _ => return None,
            };
            let new_fallback = if let Type::Instance {
                type_ref,
                last_known_value,
                extra_attrs,
                ..
            } = fallback
            {
                Type::Instance {
                    type_ref: type_ref.clone(),
                    args: vec![repl.clone()],
                    last_known_value: last_known_value.clone(),
                    extra_attrs: extra_attrs.clone(),
                }
            } else {
                return None;
            };
            Some(vec![Type::UnpackType {
                typ: Box::new(new_fallback),
            }])
        }
        _ => None, // invalid replacement: defer to Python
    }
}

/// builtins.tuple arg normalization (expandtype.py:228-237). When the
/// single arg of `builtins.tuple` is an UnpackType wrapping a
/// builtins.tuple Instance, replace the arg list with that Instance's
/// args. Returns `new_args` unchanged otherwise.
fn normalize_tuple_unpack(arg: &Type) -> Vec<Type> {
    if let Some(Type::Instance { args, .. }) = normalize_tuple_unpack_to_instance(arg) {
        return args.clone();
    }
    vec![arg.clone()]
}

/// Check if `typ` is a leaf type with no TypeVar references to substitute.
/// Python's `ExpandTypeVisitor` returns `t` unchanged for these, so the
/// wire round-trip must defer to preserve object identity.
fn is_leaf_type(typ: &Type) -> bool {
    matches!(
        typ,
        Type::AnyType { .. }
            | Type::NoneType
            | Type::UninhabitedType { .. }
            | Type::DeletedType { .. }
            | Type::UnboundType { .. }
    )
}

/// True if `typ` contains any TypeVar-like node. Such results do not
/// survive a wire round-trip intact (object identity is lost), so the
/// caller defers to Python.
pub(crate) fn result_has_typevar(typ: &Type) -> bool {
    let mut stack = vec![typ];
    while let Some(cur) = stack.pop() {
        match cur {
            Type::TypeVarType { .. }
            | Type::ParamSpecType { .. }
            | Type::TypeVarTupleType { .. } => {
                // A TypeVar-like node means the expansion keeps a TypeVar that Python
                // preserves by object identity; defer. Nested contents
                // cannot matter for identity.
                return true;
            }
            Type::Instance { args, .. } => stack.extend(args.iter()),
            Type::TypeAliasType { args, .. } => stack.extend(args.iter()),
            Type::CallableType {
                arg_types,
                ret_type,
                fallback,
                instance_type,
                variables,
                ..
            } => {
                stack.extend(arg_types.iter());
                stack.push(ret_type);
                stack.push(fallback);
                if let Some(it) = instance_type {
                    stack.push(it);
                }
                stack.extend(variables.iter());
            }
            Type::TupleType {
                items,
                partial_fallback,
                ..
            } => {
                stack.extend(items.iter());
                stack.push(partial_fallback);
            }
            Type::TypedDictType {
                items, fallback, ..
            } => {
                stack.extend(items.iter().map(|(_, t)| t));
                stack.push(fallback);
            }
            Type::UnionType { items, .. } => stack.extend(items.iter()),
            Type::Overloaded { items, .. } => stack.extend(items.iter()),
            Type::Parameters(params) => {
                stack.extend(params.arg_types.iter());
                stack.extend(params.variables.iter());
            }
            Type::TypeType { item, .. } => stack.push(item),
            Type::UnpackType { typ } => stack.push(typ),
            Type::LiteralType { fallback, .. } => stack.push(fallback),
            _ => {}
        }
    }
    false
}

/// True if `typ` contains any TypeAliasType node. Wire round-trips decode
/// TypeAliasType with alias=None, which Python asserts against on access
/// (`TypeAliasType.is_recursive`, types.py:397), so such results must defer
/// to the Python visitor which preserves the original alias object.
pub(crate) fn result_contains_typealias(typ: &Type) -> bool {
    let mut stack = vec![typ];
    while let Some(cur) = stack.pop() {
        match cur {
            Type::TypeAliasType { .. } => {
                return true;
            }
            Type::Instance { args, .. } => stack.extend(args.iter()),
            Type::CallableType {
                arg_types,
                ret_type,
                fallback,
                instance_type,
                variables,
                ..
            } => {
                stack.extend(arg_types.iter());
                stack.push(ret_type);
                stack.push(fallback);
                if let Some(it) = instance_type {
                    stack.push(it);
                }
                stack.extend(variables.iter());
            }
            Type::TupleType {
                items,
                partial_fallback,
                ..
            } => {
                stack.extend(items.iter());
                stack.push(partial_fallback);
            }
            Type::TypedDictType {
                items, fallback, ..
            } => {
                stack.extend(items.iter().map(|(_, t)| t));
                stack.push(fallback);
            }
            Type::UnionType { items, .. } => stack.extend(items.iter()),
            Type::Overloaded { items, .. } => stack.extend(items.iter()),
            Type::Parameters(params) => {
                stack.extend(params.arg_types.iter());
                stack.extend(params.variables.iter());
            }
            Type::TypeType { item, .. } => stack.push(item),
            Type::UnpackType { typ } => stack.push(typ),
            Type::LiteralType { fallback, .. } => stack.push(fallback),
            _ => {}
        }
    }
    false
}

/// If `arg` is an UnpackType wrapping a builtins.tuple Instance, return
/// that Instance. Used by the TupleType single-item normalization
/// (expandtype.py:536-551) which returns the unpacked Instance directly.
/// Returns None otherwise.
fn normalize_tuple_unpack_to_instance(arg: &Type) -> Option<Type> {
    if let Type::UnpackType { typ: inner } = arg {
        if let Type::Instance { type_ref, .. } = inner.as_ref() {
            if type_ref == "builtins.tuple" {
                return Some((**inner).clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    fn snap_with_tvar(fullname: &str, raw_id: i64) -> TypeInfoSnapshot {
        TypeInfoSnapshot {
            fullname: fullname.to_owned(),
            name: fullname.to_owned(),
            has_type_var_tuple_type: false,
            type_var_raw_ids: vec![raw_id],
            ..Default::default()
        }
    }

    fn any() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn tvar(raw_id: i64) -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "__main__.T".to_string(),
            raw_id,
            namespace: String::new(),
            values: Vec::new(),
            upper_bound: Box::new(any()),
            default: Box::new(any()),
            variance: 0,
            meta_level: 0,
        }
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn expand_by_instance_substitutes_tvar_in_args() {
        // List[T] applied to List[int] expands the arg T -> int.
        let typ = instance("builtins.list", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::from([((0, 0, String::new()), any())]);
        let out = expand_type_inner(&typ, &env, false).unwrap();
        match out {
            Type::Instance { args, .. } => {
                assert!(matches!(args.as_slice(), [Type::AnyType { .. }]));
            }
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn expand_by_instance_unmatched_tvar_leaves_typevar() {
        // List[T] applied with an env that lacks T stays a TypeVarType.
        let typ = instance("builtins.list", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        let out = expand_type_inner(&typ, &env, false).unwrap();
        match out {
            Type::Instance { args, .. } => {
                assert!(matches!(args.as_slice(), [Type::TypeVarType { .. }]));
            }
            _ => panic!("expected Instance"),
        }
    }

    #[test]
    fn etbi_sentinel_raw_id_never_matches_typevar() {
        // A -1 sentinel key never matches a real TypeVar (raw_id >= 0),
        // so expand_by_instance with an unreadable typevar defers.
        let snap = snap_with_tvar("foo.Box", -1);
        assert_eq!(snap.type_var_raw_ids, vec![-1]);
        let typ = instance("foo.Box", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        let out = expand_type_inner(&typ, &env, false).unwrap();
        assert!(matches!(out, Type::Instance { ref args, .. } if matches!(
            args.as_slice(), [Type::TypeVarType { .. }])));
    }

    fn tuple_instance() -> Type {
        instance("builtins.tuple", vec![any()])
    }

    fn type_var_tuple(raw_id: i64) -> Type {
        Type::TypeVarTupleType {
            tuple_fallback: Box::new(tuple_instance()),
            name: "Ts".to_string(),
            fullname: "__main__.Ts".to_string(),
            raw_id,
            // Wire-decoded TypeVars carry the declaring class fullname as
            // namespace (types.py:554); expand_type_by_instance_core keys
            // its env with the instance's type_ref, which must line up.
            namespace: "foo.Pair".to_string(),
            upper_bound: Box::new(any()),
            default: Box::new(any()),
            min_len: 0,
        }
    }

    fn snap_with_tvt(
        fullname: &str,
        prefix: usize,
        suffix: usize,
        fallback: Option<Vec<u8>>,
        raw_ids: Vec<i64>,
        tvars: Vec<(String, i64, i64)>,
    ) -> TypeInfoSnapshot {
        TypeInfoSnapshot {
            fullname: fullname.to_owned(),
            name: fullname.to_owned(),
            has_type_var_tuple_type: true,
            type_var_tuple_prefix: Some(prefix),
            type_var_tuple_suffix: Some(suffix),
            type_var_tuple_fallback: fallback,
            type_var_raw_ids: raw_ids,
            type_vars_with_variance: tvars,
            ..Default::default()
        }
    }

    fn encode(t: &Type) -> Vec<u8> {
        let mut wbuf = WriteBuffer::new();
        write_type(&mut wbuf, t).unwrap();
        wbuf.into_bytes()
    }

    #[test]
    fn etbi_variadic_binds_tvt_to_tuple_type() {
        // tuple[A, *Ts, B] applied to tuple[int, str, float, bytes]:
        // prefix=1, suffix=1; middle args [str, float] bind *Ts as
        // Tuple[str, float] (expandtype.py:390-391).
        let fallback = encode(&tuple_instance());
        let snap = snap_with_tvt(
            "foo.Pair",
            1,
            1,
            Some(fallback),
            vec![10, 42, 11], // T, *Ts, B raw ids
            vec![
                ("T".to_string(), 0, 0),
                ("Ts".to_string(), 0, 2),
                ("B".to_string(), 0, 0),
            ],
        );
        let mut resolver = TypeResolver::new();
        resolver.insert("foo.Pair".to_string(), snap);
        // typ = tuple[Unpack[Ts]] referencing *Ts as a TypeVarTupleType
        // with raw_id 42, applied to the variadic Pair instance; the
        // expansion must replace *Ts with Tuple[str, float].
        let typ = Type::TupleType {
            partial_fallback: Box::new(tuple_instance()),
            items: vec![Type::UnpackType {
                typ: Box::new(type_var_tuple(42)),
            }],
            implicit: false,
        };
        let instance_typ = instance(
            "foo.Pair",
            vec![
                Type::LiteralType {
                    value: crate::wire::LiteralValue::Int(1),
                    fallback: Box::new(instance("builtins.int", vec![])),
                },
                Type::LiteralType {
                    value: crate::wire::LiteralValue::Str("a".to_string()),
                    fallback: Box::new(instance("builtins.str", vec![])),
                },
                Type::LiteralType {
                    value: crate::wire::LiteralValue::Str("b".to_string()),
                    fallback: Box::new(instance("builtins.str", vec![])),
                },
                Type::LiteralType {
                    value: crate::wire::LiteralValue::Int(2),
                    fallback: Box::new(instance("builtins.int", vec![])),
                },
            ],
        );
        let out = expand_type_by_instance_core(&typ, &instance_typ, &resolver, false)
            .expect("variadic expansion must not defer");
        match out {
            Type::TupleType { items, .. } => {
                // expand_unpack splices the middle items directly
                // (expandtype.py:382-390): *Ts = Tuple[str, float]
                // expands to [str, float].
                assert_eq!(items.len(), 2);
            }
            _ => panic!("expected TupleType with spliced middle"),
        }
    }

    #[test]
    fn etbi_variadic_missing_fallback_defers() {
        // No fallback -> cannot build the exact TupleType; must defer.
        let snap = snap_with_tvt(
            "foo.Pair",
            1,
            1,
            None,
            vec![10, 42, 11],
            vec![
                ("T".to_string(), 0, 0),
                ("Ts".to_string(), 0, 2),
                ("B".to_string(), 0, 0),
            ],
        );
        let mut resolver = TypeResolver::new();
        resolver.insert("foo.Pair".to_string(), snap);
        let typ = Type::TupleType {
            partial_fallback: Box::new(tuple_instance()),
            items: vec![Type::UnpackType {
                typ: Box::new(type_var_tuple(42)),
            }],
            implicit: false,
        };
        let instance_typ = instance("foo.Pair", vec![any(), any(), any(), any()]);
        assert!(expand_type_by_instance_core(&typ, &instance_typ, &resolver, false).is_none());
    }

    #[test]
    fn expand_type_var_tuple_any_builds_fallback() {
        // visit_type_var_tuple (expandtype.py:703-715): *Ts = Any
        // replaces the TypeVarTupleType node with
        // tuple_fallback[args=[Any]].
        let typ = type_var_tuple(7);
        let env: HashMap<EnvKey, Type> = HashMap::from([((7, 0, "foo.Pair".to_string()), any())]);
        let out = expand_type_inner(&typ, &env, false).unwrap();
        match out {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.tuple");
                assert!(matches!(args.as_slice(), [Type::AnyType { .. }]));
            }
            _ => panic!("expected tuple fallback Instance"),
        }
    }

    #[test]
    fn expand_type_var_tuple_uninhabited_builds_fallback() {
        // *Ts = Never replaces the node with
        // tuple_fallback[args=[UninhabitedType]].
        let typ = type_var_tuple(7);
        let env: HashMap<EnvKey, Type> = HashMap::from([(
            (7, 0, "foo.Pair".to_string()),
            Type::UninhabitedType { ambiguous: false },
        )]);
        let out = expand_type_inner(&typ, &env, false).unwrap();
        match out {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.tuple");
                assert!(matches!(args.as_slice(), [Type::UninhabitedType { .. }]));
            }
            _ => panic!("expected tuple fallback Instance"),
        }
    }

    #[test]
    fn expand_type_var_tuple_unbound_keeps_tvt() {
        // Unmatched *Ts stays a TypeVarTupleType copy (no deferral).
        let typ = type_var_tuple(7);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        let out = expand_type_inner(&typ, &env, false).unwrap();
        match out {
            Type::TypeVarTupleType { raw_id, .. } => assert_eq!(raw_id, 7),
            _ => panic!("expected TypeVarTupleType"),
        }
    }

    #[test]
    fn expand_type_var_tuple_tuple_binding_defers() {
        // A non-Any/non-Never binding (e.g. a TupleType) is not
        // representable here; Python raises NotImplementedError, so the
        // seam defers to the pure-Python caller.
        let typ = type_var_tuple(7);
        let tuple_repl = Type::TupleType {
            partial_fallback: Box::new(tuple_instance()),
            items: vec![any()],
            implicit: false,
        };
        let env: HashMap<EnvKey, Type> =
            HashMap::from([((7, 0, "foo.Pair".to_string()), tuple_repl)]);
        assert!(expand_type_inner(&typ, &env, false).is_none());
    }

    #[test]
    fn empty_env_typevar_free_succeeds_natively() {
        // `expand_type(List[int], {})` must not defer: an empty env makes no
        // substitution, the tree rebuilds, and no TypeVar survives. Regression
        // for the removed `env.is_empty()` eager bail in rust_expand_type.
        let typ = instance("builtins.list", vec![instance("builtins.int", vec![])]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        match expand_type_with_env(&typ, &env, false) {
            Some(Type::Instance { type_ref, args, .. }) => {
                assert_eq!(type_ref, "builtins.list");
                match args.as_slice() {
                    [Type::Instance { type_ref: name, .. }] => assert_eq!(name, "builtins.int"),
                    other => panic!("expected [int] arg, got {:?}", other),
                }
            }
            other => panic!(
                "typevar-free empty-env expansion must complete, deferred={:?}",
                other.is_none()
            ),
        }
    }

    #[test]
    fn empty_env_typevar_still_defers_for_identity() {
        // `expand_type(List[T], {})` leaves T unmatched. Python preserves the
        // original T by object identity; a wire clone would break that, so the
        // `result_has_typevar` guard still defers to Python.
        let typ = instance("builtins.list", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        assert!(expand_type_with_env(&typ, &env, false).is_none());
    }

    fn alias_type(type_ref: &str, args: Vec<Type>) -> Type {
        // Wire-decoded shape: alias=None is not carried, only args + type_ref
        // (the Python fixup re-links the live TypeAlias via the alias map).
        Type::TypeAliasType {
            args,
            type_ref: type_ref.to_string(),
        }
    }

    #[test]
    fn expand_alias_entry_substitutes_args_natively() {
        // `expand_type(X[int], {T: int})`: alias args expand in Rust
        // (mirrors visit_type_alias_type); the shim re-links the survivor.
        let typ = alias_type("foo.Alias", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::from([((0, 0, String::new()), any())]);
        match expand_with_env(&typ, &env, false, true) {
            Some(bytes) => {
                let out = decode_type(&bytes).unwrap();
                match out {
                    Type::TypeAliasType { args, type_ref } => {
                        assert_eq!(type_ref, "foo.Alias");
                        assert!(matches!(args.as_slice(), [Type::AnyType { .. }]));
                    }
                    other => panic!("expected TypeAliasType, got {:?}", other),
                }
            }
            None => panic!("alias-arg expansion must not defer"),
        }
    }

    #[test]
    fn expand_alias_entry_empty_args_passes_through() {
        // Bare alias reference `X` (no args): Python returns t unchanged;
        // the wire clone carries type_ref for the shim's alias fixup.
        let typ = alias_type("foo.Alias", vec![]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        match expand_with_env(&typ, &env, false, true) {
            Some(bytes) => match decode_type(&bytes).unwrap() {
                Type::TypeAliasType { args, type_ref } => {
                    assert!(args.is_empty());
                    assert_eq!(type_ref, "foo.Alias");
                }
                other => panic!("expected TypeAliasType, got {:?}", other),
            },
            None => panic!("bare alias pass-through must not defer"),
        }
    }

    #[test]
    fn expand_alias_entry_unmatched_tvar_expands_via_relink() {
        // `expand_type(X[T], {})` leaves T unmatched inside the alias args;
        // with relink_ok the FFI entry returns the expansion and the Python
        // shim relinks the decoded TypeVar onto the live original
        // (`wirefixup.resync_var_identities`, NativeExpandTypeEmptyEnvSuite).
        let typ = alias_type("foo.Alias", vec![tvar(0)]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        match expand_with_env(&typ, &env, false, true) {
            Some(bytes) => match decode_type(&bytes).unwrap() {
                Type::TypeAliasType { args, type_ref } => {
                    assert_eq!(type_ref, "foo.Alias");
                    assert!(matches!(args.as_slice(), [Type::TypeVarType { .. }]));
                }
                other => panic!("expected TypeAliasType, got {:?}", other),
            },
            None => panic!("relink entry must not defer on a leftover tvar"),
        }
        // The old relink-less wrapper (infer_variance / expand_variants
        // callers) keeps the identity defer.
        assert!(expand_type_with_env(&typ, &env, false).is_none());
    }

    #[test]
    fn expand_alias_survivor_defers_without_alias_ok() {
        // The shared wrapper (infer_variance / expand_variants callers)
        // keeps the old contract: a result still carrying a TypeAliasType
        // defers, because those callers have no resolve_aliases fixup.
        let typ = instance("builtins.list", vec![alias_type("foo.Alias", vec![])]);
        let env: HashMap<EnvKey, Type> = HashMap::new();
        assert!(expand_type_with_env(&typ, &env, false).is_none());
        // Same input via the alias_ok=true entry completes.
        assert!(expand_with_env(&typ, &env, false, true).is_some());
    }

    // --- alias-expanding union flatten (types.py:5057, issue #1203) ---

    use crate::aliases::{AliasTvar, TypeAliasSnapshot};

    fn union(items: Vec<Type>) -> Type {
        let can_be_true = items.iter().any(union_item_can_be_true);
        let can_be_false = items.iter().any(union_item_can_be_false);
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true,
            can_be_false,
        }
    }

    fn alias_snapshot(fullname: &str, target: &Type) -> TypeAliasSnapshot {
        TypeAliasSnapshot {
            fullname: fullname.to_owned(),
            target: encode(target),
            alias_tvars: vec![],
            tvar_tuple_index: None,
            no_args: false,
            python_3_12_type_alias: false,
        }
    }

    fn alias_tvar(raw_id: i64) -> AliasTvar {
        AliasTvar {
            name: "T".to_string(),
            raw_id,
            meta_level: 0,
            namespace: String::new(),
            is_type_var_tuple: false,
        }
    }

    fn with_alias_map(map: HashMap<String, TypeAliasSnapshot>, f: impl FnOnce()) {
        FLAT_ALIASES.with(|c| *c.borrow_mut() = Some(std::sync::Arc::new(map)));
        f();
        // Restore the unset state so test order cannot interfere.
        FLAT_ALIASES.with(|c| *c.borrow_mut() = None);
    }

    #[test]
    fn flatten_expands_top_level_alias_union_item() {
        // A = Union[int, str]: flatten([A, bytes]) -> [int, str, bytes].
        let mut map = HashMap::new();
        map.insert(
            "testmod.A".to_string(),
            alias_snapshot(
                "testmod.A",
                &union(vec![
                    instance("builtins.int", vec![]),
                    instance("builtins.str", vec![]),
                ]),
            ),
        );
        with_alias_map(map, || {
            let out = flatten_union_expanding_aliases(&[
                alias_type("testmod.A", vec![]),
                instance("builtins.bytes", vec![]),
            ])
            .unwrap();
            match out.as_slice() {
                [t1, t2, t3] => {
                    let refs: Vec<&str> = [t1, t2, t3]
                        .iter()
                        .map(|t| match t {
                            Type::Instance { type_ref, .. } => type_ref.as_str(),
                            other => panic!("expected Instance, got {:?}", other),
                        })
                        .collect();
                    assert_eq!(refs, ["builtins.int", "builtins.str", "builtins.bytes"]);
                }
                other => panic!("expected 3 flat items, got {:?}", other),
            }
        });
    }

    #[test]
    fn flatten_preserves_alias_non_union_item() {
        // A = list[int]: the expansion is not a union, so the ORIGINAL
        // alias node is appended (types.py:5098-5099).
        let mut map = HashMap::new();
        map.insert(
            "testmod.A".to_string(),
            alias_snapshot(
                "testmod.A",
                &instance("builtins.list", vec![instance("builtins.int", vec![])]),
            ),
        );
        with_alias_map(map, || {
            let out = flatten_union_expanding_aliases(&[alias_type("testmod.A", vec![])]).unwrap();
            match out.as_slice() {
                [Type::TypeAliasType { type_ref, args }] => {
                    assert_eq!(type_ref, "testmod.A");
                    assert!(args.is_empty());
                }
                other => panic!("expected the original alias node, got {:?}", other),
            }
        });
    }

    #[test]
    fn flatten_defers_on_tvt_alias() {
        // A `tvar_tuple_index` alias cannot be substituted exactly here:
        // Python splits the middle (split_with_prefix_and_suffix), the
        // kernel zips, so flatten defers (parity-safe).
        let mut snap = alias_snapshot(
            "testmod.Pair",
            &union(vec![
                instance("builtins.int", vec![]),
                instance("builtins.str", vec![]),
            ]),
        );
        snap.tvar_tuple_index = Some(0);
        let mut map = HashMap::new();
        map.insert("testmod.Pair".to_string(), snap);
        with_alias_map(map, || {
            assert!(
                flatten_union_expanding_aliases(&[alias_type("testmod.Pair", vec![])]).is_none()
            );
        });
    }

    #[test]
    fn flatten_defers_on_missing_snapshot() {
        with_alias_map(HashMap::new(), || {
            assert!(
                flatten_union_expanding_aliases(&[alias_type("testmod.Missing", vec![])]).is_none()
            );
        });
    }

    #[test]
    fn flatten_defers_on_alias_chain_cycle() {
        // A -> B -> A (mirrors get_proper_type's guardless loop; the
        // kernel's seen-walk defers).
        let mut map = HashMap::new();
        map.insert("testmod.A".to_string(), {
            let mut s = alias_snapshot("testmod.A", &alias_type("testmod.B", vec![]));
            s.fullname = "testmod.A".to_string();
            s
        });
        map.insert("testmod.B".to_string(), {
            let mut s = alias_snapshot("testmod.B", &alias_type("testmod.A", vec![]));
            s.fullname = "testmod.B".to_string();
            s
        });
        with_alias_map(map, || {
            assert!(flatten_union_expanding_aliases(&[alias_type("testmod.A", vec![])]).is_none());
        });
    }

    #[test]
    fn flatten_follows_chain_to_union() {
        // A = B, B = Union[int, str]: the chain ends on a union, so the
        // items are flattened.
        let mut map = HashMap::new();
        map.insert(
            "testmod.A".to_string(),
            alias_snapshot("testmod.A", &alias_type("testmod.B", vec![])),
        );
        map.insert(
            "testmod.B".to_string(),
            alias_snapshot(
                "testmod.B",
                &union(vec![
                    instance("builtins.int", vec![]),
                    instance("builtins.str", vec![]),
                ]),
            ),
        );
        with_alias_map(map, || {
            let out = flatten_union_expanding_aliases(&[alias_type("testmod.A", vec![])]).unwrap();
            assert_eq!(out.len(), 2);
        });
    }

    #[test]
    fn flatten_substitutes_alias_args_without_simplifying() {
        // A[T] = Union[list[T], str]: the target substitutes under the
        // InstantiateAliasVisitor contract, then recurses into the
        // result's union items.
        let target = union(vec![
            instance("builtins.list", vec![tvar(0)]),
            instance("builtins.str", vec![]),
        ]);
        let mut snap = alias_snapshot("testmod.A", &target);
        snap.alias_tvars = vec![alias_tvar(0)];
        let mut map = HashMap::new();
        map.insert("testmod.A".to_string(), snap);
        with_alias_map(map, || {
            let out = flatten_union_expanding_aliases(&[alias_type(
                "testmod.A",
                vec![instance("builtins.int", vec![])],
            )])
            .unwrap();
            match out.as_slice() {
                [t1, t2] => {
                    assert!(matches!(t1, Type::Instance { type_ref, args, .. }
                        if type_ref == "builtins.list"));
                    if let Type::Instance { args, .. } = t1 {
                        assert!(matches!(args.as_slice(),
                            [Type::Instance { type_ref: ir, .. }] if ir == "builtins.int"));
                    }
                    assert!(
                        matches!(t2, Type::Instance { type_ref, .. } if type_ref == "builtins.str")
                    );
                }
                other => panic!("expected substituted union items, got {:?}", other),
            }
            // The nested substitution must not have leaked INSTANTIATE.
            assert!(!INSTANTIATE.get());
        });
    }

    #[test]
    fn instantiate_mode_rebuilds_plain_union() {
        // InstantiateAliasVisitor.visit_union_type (types.py:5513-5525):
        // no flatten, no simplify; structure and truthiness flags only.
        let typ = union(vec![
            union(vec![
                instance("builtins.int", vec![]),
                instance("builtins.str", vec![]),
            ]),
            instance("builtins.bytes", vec![]),
        ]);
        INSTANTIATE.with(|c| c.set(true));
        let out = expand_type_inner(&typ, &HashMap::new(), false).unwrap();
        INSTANTIATE.with(|c| c.set(false));
        match out {
            Type::UnionType {
                items,
                can_be_true,
                can_be_false,
                ..
            } => {
                assert_eq!(items.len(), 2); // nested union preserved
                assert!(matches!(items[0], Type::UnionType { .. }));
                assert!(can_be_true);
                assert!(can_be_false);
            }
            other => panic!("expected plain rebuilt union, got {:?}", other),
        }
        // Normal mode flattens the same input to 3 items.
        let out = expand_type_inner(&typ, &HashMap::new(), false).unwrap();
        match out {
            Type::UnionType { items, .. } => assert_eq!(items.len(), 3),
            other => panic!("expected flattened union, got {:?}", other),
        }
    }

    #[test]
    fn union_arm_defers_on_alias_without_alias_map() {
        // TLS unset (non-FFI callers without the guard): an alias item
        // keeps the pre-1203 defer.
        let typ = union(vec![
            alias_type("testmod.A", vec![]),
            instance("builtins.int", vec![]),
        ]);
        assert!(expand_type_inner(&typ, &HashMap::new(), false).is_none());
    }

    // --- expand_type_by_instance relink entry alias round-trip (#1289) ---

    fn class_tvar(raw_id: i64, namespace_str: &str) -> Type {
        // A class typevar: its env key carries the declaring class fullname
        // as namespace (expandtype.py:554 / by_instance_inner env_ns).
        let mut t = tvar(raw_id);
        if let Type::TypeVarType { namespace, .. } = &mut t {
            *namespace = namespace_str.to_string();
        }
        t
    }

    #[test]
    fn ebi_relink_expands_alias_input_natively() {
        // List[X[T]] applied to Box[int]: the alias arg expands in Rust and
        // the surviving alias node carries type_ref for the shim's
        // resolve_aliases fixup. The old alias-input gate is gone.
        let mut resolver = TypeResolver::new();
        resolver.insert("foo.Box".to_string(), snap_with_tvar("foo.Box", 0));
        let typ = instance(
            "builtins.list",
            vec![alias_type("testmod.X", vec![class_tvar(0, "foo.Box")])],
        );
        let inst = instance("foo.Box", vec![instance("builtins.int", vec![])]);
        let out = expand_type_by_instance_relink(&typ, &inst, &resolver, false)
            .expect("alias-bearing by-instance expansion must not defer");
        match out {
            Type::Instance { type_ref, args, .. } => {
                assert_eq!(type_ref, "builtins.list");
                match args.as_slice() {
                    [Type::TypeAliasType {
                        args: alias_args,
                        type_ref,
                    }] => {
                        // The tvar env key is (0, meta 0, "foo.Box"), so the
                        // written alias arg expands from T to int in Rust.
                        assert_eq!(type_ref, "testmod.X");
                        assert!(matches!(
                            alias_args.as_slice(),
                            [Type::Instance { type_ref, .. }] if type_ref == "builtins.int"
                        ));
                    }
                    other => panic!("expected [X[int]] alias arg, got {:?}", other),
                }
            }
            other => panic!("expected Instance, got {:?}", other),
        }
    }

    #[test]
    fn ebi_core_still_defers_on_alias_input() {
        // Internal callers (core/free variants) keep the old contract: an
        // alias-bearing input defers because they have no resolve_aliases
        // fixup on their Python side.
        let mut resolver = TypeResolver::new();
        resolver.insert("foo.Box".to_string(), snap_with_tvar("foo.Box", 0));
        let typ = instance(
            "builtins.list",
            vec![alias_type("testmod.X", vec![class_tvar(0, "foo.Box")])],
        );
        let inst = instance("foo.Box", vec![instance("builtins.int", vec![])]);
        assert!(expand_type_by_instance_core(&typ, &inst, &resolver, false).is_none());
    }

    #[test]
    fn ebi_relink_flattens_alias_union_item() {
        // union[X[T], int] with X = list[T], applied to Box[int]: the FFI
        // entry installs FLAT_ALIASES, so the union flatten resolves the
        // chain (issue #1203 path) instead of deferring the whole call.
        let mut resolver = TypeResolver::new();
        resolver.insert("foo.Box".to_string(), snap_with_tvar("foo.Box", 0));
        let mut map = HashMap::new();
        map.insert(
            "testmod.X".to_string(),
            alias_snapshot(
                "testmod.X",
                &instance("builtins.list", vec![instance("builtins.int", vec![])]),
            ),
        );
        let typ = union(vec![
            alias_type("testmod.X", vec![class_tvar(0, "foo.Box")]),
            instance("builtins.bytes", vec![]),
        ]);
        let inst = instance("foo.Box", vec![instance("builtins.int", vec![])]);
        with_alias_map(map, || {
            let out = expand_type_by_instance_relink(&typ, &inst, &resolver, false)
                .expect("alias union item must flatten, not defer");
            match out {
                Type::UnionType { items, .. } => {
                    assert_eq!(items.len(), 2);
                    assert!(matches!(items[0], Type::TypeAliasType { .. }));
                }
                other => panic!("expected UnionType, got {:?}", other),
            }
        });
    }

    #[test]
    fn ebi_relink_defers_on_alias_chain_cycle() {
        // A -> B -> A: the chain walk defers (parity-safe), the whole
        // relink call falls back to Python.
        let mut resolver = TypeResolver::new();
        resolver.insert("foo.Box".to_string(), snap_with_tvar("foo.Box", 0));
        let mut map = HashMap::new();
        map.insert(
            "testmod.A".to_string(),
            alias_snapshot("testmod.A", &alias_type("testmod.B", vec![])),
        );
        map.insert(
            "testmod.B".to_string(),
            alias_snapshot("testmod.B", &alias_type("testmod.A", vec![])),
        );
        let typ = union(vec![alias_type("testmod.A", vec![])]);
        let inst = instance("foo.Box", vec![instance("builtins.int", vec![])]);
        with_alias_map(map, || {
            assert!(expand_type_by_instance_relink(&typ, &inst, &resolver, false).is_none());
        });
    }
}
