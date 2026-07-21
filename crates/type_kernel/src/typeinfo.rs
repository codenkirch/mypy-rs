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

/// Serialize each `TypeInfo._promote` Type to bytes via mypy's WriteBuffer.
/// Returns a Vec of byte blobs; Stage 3c decodes via `wire::read_type`.
fn read_promote_bytes(py: Python<'_>, obj: &PyAny) -> Vec<Vec<u8>> {
    let promote = match obj.getattr("_promote") {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let list = match promote.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    let write_buffer_cls = match py.import("librt.internal") {
        Ok(mod_) => match mod_.getattr("WriteBuffer") {
            Ok(cls) => cls,
            Err(_) => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for item in list.iter() {
        // Each item is a mypy.types.Type; call `item.write(buf)` then
        // `buf.getvalue()`. Skip on any failure.
        let buf = match write_buffer_cls.call0() {
            Ok(b) => b,
            Err(_) => continue,
        };
        let write = match item.getattr("write") {
            Ok(w) => w,
            Err(_) => continue,
        };
        if write.call1((buf,)).is_err() {
            continue;
        }
        let bytes = match buf.getattr("getvalue") {
            Ok(g) => match g.call0() {
                Ok(v) => v,
                Err(_) => continue,
            },
            Err(_) => continue,
        };
        if let Ok(b) = bytes.extract::<Vec<u8>>() {
            out.push(b);
        }
    }
    out
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
}
