//! Stage 3e: `mypy.typeops.supported_self_type` (typeops.py:654-670).
//!
//! Is `typ` a supported kind of explicit self-type? Currently this means
//! an `X` or `Type[X]`, where `X` is an instance (other than the fully
//! generic `C[...]` form) or a type variable with an instance upper bound;
//! a callable is allowed when `allow_callable`. The Port mirrors the Python
//! body: `TypeType` recurses on its item (resetting the flags to their
//! defaults, exactly like the Python recursion `supported_self_type(typ.item)`
//! does), a callable answers `true` only when allowed, and the trailing
//! predicate answers `true` only for a `TypeVarType` or for an `Instance`
//! whose args differ from `fill_typevars(typ.type)`.
//!
//! The one non-trivial branch is `typ != fill_typevars(typ.type)`: an
//! Instance is unsupported exactly when its args ARE the class's declared
//! type parameters (the fully generic form). The Rust port reads the class's
//! declared `defn.type_vars` live from the resolver's TypeInfo map and
//! compares each arg's full `TypeVarId` identity (`raw_id`, `meta_level`,
//! `namespace`, types.py:574-576). It defers (`None`) whenever the exact
//! comparison cannot be resolved: the arg count matches but a declared tvar
//! is not a plain `TypeVarType`, or the live TypeInfo map is unavailable.
//! `None` means "let the Python shim fall through to the pure-Python body".

use pyo3::prelude::*;
use pyo3::types::PyList;

use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, ReadBuffer, Type};

/// Decode a wire `Type` blob; `None` on any read failure (defer to Python).
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `#[pyfunction]` entry for `supported_self_type` (typeops.py:654-670).
#[pyfunction]
#[pyo3(signature = (type_bytes, resolver, allow_callable=true, allow_instances=true))]
pub(crate) fn rust_supported_self_type(
    py: Python<'_>,
    type_bytes: &[u8],
    resolver: &NativeTypeResolver,
    allow_callable: bool,
    allow_instances: bool,
) -> Option<bool> {
    let typ = decode_type(type_bytes)?;
    supported_self_type_inner(py, &typ, resolver, allow_callable, allow_instances)
}

/// Core dispatch. The `TypeType` recursion resets the flags to their
/// defaults (`allow_callable=True`, `allow_instances=True`), mirroring the
/// Python body's `supported_self_type(typ.item)` which drops the caller's
/// flags. A bare non-proper node (e.g. a `TypeAliasType`) answers `false`,
/// matching the Python tail predicate.
pub(crate) fn supported_self_type_inner(
    py: Python<'_>,
    typ: &Type,
    resolver: &NativeTypeResolver,
    allow_callable: bool,
    allow_instances: bool,
) -> Option<bool> {
    match typ {
        Type::TypeType { item, .. } => supported_self_type_inner(py, item, resolver, true, true),
        Type::CallableType { .. } if allow_callable => Some(true),
        Type::TypeVarType { .. } => Some(true),
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } if allow_instances => instance_supported(
            py,
            type_ref,
            args,
            last_known_value.as_deref(),
            extra_attrs.as_ref(),
            resolver,
        ),
        _ => Some(false),
    }
}

/// The `allow_instances` tail: `typ != fill_typevars(typ.type)`.
///
/// `fill_typevars` (typevars.py:43-85) rebuilds an `Instance` whose args are
/// the class's declared type parameters and whose `last_known_value` /
/// `extra_attrs` are unset, so:
///   * any `last_known_value` / `extra_attrs` on the input makes it unequal
///     (supported) under `Instance.__eq__` (types.py:1787-1795);
///   * otherwise the args must each be the class's own `TypeVarType` (full
///     `TypeVarId` identity) for the instance to equal the generic form.
///
/// Defers (`None`) when the live TypeInfo map cannot supply the class's
/// declared tvar identities.
fn instance_supported(
    py: Python<'_>,
    type_ref: &str,
    args: &[Type],
    last_known_value: Option<&Type>,
    extra_attrs: Option<&crate::wire::ExtraAttrs>,
    resolver: &NativeTypeResolver,
) -> Option<bool> {
    if last_known_value.is_some() || extra_attrs.is_some() {
        return Some(true);
    }
    let info = resolver.live_typeinfo(py, type_ref)?;
    let defn = info.getattr("defn").ok()?;
    let tvars = defn.getattr("type_vars").ok()?.downcast::<PyList>().ok()?;
    let mut declared: Vec<(i64, i64, String)> = Vec::with_capacity(tvars.len());
    for item in tvars.iter() {
        let class_name = item.get_type().name().unwrap_or("").to_string();
        if class_name != "TypeVarType" {
            // ParamSpec / TypeVarTuple class parameters make the generic form
            // a `ParamSpecType` / `UnpackType` arg; resolving that equality
            // exactly is out of scope, so defer to Python.
            return None;
        }
        let id = item.getattr("id").ok()?;
        let raw_id: i64 = id.getattr("raw_id").and_then(|v| v.extract()).ok()?;
        let meta_level: i64 = id
            .getattr("meta_level")
            .and_then(|v| v.extract())
            .unwrap_or(0);
        let namespace: String = id
            .getattr("namespace")
            .and_then(|v| v.extract())
            .unwrap_or_default();
        declared.push((raw_id, meta_level, namespace));
    }
    if args.len() != declared.len() {
        // `fill_typevars` instantiates every class tvar, so an arity mismatch
        // cannot equal the generic form.
        return Some(true);
    }
    for (arg, (raw_id, meta_level, namespace)) in args.iter().zip(&declared) {
        let matches = match arg {
            Type::TypeVarType {
                raw_id: a_raw_id,
                meta_level: a_meta_level,
                namespace: a_namespace,
                ..
            } => a_raw_id == raw_id && a_meta_level == meta_level && a_namespace == namespace,
            _ => false,
        };
        if !matches {
            return Some(true);
        }
    }
    Some(false)
}
