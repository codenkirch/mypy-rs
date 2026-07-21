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

use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::PyObject;

use crate::typeinfo::serialize_type_to_bytes;

/// Frozen snapshot of a `mypy.nodes.TypeAlias`, keyed by `fullname`.
///
/// Field set is the union of Stage 3c `is_subtype` consumers:
/// `target` (expand the alias), `alias_tvars` (name list for
/// arg-position dispatch), `tvar_tuple_index` (variadic alias dispatch),
/// `no_args` (`A = List` vs `A = List[Any]` distinction, nodes.py:4560).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct TypeAliasSnapshot {
    /// `TypeAlias._fullname` (nodes.py:4609). Equals the `type_ref` key.
    pub fullname: String,
    /// `TypeAlias.target` serialized as a wire-format `Type` blob
    /// (nodes.py:4611). Stage 3c decodes via `wire::read_type`.
    pub target: Vec<u8>,
    /// `TypeAlias.alias_tvars` as a list of names (nodes.py:4614).
    /// Each entry is a `TypeVarLikeType.name`; the kind (TypeVar vs
    /// ParamSpec vs TypeVarTuple) is not needed for the M8b nominal
    /// path, which returns `None` for `TypeAliasType`.
    pub alias_tvars: Vec<String>,
    /// `TypeAlias.tvar_tuple_index` (nodes.py:4622). `None` if the
    /// alias has no `TypeVarTupleType` in `alias_tvars`.
    pub tvar_tuple_index: Option<usize>,
    /// `TypeAlias.no_args` (nodes.py:4615). Distinguishes `A = List`
    /// (no_args=True, no arg substitution) from `A = List[Any]`
    /// (no_args=False).
    pub no_args: bool,
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
#[allow(dead_code)]
pub(crate) struct TypeAliasResolver {
    snapshots: HashMap<String, TypeAliasSnapshot>,
}

#[allow(dead_code)]
impl TypeAliasResolver {
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
        }
    }

    pub fn insert(&mut self, fullname: String, snap: TypeAliasSnapshot) {
        self.snapshots.insert(fullname, snap);
    }

    pub fn get(&self, fullname: &str) -> Option<&TypeAliasSnapshot> {
        self.snapshots.get(fullname)
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }
}

impl Default for TypeAliasResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Read `TypeAlias.alias_tvars` (a `list[TypeVarLikeType]`) as a Vec
/// of the tvar names. Skips entries whose `.name` is unreadable.
fn read_alias_tvar_names(obj: &PyAny) -> Vec<String> {
    let tvars = match obj.getattr("alias_tvars") {
        Ok(t) => match t.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::with_capacity(tvars.len());
    for item in tvars.iter() {
        if let Ok(n) = item.getattr("name").and_then(|n| n.extract::<String>()) {
            out.push(n);
        }
    }
    out
}

/// Read `TypeAlias.tvar_tuple_index` as `Option<usize>`. `None` when
/// the alias has no variadic tvar (nodes.py:4622).
fn read_tvar_tuple_index(obj: &PyAny) -> Option<usize> {
    let v = obj.getattr("tvar_tuple_index").ok()?;
    if v.is_none() {
        return None;
    }
    v.extract::<usize>().ok()
}

/// Build a `TypeAliasResolver` (Python `dict[str, dict]`) from an
/// iterable of live `mypy.nodes.TypeAlias` objects.
///
/// Each alias is read into a snapshot-fields dict. On any per-item read
/// failure (missing `fullname`, unserializable `target`) the item is
/// skipped, mirroring the strangler-fig degrade-gracefully pattern from
/// `typeinfo::build_resolver`.
///
/// The returned dict is consumed by `NativeTypeResolver` (typeinfo.rs),
/// which holds both the `TypeResolver` and `TypeAliasResolver` HashMaps
/// in Rust for zero-FFI-per-lookup access by Stage 3c.
#[pyfunction]
pub(crate) fn build_alias_resolver(py: Python<'_>, aliases: &PyAny) -> PyResult<PyObject> {
    let result = pyo3::types::PyDict::new(py);
    for item in aliases.iter()? {
        let item = item?;
        let fullname: String = match item.getattr("fullname").and_then(|f| f.extract()) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let target_bytes = match serialize_type_to_bytes(py, item) {
            Some(b) => b,
            None => continue,
        };
        let snap_dict = pyo3::types::PyDict::new(py);
        snap_dict.set_item("fullname", &fullname)?;
        snap_dict.set_item("target", pyo3::types::PyBytes::new(py, &target_bytes))?;
        let tvar_names = read_alias_tvar_names(item);
        snap_dict.set_item("alias_tvars", PyList::new(py, &tvar_names))?;
        match read_tvar_tuple_index(item) {
            Some(i) => snap_dict.set_item("tvar_tuple_index", i)?,
            None => snap_dict.set_item("tvar_tuple_index", py.None())?,
        }
        let no_args: bool = item
            .getattr("no_args")
            .ok()
            .and_then(|v| v.extract().ok())
            .unwrap_or(false);
        snap_dict.set_item("no_args", no_args)?;
        result.set_item(fullname, snap_dict)?;
    }
    Ok(result.into())
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
