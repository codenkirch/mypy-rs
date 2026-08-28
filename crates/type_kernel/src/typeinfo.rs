//! Stage 3b: TypeInfo snapshot protocol.
//!
//! Resolves `type_ref` (the unresolved `type.fullname` carried by
//! `Type::Instance` / `Type::TypeAliasType` in the wire format) into a
//! frozen `TypeInfoSnapshot` carrying the metadata the Stage 3a `Display`
//! impl needs for production-correct rendering (prefix-strip, enum-literal,
//! bytes-literal, the `[()]` variadic-tuple branch) and the future Stage 3c
//! `is_subtype` needs (mro, protocol_members, promote, etc.).
//!
//! Mirrors `mypy.nodes.TypeInfo` (nodes.py:3623). Mutable scratch fields
//! (`assuming`, `assuming_proper`, `inferring`, `metadata`) are NOT
//! snapshotted; they remain Python-side as a recursion-guard sidecar.
//!
//! Parity contract for the Stage 3b consumer:
//!   `str(python_type) == read_type_to_str_with_resolver(bytes, resolver)`
//! over the `TypeFixture` corpus (see `NativeTypeWireSuite` in
//! `testtypes.py`).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyList, PySet};

use crate::wire::{self, LiteralValue, ReadBuffer, Type};

/// Frozen snapshot of a `mypy.nodes.TypeInfo`, keyed by `fullname`.
///
/// Field set is the union of (a) Stage 3b rendering consumers and
/// (b) Stage 3c `is_subtype` consumers, so the struct does not need to be
/// reshaped when Stage 3c lands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct TypeInfoSnapshot {
    /// `TypeInfo._fullname` (nodes.py:3939). Equals the `type_ref` key.
    pub fullname: String,
    /// `TypeInfo.name` = `defn.name` (nodes.py:3934). Short class name.
    pub name: String,
    /// `TypeInfo.is_protocol` (nodes.py:3705). subtypes.py:536,627,1252.
    pub is_protocol: bool,
    /// `TypeInfo.is_enum` (nodes.py:3753). subtypes.py:339,500; value_repr:3368.
    pub is_enum: bool,
    /// `TypeInfo.enum_members` (nodes.py:3977, @property list[str]).
    /// typeops.py:1144 uses it for enum literal contraction in
    /// `try_contracting_literals_in_union`. Empty when `is_enum` is
    /// false (the property returns [] for non-enum types).
    pub enum_members: Vec<String>,
    /// `TypeInfo.fallback_to_any` (nodes.py:3759). subtypes.py:493,1494.
    pub fallback_to_any: bool,
    /// `TypeInfo.meta_fallback_to_any` (nodes.py:3763). subtypes.py:1494.
    pub meta_fallback_to_any: bool,
    /// `TypeInfo.is_named_tuple` (nodes.py:3800). subtypes.py:559.
    pub is_named_tuple: bool,
    /// `TypeInfo.is_newtype` (nodes.py:3806). checker.py conditional_types
    /// unwraps NewTypes to `type.bases[0]` before narrowing.
    pub is_newtype: bool,
    /// `TypeInfo.has_type_var_tuple_type` (nodes.py:3921). Display `[()]`.
    pub has_type_var_tuple_type: bool,
    /// `TypeInfo.has_param_spec_type` (nodes.py:3830). typeanal.py:1201
    /// uses it for the ParamSpec pack step, and validates a single ParamSpec
    /// with exactly one argument.
    pub has_param_spec_type: bool,
    /// `TypeInfo.is_abstract` (nodes.py:3704). checkexpr hot path.
    pub is_abstract: bool,
    /// `TypeInfo.type_vars` (nodes.py:3768, list[str]). subtypes.py:1358.
    pub type_vars: Vec<String>,
    /// `TypeInfo.mro` as fullname strings (nodes.py:3692). subtypes.py:537.
    pub mro: Vec<String>,
    /// `TypeInfo.protocol_members` (nodes.py:3959, @property). subtypes.py:471.
    pub protocol_members: Vec<String>,
    /// Precomputed `has_base(name)` set: fullnames of all entries in mro.
    /// subtypes.py:511,527,555.
    pub has_base: HashSet<String>,
    /// `TypeInfo._promote` serialized as bytes. subtypes.py:538-539. Each
    /// element is a wire-format Type blob; Stage 3c decodes via
    /// `wire::read_type`. Rebuilt per pass, so staleness is bounded.
    pub promote_bytes: Vec<Vec<u8>>,
    /// `TypeInfo.alt_promote` fullname, if any (nodes.py:3790). subtypes.py:546.
    pub alt_promote_fullname: Option<String>,
    /// `TypeInfo.metaclass_type` fullname, if any (nodes.py:3701).
    /// subtypes.py:1195,1433.
    pub metaclass_fullname: Option<String>,
    /// `TypeInfo.bases` serialized as wire-format `Instance` blobs
    /// (nodes.py:3880). Each element is a `Type::Instance` blob; Stage 3c
    /// decodes via `wire::read_type` for `map_instance_to_supertype`.
    /// Mirrors the promote_bytes pattern.
    pub bases: Vec<Vec<u8>>,
    /// `TypeInfo.tuple_type` serialized as a wire-format `TupleType` blob,
    /// or `None` (nodes.py:3905). maptype.py:78 special-cases
    /// `builtins.tuple` bases when set.
    pub tuple_type: Option<Vec<u8>>,
    /// `TypeInfo.type_var_tuple_prefix` (nodes.py:3895). subtypes.py:572.
    pub type_var_tuple_prefix: Option<usize>,
    /// `TypeInfo.type_var_tuple_suffix` (nodes.py:3896). subtypes.py:575.
    pub type_var_tuple_suffix: Option<usize>,
    /// `TypeVarTupleType.tuple_fallback` of the class's variadic tvar,
    /// serialized as a wire-format `Instance` blob (types.py:991-1001),
    /// or `None` when the class is not variadic / the fallback is
    /// unreadable. expandtype.rs uses it to build the TupleType that
    /// binds a TypeVarTuple in `expand_type_by_instance`
    /// (expandtype.py:390: `TupleType(list(args_middle),
    /// tvar.tuple_fallback)`).
    pub type_var_tuple_fallback: Option<Vec<u8>>,
    /// `(name, variance, kind)` for each `defn.type_vars` entry.
    /// variance: 0=INVARIANT, 1=COVARIANT, 2=CONTRAVARIANT,
    /// 3=VARIANCE_NOT_READY (nodes.py:3146). kind: 0=TypeVarType,
    /// 1=ParamSpecType, 2=TypeVarTupleType. Stage 3c dispatches
    /// `check_type_parameter` on (variance, kind).
    pub type_vars_with_variance: Vec<(String, i64, i64)>,
    /// Serialized `TypeVarType.upper_bound` for each TypeVar (parallel
    /// to `type_vars_with_variance`). Stage 3c checks
    /// `is_subtype(new_type, upper_bound)` in the covariant branch
    /// (subtypes.py:612-619). Empty blob for ParamSpec/TypeVarTuple.
    pub type_var_upper_bounds: Vec<Vec<u8>>,
    /// `TypeVarId.raw_id` per `defn.type_vars` entry, parallel to
    /// `type_vars_with_variance`. expandtype.rs uses it to build the env
    /// of `expand_type_by_instance` (class type vars bind
    /// `TypeVarId(raw_id, meta_level=0)`). -1 sentinel when the attribute
    /// is unreadable; a -1 key never matches a real raw_id (>= 0), so the
    /// result keeps a TypeVar and defers to Python.
    pub type_var_raw_ids: Vec<i64>,
    /// `TypeInfo.names` read as `name -> (implicit, has_explicit_value)`.
    /// `implicit` is `SymbolTableNode.implicit`; `has_explicit_value` is
    /// `Var.has_explicit_value` when the node is a Var (false for non-Var
    /// nodes and unreadable attributes). Feeds the M20 checkmember kernel
    /// (`has_operator`, `meta_has_operator`, `defined_in_superclass`).
    pub member_info: HashMap<String, (bool, bool)>,
    /// Per-member node kind + definer fullname, for `custom_special_method`
    /// (typeops.py:1555). Keyed by member name. `node_kind`: 0=FuncBase
    /// (FuncDef/OverloadedFuncDef), 1=Decorator, 2=Var, -1=other/unknown.
    /// `definer_fullname` is `node.info.fullname` (where the member was
    /// defined); `custom_special_method` returns False when it starts with
    /// `builtins.` or `typing.`. Empty for members that are not
    /// FuncBase/Decorator/Var, or when unreadable.
    pub member_definers: HashMap<String, (i64, String)>,
}

#[allow(dead_code)]
impl TypeInfoSnapshot {
    /// `TypeInfo.has_base(name)`: true iff `name` is in the precomputed set.
    pub fn has_base(&self, name: &str) -> bool {
        self.has_base.contains(name)
    }

    /// Whether this TypeInfo lives under `builtins.*` (for the Display
    /// prefix-strip in `TypeStrVisitor.visit_instance`).
    pub fn is_builtins(&self) -> bool {
        self.fullname.starts_with("builtins.")
    }
}

/// Resolver: maps `type.fullname` (the `type_ref` string) to a snapshot.
///
/// Built once per type-checking pass by reading the live Python TypeInfo
/// graph via PyO3. Lookups are `O(1)` HashMap. The future Stage 3c
/// `is_subtype` calls `resolver.get(type_ref)` per Instance.
/// Frozen snapshot of a `mypy.nodes.MypyFile`'s symbol table, keyed by
/// module fullname. Backs the MypyFile branch of `rust_lookup_qualified`
/// (semanal_lookup.rs): direct name hits in `module.names`.
///
/// For each name in `node.names` we capture `module_hidden` plus the
/// node kind + fullname when the node is a `MypyFile`. The fullname is
/// what Python's `get_module_symbol` reaches through a submodule chain:
/// Rust must descend into the exact module the symbol names, not a guess
/// from name joining (a name can alias a different module, or be a
/// non-module while a same-named module exists).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct ModuleSnapshot {
    /// `name -> (module_hidden, Optional<(is_module, module_fullname)>)`.
    /// `module_fullname` is present only when the symbol's node is a
    /// `MypyFile`; it is the node's `fullname` (what Python descends
    /// into), which may differ from `module + "." + name`.
    pub symbols: HashMap<String, (bool, Option<(bool, String)>)>,
}

impl ModuleSnapshot {
    /// Whether `name` is a direct hit and not hidden.
    pub fn visible(&self, name: &str) -> Option<bool> {
        self.symbols.get(name).map(|(hidden, _)| !*hidden)
    }

    /// The module fullname this symbol descends into, when its node is a
    /// `MypyFile`. `None` when the symbol is absent, hidden, or not a
    /// module.
    pub fn module_fullname(&self, name: &str) -> Option<&str> {
        let (hidden, node) = self.symbols.get(name)?;
        if *hidden {
            return None;
        }
        let (is_module, fullname) = node.as_ref()?;
        if !*is_module {
            return None;
        }
        Some(fullname)
    }
}

#[allow(dead_code)]
pub(crate) struct TypeResolver {
    snapshots: HashMap<String, TypeInfoSnapshot>,
    /// `fullname -> ModuleSnapshot` for loaded modules. Populated from
    /// `BuildManager.modules`; mirrors the `module.names` SymbolTable.
    modules: HashMap<String, ModuleSnapshot>,
    /// Live `fullname -> TypeInfo` map, moved here from
    /// `NativeTypeResolver` so the subtype engine (which only holds a
    /// `&TypeResolver`) can reach live TypeInfos for the protocol-right
    /// port (member flags, `record_protocol_subtype_check`). Populated
    /// via `set_live_typeinfo_map` from `BuildManager._native_typeinfo_map`.
    /// `None` until set (engine protocol-right defers without it).
    live_info_map: Option<PyObject>,
}

#[allow(dead_code)]
impl TypeResolver {
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            modules: HashMap::new(),
            live_info_map: None,
        }
    }

    /// Whether a live TypeInfo map is installed (engine protocol-right
    /// sites defer without one, keeping pure-Rust tests interpreter-free).
    pub(crate) fn has_live_info_map(&self) -> bool {
        self.live_info_map.is_some()
    }

    /// Look up a live `TypeInfo` (as `&PyAny`) by fullname from the
    /// `live_info_map` installed by `set_live_typeinfo_map`. `None` when no
    /// map is installed or the fullname is absent. Used by enum-member reads
    /// that need current (non-snapshot) data, and by the protocol-right
    /// subtype site.
    pub(crate) fn live_typeinfo<'py>(
        &'py self,
        py: Python<'py>,
        fullname: &str,
    ) -> Option<&'py PyAny> {
        let map = self.live_info_map.as_ref()?;
        let dict = map.as_ref(py).downcast::<PyDict>().ok()?;
        dict.get_item(fullname).ok()?
    }

    pub fn insert(&mut self, fullname: String, snap: TypeInfoSnapshot) {
        self.snapshots.insert(fullname, snap);
    }

    pub fn get(&self, fullname: &str) -> Option<&TypeInfoSnapshot> {
        self.snapshots.get(fullname)
    }

    pub fn insert_module(&mut self, fullname: String, snap: ModuleSnapshot) {
        self.modules.insert(fullname, snap);
    }

    pub fn get_module(&self, fullname: &str) -> Option<&ModuleSnapshot> {
        self.modules.get(fullname)
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    /// Iterate over all `(fullname, snapshot)` pairs. Used by
    /// `NativeTypeResolver::render_dict` to build the lazy dict view.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &TypeInfoSnapshot)> {
        self.snapshots.iter()
    }
}

impl Default for TypeResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Read a `bool` attribute from a Python `TypeInfo` object, or `None` on
/// any read failure (so a partially-constructed TypeInfo does not fail the
/// whole resolver build).
pub(crate) fn read_bool_attr(obj: &PyAny, attr: &str) -> Option<bool> {
    obj.getattr(attr)
        .and_then(|v| {
            if let Ok(b) = v.extract::<bool>() {
                Ok(b)
            } else if let Ok(b) = v.downcast::<PyBool>() {
                Ok(b.is_true())
            } else {
                Err(pyo3::PyErr::fetch(v.py()))
            }
        })
        .ok()
}

/// Read a `str` attribute, or `None` on failure.
fn read_str_attr(obj: &PyAny, attr: &str) -> Option<String> {
    obj.getattr(attr).and_then(|v| v.extract::<String>()).ok()
}

/// Read an `Option[Instance]` attribute as the Instance's `type.fullname`
/// string, or `None` if the attribute is `None` or unreadable.
fn read_opt_instance_fullname(obj: &PyAny, attr: &str) -> Option<String> {
    let value = obj.getattr(attr).ok()?;
    if value.is_none() {
        return None;
    }
    // `Instance.type` is the TypeInfo; read its `fullname`.
    let type_info = value.getattr("type").ok()?;
    type_info
        .getattr("fullname")
        .and_then(|f| f.extract::<String>())
        .ok()
}

/// Read a `list[TypeInfo]` attribute as a Vec of fullname strings.
pub(crate) fn read_mro_fullnames(obj: &PyAny, attr: &str) -> Option<Vec<String>> {
    let value = obj.getattr(attr).ok()?;
    let list = value.downcast::<PyList>().ok()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        let fullname = item
            .getattr("fullname")
            .and_then(|f| f.extract::<String>())
            .ok()?;
        out.push(fullname);
    }
    Some(out)
}

/// Read a `list[str]` attribute (e.g. `type_vars`, `protocol_members`).
pub(crate) fn read_str_list_attr(obj: &PyAny, attr: &str) -> Option<Vec<String>> {
    let value = obj.getattr(attr).ok()?;
    let list = value.downcast::<PyList>().ok()?;
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        out.push(item.extract::<String>().ok()?);
    }
    Some(out)
}

/// Serialize a single `mypy.types.Type` (or any object with `.write(buf)`)
/// to its wire-format bytes via mypy's `librt.internal.WriteBuffer`.
/// Returns `None` on any failure. Used for `_promote`, `bases`,
/// `tuple_type` — any field Stage 3c decodes via `wire::read_type`.
pub(crate) fn serialize_type_to_bytes(py: Python<'_>, obj: &PyAny) -> Option<Vec<u8>> {
    let write_buffer_cls = py
        .import("librt.internal")
        .ok()?
        .getattr("WriteBuffer")
        .ok()?;
    let buf = write_buffer_cls.call0().ok()?;
    let write = obj.getattr("write").ok()?;
    write.call1((buf,)).ok()?;
    let bytes = buf.getattr("getvalue").ok()?.call0().ok()?;
    bytes.extract::<Vec<u8>>().ok()
}

/// Serialize each element of a `list[Type]` attribute to wire-format bytes.
/// Returns an empty Vec if the attribute is missing or not a list; skips
/// individual items that fail to serialize.
fn read_type_list_bytes(py: Python<'_>, obj: &PyAny, attr: &str) -> Vec<Vec<u8>> {
    let list = match obj.getattr(attr) {
        Ok(l) => match l.downcast::<PyList>() {
            Ok(list) => list,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::with_capacity(list.len());
    for item in list.iter() {
        if let Some(b) = serialize_type_to_bytes(py, item) {
            out.push(b);
        }
    }
    out
}

/// Serialize an `Optional[Type]` attribute (e.g. `tuple_type`) to
/// `Option<Vec<u8>>`. Returns `None` if the attribute is `None` or missing.
fn read_opt_type_bytes(py: Python<'_>, obj: &PyAny, attr: &str) -> Option<Vec<u8>> {
    let value = obj.getattr(attr).ok()?;
    if value.is_none() {
        return None;
    }
    serialize_type_to_bytes(py, value)
}

/// Serialize each `TypeInfo._promote` Type to bytes via mypy's WriteBuffer.
/// Returns a Vec of byte blobs; Stage 3c decodes via `wire::read_type`.
fn read_promote_bytes(py: Python<'_>, obj: &PyAny) -> Vec<Vec<u8>> {
    read_type_list_bytes(py, obj, "_promote")
}

/// Read `TypeInfo.defn.type_vars` as `(name, variance, kind)` triples.
/// variance: 0=INVARIANT, 1=COVARIANT, 2=CONTRAVARIANT, 3=VARIANCE_NOT_READY
/// (nodes.py:3146). kind: 0=TypeVarType, 1=ParamSpecType, 2=TypeVarTupleType.
/// ParamSpec and TypeVarTuple default to variance=0 (INVARIANT) since
/// `check_type_parameter` (subtypes.py:617-621) treats them as invariant
/// unless overridden. Also returns the serialized `upper_bound` blob for
/// each TypeVar (empty for ParamSpec/TypeVarTuple); Stage 3c checks
/// `is_subtype(new_type, upper_bound)` in the covariant branch
/// (subtypes.py:612-619).
fn read_type_vars_with_variance(py: Python<'_>, obj: &PyAny) -> Vec<(String, i64, i64, Vec<u8>)> {
    let defn = match obj.getattr("defn") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let tvars = match defn.getattr("type_vars") {
        Ok(t) => match t.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::with_capacity(tvars.len());
    for item in tvars.iter() {
        let name = match item.getattr("name").and_then(|n| n.extract::<String>()) {
            Ok(n) => n,
            Err(_) => continue,
        };
        // Class-name dispatch: TypeVarType has `.variance`; others default 0.
        let class_name: String = item.get_type().name().unwrap_or("").to_string();
        let (variance, kind) = match class_name.as_str() {
            "TypeVarType" => {
                let v: i64 = item
                    .getattr("variance")
                    .ok()
                    .and_then(|x| x.extract().ok())
                    .unwrap_or(0);
                (v, 0)
            }
            "ParamSpecType" => (0, 1),
            "TypeVarTupleType" => (0, 2),
            _ => (0, 0),
        };
        // upper_bound blob: TypeVarType has `.upper_bound`; ParamSpec
        // / TypeVarTuple have it too but the join visitor doesn't read
        // it for those kinds. Empty blob signals "no bound check".
        let upper_bound = if kind == 0 {
            item.getattr("upper_bound")
                .ok()
                .and_then(|ub| serialize_type_to_bytes(py, ub))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        out.push((name, variance, kind, upper_bound));
    }
    out
}

/// Read `TypeInfo.type_var_tuple_prefix` / `_suffix` as `Option<usize>`.
fn read_opt_usize_attr(obj: &PyAny, attr: &str) -> Option<usize> {
    let v = obj.getattr(attr).ok()?;
    if v.is_none() {
        return None;
    }
    v.extract::<usize>().ok()
}

/// Read the `TypeVarTupleType.tuple_fallback` of the class's variadic
/// type var, serialized to wire bytes, or `None` when the class is not
/// variadic / the fallback is unreadable. Walks `defn.type_vars` and
/// picks the `TypeVarTupleType` entry (expandtype.py:389-390 reads
/// `tvars_middle[0]` as the TypeVarTuple).
fn read_type_var_tuple_fallback(py: Python<'_>, obj: &PyAny) -> Option<Vec<u8>> {
    let defn = obj.getattr("defn").ok()?;
    let tvars = defn.getattr("type_vars").ok()?;
    let list = tvars.downcast::<PyList>().ok()?;
    for item in list.iter() {
        if item.get_type().name().unwrap_or("") == "TypeVarTupleType" {
            let fallback = item.getattr("tuple_fallback").ok()?;
            return serialize_type_to_bytes(py, fallback);
        }
    }
    None
}

/// Read `TypeVarId.raw_id` per `defn.type_vars` entry, parallel to
/// Read `TypeVarId.raw_id` per `defn.type_vars` entry, parallel to
/// `read_type_vars_with_variance`. Used by `expand_type_by_instance` to
/// key the substitution env: class type vars bind `(raw_id, 0, "")`.
/// Duplicate walk is build-time-only cost. `-1` sentinel when unreadable.
fn read_type_var_raw_ids(obj: &PyAny) -> Vec<i64> {
    let defn = match obj.getattr("defn") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let tvars = match defn.getattr("type_vars") {
        Ok(t) => match t.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::with_capacity(tvars.len());
    for item in tvars.iter() {
        let raw_id = item
            .getattr("id")
            .ok()
            .and_then(|i| i.getattr("raw_id").ok())
            .and_then(|r| r.extract::<i64>().ok())
            .unwrap_or(-1);
        out.push(raw_id);
    }
    out
}

/// Read `TypeInfo.names` (a `SymbolTable`, a dict subclass) as
/// `name -> (implicit, has_explicit_value)`. `implicit` is
/// `SymbolTableNode.implicit`; `has_explicit_value` is
/// `Var.has_explicit_value` when the node is a Var (false for non-Var
/// nodes and unreadable attributes). Returns an empty map on any read
/// failure so a partially-constructed TypeInfo does not fail the resolver
/// build. Feeds the M20 checkmember kernel (`has_operator`,
/// `meta_has_operator`, `defined_in_superclass`).
fn read_member_info(obj: &PyAny) -> HashMap<String, (bool, bool)> {
    let names = match obj.getattr("names") {
        Ok(n) => match n.downcast::<PyDict>() {
            Ok(d) => d,
            Err(_) => return HashMap::new(),
        },
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    for (key, value) in names.iter() {
        let name = match key.extract::<String>() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let implicit = value
            .getattr("implicit")
            .and_then(|v| v.extract::<bool>())
            .unwrap_or(false);
        let has_explicit = value
            .getattr("node")
            .ok()
            .and_then(|n| n.getattr("has_explicit_value").ok())
            .and_then(|v| v.extract::<bool>().ok())
            .unwrap_or(false);
        out.insert(name, (implicit, has_explicit));
    }
    out
}

/// Read `TypeInfo.names` into a `name -> (node_kind, definer_fullname)`
/// map for `custom_special_method` (typeops.py:1555). `node_kind`: 0=FuncBase
/// (FuncDef/OverloadedFuncDef), 1=Decorator, 2=Var, -1=other. Only entries
/// where the node is FuncBase/Decorator/Var are stored; `definer_fullname`
/// is `node.info.fullname`.
fn read_member_definers(obj: &PyAny) -> HashMap<String, (i64, String)> {
    let names = match obj.getattr("names") {
        Ok(n) => match n.downcast::<PyDict>() {
            Ok(d) => d,
            Err(_) => return HashMap::new(),
        },
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    for (key, value) in names.iter() {
        let name = match key.extract::<String>() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let node = match value.getattr("node") {
            Ok(n) => n,
            Err(_) => continue,
        };
        let kind = node_kind(node);
        if kind < 0 {
            continue;
        }
        let definer = node
            .getattr("info")
            .ok()
            .and_then(|info| info.getattr("fullname").ok())
            .and_then(|f| f.extract::<String>().ok())
            .unwrap_or_default();
        out.insert(name, (kind, definer));
    }
    out
}

/// Classify a `SymbolTableNode.node` for `custom_special_method`.
/// 0 = FuncBase (FuncDef / OverloadedFuncDef), 1 = Decorator, 2 = Var,
/// -1 = other / unreadable. Mirrors the isinstance check in
/// `typeops.py:1563` against `SYMBOL_FUNCBASE_TYPES` (nodes.py:1370).
fn node_kind(node: &PyAny) -> i64 {
    let type_name = match node.get_type().name() {
        Ok(n) => n.to_string(),
        Err(_) => return -1,
    };
    match type_name.as_str() {
        "FuncDef" | "OverloadedFuncDef" => 0,
        "Decorator" => 1,
        "Var" => 2,
        _ => -1,
    }
}

/// Build a resolver (Python `dict[str, dict]`) from an iterable of live
/// `mypy.nodes.TypeInfo` objects.
///
/// Each TypeInfo is read into a snapshot-fields dict (all
/// JSON-serializable: strings, bools, lists of strings, list of
/// bytes-as-Python-bytes). `read_type_to_str_with_resolver` consumes the
/// returned dict via PyO3 `PyDict` per lookup. FFI-per-lookup cost is
/// acceptable for parity; Stage 3c will replace with a `#[pyclass]`
/// `NativeTypeResolver` holding the `TypeResolver` in Rust.
///
/// On any per-item read failure the item is skipped (the resolver still
/// builds for the items that succeeded), mirroring the strangler-fig
/// degrade-gracefully pattern from `erase::erase_type`.
#[pyfunction]
pub(crate) fn build_resolver(py: Python<'_>, type_infos: &PyAny) -> PyResult<PyObject> {
    let result = PyDict::new(py);
    let iter = type_infos.iter()?;
    for item in iter {
        let item = item?;
        let fullname = match read_str_attr(item, "fullname") {
            Some(f) => f,
            None => continue,
        };
        let name = read_str_attr(item, "name").unwrap_or_else(|| {
            // `name` is `defn.name`; if missing, fall back to the last
            // component of `fullname`.
            fullname.rsplit('.').next().unwrap_or(&fullname).to_owned()
        });
        let snap_dict = PyDict::new(py);
        snap_dict.set_item("fullname", &fullname)?;
        snap_dict.set_item("name", &name)?;
        snap_dict.set_item(
            "is_protocol",
            read_bool_attr(item, "is_protocol").unwrap_or(false),
        )?;
        snap_dict.set_item("is_enum", read_bool_attr(item, "is_enum").unwrap_or(false))?;
        snap_dict.set_item(
            "enum_members",
            PyList::new(
                py,
                read_str_list_attr(item, "enum_members").unwrap_or_default(),
            ),
        )?;
        snap_dict.set_item(
            "fallback_to_any",
            read_bool_attr(item, "fallback_to_any").unwrap_or(false),
        )?;
        snap_dict.set_item(
            "meta_fallback_to_any",
            read_bool_attr(item, "meta_fallback_to_any").unwrap_or(false),
        )?;
        snap_dict.set_item(
            "is_named_tuple",
            read_bool_attr(item, "is_named_tuple").unwrap_or(false),
        )?;
        snap_dict.set_item(
            "has_type_var_tuple_type",
            read_bool_attr(item, "has_type_var_tuple_type").unwrap_or(false),
        )?;
        snap_dict.set_item(
            "is_abstract",
            read_bool_attr(item, "is_abstract").unwrap_or(false),
        )?;

        // type_vars: list[str].
        if let Some(tv) = read_str_list_attr(item, "type_vars") {
            let py_list = PyList::new(py, &tv);
            snap_dict.set_item("type_vars", py_list)?;
        } else {
            snap_dict.set_item("type_vars", PyList::empty(py))?;
        }

        // mro: list[TypeInfo] -> list[fullname str]. has_base is the set
        // of all mro fullnames (TypeInfo.has_base walks the mro).
        let mro = read_mro_fullnames(item, "mro").unwrap_or_default();
        let has_base_set: HashSet<&str> = mro.iter().map(String::as_str).collect();
        let py_mro = PyList::new(py, &mro);
        snap_dict.set_item("mro", py_mro)?;
        let py_has_base = PySet::new(py, &mro)?;
        snap_dict.set_item("has_base", py_has_base)?;
        let _ = has_base_set;

        // protocol_members: list[str] (@property).
        if let Ok(pm) = item.getattr("protocol_members") {
            if let Ok(list) = pm.downcast::<PyList>() {
                let strs: Vec<String> = list
                    .iter()
                    .filter_map(|x| x.extract::<String>().ok())
                    .collect();
                let py_pm = PyList::new(py, &strs);
                snap_dict.set_item("protocol_members", py_pm)?;
            } else {
                snap_dict.set_item("protocol_members", PyList::empty(py))?;
            }
        } else {
            snap_dict.set_item("protocol_members", PyList::empty(py))?;
        }

        // _promote: serialize each Type to bytes.
        let promote = read_promote_bytes(py, item);
        let py_promote = PyList::new(
            py,
            promote.iter().map(|b| PyBytes::new(py, b).to_object(py)),
        );
        snap_dict.set_item("promote_bytes", py_promote)?;

        // alt_promote: Option[Instance] -> Option[fullname].
        let alt = read_opt_instance_fullname(item, "alt_promote");
        snap_dict.set_item("alt_promote_fullname", alt.as_ref())?;

        // metaclass_type: Option[Instance] -> Option[fullname].
        let meta = read_opt_instance_fullname(item, "metaclass_type");
        snap_dict.set_item("metaclass_fullname", meta.as_ref())?;

        // bases: serialize each `TypeInfo.bases` Instance to wire bytes.
        let bases = read_type_list_bytes(py, item, "bases");
        let py_bases = PyList::new(py, bases.iter().map(|b| PyBytes::new(py, b).to_object(py)));
        snap_dict.set_item("bases", py_bases)?;

        // tuple_type: Optional[TupleType] -> Option[wire bytes].
        let tuple_type = read_opt_type_bytes(py, item, "tuple_type");
        match &tuple_type {
            Some(b) => snap_dict.set_item("tuple_type", PyBytes::new(py, b))?,
            None => snap_dict.set_item("tuple_type", py.None())?,
        }

        // type_var_tuple_prefix / _suffix: Option[usize].
        let prefix = read_opt_usize_attr(item, "type_var_tuple_prefix");
        match prefix {
            Some(p) => snap_dict.set_item("type_var_tuple_prefix", p)?,
            None => snap_dict.set_item("type_var_tuple_prefix", py.None())?,
        }
        let suffix = read_opt_usize_attr(item, "type_var_tuple_suffix");
        match suffix {
            Some(s) => snap_dict.set_item("type_var_tuple_suffix", s)?,
            None => snap_dict.set_item("type_var_tuple_suffix", py.None())?,
        }

        // type_var_tuple_fallback: Option[Instance] -> Option[wire bytes].
        let tvf = read_type_var_tuple_fallback(py, item);
        match &tvf {
            Some(b) => snap_dict.set_item("type_var_tuple_fallback", PyBytes::new(py, b))?,
            None => snap_dict.set_item("type_var_tuple_fallback", py.None())?,
        }

        // type_vars_with_variance: Vec<(name, variance, kind, upper_bound)>.
        // The dict path stores the 3-tuple (no upper_bound); the
        // #[pyclass] path keeps the full 4-tuple.
        let tvw = read_type_vars_with_variance(py, item);
        let py_tvw = PyList::new(
            py,
            tvw.iter().map(|(n, v, k, _)| {
                let tup = (n.as_str(), *v, *k).to_object(py);
                tup
            }),
        );
        snap_dict.set_item("type_vars_with_variance", py_tvw)?;

        result.set_item(fullname, snap_dict)?;
    }
    Ok(result.into())
}

/// Look up a snapshot dict by `fullname`, returning the `is_enum` /
/// `name` / `has_type_var_tuple_type` / `type_vars` fields we need for
/// rendering. Returns `None` if the fullname is not in the resolver or
/// the fields cannot be read.
fn lookup_render_fields(resolver: &PyDict, fullname: &str) -> Option<RenderFields> {
    let snap = resolver.get_item(fullname).ok()??;
    let snap_dict = snap.downcast::<PyDict>().ok()?;
    let name: String = snap_dict.get_item("name").ok()??.extract().ok()?;
    let is_enum: bool = snap_dict.get_item("is_enum").ok()??.extract().ok()?;
    let has_tvt: bool = snap_dict
        .get_item("has_type_var_tuple_type")
        .ok()??
        .extract()
        .ok()?;
    let type_vars_len: usize = snap_dict
        .get_item("type_vars")
        .ok()??
        .downcast::<PyList>()
        .ok()?
        .len();
    Some(RenderFields {
        name,
        is_enum,
        has_type_var_tuple_type: has_tvt,
        type_vars_len,
    })
}

struct RenderFields {
    name: String,
    is_enum: bool,
    has_type_var_tuple_type: bool,
    type_vars_len: usize,
}

/// Render a `Type` to its `str(t)` form, optionally resolving `type_ref`
/// via `resolver` for the Stage 3b deferred renderings.
///
/// When `resolver` is `None`, this delegates to the Stage 3a `Display`
/// impl (`t.to_string()`) for every variant, so callers without a
/// resolver get the existing behavior with no regression.
///
/// When `resolver` is `Some`, the Instance and LiteralType variants
/// consult the resolver to (a) strip the `builtins.` prefix on Instance,
/// (b) apply the `[()]` variadic-tuple branch, (c) render enum-literal and
/// bytes-literal `value_repr`. All other variants delegate to `Display`.
pub(crate) fn render_type(py: Python<'_>, t: &Type, resolver: Option<&PyDict>) -> String {
    let Some(resolver) = resolver else {
        return t.to_string();
    };
    match t {
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            ..
        } => render_instance(py, type_ref, args, last_known_value.as_deref(), resolver),
        Type::LiteralType { fallback, value } => render_literal(fallback, value, resolver),
        _ => t.to_string(),
    }
}

/// Render an `Instance`, consulting the resolver for the `builtins.`
/// prefix strip and the `[()]` variadic-tuple branch.
///
/// Mirrors `TypeStrVisitor.visit_instance` (mypy/types.py:3961-4039):
/// - `last_known_value` renders as `{lkv}?` when args is empty.
/// - The name is `type.name` (short) when
///   `not reveal_verbose_types and fullname.startswith("builtins.")`,
///   else the fullname.
/// - `builtins.tuple` with one arg renders `tuple[T, ...]`.
/// - `has_type_var_tuple_type && len(type_vars) == 1` renders `[()]`.
fn render_instance(
    py: Python<'_>,
    type_ref: &str,
    args: &[Type],
    last_known_value: Option<&Type>,
    resolver: &PyDict,
) -> String {
    let fields = lookup_render_fields(resolver, type_ref);
    // Name: short if builtins.*, else the fullname (type_ref verbatim).
    let name: &str = if let Some(f) = &fields {
        if type_ref.starts_with("builtins.") {
            &f.name
        } else {
            type_ref
        }
    } else {
        type_ref
    };

    if let Some(lkv) = last_known_value {
        if args.is_empty() {
            let lkv_str = render_type(py, lkv, Some(resolver));
            return format!("{lkv_str}?");
        }
    }

    let mut out = String::new();
    let _ = write!(out, "{name}");
    if !args.is_empty() {
        if type_ref == "builtins.tuple" {
            // `tuple[T, ...]` (single arg, mirrored from
            // `assert len(t.args) == 1`).
            let _ = write!(out, "[");
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    let _ = write!(out, ", ");
                }
                let _ = write!(out, "{}", render_type(py, a, Some(resolver)));
            }
            let _ = write!(out, ", ...]");
        } else {
            let _ = write!(out, "[");
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    let _ = write!(out, ", ");
                }
                let _ = write!(out, "{}", render_type(py, a, Some(resolver)));
            }
            let _ = write!(out, "]");
        }
    } else if let Some(f) = &fields {
        // The `has_type_var_tuple_type && len(type_vars) == 1` branch
        // renders `[()]` (an empty tuple as the variadic shape),
        // mirroring `visit_instance`'s variadic-generic special case.
        if f.has_type_var_tuple_type && f.type_vars_len == 1 {
            let _ = write!(out, "[()]");
        }
    }
    out
}

/// Render a `LiteralType`, consulting the resolver for the enum-literal
/// and bytes-literal `value_repr` branches.
///
/// Mirrors `LiteralType.value_repr` (mypy/types.py:3370-3392):
/// - enum literal: `f"{fallback_name}.{self.value}"`.
/// - `fallback_name == "builtins.bytes"`: `"b" + repr(self.value)`.
/// - else: `repr(self.value)` (the existing `LiteralValue::Display`).
fn render_literal(fallback: &Type, value: &LiteralValue, resolver: &PyDict) -> String {
    // Extract the fallback's type_ref (the Instance fullname).
    let fallback_ref = match fallback {
        Type::Instance { type_ref, .. } => Some(type_ref.as_str()),
        _ => None,
    };
    let fields = fallback_ref.and_then(|r| lookup_render_fields(resolver, r));

    if let Some(f) = &fields {
        if f.is_enum {
            // Enum literal: `{fallback_fullname}.{value}`. The value is
            // the enum member name (a str). For non-str values, fall
            // back to the value's Display.
            let value_name = match value {
                LiteralValue::Str(s) => s.clone(),
                _ => value.to_string(),
            };
            let fullname = fallback_ref.unwrap_or("");
            return format!("Literal[{fullname}.{value_name}]");
        }
    }
    if fallback_ref == Some("builtins.bytes") {
        // bytes-literal: `"b" + repr(self.value)`. mypy stores the
        // value as bytes; the wire format carries `LiteralValue::Bytes`
        // (added in Stage 3b). Result: `bb'x'`, matching Python.
        let raw = value.to_string();
        return format!("Literal[b{raw}]");
    }
    // Default: render `Literal[{value}]` via the existing Display.
    format!("Literal[{value}]")
}

/// Read a serialized Type from bytes, resolving `type_ref` via
/// `resolver` (a dict from `build_resolver`), and return `str(t)`.
///
/// Stage 3b consumer: same as `wire::read_type_to_str` but with ref
/// resolution for prefix-strip, enum-literal, bytes-literal, and the
/// `[()]` variadic-tuple branch.
///
/// Parity contract:
///   `str(python_type) == read_type_to_str_with_resolver(bytes, resolver)`
///
/// Errors (truncated input, unknown tags, invalid varints) raise as
/// `ValueError` on the Python side, matching `wire::read_type_to_str`.
/// No production code calls this yet: `Options.native_type_kernel` still
/// defaults to `False` and `mypy/subtypes.py` is unchanged.
#[pyfunction]
pub(crate) fn read_type_to_str_with_resolver(
    py: Python<'_>,
    bytes: &[u8],
    resolver: &PyAny,
) -> PyResult<String> {
    let mut buf = ReadBuffer::new(bytes);
    let typ = wire::read_type(&mut buf, None)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let resolver_dict = resolver.downcast::<PyDict>()?;
    Ok(render_type(py, &typ, Some(resolver_dict)))
}

/// `#[pyclass]` wrapper holding the `TypeResolver` and `TypeAliasResolver`
/// in Rust, so Stage 3c `is_subtype` can consult them with zero FFI per
/// lookup. The Stage 3b `render_type` path still needs a `&PyDict`, so the
/// pyclass lazily builds and caches a dict view on first render.
///
/// Built once per type-checking pass by `build_native_resolver` from the
/// live Python TypeInfo graph + alias symbol table. Held by the build
/// manager (`mypy.build`) and threaded into `mypy.subtypes` in M8b.
#[pyclass]
#[allow(dead_code)]
pub(crate) struct NativeTypeResolver {
    resolver: TypeResolver,
    alias_resolver: crate::aliases::TypeAliasResolver,
    /// Lazily-built dict view for the Stage 3b `render_type` path.
    /// `None` until first `render_dict()` call. Kept on the Python heap
    /// because `render_type` takes `&PyDict`.
    cached_dict: Option<PyObject>,
    /// Live `fullname -> TypeInfo` map for enum-member reads that the frozen
    /// snapshot can go stale on (issue-tracking the maptype/expandtype enum
    /// deferrals). Snapshot `enum_members` is captured when the class's own
    /// SCC sealed it; members that resolve later (e.g. nonmember members)
    /// would leave stale entries, so `coerce_to_literal` and the singleton
    /// helpers read `is_enum` / `enum_members` live from here instead.
    /// Populated from `BuildManager._native_typeinfo_map` (live TypeInfos)
    /// at each `_build_native_resolvers` call. `None` until set.
    /// (Storage lives on the inner `TypeResolver` so the subtype engine can
    /// reach it; this struct only forwards the setter.)
    /// Module fullnames snapshotted so far. `update` re-reads `self.modules`
    /// each call but only snapshots modules not seen (first seal wins, like
    /// the TypeInfo side; a module's symbol table is final once its own SCC
    /// sealed it).
    seen_modules: HashSet<String>,
}

#[pymethods]
#[allow(dead_code)]
impl NativeTypeResolver {
    /// Number of TypeInfo snapshots held.
    #[getter]
    fn len(&self) -> usize {
        self.resolver.len()
    }

    /// Number of TypeAlias snapshots held.
    #[getter]
    fn alias_len(&self) -> usize {
        self.alias_resolver.len()
    }

    /// Return (and lazily build) the dict view of the TypeInfo resolver,
    /// for the Stage 3b `render_type` path. Subsequent calls return the
    /// cached dict without rebuilding.
    fn render_dict(&mut self, py: Python<'_>) -> PyResult<PyObject> {
        if let Some(d) = &self.cached_dict {
            return Ok(d.clone_ref(py));
        }
        let dict = PyDict::new(py);
        // The dict view mirrors build_resolver's output shape so the
        // existing render_type lookup_render_fields works unchanged.
        for (fullname, snap) in self.resolver_snapshots_for_render() {
            let inner = PyDict::new(py);
            inner.set_item("fullname", &snap.fullname)?;
            inner.set_item("name", &snap.name)?;
            inner.set_item("is_protocol", snap.is_protocol)?;
            inner.set_item("is_enum", snap.is_enum)?;
            inner.set_item("fallback_to_any", snap.fallback_to_any)?;
            inner.set_item("has_type_var_tuple_type", snap.has_type_var_tuple_type)?;
            let tv: Vec<String> = snap.type_vars.clone();
            inner.set_item("type_vars", PyList::new(py, &tv))?;
            dict.set_item(fullname, inner)?;
        }
        let obj: PyObject = dict.into();
        self.cached_dict = Some(obj.clone_ref(py));
        Ok(obj)
    }

    /// Incrementally extend the resolver with TypeInfos / TypeAliases from
    /// the (growing) live TypeInfo graph (issue #599).
    ///
    /// Existing fullnames are kept (first seal wins): classes belong to
    /// exactly one SCC, so a snapshot taken after its defining SCC was
    /// semanalized is final; later SCCs must not overwrite it. The one
    /// exception is `builtins.*` classes, whose `_promote` lists accumulate
    /// promotions from later SCCs' `calculate_class_properties` (native
    /// ints + TYPE_PROMOTIONS), so they are re-snapshotted on every call
    /// (a small constant set). This makes `update` safe to call once per
    /// SCC in `process_stale_scc` without re-serializing the full TypeInfo
    /// graph (~8490 items) on every call. Returns `(added_infos,
    /// added_aliases)` so the Python side can grow its accumulated
    /// `typeinfo_map` in lockstep.
    #[pyo3(signature = (type_infos, aliases, modules=None))]
    fn update(
        &mut self,
        py: Python<'_>,
        type_infos: &PyAny,
        aliases: &PyAny,
        modules: Option<&PyAny>,
    ) -> PyResult<(usize, usize)> {
        let mut added_infos = 0usize;
        for item in type_infos.iter()? {
            let item = item?;
            let fullname = match read_str_attr(item, "fullname") {
                Some(f) => f,
                None => continue,
            };
            // Promotion sinks (`builtins.int`, `builtins.float`,
            // `builtins.bytearray`, `builtins.memoryview`) accumulate
            // `_promote` entries from later SCCs' `calculate_class_properties`

            // (native ints + TYPE_PROMOTIONS, semanal_classprop.py:205-223),
            // so a first-seal-wins snapshot of them goes stale. Always
            // re-snapshot `builtins.*` classes (a small constant set) so

            // their promotion lists stay current; the remaining ~full graph
            // keeps the first-seal semantics (a class's own SCC seals it
            // once, then it does not change).
            let re_snapshot = fullname.starts_with("builtins.");
            if !re_snapshot && self.resolver.get(&fullname).is_some() {
                continue;
            }
            let Some(snap) = snapshot_type_info(py, item, &fullname) else {
                continue;
            };
            let fresh = self.resolver.get(&fullname).is_none();
            self.resolver.insert(snap.fullname.clone(), snap);
            if fresh {
                added_infos += 1;
            }
        }
        let mut added_aliases = 0usize;
        for item in aliases.iter()? {
            let item = item?;
            let fullname: String = match item.getattr("fullname").and_then(|f| f.extract()) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if self.alias_resolver.get(&fullname).is_some() {
                continue;
            }
            let Some(snap) = snapshot_type_alias(py, item, &fullname) else {
                continue;
            };
            self.alias_resolver.insert(fullname, snap);
            added_aliases += 1;
        }
        // Module snapshots: first seal wins (a module's symbol table is
        // final once its own SCC sealed it; later SCCs must not overwrite
        // it). Fresh-cache / dependency modules are already final here.
        if let Some(modules) = modules {
            self.snapshot_modules(py, modules);
        }
        // The dict view is keyed by the full TypeInfo set; any growth
        // invalidates it (rebuilt lazily on next render).
        self.cached_dict = None;
        let _ = py;
        Ok((added_infos, added_aliases))
    }

    /// Install the live `fullname -> TypeInfo` map for enum-member reads
    /// that the frozen snapshot can go stale on. Held as `PyObject` on the
    /// Python heap; `None` clears it. Populated from the build manager's
    /// `_native_typeinfo_map` (live TypeInfo objects) at each
    /// `_build_native_resolvers` call, so member lists read through it are
    /// always current (coerce_to_literal / singleton helpers).
    fn set_live_typeinfo_map(&mut self, py: Python<'_>, map: Option<PyObject>) -> PyResult<()> {
        self.resolver.live_info_map = map;
        let _ = py;
        Ok(())
    }

    /// Snapshot `BuildManager.modules` (a `fullname -> MypyFile` dict) into
    /// the module table. First seal wins: modules already snapshotted are
    /// skipped, matching the TypeInfo side. Individual read failures skip
    /// that module (it defers to Python at lookup time).
    fn snapshot_modules(&mut self, py: Python<'_>, modules: &PyAny) {
        let Ok(modules) = modules.downcast::<PyDict>() else {
            return;
        };
        for (key, value) in modules.iter() {
            let fullname: String = match key.extract() {
                Ok(f) => f,
                Err(_) => continue,
            };
            if !self.seen_modules.insert(fullname.clone()) {
                continue;
            }
            let Some(snap) = snapshot_module(py, value) else {
                continue;
            };
            self.resolver.insert_module(fullname, snap);
        }
    }
}

/// Snapshot one live `mypy.nodes.TypeInfo` object into a
/// `TypeInfoSnapshot`. Returns `None` (caller skips the item) when the
/// `fullname` attribute is unreadable. Shared by `build_native_resolver`
/// (fresh full build) and `NativeTypeResolver::update` (per-SCC extend).
fn snapshot_type_info(py: Python<'_>, item: &PyAny, fullname: &str) -> Option<TypeInfoSnapshot> {
    let name = read_str_attr(item, "name")
        .unwrap_or_else(|| fullname.rsplit('.').next().unwrap_or(fullname).to_owned());
    let is_protocol = read_bool_attr(item, "is_protocol").unwrap_or(false);
    let is_enum = read_bool_attr(item, "is_enum").unwrap_or(false);
    let enum_members = read_str_list_attr(item, "enum_members").unwrap_or_default();
    let fallback_to_any = read_bool_attr(item, "fallback_to_any").unwrap_or(false);
    let meta_fallback_to_any = read_bool_attr(item, "meta_fallback_to_any").unwrap_or(false);
    let is_named_tuple = read_bool_attr(item, "is_named_tuple").unwrap_or(false);
    let is_newtype = read_bool_attr(item, "is_newtype").unwrap_or(false);
    let has_type_var_tuple_type = read_bool_attr(item, "has_type_var_tuple_type").unwrap_or(false);
    let has_param_spec_type = read_bool_attr(item, "has_param_spec_type").unwrap_or(false);
    let is_abstract = read_bool_attr(item, "is_abstract").unwrap_or(false);
    let type_vars = read_str_list_attr(item, "type_vars").unwrap_or_default();
    let mro = read_mro_fullnames(item, "mro").unwrap_or_default();
    let has_base: HashSet<String> = mro.iter().cloned().collect();
    let protocol_members = item
        .getattr("protocol_members")
        .ok()
        .and_then(|pm| pm.downcast::<PyList>().ok())
        .map(|list| {
            list.iter()
                .filter_map(|x| x.extract::<String>().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let promote_bytes = read_promote_bytes(py, item);
    let alt_promote_fullname = read_opt_instance_fullname(item, "alt_promote");
    let metaclass_fullname = read_opt_instance_fullname(item, "metaclass_type");
    let bases = read_type_list_bytes(py, item, "bases");
    let tuple_type = read_opt_type_bytes(py, item, "tuple_type");
    let type_var_tuple_prefix = read_opt_usize_attr(item, "type_var_tuple_prefix");
    let type_var_tuple_suffix = read_opt_usize_attr(item, "type_var_tuple_suffix");
    let type_var_tuple_fallback = read_type_var_tuple_fallback(py, item);
    let type_vars_with_variance_full = read_type_vars_with_variance(py, item);
    let type_vars_with_variance: Vec<(String, i64, i64)> = type_vars_with_variance_full
        .iter()
        .map(|(n, v, k, _)| (n.clone(), *v, *k))
        .collect();
    let type_var_upper_bounds: Vec<Vec<u8>> = type_vars_with_variance_full
        .into_iter()
        .map(|(_, _, _, ub)| ub)
        .collect();
    let type_var_raw_ids = read_type_var_raw_ids(item);
    let member_info = read_member_info(item);
    let member_definers = read_member_definers(item);

    Some(TypeInfoSnapshot {
        fullname: fullname.to_owned(),
        name,
        is_protocol,
        is_enum,
        enum_members,
        fallback_to_any,
        meta_fallback_to_any,
        is_named_tuple,
        is_newtype,
        has_type_var_tuple_type,
        has_param_spec_type,
        is_abstract,
        type_vars,
        mro,
        protocol_members,
        has_base,
        promote_bytes,
        alt_promote_fullname,
        metaclass_fullname,
        bases,
        tuple_type,
        type_var_tuple_prefix,
        type_var_tuple_suffix,
        type_var_tuple_fallback,
        type_vars_with_variance,
        type_var_upper_bounds,
        type_var_raw_ids,
        member_info,
        member_definers,
    })
}

/// Snapshot one live `mypy.nodes.TypeAlias` object into a
/// `TypeAliasSnapshot`. Returns `None` when the alias has no serializable
/// `target` (caller skips the item). Shared by `build_native_resolver`
/// (fresh full build) and `NativeTypeResolver::update` (per-SCC extend).
fn snapshot_type_alias(
    py: Python<'_>,
    item: &PyAny,
    fullname: &str,
) -> Option<crate::aliases::TypeAliasSnapshot> {
    let target = {
        let t = item.getattr("target").ok()?;
        serialize_type_to_bytes(py, t)?
    };
    let alias_tvars = read_alias_tvars_pub(item);
    let tvar_tuple_index = read_tvar_tuple_index_pub(item);
    let no_args: bool = item
        .getattr("no_args")
        .ok()
        .and_then(|v| v.extract().ok())
        .unwrap_or(false);
    let python_3_12_type_alias: bool = item
        .getattr("python_3_12_type_alias")
        .ok()
        .and_then(|v| v.extract().ok())
        .unwrap_or(false);
    Some(crate::aliases::TypeAliasSnapshot {
        fullname: fullname.to_owned(),
        target,
        alias_tvars,
        tvar_tuple_index,
        no_args,
        python_3_12_type_alias,
    })
}

/// Snapshot one live `mypy.nodes.MypyFile` into a `ModuleSnapshot`:
/// read the `names` SymbolTable, capturing `module_hidden` and (for
/// `MypyFile` nodes) the node's fullname per entry. Returns `None`
/// (caller skips the item) when `names` is unreadable or not a dict.
/// Individual reads are defensive: an unreadable `module_hidden` is
/// captured as `false` (a visible hit then answers natively), and an
/// unreadable node kind is captured as a non-module (Rust then declines
/// to descend, deferring to Python). The name set itself is exact, so a
/// miss only makes Rust defer to Python, never invent a symbol.
fn snapshot_module(py: Python<'_>, item: &PyAny) -> Option<ModuleSnapshot> {
    let names = item.getattr("names").ok()?;
    let names = names.downcast::<PyDict>().ok()?;
    let mut symbols = HashMap::with_capacity(names.len());
    for (key, value) in names.iter() {
        let name: String = key.extract().ok()?;
        let hidden = value
            .getattr("module_hidden")
            .ok()
            .and_then(|v| v.extract::<bool>().ok())
            .unwrap_or(false);
        let node = value.getattr("node").ok();
        let node = node.as_ref().and_then(|n| {
            let is_module = n
                .getattr("__class__")
                .and_then(|c| c.getattr("__name__").and_then(|n| n.extract::<String>()))
                .ok()
                .map(|cls| cls == "MypyFile")
                .unwrap_or(false);
            if !is_module {
                return None;
            }
            let fullname = n
                .getattr("fullname")
                .and_then(|f| f.extract::<String>())
                .ok()?;
            Some((true, fullname))
        });
        symbols.insert(name, (hidden, node));
    }
    let _ = py;
    Some(ModuleSnapshot { symbols })
}

impl NativeTypeResolver {
    pub(crate) fn new(
        resolver: TypeResolver,
        alias_resolver: crate::aliases::TypeAliasResolver,
    ) -> Self {
        Self {
            resolver,
            alias_resolver,
            cached_dict: None,
            seen_modules: HashSet::new(),
        }
    }

    /// Borrow the inner `TypeResolver` so Stage 3c `is_subtype` can look
    /// up `TypeInfoSnapshot`s without FFI. Used by `subtypes::rust_is_subtype`.
    pub(crate) fn resolver(&self) -> &TypeResolver {
        &self.resolver
    }

    /// Wrap an already-built `TypeResolver` (tests only).
    #[cfg(test)]
    pub(crate) fn from_resolver(resolver: TypeResolver) -> Self {
        Self::new(resolver, crate::aliases::TypeAliasResolver::new())
    }

    /// Look up a live `TypeInfo` (as `&PyAny`) by fullname from the
    /// `live_info_map` installed by `set_live_typeinfo_map`. `None` when no
    /// map is installed or the fullname is absent. Used by enum-member reads
    /// that need current (non-snapshot) data.
    pub(crate) fn live_typeinfo<'py>(
        &'py self,
        py: Python<'py>,
        fullname: &str,
    ) -> Option<&'py PyAny> {
        self.resolver.live_typeinfo(py, fullname)
    }

    /// Borrow the `TypeAliasResolver` so checkexpr helpers can expand
    /// `TypeAliasType` wire nodes (B3a). Used by `checkexpr_functions`.
    pub(crate) fn alias_resolver(&self) -> &crate::aliases::TypeAliasResolver {
        &self.alias_resolver
    }

    /// Borrow the snapshots for the dict-view builder. Returns an iterator
    /// of `(fullname, &TypeInfoSnapshot)`.
    fn resolver_snapshots_for_render(
        &mut self,
    ) -> impl Iterator<Item = (&String, &TypeInfoSnapshot)> {
        self.resolver_snapshots_iter()
    }

    fn resolver_snapshots_iter(&self) -> impl Iterator<Item = (&String, &TypeInfoSnapshot)> {
        self.resolver.iter()
    }
}

/// Build a `NativeTypeResolver` pyclass from an iterable of live
/// `mypy.nodes.TypeInfo` objects, an iterable of `mypy.nodes.TypeAlias`
/// objects, and a `fullname -> MypyFile` modules dict. Holds both
/// resolvers in Rust; the dict view is built lazily on first
/// `render_dict()` call.
///
/// Mirrors `build_resolver` (dict-returning, Stage 3b) but returns the
/// Rust-owned pyclass for zero-FFI-per-lookup access by Stage 3c
/// `is_subtype`. The dict-returning `build_resolver` remains for one
/// release as a deprecated alias so Stage 3b parity tests don't break.
#[pyfunction]
#[pyo3(signature = (type_infos, aliases, modules=None))]
pub(crate) fn build_native_resolver(
    py: Python<'_>,
    type_infos: &PyAny,
    aliases: &PyAny,
    modules: Option<&PyAny>,
) -> PyResult<Py<NativeTypeResolver>> {
    let mut resolver = TypeResolver::new();
    for item in type_infos.iter()? {
        let item = item?;
        let fullname = match read_str_attr(item, "fullname") {
            Some(f) => f,
            None => continue,
        };
        let Some(snap) = snapshot_type_info(py, item, &fullname) else {
            continue;
        };
        resolver.insert(snap.fullname.clone(), snap);
    }

    let mut alias_resolver = crate::aliases::TypeAliasResolver::new();
    for item in aliases.iter()? {
        let item = item?;
        let fullname: String = match item.getattr("fullname").and_then(|f| f.extract()) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let Some(snap) = snapshot_type_alias(py, item, &fullname) else {
            continue;
        };
        alias_resolver.insert(fullname, snap);
    }

    let mut seen_modules = HashSet::new();
    if let Some(modules) = modules {
        if let Ok(modules) = modules.downcast::<PyDict>() {
            for (key, value) in modules.iter() {
                let fullname: String = match key.extract() {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                if !seen_modules.insert(fullname.clone()) {
                    continue;
                }
                let Some(snap) = snapshot_module(py, value) else {
                    continue;
                };
                resolver.insert_module(fullname, snap);
            }
        }
    }

    let native = NativeTypeResolver::new(resolver, alias_resolver);
    Py::new(py, native)
}

/// Read `TypeAlias.alias_tvars` as declaration-ordered identities.
/// Mirrors the private helper in `aliases.rs` but `pub(crate)` so
/// `build_native_resolver` reuses it without exposing the alias-iter
/// logic. Each declared tvar contributes its `TypeVarId` identity
/// (`(raw_id, meta_level, namespace)`, types.py:574-576) plus whether it
/// is a `TypeVarTupleType`. This is the data `expanded_alias_target` needs to
/// build the substitution env mirroring `TypeAliasType._expand_once`.
fn read_alias_tvars_pub(obj: &PyAny) -> Vec<crate::aliases::AliasTvar> {
    use crate::aliases::AliasTvar;
    let tvars = match obj.getattr("alias_tvars") {
        Ok(t) => match t.downcast::<PyList>() {
            Ok(l) => l,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::with_capacity(tvars.len());
    for item in tvars.iter() {
        let name: String = match item.getattr("name").and_then(|n| n.extract()) {
            Ok(n) => n,
            Err(_) => continue,
        };
        let id = match item.getattr("id") {
            Ok(i) => i,
            Err(_) => continue,
        };
        let raw_id: i64 = id.getattr("raw_id").and_then(|v| v.extract()).unwrap_or(0);
        // meta_level / namespace sit on the id as attributes or defaults.
        let meta_level: i64 = id
            .getattr("meta_level")
            .and_then(|v| v.extract())
            .unwrap_or(0);
        let namespace: String = id
            .getattr("namespace")
            .and_then(|v| v.extract())
            .unwrap_or_default();
        let is_type_var_tuple = item
            .getattr("__class__")
            .and_then(|c| c.getattr("__name__").and_then(|n| n.extract::<String>()))
            .ok()
            .map(|n| n == "TypeVarTupleType" || n == "TypeVarTupleDef")
            .unwrap_or(false);
        out.push(AliasTvar {
            name,
            raw_id,
            meta_level,
            namespace,
            is_type_var_tuple,
        });
    }
    out
}

/// Read `TypeAlias.tvar_tuple_index` as `Option<usize>`.
fn read_tvar_tuple_index_pub(obj: &PyAny) -> Option<usize> {
    let v = obj.getattr("tvar_tuple_index").ok()?;
    if v.is_none() {
        return None;
    }
    v.extract::<usize>().ok()
}

/// Read a serialized Type from bytes, resolving `type_ref` via the
/// `NativeTypeResolver` pyclass (built by `build_native_resolver`),
/// and return `str(t)`. This is the M8a zero-FFI-per-lookup path: the
/// resolver is Rust-owned, only the final str crosses the boundary.
///
/// Parity contract:
///   `str(python_type) == read_type_to_str_with_native_resolver(bytes, resolver)`
#[pyfunction]
pub(crate) fn read_type_to_str_with_native_resolver(
    py: Python<'_>,
    bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> PyResult<String> {
    let mut buf = ReadBuffer::new(bytes);
    let typ = wire::read_type(&mut buf, None)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    let dict_obj = resolver.render_dict(py)?;
    let dict = dict_obj.downcast::<PyDict>(py)?;
    Ok(render_type(py, &typ, Some(dict)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(fullname: &str, name: &str) -> TypeInfoSnapshot {
        TypeInfoSnapshot {
            fullname: fullname.to_owned(),
            name: name.to_owned(),
            ..Default::default()
        }
    }

    fn snap_enum(fullname: &str, name: &str) -> TypeInfoSnapshot {
        TypeInfoSnapshot {
            fullname: fullname.to_owned(),
            name: name.to_owned(),
            is_enum: true,
            ..Default::default()
        }
    }

    fn snap_tvt(fullname: &str, name: &str, type_vars: &[&str]) -> TypeInfoSnapshot {
        TypeInfoSnapshot {
            fullname: fullname.to_owned(),
            name: name.to_owned(),
            has_type_var_tuple_type: true,
            type_vars: type_vars.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn snapshot_has_base_true_for_mro_member() {
        let mut s = snap("builtins.int", "int");
        s.has_base.insert("builtins.object".to_string());
        assert!(s.has_base("builtins.object"));
        assert!(!s.has_base("builtins.str"));
    }

    #[test]
    fn snapshot_is_builtins_true_for_builtins_prefix() {
        let s = snap("builtins.int", "int");
        assert!(s.is_builtins());
        let s2 = snap("typing.Sequence", "Sequence");
        assert!(!s2.is_builtins());
    }

    #[test]
    fn resolver_get_returns_inserted_snapshot() {
        let mut r = TypeResolver::new();
        assert!(r.is_empty());
        r.insert("builtins.int".to_string(), snap("builtins.int", "int"));
        assert_eq!(r.len(), 1);
        assert!(r.get("builtins.int").is_some());
        assert!(r.get("builtins.str").is_none());
    }

    #[test]
    fn resolver_len_and_is_empty() {
        let mut r = TypeResolver::new();
        assert!(r.is_empty());
        r.insert("a".to_string(), snap("a", "a"));
        r.insert("b".to_string(), snap("b", "b"));
        assert_eq!(r.len(), 2);
        assert!(!r.is_empty());
    }

    // --- render_type tests (pure Rust, no Python resolver) ---

    #[test]
    fn render_type_without_resolver_matches_display_for_any() {
        // Without a resolver, render_type delegates to Display. Pure-Rust
        // path; no Python needed.
        let t = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        // Without GIL: render_type takes a Python<'_> only because the
        // resolver path needs it. The None path doesn't use py, so we
        // can pass a borrowed Python from with_gil.
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let rendered = render_type(py, &t, None);
            assert_eq!(rendered, t.to_string());
            assert_eq!(rendered, "Any");
        });
    }

    #[test]
    fn render_type_without_resolver_matches_display_for_instance() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let rendered = render_type(py, &t, None);
            // Without resolver, no prefix strip; matches Stage 3a Display.
            assert_eq!(rendered, "builtins.int");
            assert_eq!(rendered, t.to_string());
        });
    }

    #[test]
    fn render_type_without_resolver_matches_display_for_literal_int() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(42),
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let rendered = render_type(py, &t, None);
            assert_eq!(rendered, "Literal[42]");
            assert_eq!(rendered, t.to_string());
        });
    }

    // --- render_type tests WITH a resolver (need Python dict) ---

    fn make_resolver_dict(py: Python<'_>, snaps: &[TypeInfoSnapshot]) -> PyObject {
        let dict = PyDict::new(py);
        for s in snaps {
            let inner = PyDict::new(py);
            inner.set_item("fullname", &s.fullname).unwrap();
            inner.set_item("name", &s.name).unwrap();
            inner.set_item("is_enum", s.is_enum).unwrap();
            inner
                .set_item("has_type_var_tuple_type", s.has_type_var_tuple_type)
                .unwrap();
            let tv: Vec<String> = s.type_vars.clone();
            let py_tv = PyList::new(py, &tv);
            inner.set_item("type_vars", py_tv).unwrap();
            dict.set_item(&s.fullname, inner).unwrap();
        }
        dict.into()
    }

    #[test]
    fn render_instance_strips_builtins_prefix_with_resolver() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let resolver_obj = make_resolver_dict(py, &[snap("builtins.int", "int")]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            assert_eq!(rendered, "int");
        });
    }

    #[test]
    fn render_instance_keeps_non_builtins_fullname() {
        let t = Type::Instance {
            type_ref: "typing.Sequence".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let resolver_obj = make_resolver_dict(py, &[snap("typing.Sequence", "Sequence")]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            // Python strips only `builtins.`, so typing.Sequence stays.
            assert_eq!(rendered, "typing.Sequence");
        });
    }

    #[test]
    fn render_instance_unknown_ref_renders_verbatim() {
        // When the resolver has no entry for type_ref, render verbatim
        // (degrade gracefully; same as Stage 3a).
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let resolver_obj = make_resolver_dict(py, &[]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            assert_eq!(rendered, "builtins.int");
        });
    }

    #[test]
    fn render_literal_enum_with_resolver() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "my.Color".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Str("RED".to_string()),
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let resolver_obj = make_resolver_dict(py, &[snap_enum("my.Color", "Color")]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            assert_eq!(rendered, "Literal[my.Color.RED]");
        });
    }

    #[test]
    fn render_literal_bytes_with_resolver() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bytes".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Str("x".to_string()),
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            // Resolver has builtins.bytes but is_enum=False.
            let resolver_obj = make_resolver_dict(py, &[snap("builtins.bytes", "bytes")]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            // bytes path: "b" + repr(value). repr("x") == "'x'", so "b'x'".
            assert_eq!(rendered, "Literal[b'x']");
        });
    }

    #[test]
    fn render_literal_int_unchanged_with_resolver() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(1),
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let resolver_obj = make_resolver_dict(py, &[snap("builtins.int", "int")]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            assert_eq!(rendered, "Literal[1]");
        });
    }

    #[test]
    fn render_instance_variadic_tuple_branch() {
        // has_type_var_tuple_type=true && len(type_vars)==1 -> `[()]`.
        let t = Type::Instance {
            type_ref: "foo.VarTuple".to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let resolver_obj =
                make_resolver_dict(py, &[snap_tvt("foo.VarTuple", "VarTuple", &["Ts"])]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            // Not builtins., so name = fullname; then `[()]` branch.
            assert_eq!(rendered, "foo.VarTuple[()]");
        });
    }

    #[test]
    fn render_instance_tuple_with_args_uses_tuple_form() {
        let t = Type::Instance {
            type_ref: "builtins.tuple".to_string(),
            args: vec![Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }],
            last_known_value: None,
            extra_attrs: None,
        };
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let resolver_obj = make_resolver_dict(py, &[snap("builtins.tuple", "tuple")]);
            let resolver = resolver_obj.downcast::<PyDict>(py).unwrap();
            let rendered = render_type(py, &t, Some(resolver));
            assert_eq!(rendered, "tuple[Any, ...]");
        });
    }

    // --- Stage 3c M8a: enriched snapshot field tests ---

    #[test]
    fn snapshot_default_has_empty_enriched_fields() {
        let s = TypeInfoSnapshot::default();
        assert!(s.bases.is_empty());
        assert!(s.tuple_type.is_none());
        assert!(s.type_var_tuple_prefix.is_none());
        assert!(s.type_var_tuple_suffix.is_none());
        assert!(s.type_vars_with_variance.is_empty());
    }

    #[test]
    fn snapshot_carries_bases_and_tuple_type_blobs() {
        let mut s = snap("builtins.int", "int");
        s.bases = vec![vec![1, 2, 3], vec![4, 5]];
        s.tuple_type = Some(vec![0xAB]);
        assert_eq!(s.bases.len(), 2);
        assert_eq!(s.bases[0], vec![1, 2, 3]);
        assert_eq!(s.tuple_type.as_deref(), Some(&[0xAB][..]));
    }

    #[test]
    fn snapshot_carries_type_var_tuple_prefix_and_suffix() {
        let mut s = snap("foo.VarTuple", "VarTuple");
        s.type_var_tuple_prefix = Some(2);
        s.type_var_tuple_suffix = Some(1);
        assert_eq!(s.type_var_tuple_prefix, Some(2));
        assert_eq!(s.type_var_tuple_suffix, Some(1));
    }

    #[test]
    fn snapshot_carries_type_vars_with_variance() {
        let mut s = snap("foo.Generic", "Generic");
        // (name, variance, kind): COVARIANT=1 TypeVar, INVARIANT=0 ParamSpec.
        s.type_vars_with_variance = vec![("T".to_string(), 1, 0), ("P".to_string(), 0, 1)];
        assert_eq!(s.type_vars_with_variance.len(), 2);
        assert_eq!(s.type_vars_with_variance[0], ("T".to_string(), 1, 0));
        assert_eq!(s.type_vars_with_variance[1], ("P".to_string(), 0, 1));
    }

    #[test]
    fn resolver_iter_yields_all_inserted_snapshots() {
        let mut r = TypeResolver::new();
        r.insert("a".to_string(), snap("a", "a"));
        r.insert("b".to_string(), snap("b", "b"));
        let mut keys: Vec<&String> = r.iter().map(|(k, _)| k).collect();
        keys.sort();
        assert_eq!(keys, vec![&"a".to_string(), &"b".to_string()]);
    }

    // --- ModuleSnapshot tests (MypyFile name tables) ---

    fn make_module_snap(entries: &[(&str, bool, Option<&str>)]) -> ModuleSnapshot {
        // (name, module_hidden, module_fullname-when-MypyFile)
        let mut symbols = HashMap::new();
        for &(name, hidden, mod_full) in entries {
            let node = mod_full.map(|f| (true, f.to_string()));
            symbols.insert(name.to_string(), (hidden, node));
        }
        ModuleSnapshot { symbols }
    }

    #[test]
    fn module_snapshot_visible_distinguishes_hidden_and_missing() {
        let m = make_module_snap(&[
            ("x", false, None),
            ("_hidden", true, None),
            ("sub", false, Some("pkg.sub")),
        ]);
        assert_eq!(m.visible("x"), Some(true));
        assert_eq!(m.visible("_hidden"), Some(false));
        assert_eq!(m.visible("absent"), None);
    }

    #[test]
    fn module_snapshot_module_fullname_only_for_visible_modules() {
        let m = make_module_snap(&[
            ("x", false, None),
            ("_hidden_mod", true, Some("pkg.hidden")),
            ("sub", false, Some("pkg.sub")),
        ]);
        assert_eq!(m.module_fullname("x"), None);
        assert_eq!(m.module_fullname("_hidden_mod"), None);
        assert_eq!(m.module_fullname("sub"), Some("pkg.sub"));
        assert_eq!(m.module_fullname("absent"), None);
    }

    #[test]
    fn resolver_module_accessors_roundtrip() {
        let mut r = TypeResolver::new();
        assert!(r.get_module("pkg").is_none());
        let m = make_module_snap(&[("x", false, None)]);
        r.insert_module("pkg".to_string(), m);
        assert!(r.get_module("pkg").is_some());
        assert!(r.get_module("other").is_none());
    }
}
