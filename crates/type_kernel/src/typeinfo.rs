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
    /// `TypeInfo.fallback_to_any` (nodes.py:3759). subtypes.py:493,1494.
    pub fallback_to_any: bool,
    /// `TypeInfo.meta_fallback_to_any` (nodes.py:3763). subtypes.py:1494.
    pub meta_fallback_to_any: bool,
    /// `TypeInfo.is_named_tuple` (nodes.py:3800). subtypes.py:559.
    pub is_named_tuple: bool,
    /// `TypeInfo.has_type_var_tuple_type` (nodes.py:3921). Display `[()]`.
    pub has_type_var_tuple_type: bool,
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
    /// `(name, variance, kind)` for each `defn.type_vars` entry.
    /// variance: 0=INVARIANT, 1=COVARIANT, 2=CONTRAVARIANT,
    /// 3=VARIANCE_NOT_READY (nodes.py:3146). kind: 0=TypeVarType,
    /// 1=ParamSpecType, 2=TypeVarTupleType. Stage 3c dispatches
    /// `check_type_parameter` on (variance, kind).
    pub type_vars_with_variance: Vec<(String, i64, i64)>,
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
#[allow(dead_code)]
pub(crate) struct TypeResolver {
    snapshots: HashMap<String, TypeInfoSnapshot>,
}

#[allow(dead_code)]
impl TypeResolver {
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
        }
    }

    pub fn insert(&mut self, fullname: String, snap: TypeInfoSnapshot) {
        self.snapshots.insert(fullname, snap);
    }

    pub fn get(&self, fullname: &str) -> Option<&TypeInfoSnapshot> {
        self.snapshots.get(fullname)
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
fn read_bool_attr(obj: &PyAny, attr: &str) -> Option<bool> {
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
fn read_mro_fullnames(obj: &PyAny, attr: &str) -> Option<Vec<String>> {
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
fn read_str_list_attr(obj: &PyAny, attr: &str) -> Option<Vec<String>> {
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
/// unless overridden.
fn read_type_vars_with_variance(obj: &PyAny) -> Vec<(String, i64, i64)> {
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
        out.push((name, variance, kind));
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

        // type_vars_with_variance: Vec<(name, variance, kind)>.
        let tvw = read_type_vars_with_variance(item);
        let py_tvw = PyList::new(
            py,
            tvw.iter().map(|(n, v, k)| {
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
}

impl NativeTypeResolver {
    fn new(resolver: TypeResolver, alias_resolver: crate::aliases::TypeAliasResolver) -> Self {
        Self {
            resolver,
            alias_resolver,
            cached_dict: None,
        }
    }

    /// Borrow the inner `TypeResolver` so Stage 3c `is_subtype` can look
    /// up `TypeInfoSnapshot`s without FFI. Used by `subtypes::rust_is_subtype`.
    pub(crate) fn resolver(&self) -> &TypeResolver {
        &self.resolver
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
/// `mypy.nodes.TypeInfo` objects and an iterable of `mypy.nodes.TypeAlias`
/// objects. Holds both resolvers in Rust; the dict view is built lazily
/// on first `render_dict()` call.
///
/// Mirrors `build_resolver` (dict-returning, Stage 3b) but returns the
/// Rust-owned pyclass for zero-FFI-per-lookup access by Stage 3c
/// `is_subtype`. The dict-returning `build_resolver` remains for one
/// release as a deprecated alias so Stage 3b parity tests don't break.
#[pyfunction]
pub(crate) fn build_native_resolver(
    py: Python<'_>,
    type_infos: &PyAny,
    aliases: &PyAny,
) -> PyResult<Py<NativeTypeResolver>> {
    let mut resolver = TypeResolver::new();
    for item in type_infos.iter()? {
        let item = item?;
        let fullname = match read_str_attr(item, "fullname") {
            Some(f) => f,
            None => continue,
        };
        let name = read_str_attr(item, "name")
            .unwrap_or_else(|| fullname.rsplit('.').next().unwrap_or(&fullname).to_owned());
        let is_protocol = read_bool_attr(item, "is_protocol").unwrap_or(false);
        let is_enum = read_bool_attr(item, "is_enum").unwrap_or(false);
        let fallback_to_any = read_bool_attr(item, "fallback_to_any").unwrap_or(false);
        let meta_fallback_to_any = read_bool_attr(item, "meta_fallback_to_any").unwrap_or(false);
        let is_named_tuple = read_bool_attr(item, "is_named_tuple").unwrap_or(false);
        let has_type_var_tuple_type =
            read_bool_attr(item, "has_type_var_tuple_type").unwrap_or(false);
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
        let type_vars_with_variance = read_type_vars_with_variance(item);

        let snap = TypeInfoSnapshot {
            fullname,
            name,
            is_protocol,
            is_enum,
            fallback_to_any,
            meta_fallback_to_any,
            is_named_tuple,
            has_type_var_tuple_type,
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
            type_vars_with_variance,
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
        let target = match serialize_type_to_bytes(py, item) {
            Some(b) => b,
            None => continue,
        };
        let alias_tvars = read_alias_tvar_names_pub(item);
        let tvar_tuple_index = read_tvar_tuple_index_pub(item);
        let no_args: bool = item
            .getattr("no_args")
            .ok()
            .and_then(|v| v.extract().ok())
            .unwrap_or(false);
        let snap = crate::aliases::TypeAliasSnapshot {
            fullname: fullname.clone(),
            target,
            alias_tvars,
            tvar_tuple_index,
            no_args,
        };
        alias_resolver.insert(fullname, snap);
    }

    let native = NativeTypeResolver::new(resolver, alias_resolver);
    Py::new(py, native)
}

/// Read `TypeAlias.alias_tvars` as a Vec of names. Mirrors the private
/// helper in `aliases.rs` but `pub(crate)` so `build_native_resolver`
/// can reuse it without exposing the alias-iter logic.
fn read_alias_tvar_names_pub(obj: &PyAny) -> Vec<String> {
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
}
