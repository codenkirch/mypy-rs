//! Native port of the base-class classification front of
//! `mypy.semanal.SemanticAnalyzer.clean_up_bases_and_infer_type_variables`
//! (semanal.py:2709-2804).
//!
//! The function visits the base expressions of a class definition and
//! decides which ones are `Generic[...]` / `Protocol[...]` type-variable
//! declarations (removed, their type vars declared for the class) and which
//! are bare `Protocol` bases (removed, class becomes a protocol). The Rust
//! port owns that per-base decision given the resolved symbol facts the
//! Python shim already computed (it performs the same `lookup_qualified`
//! calls in the same order, so lookup side effects are identical). Python
//! keeps every side effect: the `analyze_type_expr` / `expr_to_unanalyzed_type`
//! preprocessing, the tvar extraction loop, the error messages, the
//! `removed_base_type_exprs` bookkeeping, and the `tvar_defs_from_tvars`
//! binding. A `None` result means the shim could not supply clean facts and
//! the pure-Python checks run unchanged.

use std::collections::HashSet;

use pyo3::class::basic::CompareOp;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyString, PyTuple, PyType};

use crate::typeanal_queries::{has_explicit_any_inner, EXPLICIT, FROM_UNIMPORTED_TYPE};
use crate::wire::{read_type, ReadBuffer, Type};

/// Fetch a class from `mypy.nodes`. Mirrors the private helper in
/// `checker_functions.rs` / `checker_visitor.rs`.
fn nodes_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    py.import("mypy.nodes")?
        .getattr(name)?
        .downcast::<PyType>()
        .map_err(Into::into)
}

/// Action tags handed to the Python shim, index-aligned with the base list:
/// - `KEEP`: not a Generic/Protocol declaration; nothing to remove.
/// - `GENERIC`: `typing.Generic` (with or without args); removed, its type
///   vars are declared for the class, `is_protocol` unchanged.
/// - `PROTOCOL_GENERIC`: `typing.Protocol` / `typing_extensions.Protocol`
///   with args; removed, its type vars declared, `is_protocol` set.
/// - `BARE_PROTOCOL`: a Protocol name without args; removed, `is_protocol`
///   set, no type vars.
pub(crate) const ACTION_KEEP: i64 = 1;
pub(crate) const ACTION_GENERIC: i64 = 2;
pub(crate) const ACTION_PROTOCOL_GENERIC: i64 = 3;
pub(crate) const ACTION_BARE_PROTOCOL: i64 = 4;

/// Action tags for the `six.with_metaclass` base-side classifier:
/// - `ACTION_NOT_WITH_METACLASS`: the base is not a compat-helper call.
/// - `ACTION_WITH_METACLASS`: the base is `six.with_metaclass(M, B1, ...)`
///   with all positional args; Python sets `with_meta_expr` and rewrites
///   `defn.base_type_exprs`.
pub(crate) const ACTION_NOT_WITH_METACLASS: i64 = 0;
pub(crate) const ACTION_WITH_METACLASS: i64 = 1;

/// The three compat-helper fullnames matched by the
/// `infer_metaclass_and_bases_from_compat_helpers` base-side block
/// (semanal.py:3329-3333), single-sourced here.
const WITH_METACLASS_FULLNAMES: [&str; 3] = [
    "six.with_metaclass",
    "future.utils.with_metaclass",
    "past.utils.with_metaclass",
];

/// The special `Generic` name is a fixed constant, mirroring
/// `sym.node.fullname == "typing.Generic"` in semanal.py:2824. The
/// `PROTOCOL_NAMES` set travels from `mypy.types` so the names stay
/// single-sourced in Python.
const GENERIC_FULLNAME: &str = "typing.Generic";

/// `clean_up_bases_and_infer_type_variables` per-base classifier — mirrors
/// the branch order of semanal.py:2817-2843 + 2768-2774.
///
/// `fullname` is the resolved `sym.node.fullname` for an `UnboundType` base
/// (or `None` when the lookup failed / the node is missing). `has_args` is
/// `bool(base.args)`. The shim only calls Rust for `UnboundType` bases whose
/// translation succeeded; every other base is skipped or kept on the Python
/// side.
///
/// Branch order mirrors Python exactly: `typing.Generic` wins first (bare
/// `Generic` is still a declaration, semanal.py:2824), then `Protocol`
/// names split on whether args are present (`Protocol[...]` declares type
/// vars, bare `Protocol` only flags the class as a protocol).
#[pyfunction]
#[pyo3(signature = (fullname, in_protocol_names, has_args))]
pub(crate) fn rust_clean_up_bases(
    fullname: Option<String>,
    in_protocol_names: bool,
    has_args: bool,
) -> PyResult<i64> {
    match fullname.as_deref() {
        Some(GENERIC_FULLNAME) => Ok(ACTION_GENERIC),
        Some(_) if in_protocol_names => Ok(if has_args {
            ACTION_PROTOCOL_GENERIC
        } else {
            ACTION_BARE_PROTOCOL
        }),
        Some(_) | None => Ok(ACTION_KEEP),
    }
}

/// Normalize a fullname argument (str, tuple of str, or list of str) into a
/// set. The magic-base names arrive as tuples, the core-builtin names as a
/// list, so all three shapes are accepted.
fn normalize_names(names: &PyAny) -> PyResult<HashSet<String>> {
    if let Ok(s) = names.downcast::<PyString>() {
        return Ok([s.to_str()?.to_string()].into_iter().collect());
    }
    if let Ok(tup) = names.downcast::<PyTuple>() {
        let mut result = HashSet::with_capacity(tup.len());
        for item in tup.iter() {
            result.insert(item.extract::<String>()?);
        }
        return Ok(result);
    }
    if let Ok(lst) = names.downcast::<PyList>() {
        let mut result = HashSet::with_capacity(lst.len());
        for item in lst.iter() {
            result.insert(item.extract::<String>()?);
        }
        return Ok(result);
    }
    Ok([names.extract::<String>()?].into_iter().collect())
}

/// `analyze_base_classes` magic-base skip (semanal.py:3110-3124). A
/// `TypedDict`/`NamedTuple` base named through a `RefExpr` (or a
/// `TypedDict(...)` call) is skipped, not analyzed as a real base.
#[pyfunction]
pub(crate) fn rust_is_magic_base(
    py: Python<'_>,
    base_expr: &PyAny,
    namedtuple_names: &PyAny,
    tpdict_names: &PyAny,
) -> PyResult<bool> {
    let namedtuple_set = normalize_names(namedtuple_names)?;
    let tpdict_set = normalize_names(tpdict_names)?;

    let nodes_mod = py.import("mypy.nodes")?;
    let ref_expr_cls: &PyType = nodes_mod.getattr("RefExpr")?.downcast()?;
    let call_expr_cls: &PyType = nodes_mod.getattr("CallExpr")?.downcast()?;

    if base_expr.is_instance(ref_expr_cls)? {
        let fullname = base_expr.getattr("fullname")?;
        if let Ok(s) = fullname.downcast::<PyString>() {
            let f = s.to_str()?;
            if namedtuple_set.contains(f) || tpdict_set.contains(f) {
                return Ok(true);
            }
        }
    }

    if base_expr.is_instance(call_expr_cls)? {
        let callee = base_expr.getattr("callee")?;
        if callee.is_instance(ref_expr_cls)? {
            let fullname = callee.getattr("fullname")?;
            if let Ok(s) = fullname.downcast::<PyString>() {
                if tpdict_set.contains(s.to_str()?) {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

/// `SemanticAnalyzer.is_core_builtin_class` (semanal.py:2552): true only for
/// the builtins module and the fixed core names (object, bool, function).
#[pyfunction]
pub(crate) fn rust_is_core_builtin_class(
    cur_mod_id: &str,
    class_name: &str,
    core_names: &PyAny,
) -> PyResult<bool> {
    let core_set = normalize_names(core_names)?;
    Ok(cur_mod_id == "builtins" && core_set.contains(class_name))
}

/// Pure decision core of the `six.with_metaclass` base-side classifier
/// (semanal.py:3327-3336). PyO3-free so the decision table is unit-tested
/// directly. The caller runs `analyze_type_expr` first (it populates
/// `callee.fullname`), then passes the fullname plus the two scalar facts.
/// Matches one of the three compat-helper names with `args_len >= 1` and
/// all positional args -> `ACTION_WITH_METACLASS`, else
/// `ACTION_NOT_WITH_METACLASS`. Always decidable.
fn classify_with_metaclass_inner(
    fullname: Option<&str>,
    args_len: usize,
    all_positional: bool,
) -> i64 {
    match fullname {
        Some(f) if WITH_METACLASS_FULLNAMES.contains(&f) => {
            if args_len >= 1 && all_positional {
                ACTION_WITH_METACLASS
            } else {
                ACTION_NOT_WITH_METACLASS
            }
        }
        _ => ACTION_NOT_WITH_METACLASS,
    }
}

/// `infer_metaclass_and_bases_from_compat_helpers` base-side classifier
/// (semanal.py:3321-3338). The Python shim runs `analyze_type_expr`
/// unconditionally before calling this, then applies the two side effects
/// (`with_meta_expr = args[0]`, `defn.base_type_exprs = args[1:]`) on
/// `ACTION_WITH_METACLASS`. No `self` dependency, so this is a module-level
/// function, unlike `_native_base_classification`.
#[pyfunction]
#[pyo3(signature = (fullname, args_len, all_positional))]
pub(crate) fn rust_classify_with_metaclass(
    fullname: Option<String>,
    args_len: usize,
    all_positional: bool,
) -> PyResult<i64> {
    Ok(classify_with_metaclass_inner(
        fullname.as_deref(),
        args_len,
        all_positional,
    ))
}

/// Action tags for the `@six.add_metaclass(M)` decorator-side classifier:
/// - `ACTION_NOT_ADD_METACLASS`: decorator is not `six.add_metaclass`.
/// - `ACTION_ADD_METACLASS`: Python captures `args[0]` and breaks.
pub(crate) const ACTION_NOT_ADD_METACLASS: i64 = 0;
pub(crate) const ACTION_ADD_METACLASS: i64 = 1;

/// The fullname matched by the decorator-side block (semanal.py:3374).
const ADD_METACLASS_FULLNAME: &str = "six.add_metaclass";

/// Pure decision core of the `@six.add_metaclass(M)` decorator-side
/// classifier (semanal.py:3373-3377). The Python shim runs
/// `dec_expr.callee.accept(self)` unconditionally first (it pops the callee
/// ref), then passes the fullname plus the two scalar facts. Matches
/// `six.add_metaclass` with exactly 1 positional arg ->
/// `ACTION_ADD_METACLASS`, else `ACTION_NOT_ADD_METACLASS`.
fn classify_add_metaclass_inner(
    fullname: Option<&str>,
    args_len: usize,
    arg_kind_0_positional: bool,
) -> i64 {
    match fullname {
        Some(ADD_METACLASS_FULLNAME) => {
            if args_len == 1 && arg_kind_0_positional {
                ACTION_ADD_METACLASS
            } else {
                ACTION_NOT_ADD_METACLASS
            }
        }
        _ => ACTION_NOT_ADD_METACLASS,
    }
}

/// `infer_metaclass_and_bases_from_compat_helpers` decorator-side
/// classifier (semanal.py:3369-3379). The Python shim runs
/// `dec_expr.callee.accept(self)` unconditionally before calling this,
/// then applies the side effect (`add_meta_expr = args[0]`, break) on
/// `ACTION_ADD_METACLASS`. Module-level like
/// `rust_classify_with_metaclass`.
#[pyfunction]
#[pyo3(signature = (fullname, args_len, arg_kind_0_positional))]
pub(crate) fn rust_classify_add_metaclass(
    fullname: Option<String>,
    args_len: usize,
    arg_kind_0_positional: bool,
) -> PyResult<i64> {
    Ok(classify_add_metaclass_inner(
        fullname.as_deref(),
        args_len,
        arg_kind_0_positional,
    ))
}

/// Decision tags for `check_lvalue_validity` (semanal.py:5445):
/// - `LVALUE_KIND_PASS`: node is neither TypeVarExpr nor TypeInfo.
/// - `LVALUE_KIND_TYPEVAR`: Python fails "Invalid assignment target".
/// - `LVALUE_KIND_TYPEINFO`: Python fails CANNOT_ASSIGN_TO_TYPE.
pub(crate) const LVALUE_KIND_PASS: i64 = 0;
pub(crate) const LVALUE_KIND_TYPEVAR: i64 = 1;
pub(crate) const LVALUE_KIND_TYPEINFO: i64 = 2;

/// `SemanticAnalyzer.check_lvalue_validity` dispatch-head port
/// (semanal.py:5445-5449). Rust reads the live `node` via PyO3
/// `is_instance` against `mypy.nodes.TypeVarExpr` and
/// `mypy.nodes.TypeInfo` and returns a branch tag. The Python shim
/// applies the `self.fail(...)` side effects. Never defers: every
/// reachable branch (including the implicit pass for any other node)
/// is classified.
#[pyfunction]
pub(crate) fn rust_classify_lvalue_validity(py: Python<'_>, node: &PyAny) -> PyResult<i64> {
    let typevar_expr_cls = nodes_class(py, "TypeVarExpr")?;
    if node.is_instance(typevar_expr_cls)? {
        return Ok(LVALUE_KIND_TYPEVAR);
    }
    let typeinfo_cls = nodes_class(py, "TypeInfo")?;
    if node.is_instance(typeinfo_cls)? {
        return Ok(LVALUE_KIND_TYPEINFO);
    }
    Ok(LVALUE_KIND_PASS)
}

/// Kind tags for the `configure_base_classes` per-base classifier, decoded
/// from the top-level wire `Type` variant (mirrors the ProperType isinstance
/// chain at semanal.py:3349-3372):
/// - `KIND_TUPLE`: `TupleType` -> Python runs `configure_tuple_base_class`.
/// - `KIND_INSTANCE`: `Instance` (splits on `is_newtype`).
/// - `KIND_ANY`: `AnyType` (splits on `disallow_subclassing_any`).
/// - `KIND_TYPEDDICT`: `TypedDictType` -> Python appends `base.fallback`.
/// - `KIND_OTHER`: anything else -> invalid base, Python fails + fallback_to_any.
pub(crate) const KIND_TUPLE: i64 = 0;
pub(crate) const KIND_INSTANCE: i64 = 1;
pub(crate) const KIND_ANY: i64 = 2;
pub(crate) const KIND_TYPEDDICT: i64 = 3;
pub(crate) const KIND_OTHER: i64 = 4;

/// Result tags handed to the Python shim for `configure_base_classes`,
/// one per base, index-aligned with the input list:
/// - `CONFIGURE_TUPLE`: Python calls `configure_tuple_base_class(defn, base)`
///   and appends its result (the TupleType arm defers per-base: the tuple
///   base-class surgery stays in Python).
/// - `CONFIGURE_INSTANCE`: plain instance base; Python appends it.
/// - `CONFIGURE_INSTANCE_NEWTYPE_FAIL`: Python fails 'Cannot subclass
///   "NewType"' and still appends the base.
/// - `CONFIGURE_ANY_OK`: `Any` base with `disallow_subclassing_any` off;
///   Python only sets `info.fallback_to_any = True`.
/// - `CONFIGURE_ANY_FAIL`: same plus the "Class cannot subclass" fail.
/// - `CONFIGURE_TYPEDDICT_FALLBACK`: Python appends `base.fallback`.
/// - `CONFIGURE_INVALID_BASE`: Python fails "Invalid base class ..." and
///   sets `fallback_to_any`.
pub(crate) const CONFIGURE_TUPLE: i64 = 1;
pub(crate) const CONFIGURE_INSTANCE: i64 = 2;
pub(crate) const CONFIGURE_INSTANCE_NEWTYPE_FAIL: i64 = 3;
pub(crate) const CONFIGURE_ANY_OK: i64 = 4;
pub(crate) const CONFIGURE_ANY_FAIL: i64 = 5;
pub(crate) const CONFIGURE_TYPEDDICT_FALLBACK: i64 = 6;
pub(crate) const CONFIGURE_INVALID_BASE: i64 = 7;

/// MRO-tail tags decided by `verify_base_classes` +
/// `verify_duplicate_base_classes` (semanal.py:3512-3526):
/// - `MRO_DUMMY`: a cyclic base -> Python fails "Cycle in inheritance
///   hierarchy" per cyclic base (indices returned alongside) and calls
///   `set_dummy_mro`.
/// - `MRO_ANY`: a duplicate direct base (name returned alongside) -> Python
///   fails 'Duplicate base class "..."' and calls `set_any_mro`, then
///   `calculate_class_mro`.
/// - `MRO_PROCEED`: clean hierarchy -> Python just calls `calculate_class_mro`.
pub(crate) const MRO_DUMMY: i64 = 1;
pub(crate) const MRO_ANY: i64 = 2;
pub(crate) const MRO_PROCEED: i64 = 3;

/// Pure decision core of the `configure_base_classes` per-base classifier
/// (semanal.py:3349-3381). PyO3-free so the decision table is unit-tested
/// directly. `kind` is the wire top-level kind tag, `unimported_any` /
/// `explicit_any` are the `has_any_from_unimported_type` /
/// `has_explicit_any` walk results (Python gates them on
/// `disallow_any_unimported` / `disallow_any_explicit` +
/// `is_typeshed_stub_file` before calling). Returns
/// `(tag, unimported_emit, explicit_emit)` or `None` when a walk deferred.
fn classify_configure_base_inner(
    kind: i64,
    is_newtype: bool,
    disallow_subclassing_any: bool,
    unimported_any: Option<bool>,
    explicit_any: Option<bool>,
) -> Option<(i64, bool, bool)> {
    let unimported_emit = unimported_any?;
    let explicit_emit = explicit_any?;
    let tag = match kind {
        KIND_TUPLE => CONFIGURE_TUPLE,
        KIND_INSTANCE => {
            if is_newtype {
                CONFIGURE_INSTANCE_NEWTYPE_FAIL
            } else {
                CONFIGURE_INSTANCE
            }
        }
        KIND_ANY => {
            if disallow_subclassing_any {
                CONFIGURE_ANY_FAIL
            } else {
                CONFIGURE_ANY_OK
            }
        }
        KIND_TYPEDDICT => CONFIGURE_TYPEDDICT_FALLBACK,
        _ => CONFIGURE_INVALID_BASE,
    };
    Some((tag, unimported_emit, explicit_emit))
}

/// Wire top-level kind of a decoded base `ProperType`. The bases list holds
/// ProperTypes (Python ran `get_proper_type` before calling), so the
/// top-level variant decides the isinstance chain 1:1; every other variant
/// falls into Python's `else` arm (INVALID_BASE).
fn wire_base_kind(t: &Type) -> i64 {
    match t {
        Type::TupleType { .. } => KIND_TUPLE,
        Type::Instance { .. } => KIND_INSTANCE,
        Type::AnyType { .. } => KIND_ANY,
        Type::TypedDictType { .. } => KIND_TYPEDDICT,
        _ => KIND_OTHER,
    }
}

/// `SemanticAnalyzer.configure_base_classes` per-base classifier
/// (semanal.py:3348-3381). One call classifies every base: the ProperType
/// isinstance chain (from the wire bytes) plus the
/// `disallow_any_unimported` / `check_for_explicit_any` predicates (walked
/// by the same kernel `has_explicit_any_inner` backs
/// `rust_has_explicit_any` / `rust_has_any_from_unimported_type`). Python
/// keeps every side effect: `configure_tuple_base_class`, the fail
/// emissions, `info.fallback_to_any`, the `base_types`/`info.bases` writes,
/// and `configure_tuple_base_class`'s tuple surgery. Defers (`None`) on an
/// undecodable blob or any any-walk that cannot conclude from the wire
/// (nested `TypeAliasType`), falling back to the pure-Python body.
#[pyfunction]
#[pyo3(signature = (bases_wire, is_newtypes, disallow_subclassing_any, disallow_any_unimported, disallow_any_explicit, is_typeshed_stub_file))]
pub(crate) fn rust_classify_configure_bases(
    bases_wire: &PyList,
    is_newtypes: &PyList,
    disallow_subclassing_any: bool,
    disallow_any_unimported: bool,
    disallow_any_explicit: bool,
    is_typeshed_stub_file: bool,
) -> PyResult<Option<Vec<(i64, bool, bool)>>> {
    if bases_wire.len() != is_newtypes.len() {
        return Ok(None);
    }
    let mut out = Vec::with_capacity(bases_wire.len());
    for (item, flag) in bases_wire.iter().zip(is_newtypes.iter()) {
        let bytes: &[u8] = match item.extract() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        let is_newtype: bool = match flag.extract() {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        let t = match decode_type(bytes) {
            Some(t) => t,
            None => return Ok(None),
        };
        let kind = wire_base_kind(&t);
        // The unimported/explicit walks mirror the Python short-circuits:
        // `has_any_from_unimported_type(base)` only runs when the option is
        // on; `check_for_explicit_any` only when not a typeshed stub file.
        let unimported_any = if disallow_any_unimported {
            has_any_from_unimported_inner(&t)
        } else {
            Some(false)
        };
        let explicit_any = if disallow_any_explicit && !is_typeshed_stub_file {
            has_explicit_any_inner(&t, EXPLICIT)
        } else {
            Some(false)
        };
        match classify_configure_base_inner(
            kind,
            is_newtype,
            disallow_subclassing_any,
            unimported_any,
            explicit_any,
        ) {
            Some(row) => out.push(row),
            None => return Ok(None),
        }
    }
    Ok(Some(out))
}

/// `has_any_from_unimported_type` walk over an already-decoded wire type
/// (typeanal_queries keeps the byte-level wrapper).
fn has_any_from_unimported_inner(t: &Type) -> Option<bool> {
    has_explicit_any_inner(t, FROM_UNIMPORTED_TYPE)
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Pure decision core of the MRO tail (semanal.py:3390-3397):
/// `verify_base_classes` + `verify_duplicate_base_classes` folded into one
/// 3-way tag. Cyclic bases win (dummy MRO, early return in Python), then a
/// duplicate direct base (Any MRO), else proceed.
fn configure_mro_tail_inner(
    cyclic: &[usize],
    dup: Option<&str>,
) -> (i64, Vec<usize>, Option<String>) {
    if !cyclic.is_empty() {
        (MRO_DUMMY, cyclic.to_vec(), None)
    } else if let Some(d) = dup {
        (MRO_ANY, Vec::new(), Some(d.to_string()))
    } else {
        (MRO_PROCEED, Vec::new(), None)
    }
}

/// `SemanticAnalyzer.is_base_class` (semanal.py:3528-3542) over live
/// TypeInfos: search the base-class graph of `s` for `t`, without the mro.
/// Python compares TypeInfos with `==` (identity for mypy TypeInfo); the
/// walk mirrors that with PyO3 identity. Returns `None` when an attribute
/// is unreadable so the caller defers to pure Python.
fn is_base_class_walk(t: &PyAny, s: &PyAny) -> Option<bool> {
    let mut worklist: Vec<&PyAny> = vec![s];
    let mut visited: Vec<&PyAny> = vec![s];
    while let Some(nxt) = worklist.pop() {
        if nxt.is(t) {
            return Some(true);
        }
        let bases = nxt.getattr("bases").ok()?;
        let bases = bases.downcast::<PyList>().ok()?;
        for base in bases.iter() {
            let baseinfo = base.getattr("type").ok()?;
            if !visited.iter().any(|v| v.is(baseinfo)) {
                visited.push(baseinfo);
                worklist.push(baseinfo);
            }
        }
    }
    Some(false)
}

/// `configure_base_classes` MRO tail (semanal.py:3390-3397 + 3512-3526).
/// Rust walks the live `TypeInfo` (`info.bases`, each `base.type`) via PyO3,
/// runs the `is_base_class` cycle walk per base and the
/// `find_duplicate(direct_base_classes())` scan, and returns one 3-way tag
/// plus the facts Python needs for its `fail` emissions: the indices of the
/// cyclic bases (one "Cycle in inheritance hierarchy" fail each, in
/// `info.bases` order) and the duplicate base's name. Python keeps
/// `set_dummy_mro` / `set_any_mro` / `calculate_class_mro` (plugins stay
/// live). Defers (`None`) on any unreadable attribute, mirroring the
/// exception-only deferral of the sibling classifiers.
#[pyfunction]
pub(crate) fn rust_classify_configure_mro(
    info: &PyAny,
) -> PyResult<Option<(i64, Vec<i64>, Option<String>)>> {
    let bases = match info.getattr("bases") {
        Ok(b) => b,
        Err(_) => return Ok(None),
    };
    let bases = match bases.downcast::<PyList>() {
        Ok(l) => l,
        Err(_) => return Ok(None),
    };
    let mut baseinfos: Vec<&PyAny> = Vec::with_capacity(bases.len());
    let mut cyclic: Vec<usize> = Vec::new();
    for (i, base) in bases.iter().enumerate() {
        let baseinfo = match base.getattr("type") {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };
        match is_base_class_walk(info, baseinfo) {
            Some(true) => cyclic.push(i),
            Some(false) => {}
            None => return Ok(None),
        }
        baseinfos.push(baseinfo);
    }
    let mut dup: Option<&PyAny> = None;
    'outer: for i in 1..baseinfos.len() {
        for j in 0..i {
            let is_eq = match baseinfos[i].rich_compare(baseinfos[j], CompareOp::Eq) {
                Ok(e) => e,
                Err(_) => return Ok(None),
            };
            match is_eq.is_true() {
                Ok(true) => {
                    dup = Some(baseinfos[i]);
                    break 'outer;
                }
                Ok(false) => {}
                Err(_) => return Ok(None),
            }
        }
    }
    let dup_name = match dup {
        Some(d) => match d.getattr("name") {
            Ok(n) => match n.extract::<String>() {
                Ok(s) => Some(s),
                Err(_) => return Ok(None),
            },
            Err(_) => return Ok(None),
        },
        None => None,
    };
    let (tag, cyclic_idx, dup_name) = configure_mro_tail_inner(&cyclic, dup_name.as_deref());
    Ok(Some((
        tag,
        cyclic_idx.into_iter().map(|i| i as i64).collect(),
        dup_name,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(fullname: Option<&str>, in_protocol_names: bool, has_args: bool) -> i64 {
        rust_clean_up_bases(fullname.map(str::to_string), in_protocol_names, has_args).unwrap()
    }

    #[test]
    fn plain_base_kept() {
        assert_eq!(classify(Some("mod.Bar"), false, false), ACTION_KEEP);
    }

    #[test]
    fn generic_base_with_args_kept() {
        // `class B(A[int])`: A[int] is not a Generic/Protocol declaration.
        assert_eq!(classify(Some("mod.A"), false, true), ACTION_KEEP);
    }

    #[test]
    fn generic_name_wins_even_without_args() {
        // Bare `Generic` is still removed (semanal.py:2824 first clause).
        assert_eq!(
            classify(Some("typing.Generic"), false, false),
            ACTION_GENERIC
        );
    }

    #[test]
    fn generic_with_args() {
        assert_eq!(
            classify(Some("typing.Generic"), false, true),
            ACTION_GENERIC
        );
    }

    #[test]
    fn protocol_with_args_declares_tvars() {
        assert_eq!(
            classify(Some("typing.Protocol"), true, true),
            ACTION_PROTOCOL_GENERIC
        );
    }

    #[test]
    fn bare_protocol_removed_no_tvars() {
        assert_eq!(
            classify(Some("typing_extensions.Protocol"), true, false),
            ACTION_BARE_PROTOCOL
        );
    }

    #[test]
    fn unresolved_symbol_kept() {
        // Missing node / failed lookup: neither declaration fires.
        assert_eq!(classify(None, false, true), ACTION_KEEP);
        assert_eq!(classify(None, false, false), ACTION_KEEP);
    }

    #[test]
    fn unrelated_name_kept() {
        assert_eq!(classify(Some("mod.NotProtocol"), false, true), ACTION_KEEP);
    }

    fn classify_meta(fullname: Option<&str>, args_len: usize, all_positional: bool) -> i64 {
        classify_with_metaclass_inner(fullname, args_len, all_positional)
    }

    #[test]
    fn with_metaclass_all_three_names_match() {
        assert_eq!(
            classify_meta(Some("six.with_metaclass"), 2, true),
            ACTION_WITH_METACLASS
        );
        assert_eq!(
            classify_meta(Some("future.utils.with_metaclass"), 2, true),
            ACTION_WITH_METACLASS
        );
        assert_eq!(
            classify_meta(Some("past.utils.with_metaclass"), 2, true),
            ACTION_WITH_METACLASS
        );
    }

    #[test]
    fn with_metaclass_zero_args_not_matched() {
        assert_eq!(
            classify_meta(Some("six.with_metaclass"), 0, true),
            ACTION_NOT_WITH_METACLASS
        );
    }

    #[test]
    fn with_metaclass_non_positional_not_matched() {
        assert_eq!(
            classify_meta(Some("six.with_metaclass"), 2, false),
            ACTION_NOT_WITH_METACLASS
        );
    }

    #[test]
    fn add_metaclass_not_matched() {
        assert_eq!(
            classify_meta(Some("six.add_metaclass"), 1, true),
            ACTION_NOT_WITH_METACLASS
        );
    }

    #[test]
    fn unrelated_name_not_matched() {
        assert_eq!(
            classify_meta(Some("mod.NotWithMeta"), 2, true),
            ACTION_NOT_WITH_METACLASS
        );
    }

    #[test]
    fn none_fullname_not_matched() {
        assert_eq!(classify_meta(None, 2, true), ACTION_NOT_WITH_METACLASS);
    }

    fn classify_add_meta(
        fullname: Option<&str>,
        args_len: usize,
        arg_kind_0_positional: bool,
    ) -> i64 {
        classify_add_metaclass_inner(fullname, args_len, arg_kind_0_positional)
    }

    #[test]
    fn test_classify_add_metaclass_matches() {
        assert_eq!(
            classify_add_meta(Some("six.add_metaclass"), 1, true),
            ACTION_ADD_METACLASS
        );
    }

    #[test]
    fn test_classify_add_metaclass_wrong_name() {
        assert_eq!(
            classify_add_meta(Some("six.with_metaclass"), 1, true),
            ACTION_NOT_ADD_METACLASS
        );
        assert_eq!(
            classify_add_meta(Some("mod.Other"), 1, true),
            ACTION_NOT_ADD_METACLASS
        );
    }

    #[test]
    fn test_classify_add_metaclass_wrong_arity() {
        assert_eq!(
            classify_add_meta(Some("six.add_metaclass"), 0, true),
            ACTION_NOT_ADD_METACLASS
        );
        assert_eq!(
            classify_add_meta(Some("six.add_metaclass"), 2, true),
            ACTION_NOT_ADD_METACLASS
        );
    }

    #[test]
    fn test_classify_add_metaclass_non_positional() {
        assert_eq!(
            classify_add_meta(Some("six.add_metaclass"), 1, false),
            ACTION_NOT_ADD_METACLASS
        );
    }

    #[test]
    fn test_classify_add_metaclass_none_fullname() {
        assert_eq!(classify_add_meta(None, 1, true), ACTION_NOT_ADD_METACLASS);
    }

    #[test]
    fn lvalue_kind_constants_are_distinct() {
        assert_eq!(LVALUE_KIND_PASS, 0);
        assert_eq!(LVALUE_KIND_TYPEVAR, 1);
        assert_eq!(LVALUE_KIND_TYPEINFO, 2);
        assert_ne!(LVALUE_KIND_PASS, LVALUE_KIND_TYPEVAR);
        assert_ne!(LVALUE_KIND_PASS, LVALUE_KIND_TYPEINFO);
        assert_ne!(LVALUE_KIND_TYPEVAR, LVALUE_KIND_TYPEINFO);
    }

    // configure_base_classes per-base classifier (pure core).

    fn classify_base(
        kind: i64,
        is_newtype: bool,
        disallow_subclassing_any: bool,
        unimported_any: Option<bool>,
        explicit_any: Option<bool>,
    ) -> Option<(i64, bool, bool)> {
        classify_configure_base_inner(
            kind,
            is_newtype,
            disallow_subclassing_any,
            unimported_any,
            explicit_any,
        )
    }

    #[test]
    fn tuple_base_tag() {
        assert_eq!(
            classify_base(KIND_TUPLE, false, false, Some(false), Some(false)),
            Some((CONFIGURE_TUPLE, false, false))
        );
    }

    #[test]
    fn instance_base_split_on_newtype() {
        assert_eq!(
            classify_base(KIND_INSTANCE, false, false, Some(false), Some(false)),
            Some((CONFIGURE_INSTANCE, false, false))
        );
        assert_eq!(
            classify_base(KIND_INSTANCE, true, false, Some(false), Some(false)),
            Some((CONFIGURE_INSTANCE_NEWTYPE_FAIL, false, false))
        );
    }

    #[test]
    fn any_base_split_on_disallow_subclassing() {
        assert_eq!(
            classify_base(KIND_ANY, false, false, Some(false), Some(false)),
            Some((CONFIGURE_ANY_OK, false, false))
        );
        assert_eq!(
            classify_base(KIND_ANY, false, true, Some(false), Some(false)),
            Some((CONFIGURE_ANY_FAIL, false, false))
        );
    }

    #[test]
    fn typeddict_fallback_tag() {
        assert_eq!(
            classify_base(KIND_TYPEDDICT, false, false, Some(false), Some(false)),
            Some((CONFIGURE_TYPEDDICT_FALLBACK, false, false))
        );
    }

    #[test]
    fn other_kinds_are_invalid_base() {
        for kind in [KIND_OTHER, 99, -1] {
            assert_eq!(
                classify_base(kind, false, false, Some(false), Some(false)),
                Some((CONFIGURE_INVALID_BASE, false, false))
            );
        }
    }

    #[test]
    fn unimported_and_explicit_flags_pass_through() {
        assert_eq!(
            classify_base(KIND_INSTANCE, false, false, Some(true), Some(true)),
            Some((CONFIGURE_INSTANCE, true, true))
        );
        assert_eq!(
            classify_base(KIND_ANY, false, true, Some(true), Some(false)),
            Some((CONFIGURE_ANY_FAIL, true, false))
        );
    }

    #[test]
    fn deferred_any_walk_defers_whole_base() {
        // A nested TypeAliasType makes the any-walks defer (None); the
        // classifier must defer the whole base.
        assert!(classify_base(KIND_INSTANCE, false, false, None, Some(false)).is_none());
        assert!(classify_base(KIND_INSTANCE, false, false, Some(false), None).is_none());
    }

    #[test]
    fn mro_tail_dummy_wins_over_any() {
        let (tag, cyclic, dup) = configure_mro_tail_inner(&[0, 2], Some("B"));
        assert_eq!(tag, MRO_DUMMY);
        assert_eq!(cyclic, vec![0, 2]);
        assert_eq!(dup, None);
    }

    #[test]
    fn mro_tail_any_on_duplicate_only() {
        let (tag, cyclic, dup) = configure_mro_tail_inner(&[], Some("B"));
        assert_eq!(tag, MRO_ANY);
        assert!(cyclic.is_empty());
        assert_eq!(dup, Some("B".to_string()));
    }

    #[test]
    fn mro_tail_proceed_when_clean() {
        let (tag, cyclic, dup) = configure_mro_tail_inner(&[], None);
        assert_eq!(tag, MRO_PROCEED);
        assert!(cyclic.is_empty());
        assert_eq!(dup, None);
    }

    #[test]
    fn mro_tail_constants_are_distinct() {
        assert_ne!(MRO_DUMMY, MRO_ANY);
        assert_ne!(MRO_DUMMY, MRO_PROCEED);
        assert_ne!(MRO_ANY, MRO_PROCEED);
    }
}
