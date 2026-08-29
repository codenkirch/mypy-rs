//! Stage 3c (M8a): `TypeAlias` snapshot protocol.
//!
//! Mirrors `mypy.nodes.TypeAlias` (nodes.py:4489). The wire format's
//! `Type::TypeAliasType` (wire.rs:483) carries only `args` + `type_ref`
//! (the unresolved `alias.fullname`); it does NOT carry the alias's
//! `target`, `alias_tvars`, `tvar_tuple_index`, or `no_args`. Stage 3c
//! `get_proper_type` / `is_subtype` need the target to expand the alias,
//! so this module snapshots the live `TypeAlias` node by `fullname`,
//! keyed alongside the `TypeResolver` (typeinfo.rs).
//!
//! Like `TypeInfoSnapshot`, this is a frozen view: mutable scratch fields
//! (`_is_recursive` cache, `default_depends`) are NOT snapshotted.
//! `_is_recursive` is computed lazily by `TypeAliasType._expand_once`
//! and stays Python-side; Stage 3c's `is_subtype` falls through to
//! Python for alias expansion (M8b returns `None` for `TypeAliasType`).

use std::collections::HashMap;

/// Frozen snapshot of a `mypy.nodes.TypeAlias`, keyed by `fullname`.
///
/// Field set is the union of Stage 3c `is_subtype` consumers:
/// `target` (expand the alias), `alias_tvars` (declared typevar
/// identities, for arg-position substitution), `tvar_tuple_index`
/// (variadic alias dispatch), `no_args` (`A = List` vs `A = List[Any]`
/// distinction, nodes.py:4560).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct TypeAliasSnapshot {
    /// `TypeAlias._fullname` (nodes.py:4609). Equals the `type_ref` key.
    pub fullname: String,
    /// `TypeAlias.target` serialized as a wire-format `Type` blob
    /// (nodes.py:4611). Stage 3c decodes via `wire::read_type`.
    pub target: Vec<u8>,
    /// `TypeAlias.alias_tvars` in declaration order (nodes.py:4614).
    /// Each entry carries the declared typevar's identity so a
    /// substitution env can be built by zipping with the alias `args`.
    pub alias_tvars: Vec<AliasTvar>,
    /// `TypeAlias.tvar_tuple_index` (nodes.py:4622). `None` if the
    /// alias has no `TypeVarTupleType` in `alias_tvars`.
    pub tvar_tuple_index: Option<usize>,
    /// `TypeAlias.no_args` (nodes.py:4615). Distinguishes `A = List`
    /// (no_args=True, no arg substitution) from `A = List[Any]`
    /// (no_args=False).
    pub no_args: bool,
    /// `TypeAlias.python_3_12_type_alias` (nodes.py:4616). The
    /// `BoolTypeQuery` alias handler visits `t.args` only for new-style
    /// (PEP 695) aliases (type_visitor.py:614).
    pub python_3_12_type_alias: bool,
}

/// Identity of one declared type variable of a `TypeAlias` (an element
/// of `TypeAlias.alias_tvars`). Mirrors `TypeVarId` equality
/// (types.py:574-576): `(raw_id, meta_level, namespace)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AliasTvar {
    /// `TypeVarLikeType.name` (nodes.py:4614), e.g. `"T"`.
    pub name: String,
    /// `TypeVarLikeType.id.raw_id`.
    pub raw_id: i64,
    /// `TypeVarLikeType.id.meta_level`.
    pub meta_level: i64,
    /// `TypeVarLikeType.id.namespace`.
    pub namespace: String,
    /// Whether this is a `TypeVarTupleType` (vs TypeVar / ParamSpec).
    pub is_type_var_tuple: bool,
}

#[allow(dead_code)]
impl TypeAliasSnapshot {
    pub fn has_tvar_tuple(&self) -> bool {
        self.tvar_tuple_index.is_some()
    }
}

/// Resolver: maps `TypeAlias.fullname` (the `type_ref` string on
/// `Type::TypeAliasType`) to a snapshot. Built once per type-checking
/// pass from the live Python symbol table. Lookups are `O(1)` HashMap.
/// Inserts are rare (first-seal-wins, per build-manager snapshot pass),
/// so the cached `shared()` view is rebuilt at most once per pass.
/// Single-threaded access: this is a build-local snapshot, owned by the
/// pyclass, so the `RefCell` is safe.
#[allow(dead_code)]
pub(crate) struct TypeAliasResolver {
    snapshots: HashMap<String, TypeAliasSnapshot>,
    /// Cheap shareable full-map view for the expand kernel's TLS install
    /// (expandtype.rs): a clone-at-most-once cache, dropped by `insert`
    /// so a later install sees the grown map.
    shared: std::cell::RefCell<Option<std::sync::Arc<HashMap<String, TypeAliasSnapshot>>>>,
}

#[allow(dead_code)]
impl TypeAliasResolver {
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            shared: std::cell::RefCell::new(None),
        }
    }

    pub fn insert(&mut self, fullname: String, snap: TypeAliasSnapshot) {
        self.snapshots.insert(fullname, snap);
        // Any snapshot change must invalidate the frozen view.
        *self.shared.borrow_mut() = None;
    }

    pub fn get(&self, fullname: &str) -> Option<&TypeAliasSnapshot> {
        self.snapshots.get(fullname)
    }

    /// Full map as a cheap-to-clone `Arc` (built at most once between
    /// inserts). The map is frozen between build-manager snapshot passes,
    /// so per-call installs in the expand kernel pay one refcount bump.
    pub(crate) fn shared(&self) -> std::sync::Arc<HashMap<String, TypeAliasSnapshot>> {
        let mut slot = self.shared.borrow_mut();
        slot.get_or_insert_with(|| std::sync::Arc::new(self.snapshots.clone()))
            .clone()
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

/// Read-only alias snapshot lookup. Lets the expand kernel's TLS hold a
/// cheap `Arc<HashMap<..>>` view while the snapshot helpers keep taking a
/// resolver-shaped reference at their existing call sites.
pub(crate) trait AliasLookup {
    fn get(&self, fullname: &str) -> Option<&TypeAliasSnapshot>;
}

impl AliasLookup for TypeAliasResolver {
    fn get(&self, fullname: &str) -> Option<&TypeAliasSnapshot> {
        self.snapshots.get(fullname)
    }
}

impl AliasLookup for std::sync::Arc<HashMap<String, TypeAliasSnapshot>> {
    fn get(&self, fullname: &str) -> Option<&TypeAliasSnapshot> {
        (**self).get(fullname)
    }
}

impl Default for TypeAliasResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(fullname: &str, target: Vec<u8>) -> TypeAliasSnapshot {
        TypeAliasSnapshot {
            fullname: fullname.to_owned(),
            target,
            ..Default::default()
        }
    }

    #[test]
    fn alias_snapshot_default_is_empty() {
        let s = TypeAliasSnapshot::default();
        assert!(s.fullname.is_empty());
        assert!(s.target.is_empty());
        assert!(s.alias_tvars.is_empty());
        assert!(s.tvar_tuple_index.is_none());
        assert!(!s.no_args);
        assert!(!s.has_tvar_tuple());
    }

    #[test]
    fn alias_resolver_get_returns_inserted_snapshot() {
        let mut r = TypeAliasResolver::new();
        assert!(r.is_empty());
        r.insert(
            "typing.List".to_string(),
            snap("typing.List", vec![1, 2, 3]),
        );
        assert_eq!(r.len(), 1);
        let got = r.get("typing.List").expect("alias must be present");
        assert_eq!(got.fullname, "typing.List");
        assert_eq!(got.target, vec![1, 2, 3]);
        assert!(r.get("typing.Dict").is_none());
    }

    #[test]
    fn alias_resolver_len_and_is_empty() {
        let mut r = TypeAliasResolver::new();
        assert!(r.is_empty());
        r.insert("a".to_string(), snap("a", Vec::new()));
        r.insert("b".to_string(), snap("b", Vec::new()));
        assert_eq!(r.len(), 2);
        assert!(!r.is_empty());
    }

    #[test]
    fn alias_snapshot_has_tvar_tuple_true_when_index_set() {
        let mut s = snap("typing.Ts", Vec::new());
        s.tvar_tuple_index = Some(0);
        assert!(s.has_tvar_tuple());
    }

    #[test]
    fn alias_snapshot_has_tvar_tuple_false_when_index_none() {
        let s = snap("typing.List", Vec::new());
        assert!(!s.has_tvar_tuple());
    }
}
