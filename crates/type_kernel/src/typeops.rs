//! Stage 3e: typeops helpers from `mypy/typeops.py`.
//!
//! Ports pure-algebra helpers as standalone `#[pyfunction]`s:
//! * `rust_make_simplified_union` — wraps `setops::make_simplified_union`.
//! * `rust_simple_literal_type` — extracts fallback Instance for simple literals.
//! * `rust_is_simple_literal` — checks if a type is a simple literal.
//! * `rust_true_only` / `rust_false_only` / `rust_true_or_false` — truthiness
//!   narrowing via discriminators (Python shim performs the `copy_type` +
//!   flag mutation on live objects).
//!
//! Parity-only, default-off. The Python shim in `mypy/typeops.py` gates each
//! call behind `Options.native_type_kernel`. `None` means "Rust doesn't handle
//! this, let Python decide".

use pyo3::prelude::*;
use pyo3::IntoPy;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, LiteralValue, ReadBuffer, Type, WriteBuffer};

use crate::setops;
use crate::subtypes::SubtypeContext;

// ---------------------------------------------------------------------------
// Wire codec helpers
// ---------------------------------------------------------------------------

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

fn encode_type(t: &Type) -> Option<Vec<u8>> {
    let mut buf = WriteBuffer::new();
    wire::write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

// ---------------------------------------------------------------------------
// simple_literal_type / is_simple_literal
// ---------------------------------------------------------------------------

/// `simple_literal_type` (typeops.py:588-594): return the fallback `Instance`
/// for a simple literal. If `t` is an `Instance` with a `last_known_value`,
/// unwrap to that literal first. If `t` is a `LiteralType`, return its
/// fallback. Otherwise return `None`.
///
/// Returns the fallback as wire-encoded bytes, or `None` if `t` is not a
/// simple literal (Python `None`).
fn simple_literal_type(t: &Type) -> Option<Type> {
    let t = match t {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => lkv.as_ref(),
        _ => t,
    };
    match t {
        Type::LiteralType { fallback, .. } => Some((**fallback).clone()),
        _ => None,
    }
}

/// `is_simple_literal` (typeops.py:597-602): check if `t` is a simple literal.
///
/// A `LiteralType` is simple if its fallback is an enum or `builtins.str`.
/// An `Instance` is simple if it has a `last_known_value` whose value is a
/// string. The `is_enum` check needs the resolver snapshot; if the snapshot
/// is missing, conservatively return `false` (defer to Python).
fn is_simple_literal(t: &Type, resolver: &TypeResolver) -> Option<bool> {
    match t {
        Type::LiteralType { fallback, .. } => {
            let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                return Some(false);
            };
            if type_ref == "builtins.str" {
                return Some(true);
            }
            // enum check needs the snapshot
            let snap = resolver.get(type_ref)?;
            Some(snap.is_enum)
        }
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => {
            if let Type::LiteralType { value, .. } = lkv.as_ref() {
                Some(matches!(value, LiteralValue::Str(_)))
            } else {
                Some(false)
            }
        }
        _ => Some(false),
    }
}

// ---------------------------------------------------------------------------
// Truthiness helpers: can_be_true_default / can_be_false_default
// ---------------------------------------------------------------------------

/// Mirrors `Type.can_be_true_default()` for each variant (types.py:295-3459).
/// Returns `None` for variants where the default depends on live Python state
/// (TypeAliasType needs `alias.target`, TupleType needs `can_be_any_bool`).
pub(crate) fn can_be_true_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(false),
        Type::LiteralType { value, fallback } => {
            if !matches!(fallback.as_ref(), Type::Instance { .. }) {
                return Some(true);
            };
            // Enum literals get TRUE from the Instance fallback, plain ones use
            // bool(value). Kernel cannot tell them apart without a snapshot
            // resolver, so defer non-Bool truthiness.
            match value {
                LiteralValue::Bool(b) => Some(*b),
                _ => None,
            }
        }
        Type::UnionType { items, .. } => {
            let mut any = false;
            for item in items {
                match can_be_true_default(item) {
                    Some(true) => {
                        any = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(any)
        }
        Type::TypeAliasType { .. } => None,
        Type::TupleType { .. } => None,
        // All other variants default to True (Instance, AnyType, CallableType,
        // Overloaded, UnboundType, DeletedType, TypeType, TypeVarType,
        // ParamSpecType, TypeVarTupleType, Parameters, UnpackType).
        _ => Some(true),
    }
}

/// Mirrors `Type.can_be_false_default()` for each variant (types.py:298-3459).
pub(crate) fn can_be_false_default(t: &Type) -> Option<bool> {
    match t {
        Type::UninhabitedType { .. } => Some(false),
        Type::NoneType => Some(true),
        Type::LiteralType { value, fallback } => {
            if !matches!(fallback.as_ref(), Type::Instance { .. }) {
                return Some(true);
            };
            // See can_be_true_default: defer non-Bool literal truthiness
            // (enum-vs-plain needs the snapshot resolver).
            match value {
                LiteralValue::Bool(b) => Some(!*b),
                _ => None,
            }
        }
        Type::UnionType { items, .. } => {
            let mut any = false;
            for item in items {
                match can_be_false_default(item) {
                    Some(true) => {
                        any = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(any)
        }
        Type::TypeAliasType { .. } => None,
        Type::TupleType { .. } => None,
        _ => Some(true),
    }
}

// ---------------------------------------------------------------------------
// Truthiness discriminators
// ---------------------------------------------------------------------------

/// Result of `true_only` / `false_only` / `true_or_false`.
///
/// The Python shim maps each variant to a live `Type`:
/// * `Uninhabited` -> `UninhabitedType(line=t.line)`
/// * `None` (strict_optional off) -> `NoneType(line=t.line)`
/// * `SameType` -> `t` unchanged
/// * `CopyTrueOnly` -> `copy_type(t)` with `can_be_false=False`
/// * `CopyFalseOnly` -> `copy_type(t)` with `can_be_true=False`
/// * `CopyReset` -> `copy_type(t)` with `can_be_true=default, can_be_false=default`
/// * `LiteralEmptyStr(fallback_bytes)` -> `LiteralType("", fallback)`
/// * `LiteralZero(fallback_bytes)` -> `LiteralType(0, fallback)`
/// * `UnionNarrow(item_discs)` -> recurse on each union item (discs[i] is
///   the discriminator for items[i])
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
enum TruthinessResult {
    Uninhabited,
    NoneType,
    SameType,
    CopyTrueOnly,
    CopyFalseOnly,
    CopyReset,
    LiteralEmptyStr(Vec<u8>),
    LiteralZero(Vec<u8>),
    /// For unions: one TruthinessResult per item, so the Python shim can
    /// recurse. The outer shape is always a `make_simplified_union`.
    UnionNarrow(Vec<TruthinessResult>),
}

// ---------------------------------------------------------------------------
// true_only
// ---------------------------------------------------------------------------

/// `true_only` (typeops.py:790-817): restrict `t` to only True-ish values.
///
/// Logic:
/// 1. If `not can_be_true` -> `UninhabitedType`
/// 2. If `not can_be_false` -> `t` (already all-true)
/// 3. If `UnionType` -> union of `true_only` on each item, filtered to
///    `can_be_true` items, via `make_simplified_union`
/// 4. Else -> `copy_type(t)` with `can_be_false=False` (unless `__bool__`/
///    `__len__` ret_type says all-false, then UninhabitedType)
///
/// Step 4's `__bool__`/`__len__` lookup needs live TypeInfo -> defer (None).
/// Union recursion (step 3) recurses via discriminators.
fn true_only(t: &Type) -> Option<TruthinessResult> {
    let cbt = can_be_true_default(t)?;
    if !cbt {
        return Some(TruthinessResult::Uninhabited);
    }
    let cbf = can_be_false_default(t)?;
    if !cbf {
        return Some(TruthinessResult::SameType);
    }
    if let Type::UnionType { items, .. } = t {
        // Keep position-ordered discs for every item: the Python side maps
        // disc[i] back to t.items[i] then filters via make_simplified_union.
        // Filtering here would shift positions and mis-decode other items.
        let mut item_results = Vec::with_capacity(items.len());
        for item in items {
            item_results.push(true_only(item)?);
        }
        return Some(TruthinessResult::UnionNarrow(item_results));
    }
    // Step 4: __bool__/__len__ lookup needs live TypeInfo -> defer.
    None
}

// ---------------------------------------------------------------------------
// false_only
// ---------------------------------------------------------------------------

/// `false_only` (typeops.py:820-862): restrict `t` to only False-ish values.
///
/// Logic:
/// 1. If `not can_be_false`:
///    - strict_optional -> `UninhabitedType`
///    - non-strict -> `NoneType`
/// 2. If `not can_be_true` -> `t` (already all-false)
/// 3. If `UnionType` -> union of `false_only` on each item, filtered to
///    `can_be_false` items, via `make_simplified_union`
/// 4. If `Instance(builtins.str)` or `Instance(builtins.bytes)` ->
///    `LiteralType("", fallback=t)`
/// 5. If `Instance(builtins.int)` -> `LiteralType(0, fallback=t)`
/// 6. Else -> `__bool__`/`__len__` lookup, or `copy_type(t)` with
///    `can_be_true=False`
///
/// Step 6's method lookup and `is_final`/`is_enum` checks need live TypeInfo
/// -> defer (None). Steps 4-5 return the literal directly.
fn false_only(t: &Type, strict_optional: bool) -> Option<TruthinessResult> {
    let cbf = can_be_false_default(t)?;
    if !cbf {
        if strict_optional {
            return Some(TruthinessResult::Uninhabited);
        } else {
            return Some(TruthinessResult::NoneType);
        }
    }
    let cbt = can_be_true_default(t)?;
    if !cbt {
        return Some(TruthinessResult::SameType);
    }
    if let Type::UnionType { items, .. } = t {
        // Position-ordered discs for every item (see true_only).
        let mut item_results = Vec::with_capacity(items.len());
        for item in items {
            item_results.push(false_only(item, strict_optional)?);
        }
        return Some(TruthinessResult::UnionNarrow(item_results));
    }
    // Steps 4-5: str/bytes/int Instance -> LiteralType("", fallback) or
    // LiteralType(0, fallback). Only fire for plain Instances (no args, no
    // last_known_value) matching the Python `isinstance(t, Instance)` check.
    if let Type::Instance { type_ref, .. } = t {
        if type_ref == "builtins.str" || type_ref == "builtins.bytes" {
            let fb_bytes = encode_type(t)?;
            return Some(TruthinessResult::LiteralEmptyStr(fb_bytes));
        }
        if type_ref == "builtins.int" {
            let fb_bytes = encode_type(t)?;
            return Some(TruthinessResult::LiteralZero(fb_bytes));
        }
    }
    // Step 6: __bool__/__len__ lookup + is_final/is_enum checks need live
    // TypeInfo -> defer.
    None
}

// ---------------------------------------------------------------------------
// true_or_false
// ---------------------------------------------------------------------------

/// `true_or_false` (typeops.py:865-878): unrestricted version of `t`.
///
/// Logic:
/// 1. If `UnionType` -> union of `true_or_false` on each item via
///    `make_simplified_union`
/// 2. Else -> `copy_type(t)` with `can_be_true=default, can_be_false=default`
fn true_or_false(t: &Type) -> Option<TruthinessResult> {
    if let Type::UnionType { items, .. } = t {
        let mut item_results = Vec::with_capacity(items.len());
        for item in items {
            item_results.push(true_or_false(item)?);
        }
        return Some(TruthinessResult::UnionNarrow(item_results));
    }
    Some(TruthinessResult::CopyReset)
}

// ---------------------------------------------------------------------------
// Discriminator serialization for Python
// ---------------------------------------------------------------------------

/// Serialize a `TruthinessResult` to a Python-friendly tuple.
///
/// The encoding is a nested structure:
/// `(tag: i64, payload)` where:
/// * 0 = `Uninhabited`
/// * 1 = `NoneType`
/// * 2 = `SameType`
/// * 3 = `CopyTrueOnly`
/// * 4 = `CopyFalseOnly`
/// * 5 = `CopyReset`
/// * 6 = `LiteralEmptyStr(fallback_bytes)`
/// * 7 = `LiteralZero(fallback_bytes)`
/// * 8 = `UnionNarrow(item_discs)` — payload is a `Vec<TruthinessOut>`
type TruthinessOut = (i64, PyObject);

fn truthiness_to_py(py: Python<'_>, r: TruthinessResult) -> TruthinessOut {
    match r {
        TruthinessResult::Uninhabited => (0, py.None()),
        TruthinessResult::NoneType => (1, py.None()),
        TruthinessResult::SameType => (2, py.None()),
        TruthinessResult::CopyTrueOnly => (3, py.None()),
        TruthinessResult::CopyFalseOnly => (4, py.None()),
        TruthinessResult::CopyReset => (5, py.None()),
        TruthinessResult::LiteralEmptyStr(bytes) => {
            (6, pyo3::types::PyBytes::new(py, &bytes).into())
        }
        TruthinessResult::LiteralZero(bytes) => (7, pyo3::types::PyBytes::new(py, &bytes).into()),
        TruthinessResult::UnionNarrow(items) => {
            let py_items: Vec<TruthinessOut> =
                items.into_iter().map(|r| truthiness_to_py(py, r)).collect();
            (8, pyo3::types::PyList::new(py, py_items).into())
        }
    }
}

// ---------------------------------------------------------------------------
// #pyfunction entry points
// ---------------------------------------------------------------------------

/// `#[pyfunction]` entry for `make_simplified_union`. Takes serialized items
/// + line/column/flags + `NativeTypeResolver`. Returns encoded result bytes
/// or `None` (defer to Python).
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_make_simplified_union(
    items_bytes: &[u8],
    line: i64,
    column: i64,
    keep_erased: bool,
    contract_literals: bool,
    handle_recursive: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    // items_bytes is a LIST_GEN-tagged list of serialized types.
    let mut buf = ReadBuffer::new(items_bytes);
    let items = wire::read_type_list(&mut buf).ok()?;
    let _ = (line, column, keep_erased, handle_recursive);
    // Match Python's _remove_redundant_union_items which calls
    // is_proper_subtype. proper_subtype=True prevents Any-absorption:
    // Instance is NOT <: AnyType, so Any | C is preserved.
    let ctx = SubtypeContext::new(false, false, false, true, true, true);
    let result =
        setops::make_simplified_union(&items, &ctx, resolver.resolver(), contract_literals)?;
    encode_type(&result)
}

/// `#[pyfunction]` entry for `simple_literal_type`. Returns encoded fallback
/// Instance bytes, or `None`.
#[pyfunction]
pub(crate) fn rust_simple_literal_type(t_bytes: &[u8]) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let fallback = simple_literal_type(&t)?;
    encode_type(&fallback)
}

/// `#[pyfunction]` entry for `is_simple_literal`. Returns `Some(true)`/
/// `Some(false)` or `None` (defer to Python when snapshot lookup is needed
/// but missing).
#[pyfunction]
pub(crate) fn rust_is_simple_literal(
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    is_simple_literal(&t, resolver.resolver())
}

/// `#[pyfunction]` entry for `is_simple_literal`'s sibling
/// `is_literal_type_like` (typeops.py:1241-1257). Accepts a serialized type
/// and returns `Some(bool)` when the wire form can be fully decoded and no
/// TypeAliasType is encountered; `None` (defer to Python) otherwise.
#[pyfunction]
pub(crate) fn rust_is_literal_type_like(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    is_literal_type_like(&t)
}

/// `mypy.typeops.is_literal_type_like` — whether a (proper) type is
/// potentially a LiteralType, a Union whose items all qualify, or a TypeVar
/// whose upper bound / values qualify.
///
/// Mirrors typeops.py:1241-1257:
/// ```
/// t = get_proper_type(t)
/// if t is None: return False
/// elif isinstance(t, LiteralType): return True
/// elif isinstance(t, UnionType): return any(is_literal_type_like(item) for item in t.items)
/// elif isinstance(t, TypeVarType):
///     return is_literal_type_like(t.upper_bound) or any(is_literal_type_like(item) for item in t.values)
/// else: return False
/// ```
///
/// TypeAliasType can't be resolved to a proper type here (no target in the
/// wire form), so it returns `None` and the Python shim defers.
fn is_literal_type_like(t: &Type) -> Option<bool> {
    match t {
        Type::TypeAliasType { .. } => None,
        Type::LiteralType { .. } => Some(true),
        Type::UnionType { items, .. } => {
            for item in items {
                match is_literal_type_like(item) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        Type::TypeVarType {
            upper_bound, values, ..
        } => {
            match is_literal_type_like(upper_bound) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
            for v in values {
                match is_literal_type_like(v) {
                    Some(true) => return Some(true),
                    None => return None,
                    Some(false) => {}
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

/// `#[pyfunction]` entry for `try_getting_str_literals_from_type`
/// (typeops.py:1186-1194). Returns the Python `list[str]` of literal values,
/// or `None` to defer to Python when any candidate is not a matching
/// LiteralType.
#[pyfunction]
pub(crate) fn rust_try_getting_str_literals_from_type(
    py: Python<'_>,
    t_bytes: &[u8],
) -> Option<PyObject> {
    let t = decode_type(t_bytes)?;
    let vals = try_getting_literals(&t, "builtins.str", LiteralKind::Str)?;
    Some(
        pyo3::types::PyList::new(
            py,
            vals.into_iter().map(|v| v.into_pyobject(py)),
        )
        .into(),
    )
}

/// `#[pyfunction]` entry for `try_getting_int_literals_from_type`
/// (typeops.py:1197-1205). Returns the Python `list[int]` of literal values,
/// or `None` to defer to Python when any candidate is not a matching
/// LiteralType.
#[pyfunction]
pub(crate) fn rust_try_getting_int_literals_from_type(
    py: Python<'_>,
    t_bytes: &[u8],
) -> Option<PyObject> {
    let t = decode_type(t_bytes)?;
    let vals = try_getting_literals(&t, "builtins.int", LiteralKind::Int)?;
    Some(
        pyo3::types::PyList::new(
            py,
            vals.into_iter().map(|v| v.into_pyobject(py)),
        )
        .into(),
    )
}

/// `#[pyfunction]` entry for `try_getting_literals_from_type` with bool
/// target (typeops.py:1236-1256, called from dataclasses.py:897 with
/// `target_literal_type=bool`). Returns the Python `list[bool]` of literal
/// values (real Python bools, not 0/1: dataclasses uses the value directly
/// as the field's default), or `None` to defer to Python.
#[pyfunction]
pub(crate) fn rust_try_getting_bool_literals_from_type(
    py: Python<'_>,
    t_bytes: &[u8],
) -> Option<PyObject> {
    let t = decode_type(t_bytes)?;
    let vals = try_getting_literals(&t, "builtins.bool", LiteralKind::Bool)?;
    Some(
        pyo3::types::PyList::new(
            py,
            vals.into_iter().map(|v| v.into_pyobject(py)),
        )
        .into(),
    )
}

/// `#[pyfunction]` entry for `try_getting_instance_fallback`
/// (typeops.py:1525-1539). Returns the Instance fallback for the type when
/// one exists, encoded as wire bytes, or `None` to defer to Python.
#[pyfunction]
pub(crate) fn rust_try_getting_instance_fallback(t_bytes: &[u8]) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let fallback = try_getting_instance_fallback(&t)?;
    encode_type(&fallback)
}

/// `#[pyfunction]` entry for `try_expanding_sum_type_to_union`
/// (typeops.py:1292-1333). Returns the expanded type encoded as wire bytes,
/// or `None` to defer to Python when a Union branch needs recursive-flatten
/// semantics the wire format cannot express (a recursive TypeAliasType), or
/// an Instance's enum snapshot is missing from the resolver.
#[pyfunction]
#[pyo3(signature = (t_bytes, target_fullname, strict_optional, resolver))]
pub(crate) fn rust_try_expanding_sum_type_to_union(
    t_bytes: &[u8],
    target_fullname: Option<String>,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let t = decode_type(t_bytes)?;
    let result =
        try_expanding_sum_type_to_union_inner(&t, target_fullname.as_deref(), strict_optional, resolver.resolver())?;
    encode_type(&result)
}

/// `UnionType.make_union` (types.py:3502-3509): more than one item makes a
/// fresh `UnionType`, one item returns the item itself, and zero items
/// returns `UninhabitedType`. The union is always `uses_pep604_syntax=False`
/// (make_union's default) and its truthiness flags are computed eagerly from
/// the items exactly as Python's lazy `can_be_true/can_be_false_default`
/// (`any(item.can_be_true)`) would compute them.
fn make_union(items: Vec<Type>) -> Type {
    match items.len() {
        0 => Type::UninhabitedType { ambiguous: false },
        1 => items.into_iter().next().unwrap(),
        _ => {
            // UnionType.__init__ flattens nested unions (types.py:3465) with
            // handle_type_alias_type=False, so type aliases pass through and
            // the helper never returns None here. Truthiness iterates the
            // flattened items, matching Python's lazy any(item.can_be_true).
            let flat = crate::visitor::flatten_nested_unions_inner(&items, false, true)
                .unwrap_or_else(|| items.clone());
            let can_be_true = flat.iter().any(crate::setops::union_item_can_be_true);
            let can_be_false = flat.iter().any(crate::setops::union_item_can_be_false);
            Type::UnionType {
                items: flat,
                uses_pep604_syntax: false,
                can_be_true,
                can_be_false,
            }
        }
    }
}

/// `mypy.typeops.try_expanding_sum_type_to_union` (typeops.py:1292-1333):
/// expand a bool Instance into `Literal[True, False]` or an enum Instance
/// into the union of its member literals, recursively down a Union.
///
/// ```
/// typ = get_proper_type(typ)
/// if isinstance(typ, UnionType):
///     items = [try_expanding_sum_type_to_union(item, target_fullname) for
///              item in remove_dups(flatten_nested_unions(typ.relevant_items()))]
///     return UnionType.make_union(items)
/// if isinstance(typ, Instance) and
///         (target_fullname is None or typ.type.fullname == target_fullname):
///     if typ.type.fullname == "builtins.bool":
///         return UnionType([LiteralType(True, typ), LiteralType(False, typ)])
///     if typ.type.is_enum:
///         items = [LiteralType(name, typ) for name in typ.type.enum_members]
///         if not items: return typ
///         return UnionType.make_union(items)
/// return typ
/// ```
///
/// A `TypeAliasType` cannot be resolved here (no target in the wire form), so
/// it returns `None` and the Python shim defers — matching `get_proper_type`.
/// Same deferral when a Union branch needs recursive alias flattening or an
/// enum snapshot is missing from the resolver.
fn try_expanding_sum_type_to_union_inner(
    typ: &Type,
    target_fullname: Option<&str>,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Type> {
    // get_proper_type: a TypeAliasType has no proper form in the wire format.
    match typ {
        Type::TypeAliasType { .. } => return None,
        _ => {}
    }
    match typ {
        Type::UnionType { items, .. } => {
            // typ.relevant_items(): with strict_optional off, NoneType items
            // are dropped (types.py:3517-3522).
            let relevant: Vec<Type> = if strict_optional {
                items.clone()
            } else {
                items
                    .iter()
                    .filter(|i| !matches!(i, Type::NoneType))
                    .cloned()
                    .collect()
            };
            // flatten_nested_unions(relevant, type_alias_type=True, recursive=True).
            // A recursive TypeAliasType inside yields None (defer); by then the
            // relevant items may include NoneTypes, which must not be dropped
            // twice, so `relevant` is the pre-None-filtered input.
            let flat = crate::visitor::flatten_nested_unions_inner(&relevant, true, true)?;
            let deduped = crate::visitor::remove_dups_inner(&flat);
            let mut out = Vec::with_capacity(deduped.len());
            for item in &deduped {
                out.push(try_expanding_sum_type_to_union_inner(
                    item,
                    target_fullname,
                    strict_optional,
                    resolver,
                )?);
            }
            Some(make_union(out))
        }
        Type::Instance { type_ref, .. } => {
            if let Some(tf) = target_fullname {
                if type_ref != tf {
                    return Some(typ.clone());
                }
            }
            if type_ref == "builtins.bool" {
                let lit = |value: bool| Type::LiteralType {
                    fallback: Box::new(typ.clone()),
                    value: LiteralValue::Bool(value),
                };
                return Some(make_union(vec![lit(true), lit(false)]));
            }
            let snap = resolver.get(type_ref)?;
            if !snap.is_enum {
                return Some(typ.clone());
            }
            let items: Vec<Type> = snap
                .enum_members
                .iter()
                .map(|name| Type::LiteralType {
                    fallback: Box::new(typ.clone()),
                    value: LiteralValue::Str(name.clone()),
                })
                .collect();
            if items.is_empty() {
                return Some(typ.clone());
            }
            Some(make_union(items))
        }
        _ => Some(typ.clone()),
    }
}

/// `mypy.typeops.try_getting_instance_fallback` — the Instance fallback for
/// a proper type, or `None` if it has no such fallback.
///
/// Mirrors typeops.py:1525-1539:
/// ```
/// t = get_proper_type(t)
/// if isinstance(t, Instance): return t
/// elif isinstance(t, LiteralType): return t.fallback
/// elif isinstance(t, (NoneType, AnyType)): return None
/// elif isinstance(t, FunctionLike): return t.fallback
/// elif isinstance(t, TypeVarType): return try_getting_instance_fallback(t.upper_bound)
/// elif isinstance(t, TupleType): return t.partial_fallback
/// elif isinstance(t, TypedDictType): return t.fallback
/// else: return None
/// ```
/// `Overloaded.fallback` is `items[0].fallback` (types.py:2758), so the
/// Overloaded arm recurses through the first item. A TypeAliasType can't be
/// resolved here (no target in the wire form), so it returns `None` and the
/// caller defers to Python.
fn try_getting_instance_fallback(t: &Type) -> Option<Type> {
    match t {
        Type::Instance { .. } => Some(t.clone()),
        Type::LiteralType { fallback, .. } => Some((**fallback).clone()),
        Type::CallableType { fallback, .. } => Some((**fallback).clone()),
        Type::Overloaded { items } => items.first().and_then(try_getting_instance_fallback),
        Type::TypeVarType { upper_bound, .. } => try_getting_instance_fallback(upper_bound),
        Type::TupleType { partial_fallback, .. } => Some((**partial_fallback).clone()),
        Type::TypedDictType { fallback, .. } => Some((**fallback).clone()),
        // NoneType (fast path) and AnyType have no fallback, matching Python.
        Type::NoneType | Type::AnyType { .. } => None,
        _ => None,
    }
}

/// The shared walk behind `try_getting_str/int_literals_from_type`
/// (typeops.py:1211-1264). Returns the scalar literal values when every
/// candidate is a `LiteralType` whose fallback fullname equals
/// `target_fullname` and whose value is of the `expect` kind; `None`
/// (defer) otherwise.
///
/// Mirrors the Python walk exactly: one candidate (the type itself or its
/// `last_known_value`) or the union items, each checked against the target.
fn try_getting_literals(t: &Type, target_fullname: &str, expect: LiteralKind) -> Option<Vec<Scalar>> {
    let candidates = match t {
        Type::Instance {
            last_known_value, ..
        } => last_known_value
            .as_ref()
            .map(|v| vec![v.as_ref().clone()])
            .unwrap_or_else(|| vec![t.clone()]),
        Type::UnionType { items, .. } => items.clone(),
        _ => vec![t.clone()],
    };
    let mut out: Vec<Scalar> = Vec::new();
    for c in candidates {
        // Python: get_proper_types(...) per candidate; a TypeAliasType is not
        // a proper type, resolving here would need a target we don't have.
        match c {
            Type::LiteralType { fallback, value } => {
                let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                    return None;
                };
                if type_ref != target_fullname {
                    return None;
                }
                let s = match value {
                    LiteralValue::Str(s) if expect == LiteralKind::Str => Scalar::Str(s.clone()),
                    LiteralValue::Int(i) if expect == LiteralKind::Int => Scalar::Int(i),
                    LiteralValue::Bool(b) if expect == LiteralKind::Int => {
                        // Python: isinstance(True, int) is True, so Literal[True]
                        // counts as an int literal.
                        Scalar::Int(if b { 1 } else { 0 })
                    }
                    LiteralValue::Bool(b) if expect == LiteralKind::Bool => Scalar::Bool(b),
                    _ => return None,
                };
                out.push(s);
            }
            _ => return None,
        }
    }
    Some(out)
}

#[derive(Clone, Copy, PartialEq)]
enum LiteralKind {
    Str,
    Int,
    Bool,
}

/// The scalar values returned by `try_getting_literals`: Python strings,
/// ints (including bools-as-ints), or bools.
#[derive(Debug, PartialEq)]
enum Scalar {
    Str(String),
    Int(i64),
    Bool(bool),
}

impl Scalar {
    fn into_pyobject(self, py: Python<'_>) -> PyObject {
        match self {
            Scalar::Str(s) => s.into_py(py),
            Scalar::Int(i) => i.into_py(py),
            Scalar::Bool(b) => b.into_py(py),
        }
    }
}

/// `#[pyfunction]` entry for `true_only`. Returns a truthiness discriminator
/// tuple or `None` (defer to Python).
#[pyfunction]
pub(crate) fn rust_true_only(t_bytes: &[u8]) -> Option<TruthinessOut> {
    let t = decode_type(t_bytes)?;
    let result = true_only(&t)?;
    Python::with_gil(|py| Some(truthiness_to_py(py, result)))
}

/// `#[pyfunction]` entry for `false_only`.
#[pyfunction]
pub(crate) fn rust_false_only(t_bytes: &[u8], strict_optional: bool) -> Option<TruthinessOut> {
    let t = decode_type(t_bytes)?;
    let result = false_only(&t, strict_optional)?;
    Python::with_gil(|py| Some(truthiness_to_py(py, result)))
}

/// `#[pyfunction]` entry for `true_or_false`.
#[pyfunction]
pub(crate) fn rust_true_or_false(t_bytes: &[u8]) -> Option<TruthinessOut> {
    let t = decode_type(t_bytes)?;
    let result = true_or_false(&t)?;
    Python::with_gil(|py| Some(truthiness_to_py(py, result)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use crate::wire::{LiteralValue, Type};

    #[test]
    fn simple_literal_type_extracts_fallback() {
        let fallback = Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let lit = Type::LiteralType {
            fallback: Box::new(fallback.clone()),
            value: LiteralValue::Str("hello".to_string()),
        };
        let result = simple_literal_type(&lit).unwrap();
        assert_eq!(result, fallback);
    }

    #[test]
    fn simple_literal_type_unwraps_last_known_value() {
        let lkv = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(42),
        };
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv.clone())),
            extra_attrs: None,
        };
        let result = simple_literal_type(&inst).unwrap();
        // simple_literal_type unwraps Instance->lkv->lkv.fallback (the int Instance).
        assert_eq!(
            result,
            Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }
        );
    }

    #[test]
    fn simple_literal_type_returns_none_for_non_literal() {
        let inst = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        assert!(simple_literal_type(&inst).is_none());
    }

    #[test]
    fn true_only_none_type_returns_uninhabited() {
        let t = Type::NoneType;
        let result = true_only(&t).unwrap();
        assert!(matches!(result, TruthinessResult::Uninhabited));
    }

    #[test]
    fn true_only_literal_true_returns_same() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(true),
        };
        let result = true_only(&t).unwrap();
        assert!(matches!(result, TruthinessResult::SameType));
    }

    #[test]
    fn false_only_none_type_returns_same() {
        let t = Type::NoneType;
        let result = false_only(&t, true).unwrap();
        assert!(matches!(result, TruthinessResult::SameType));
    }

    #[test]
    fn false_only_literal_false_returns_same() {
        let t = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(false),
        };
        let result = false_only(&t, true).unwrap();
        assert!(matches!(result, TruthinessResult::SameType));
    }

    #[test]
    fn true_or_false_instance_returns_copy_reset() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = true_or_false(&t).unwrap();
        assert!(matches!(result, TruthinessResult::CopyReset));
    }

    #[test]
    fn true_only_union_narrows_items() {
        // NoneType can_be_true=False -> discarded (Uninhabited, position kept).
        // LiteralType(True): can_be_false=False -> SameType.
        let t = Type::UnionType {
            items: vec![
                Type::NoneType,
                Type::LiteralType {
                    fallback: Box::new(Type::Instance {
                        type_ref: "builtins.bool".to_string(),
                        args: vec![],
                        last_known_value: None,
                        extra_attrs: None,
                    }),
                    value: LiteralValue::Bool(true),
                },
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = true_only(&t).unwrap();
        match result {
            TruthinessResult::UnionNarrow(items) => {
                // Positions preserved: NoneType -> Uninhabited, LiteralType(True)
                // -> SameType. Python remaps result[i] to t.items[i]
                // positionally (typeops.py:878).
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], TruthinessResult::Uninhabited);
                assert_eq!(items[1], TruthinessResult::SameType);
            }
            _ => panic!("expected UnionNarrow"),
        }
    }

    #[test]
    fn false_only_str_returns_literal_empty() {
        let t = Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = false_only(&t, true).unwrap();
        match result {
            TruthinessResult::LiteralEmptyStr(_) => {}
            _ => panic!("expected LiteralEmptyStr"),
        }
    }

    #[test]
    fn false_only_int_returns_literal_zero() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let result = false_only(&t, true).unwrap();
        match result {
            TruthinessResult::LiteralZero(_) => {}
            _ => panic!("expected LiteralZero"),
        }
    }

    // ------------------------------------------------------------------
    // is_literal_type_like
    // ------------------------------------------------------------------

    fn lit_str(value: &str) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.str".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Str(value.to_string()),
        }
    }

    fn plain_instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn literal_type_like_is_true() {
        assert_eq!(is_literal_type_like(&lit_str("x")), Some(true));
    }

    #[test]
    fn literal_type_like_instance_is_false() {
        assert_eq!(is_literal_type_like(&plain_instance("builtins.str")), Some(false));
    }

    #[test]
    fn literal_type_like_union_of_literals_is_true() {
        let t = Type::UnionType {
            items: vec![lit_str("a"), lit_str("b")],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_union_mixed_is_true_via_any() {
        // Python uses `any(...)` over union items, so a single literal item
        // makes the whole union "literal-like" regardless of the others.
        let t = Type::UnionType {
            items: vec![lit_str("a"), plain_instance("builtins.int")],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_typevar_bound_to_literal_is_true() {
        let t = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "1".to_string(),
            values: vec![],
            upper_bound: Box::new(lit_str("x")),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 1,
            meta_level: 0,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_typevar_values_literal_is_true() {
        let t = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "1".to_string(),
            values: vec![lit_str("v")],
            upper_bound: Box::new(plain_instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 1,
            meta_level: 0,
        };
        assert_eq!(is_literal_type_like(&t), Some(true));
    }

    #[test]
    fn literal_type_like_alias_defers() {
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(is_literal_type_like(&t), None);
    }

    #[test]
    fn literal_type_like_callable_is_false() {
        let t = Type::CallableType {
            fallback: Box::new(plain_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        assert_eq!(is_literal_type_like(&t), Some(false));
    }

    // ------------------------------------------------------------------
    // try_getting_literals (str / int variants)
    // ------------------------------------------------------------------

    fn lit_int(value: i64) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(value),
        }
    }

    fn lit_bool(value: bool) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(value),
        }
    }

    fn union_of(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        }
    }

    fn instance_with_last_known(value: Type) -> Type {
        Type::Instance {
            type_ref: "builtins.str".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(value)),
            extra_attrs: None,
        }
    }

    #[test]
    fn literals_str_literal_returns_value() {
        assert_eq!(
            try_getting_literals(&lit_str("x"), "builtins.str", LiteralKind::Str),
            Some(vec![Scalar::Str("x".to_string())])
        );
    }

    #[test]
    fn literals_int_literal_returns_value() {
        assert_eq!(
            try_getting_literals(&lit_int(42), "builtins.int", LiteralKind::Int),
            Some(vec![Scalar::Int(42)])
        );
    }

    #[test]
    fn literals_bool_literal_counts_as_int() {
        // Python: isinstance(True, int) is True, so Literal[True] is an int.
        assert_eq!(
            try_getting_literals(&lit_bool(true), "builtins.int", LiteralKind::Int),
            Some(vec![Scalar::Int(1)])
        );
        assert_eq!(
            try_getting_literals(&lit_bool(false), "builtins.int", LiteralKind::Int),
            Some(vec![Scalar::Int(0)])
        );
    }

    #[test]
    fn literals_plain_instance_defers() {
        assert_eq!(
            try_getting_literals(&plain_instance("builtins.str"), "builtins.str", LiteralKind::Str),
            None
        );
    }

    #[test]
    fn literals_instance_with_last_known_uses_it() {
        let t = instance_with_last_known(lit_str("x"));
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Some(vec![Scalar::Str("x".to_string())])
        );
    }

    #[test]
    fn literals_union_of_matching_literals_returns_all() {
        let t = union_of(vec![lit_str("a"), lit_str("b")]);
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            Some(vec![Scalar::Str("a".to_string()), Scalar::Str("b".to_string())])
        );
    }

    #[test]
    fn literals_union_mixed_kind_defers() {
        // Python returns None as soon as any candidate is not a matching
        // literal.
        let t = union_of(vec![lit_str("a"), lit_int(1)]);
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            None
        );
    }

    #[test]
    fn literals_union_wrong_fallback_defers() {
        let t = union_of(vec![lit_str("a"), lit_str("b")]);
        assert_eq!(
            try_getting_literals(&t, "builtins.int", LiteralKind::Int),
            None
        );
    }

    #[test]
    fn literals_type_alias_defers() {
        let t = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
        };
        assert_eq!(
            try_getting_literals(&t, "builtins.str", LiteralKind::Str),
            None
        );
    }

    fn lit_bool_typed(value: bool) -> Type {
        Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(value),
        }
    }

    #[test]
    fn literals_bool_kind_returns_bool() {
        assert_eq!(
            try_getting_literals(&lit_bool_typed(true), "builtins.bool", LiteralKind::Bool),
            Some(vec![Scalar::Bool(true)])
        );
        assert_eq!(
            try_getting_literals(&lit_bool_typed(false), "builtins.bool", LiteralKind::Bool),
            Some(vec![Scalar::Bool(false)])
        );
    }

    #[test]
    fn literals_bool_kind_union_returns_all() {
        let t = union_of(vec![lit_bool_typed(true), lit_bool_typed(false)]);
        assert_eq!(
            try_getting_literals(&t, "builtins.bool", LiteralKind::Bool),
            Some(vec![Scalar::Bool(true), Scalar::Bool(false)])
        );
    }

    #[test]
    fn literals_bool_kind_wrong_fallback_defers() {
        // Python requires lit.fallback.type.fullname == "builtins.bool"; an
        // int-backed Literal[True] does not match the bool target.
        assert_eq!(
            try_getting_literals(&lit_bool(true), "builtins.bool", LiteralKind::Bool),
            None
        );
    }

    #[test]
    fn literals_bool_kind_int_literal_defers() {
        // Literal[1] has an int fallback: not a bool literal for the bool
        // target.
        assert_eq!(
            try_getting_literals(&lit_int(1), "builtins.bool", LiteralKind::Bool),
            None
        );
    }

    // ------------------------------------------------------------------
    // try_getting_instance_fallback
    // ------------------------------------------------------------------

    #[test]
    fn instance_fallback_returns_instance_itself() {
        let inst = plain_instance("builtins.int");
        assert_eq!(try_getting_instance_fallback(&inst), Some(inst.clone()));
    }

    #[test]
    fn instance_fallback_unwraps_literal_to_literal_fallback() {
        let lit = lit_str("hello");
        let result = try_getting_instance_fallback(&lit).unwrap();
        assert_eq!(
            result,
            plain_instance("builtins.str")
        );
    }

    #[test]
    fn instance_fallback_none_type_defers() {
        assert_eq!(try_getting_instance_fallback(&Type::NoneType), None);
    }

    #[test]
    fn instance_fallback_any_type_defers() {
        let any = Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(try_getting_instance_fallback(&any), None);
    }

    #[test]
    fn instance_fallback_typevar_recurses_upper_bound() {
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "test".to_string(),
            values: vec![],
            upper_bound: Box::new(plain_instance("builtins.object")),
            default: Box::new(Type::AnyType {
                type_of_any: 1,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 1,
            meta_level: 1,
        };
        assert_eq!(
            try_getting_instance_fallback(&tv),
            Some(plain_instance("builtins.object"))
        );
    }

    #[test]
    fn instance_fallback_typevar_none_upper_bound_defers() {
        let tv = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "test".to_string(),
            values: vec![],
            upper_bound: Box::new(Type::NoneType),
            default: Box::new(Type::NoneType),
            variance: 1,
            meta_level: 1,
        };
        assert_eq!(try_getting_instance_fallback(&tv), None);
    }

    #[test]
    fn instance_fallback_tuple_uses_partial_fallback() {
        let tup = Type::TupleType {
            partial_fallback: Box::new(plain_instance("builtins.tuple")),
            items: vec![],
            implicit: true,
        };
        assert_eq!(
            try_getting_instance_fallback(&tup),
            Some(plain_instance("builtins.tuple"))
        );
    }

    #[test]
    fn instance_fallback_typeddict_uses_fallback() {
        let td = Type::TypedDictType {
            fallback: Box::new(plain_instance("builtins.dict")),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        assert_eq!(
            try_getting_instance_fallback(&td),
            Some(plain_instance("builtins.dict"))
        );
    }

    #[test]
    fn instance_fallback_callable_uses_fallback() {
        let callable = Type::CallableType {
            fallback: Box::new(plain_instance("builtins.function")),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(Type::NoneType),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        assert_eq!(
            try_getting_instance_fallback(&callable),
            Some(plain_instance("builtins.function"))
        );
    }

    #[test]
    fn instance_fallback_overloaded_uses_first_item() {
        let overloaded = Type::Overloaded {
            items: vec![Type::CallableType {
                fallback: Box::new(plain_instance("builtins.function")),
                instance_type: None,
                is_ellipsis_args: false,
                implicit: false,
                is_bound: false,
                from_concatenate: false,
                imprecise_arg_kinds: false,
                unpack_kwargs: false,
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                ret_type: Box::new(Type::NoneType),
                name: None,
                variables: vec![],
                type_guard: None,
                type_is: None,
            }],
        };
        assert_eq!(
            try_getting_instance_fallback(&overloaded),
            Some(plain_instance("builtins.function"))
        );
    }

    #[test]
    fn instance_fallback_alias_defers() {
        let alias = Type::TypeAliasType {
            type_ref: "mod.Alias".to_string(),
            args: vec![],
        };
        assert_eq!(try_getting_instance_fallback(&alias), None);
    }

    // ------------------------------------------------------------------
    // try_expanding_sum_type_to_union
    // ------------------------------------------------------------------

    fn lit_enum(color: &Type, member: &str) -> Type {
        Type::LiteralType {
            fallback: Box::new(color.clone()),
            value: LiteralValue::Str(member.to_string()),
        }
    }

    fn resolver_with_enum(members: Vec<String>) -> TypeResolver {
        let mut r = TypeResolver::new();
        // builtins.int must resolve (as a non-enum) for non-target Instance
        // branches: otherwise lookup fails and the whole expansion defers.
        let mut int_snap = TypeInfoSnapshot::default();
        int_snap.fullname = "builtins.int".to_string();
        r.insert("builtins.int".to_string(), int_snap);
        let mut color = TypeInfoSnapshot::default();
        color.fullname = "tests.Color".to_string();
        color.is_enum = true;
        color.enum_members = members;
        r.insert("tests.Color".to_string(), color);
        r
    }

    #[test]
    fn expand_sum_enum_expands_to_member_literals() {
        let color = plain_instance("tests.Color");
        let r = resolver_with_enum(vec!["RED".to_string(), "GREEN".to_string(), "BLUE".to_string()]);
        let result = try_expanding_sum_type_to_union_inner(&color, None, true, &r).unwrap();
        match &result {
            Type::UnionType { items, uses_pep604_syntax, .. } => {
                assert!(!uses_pep604_syntax);
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], lit_enum(&color, "RED"));
                assert_eq!(items[1], lit_enum(&color, "GREEN"));
                assert_eq!(items[2], lit_enum(&color, "BLUE"));
            }
            _ => panic!("expected Union"),
        }
        // truthiness: enum member literals inherit fallback Instance truthiness
        // (both True in Python, canonical Type defaults), so the member union
        // can_be_true and can_be_false are both true.
        assert!(crate::setops::union_item_can_be_true(&result));
        assert!(crate::setops::union_item_can_be_false(&result));
    }

    #[test]
    fn expand_sum_enum_with_no_members_returns_instance() {
        let color = plain_instance("tests.Color");
        let r = resolver_with_enum(vec![]);
        let result =
            try_expanding_sum_type_to_union_inner(&color, None, true, &r).unwrap();
        assert_eq!(result, color);
    }

    #[test]
    fn expand_sum_bool_expands_to_literals() {
        let b = plain_instance("builtins.bool");
        let r = resolver_with_enum(vec![]);
        let result = try_expanding_sum_type_to_union_inner(&b, None, true, &r).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(
                    items[0],
                    Type::LiteralType {
                        fallback: Box::new(b.clone()),
                        value: LiteralValue::Bool(true),
                    }
                );
                assert_eq!(
                    items[1],
                    Type::LiteralType {
                        fallback: Box::new(b.clone()),
                        value: LiteralValue::Bool(false),
                    }
                );
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn expand_sum_bool_requires_matching_target() {
        let b = plain_instance("builtins.bool");
        let r = resolver_with_enum(vec![]);
        // target different from the instance fullname -> returned unchanged.
        let result =
            try_expanding_sum_type_to_union_inner(&b, Some("builtins.int"), true, &r).unwrap();
        assert_eq!(result, b);
    }

    #[test]
    fn expand_sum_non_enum_instance_unchanged() {
        let i = plain_instance("builtins.int");
        let r = resolver_with_enum(vec![]);
        let result = try_expanding_sum_type_to_union_inner(&i, None, true, &r).unwrap();
        assert_eq!(result, i);
    }

    #[test]
    fn expand_sum_union_expands_enum_but_not_int() {
        let color = plain_instance("tests.Color");
        let i = plain_instance("builtins.int");
        let r = resolver_with_enum(vec!["RED".to_string(), "GREEN".to_string()]);
        let t = Type::UnionType {
            items: vec![color.clone(), i.clone()],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        let result = try_expanding_sum_type_to_union_inner(&t, None, true, &r).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], lit_enum(&color, "RED"));
                assert_eq!(items[1], lit_enum(&color, "GREEN"));
                assert_eq!(items[2], i);
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn expand_sum_union_drops_none_without_strict_optional() {
        let color = plain_instance("tests.Color");
        let r = resolver_with_enum(vec!["RED".to_string()]);
        let t = Type::UnionType {
            items: vec![Type::NoneType, color.clone()],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        let result = try_expanding_sum_type_to_union_inner(&t, None, false, &r).unwrap();
        // NoneType dropped (strict_optional off), enum expands to a single
        // literal, make_union of one item returns the literal itself.
        assert_eq!(result, lit_enum(&color, "RED"));
    }

    #[test]
    fn expand_sum_union_strict_optional_keeps_none() {
        let i = plain_instance("builtins.int");
        let r = resolver_with_enum(vec![]);
        let t = Type::UnionType {
            items: vec![Type::NoneType, i.clone()],
            uses_pep604_syntax: true,
            can_be_true: true,
            can_be_false: true,
        };
        let result = try_expanding_sum_type_to_union_inner(&t, None, true, &r).unwrap();
        match result {
            Type::UnionType { items, .. } => {
                // strict_optional on: NoneType is a relevant item; no enum or
                // bool to expand, so both items survive as-is (NoneType is not
                // removed by make_union, and remove_dups keeps both).
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], Type::NoneType);
                assert_eq!(items[1], i);
            }
            _ => panic!("expected Union"),
        }
    }

    #[test]
    fn expand_sum_alias_defers() {
        let alias = Type::TypeAliasType {
            type_ref: "mod.Alias".to_string(),
            args: vec![],
        };
        assert_eq!(
            try_expanding_sum_type_to_union_inner(&alias, None, true, &TypeResolver::new()),
            None
        );
    }
}
