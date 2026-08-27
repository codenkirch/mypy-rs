//! #434: generator/coroutine return-type helpers (mypy/checker.py:1422-1560),
//! Rust port.
//!
//! Ports the six `TypeChecker` methods that classify a function's declared
//! return type as a generator/coroutine kind and extract its three type
//! parameters (yield type `ty`, receive type `tc`, return type `tr`):
//!
//! * `is_generator_return_type(t, c)` — supertype of `Generator[Any,Any,Any]`
//!   (non-coroutine) or `Awaitable[Any]` (coroutine), or exactly
//!   `AwaitableGenerator` (modulo params).
//! * `is_async_generator_return_type(t)` — supertype of
//!   `AsyncGenerator[Any,Any]`.
//! * `get_generator_yield_type(t, c)` — ty.
//! * `get_generator_receive_type(t, c)` — tc.
//! * `get_generator_return_type(t, c)` — tr.
//! * `get_coroutine_return_type(t)` — args[2] of the `Coroutine` instance
//!   (called only on coroutine functions).
//!
//! The named generic operands (`typing.Awaitable[Any]`,
//! `typing.Generator[Any,Any,Any]`, `typing.AsyncGenerator[Any,Any]`) are
//! built in Rust directly as `Type::Instance` records carrying the literal
//! fullname; Python's `named_generic_type` is only a string->TypeInfo
//! lookup, so no live `TypeInfo` is needed for the construction.
//!
//! Every deferral (`TypeAliasType`, an `is_subtype` pair the nominal path
//! cannot decide, a decode failure, an output type that cannot cross the
//! wire without poisoning) returns `None`, and the Python wrapper then falls
//! through to the pure-Python method body unchanged. This is the
//! strangler-fig per-call gate.

use pyo3::prelude::*;
use pyo3::types::PyBytes;

use crate::setops::make_simplified_union;
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WriteBuffer};

/// `TypeOfAny.special_form` == 6 (types.py:233).
const ANY_SPECIAL_FORM: i64 = 6;
/// `TypeOfAny.from_error` == 5 (types.py:231).
const ANY_FROM_ERROR: i64 = 5;
/// `TypeOfAny.from_another_any` == 7 (types.py:237).
const ANY_FROM_ANOTHER_ANY: i64 = 7;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Encode `t` to the type wire format, or `None` to defer to Python.
///
/// `TypeAliasType` is deferred (the wire format stores only the `type_ref`
/// string, not the live `TypeAlias` node; checker_stmts.rs `encode_type_owned`
/// documents the crash that results when a poisoned alias crosses the seam).
fn encode_type(t: &Type) -> Option<Vec<u8>> {
    if matches!(t, Type::TypeAliasType { .. }) {
        return None;
    }
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, t).ok()?;
    Some(buf.into_bytes())
}

/// `get_proper_type` for the wire format. The wire cannot expand
/// `TypeAliasType` (no resolved target), so aliases defer.
fn get_proper_or_defer(typ: &Type) -> Option<&Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        t => Some(t),
    }
}

fn any_type(type_of_any: i64, source_any: Option<Box<Type>>) -> Type {
    Type::AnyType {
        type_of_any,
        source_any,
        missing_import_name: None,
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

/// The subtyping context used for every generator/coroutine comparison.
///
/// Python calls these methods through `is_subtype` with all-default flags
/// (`ignore_type_params=False`, `ignore_pos_arg_names=False`,
/// `ignore_declared_variance=False`, `always_covariant=False`,
/// `ignore_promotions=False`) and `options=None` — i.e. a non-proper subtype
/// with `state.strict_optional` (subtypes.py:170-203). The Rust
/// `SubtypeContext` carries the six flags (`ignore_pos_arg_names` is not
/// modeled; the nominal Instance path does not read it).
fn subtype_ctx(strict_optional: bool) -> SubtypeContext {
    SubtypeContext::new(false, false, false, false, false, strict_optional)
}

/// `named_generic_type("typing.Awaitable", [Any])` (checker.py:1431).
fn awaitable_any() -> Type {
    instance("typing.Awaitable", vec![any_type(ANY_SPECIAL_FORM, None)])
}

/// `named_generic_type("typing.Generator", [Any, Any, Any])`
/// (checker.py:1436).
fn generator_any() -> Type {
    instance(
        "typing.Generator",
        vec![
            any_type(ANY_SPECIAL_FORM, None),
            any_type(ANY_SPECIAL_FORM, None),
            any_type(ANY_SPECIAL_FORM, None),
        ],
    )
}

/// `named_generic_type("typing.AsyncGenerator", [Any, Any])`
/// (checker.py:1448).
///
/// Python wraps the construction in a `try/except KeyError` that returns
/// `False` when the running `typing` version predates `AsyncGenerator`. This
/// module decides the flag locally: the fullname lookup itself never raises
/// here, so the `KeyError` guard only ever fired when python's `typing`
/// lacked `AsyncGenerator` — a fixed, old-version concern. We build the
/// Instance unconditionally (parity with any modern Python), matching how the
/// other `named_generic_type` calls behave.
fn async_generator_any() -> Type {
    instance(
        "typing.AsyncGenerator",
        vec![
            any_type(ANY_SPECIAL_FORM, None),
            any_type(ANY_SPECIAL_FORM, None),
        ],
    )
}

fn is_instance_of(t: &Type, fullname: &str) -> bool {
    matches!(t, Type::Instance { type_ref, .. } if type_ref == fullname)
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// `is_generator_return_type` (checker.py:1422-1439).
///
/// Returns `None` when an `is_subtype` defers (the wire operands are the
/// always-decidable constructed Instances against the caller's type, so a
/// defer means the right-hand side is a variant the nominal path cannot
/// judge — e.g. a callable or a tuple/type-type/overloaded — and Python must
/// decide).
fn is_generator_return_type(
    typ: &Type,
    is_coroutine: bool,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<bool> {
    let typ = get_proper_or_defer(typ)?;
    if is_coroutine {
        if is_subtype(&awaitable_any(), typ, &subtype_ctx(strict_optional), res)? {
            return Some(true);
        }
    } else {
        if is_subtype(&generator_any(), typ, &subtype_ctx(strict_optional), res)? {
            return Some(true);
        }
    }
    Some(is_instance_of(typ, "typing.AwaitableGenerator"))
}

/// `is_async_generator_return_type` (checker.py:1441-1452). The `KeyError`
/// guard is not applicable (see `async_generator_any`).
fn is_async_generator_return_type(
    typ: &Type,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<bool> {
    let typ = get_proper_or_defer(typ)?;
    is_subtype(
        &async_generator_any(),
        typ,
        &subtype_ctx(strict_optional),
        res,
    )
}

/// Classify whether `typ` (already proper) is a generator/async-generator
/// return, used by the yield/receive helpers' else-branch. Returns `None` if
/// either classification defers.
fn is_any_generator_return_type(
    typ: &Type,
    is_coroutine: bool,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<bool> {
    let gen = is_generator_return_type(typ, is_coroutine, strict_optional, res)?;
    if gen {
        return Some(true);
    }
    is_async_generator_return_type(typ, strict_optional, res)
}

// ---------------------------------------------------------------------------
// get_generator_yield_type
// ---------------------------------------------------------------------------

/// `get_generator_yield_type` (checker.py:1454-1486), ty extraction.
fn get_generator_yield_type_inner(
    return_type: &Type,
    is_coroutine: bool,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<Type> {
    let return_type = get_proper_or_defer(return_type)?;
    match return_type {
        Type::AnyType { .. } => Some(any_type(
            ANY_FROM_ANOTHER_ANY,
            Some(Box::new(return_type.clone())),
        )),
        Type::UnionType { items, .. } => {
            let mut outs = Vec::with_capacity(items.len());
            for item in items {
                outs.push(get_generator_yield_type_inner(
                    item,
                    is_coroutine,
                    strict_optional,
                    res,
                )?);
            }
            let union_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            make_simplified_union(&outs, &union_ctx, res, true)
        }
        _ => {
            if !is_any_generator_return_type(return_type, is_coroutine, strict_optional, res)? {
                return Some(any_type(ANY_FROM_ERROR, None));
            }
            let Type::Instance { type_ref, args, .. } = return_type else {
                return Some(any_type(ANY_FROM_ERROR, None));
            };
            if type_ref == "typing.Awaitable" {
                return Some(any_type(ANY_SPECIAL_FORM, None));
            }
            if !args.is_empty() {
                return Some(args[0].clone());
            }
            Some(any_type(ANY_SPECIAL_FORM, None))
        }
    }
}

// ---------------------------------------------------------------------------
// get_generator_receive_type
// ---------------------------------------------------------------------------

/// `get_generator_receive_type` (checker.py:1488-1521), tc extraction.
fn get_generator_receive_type_inner(
    return_type: &Type,
    is_coroutine: bool,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<Type> {
    let return_type = get_proper_or_defer(return_type)?;
    match return_type {
        Type::AnyType { .. } => Some(any_type(
            ANY_FROM_ANOTHER_ANY,
            Some(Box::new(return_type.clone())),
        )),
        Type::UnionType { items, .. } => {
            let mut outs = Vec::with_capacity(items.len());
            for item in items {
                outs.push(get_generator_receive_type_inner(
                    item,
                    is_coroutine,
                    strict_optional,
                    res,
                )?);
            }
            let union_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            make_simplified_union(&outs, &union_ctx, res, true)
        }
        _ => {
            if !is_any_generator_return_type(return_type, is_coroutine, strict_optional, res)? {
                return Some(any_type(ANY_FROM_ERROR, None));
            }
            let Type::Instance { type_ref, args, .. } = return_type else {
                return Some(any_type(ANY_FROM_ERROR, None));
            };
            if type_ref == "typing.Awaitable" {
                return Some(any_type(ANY_SPECIAL_FORM, None));
            }
            if matches!(
                type_ref.as_str(),
                "typing.Generator" | "typing.AwaitableGenerator"
            ) && args.len() >= 3
            {
                return Some(args[1].clone());
            }
            if type_ref == "typing.AsyncGenerator" && args.len() >= 2 {
                return Some(args[1].clone());
            }
            Some(Type::NoneType)
        }
    }
}

// ---------------------------------------------------------------------------
// get_coroutine_return_type
// ---------------------------------------------------------------------------

/// `get_coroutine_return_type` (checker.py:1523-1529). Pure Type-in/Type-out;
/// ported. `return_type` is asserted by the caller to be a `Coroutine`
/// Instance, so no generator classification runs here.
pub(crate) fn get_coroutine_return_type_inner(return_type: &Type) -> Option<Type> {
    let return_type = get_proper_or_defer(return_type)?;
    match return_type {
        Type::AnyType { .. } => Some(any_type(
            ANY_FROM_ANOTHER_ANY,
            Some(Box::new(return_type.clone())),
        )),
        Type::Instance { args, .. } => args.get(2).cloned(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// get_generator_return_type
// ---------------------------------------------------------------------------

/// `get_generator_return_type` (checker.py:1531-1560), tr extraction.
///
/// NOTE: this body differs from yield/receive in the tail. The Python
/// `not is_generator_return_type(...)` branch (checker.py:1541) does NOT also
/// check `is_async_generator_return_type` (only yield/receive do), and the
/// two final branches are ordered `Awaitable`-with-exactly-1-arg THEN
/// Generator/AwaitableGenerator-with->=3-args THEN `NoneType`. Copied exactly.
pub(crate) fn get_generator_return_type_inner(
    return_type: &Type,
    is_coroutine: bool,
    strict_optional: bool,
    res: &TypeResolver,
) -> Option<Type> {
    let return_type = get_proper_or_defer(return_type)?;
    match return_type {
        Type::AnyType { .. } => Some(any_type(
            ANY_FROM_ANOTHER_ANY,
            Some(Box::new(return_type.clone())),
        )),
        Type::UnionType { items, .. } => {
            let mut outs = Vec::with_capacity(items.len());
            for item in items {
                outs.push(get_generator_return_type_inner(
                    item,
                    is_coroutine,
                    strict_optional,
                    res,
                )?);
            }
            let union_ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
            make_simplified_union(&outs, &union_ctx, res, true)
        }
        _ => {
            if !is_generator_return_type(return_type, is_coroutine, strict_optional, res)? {
                return Some(any_type(ANY_FROM_ERROR, None));
            }
            let Type::Instance { type_ref, args, .. } = return_type else {
                return Some(any_type(ANY_FROM_ERROR, None));
            };
            if type_ref == "typing.Awaitable" && args.len() == 1 {
                return Some(args[0].clone());
            }
            if matches!(
                type_ref.as_str(),
                "typing.Generator" | "typing.AwaitableGenerator"
            ) && args.len() >= 3
            {
                return Some(args[2].clone());
            }
            Some(Type::NoneType)
        }
    }
}

// ---------------------------------------------------------------------------
// #[pyfunction] entries
// ---------------------------------------------------------------------------

/// `mypy.checker.TypeChecker.is_generator_return_type`, Rust port.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_generator_return_type(
    typ_bytes: &[u8],
    is_coroutine: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let t = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_generator_return_type(
        &t,
        is_coroutine,
        strict_optional,
        resolver.resolver(),
    ))
}

/// `mypy.checker.TypeChecker.is_async_generator_return_type`, Rust port.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_is_async_generator_return_type(
    typ_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<bool>> {
    let t = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(is_async_generator_return_type(
        &t,
        strict_optional,
        resolver.resolver(),
    ))
}

/// `mypy.checker.TypeChecker.get_generator_yield_type`, Rust port. Returns
/// the serialized-wire result `Type`, or `None` to defer.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_get_generator_yield_type<'py>(
    py: Python<'py>,
    return_type_bytes: &[u8],
    is_coroutine: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<&'py PyBytes>> {
    let t = match decode_type(return_type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let out = match get_generator_yield_type_inner(
        &t,
        is_coroutine,
        strict_optional,
        resolver.resolver(),
    ) {
        Some(o) => o,
        None => return Ok(None),
    };
    Ok(encode_type(&out).map(|b| PyBytes::new(py, &b)))
}

/// `mypy.checker.TypeChecker.get_generator_receive_type`, Rust port.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_get_generator_receive_type<'py>(
    py: Python<'py>,
    return_type_bytes: &[u8],
    is_coroutine: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<&'py PyBytes>> {
    let t = match decode_type(return_type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let out = match get_generator_receive_type_inner(
        &t,
        is_coroutine,
        strict_optional,
        resolver.resolver(),
    ) {
        Some(o) => o,
        None => return Ok(None),
    };
    Ok(encode_type(&out).map(|b| PyBytes::new(py, &b)))
}

/// `mypy.checker.TypeChecker.get_generator_return_type`, Rust port.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_get_generator_return_type<'py>(
    py: Python<'py>,
    return_type_bytes: &[u8],
    is_coroutine: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> PyResult<Option<&'py PyBytes>> {
    let t = match decode_type(return_type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let out = match get_generator_return_type_inner(
        &t,
        is_coroutine,
        strict_optional,
        resolver.resolver(),
    ) {
        Some(o) => o,
        None => return Ok(None),
    };
    Ok(encode_type(&out).map(|b| PyBytes::new(py, &b)))
}

/// `mypy.checker.TypeChecker.get_coroutine_return_type`, Rust port.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_get_coroutine_return_type<'py>(
    py: Python<'py>,
    return_type_bytes: &[u8],
) -> PyResult<Option<&'py PyBytes>> {
    let t = match decode_type(return_type_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    Ok(get_coroutine_return_type_inner(&t)
        .and_then(|o| encode_type(&o).map(|b| PyBytes::new(py, &b))))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtypes::COVARIANT;
    use crate::typeinfo::TypeInfoSnapshot;

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    /// A snapshot with its own fullname in mro/has_base and `n` covariant
    /// type vars `(Ti, COVARIANT, kind=0)`, so an exactly-matching Instance
    /// with `n` args can be judged by the nominal path.
    fn generic_snap(fullname: &str, name: &str, n_tvars: usize) -> TypeInfoSnapshot {
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        s.type_vars_with_variance = (0..n_tvars)
            .map(|i| (format!("T{i}"), COVARIANT, 0))
            .collect();
        s
    }

    fn snap(fullname: &str, name: &str) -> TypeInfoSnapshot {
        generic_snap(fullname, name, 0)
    }

    fn empty_resolver() -> TypeResolver {
        TypeResolver::new()
    }

    fn any() -> Type {
        any_type(ANY_SPECIAL_FORM, None)
    }

    fn err_any() -> Type {
        any_type(ANY_FROM_ERROR, None)
    }

    fn i(type_ref: &str, args: Vec<Type>) -> Type {
        instance(type_ref, args)
    }

    fn union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        }
    }

    fn alias() -> Type {
        Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.A".to_string(),
        }
    }

    /// The real typing.Generator has 3 covariant params.
    fn generator_snap() -> TypeInfoSnapshot {
        generic_snap("typing.Generator", "Generator", 3)
    }

    /// The real typing.AsyncGenerator has 2 covariant params.
    fn async_generator_snap() -> TypeInfoSnapshot {
        generic_snap("typing.AsyncGenerator", "AsyncGenerator", 2)
    }

    /// The real typing.Awaitable has 1 covariant param.
    fn awaitable_snap() -> TypeInfoSnapshot {
        generic_snap("typing.Awaitable", "Awaitable", 1)
    }

    // -- is_generator_return_type --

    #[test]
    fn generator_coroutine_awaitable_any() {
        let r = make_resolver(vec![awaitable_snap()]);
        let t = awaitable_any();
        assert_eq!(is_generator_return_type(&t, true, true, &r), Some(true));
    }

    #[test]
    fn generator_coroutine_non_awaitable_is_false() {
        // int is not a supertype of Awaitable[Any]: is_subtype returns
        // Some(false) and the AwaitableGenerator exact check misses.
        let r = make_resolver(vec![snap("builtins.int", "int"), awaitable_snap()]);
        let t = i("builtins.int", vec![]);
        assert_eq!(is_generator_return_type(&t, true, true, &r), Some(false));
    }

    #[test]
    fn generator_non_coroutine_matches_generator() {
        let r = make_resolver(vec![
            generator_snap(),
            awaitable_snap(),
            snap("builtins.object", "object"),
            snap("builtins.int", "int"),
            snap("builtins.str", "str"),
        ]);
        let gen = generator_any();
        assert_eq!(is_generator_return_type(&gen, false, true, &r), Some(true));
        // Generator[int,str,bool] <: Generator[Any,Any,Any]: exact match, each
        // arg is_subtype(_, Any) is not-proper -> True.
        let concrete = i(
            "typing.Generator",
            vec![
                i("builtins.int", vec![]),
                i("builtins.str", vec![]),
                i("builtins.int", vec![]),
            ],
        );
        assert_eq!(
            is_generator_return_type(&concrete, false, true, &r),
            Some(true)
        );
        // object is a supertype of Generator, hence a valid generator return:
        // Generator[Any,Any,Any] <: object (builtins.object fast-path). This
        // is the Rust-coverable instance of the "supertype of Generator"

        // clause (the nominal Iterator/Iterable derivations need bases
        // fixtures this module does not model).
        let obj = i("builtins.object", vec![]);
        assert_eq!(is_generator_return_type(&obj, false, true, &r), Some(true));
        // int is not a supertype of Generator[Any,Any,Any] and not an
        // AwaitableGenerator.
        let b = i("builtins.int", vec![]);
        assert_eq!(is_generator_return_type(&b, false, true, &r), Some(false));
    }

    #[test]
    fn generator_awaitable_generator_exact() {
        // AwaitableGenerator is matched by the exact-instance branch only
        // (it is not a supertype of the constructed operands). The resolver
        // still needs the left snapshots for the subtype probes to start.
        let r = make_resolver(vec![generator_snap(), awaitable_snap()]);
        let t = i("typing.AwaitableGenerator", vec![any()]);
        assert_eq!(is_generator_return_type(&t, true, true, &r), Some(true));
        // Not an AwaitableGenerator: falls through the subtype checks.
        assert_eq!(is_generator_return_type(&t, false, true, &r), Some(true));
    }

    #[test]
    fn generator_alias_defers() {
        assert_eq!(
            is_generator_return_type(&alias(), false, true, &empty_resolver()),
            None
        );
    }

    // -- is_async_generator_return_type --

    #[test]
    fn async_generator_matches() {
        let r = make_resolver(vec![async_generator_snap()]);
        let ag = async_generator_any();
        assert_eq!(is_async_generator_return_type(&ag, true, &r), Some(true));
        let concrete = i(
            "typing.AsyncGenerator",
            vec![i("builtins.int", vec![]), any()],
        );
        assert_eq!(
            is_async_generator_return_type(&concrete, true, &r),
            Some(true)
        );
    }

    #[test]
    fn async_generator_non_generator_is_false() {
        let r = make_resolver(vec![async_generator_snap(), snap("builtins.int", "int")]);
        let b = i("builtins.int", vec![]);
        assert_eq!(is_async_generator_return_type(&b, true, &r), Some(false));
    }

    #[test]
    fn async_generator_alias_defers() {
        assert_eq!(
            is_async_generator_return_type(&alias(), true, &empty_resolver()),
            None
        );
    }

    // -- get_generator_yield_type --

    #[test]
    fn yield_type_any_passthrough() {
        let r = empty_resolver();
        let t = any();
        assert_eq!(
            get_generator_yield_type_inner(&t, false, true, &r),
            Some(any_type(ANY_FROM_ANOTHER_ANY, Some(Box::new(any()))))
        );
    }

    #[test]
    fn yield_type_generator_args() {
        let r = make_resolver(vec![
            generator_snap(),
            snap("builtins.int", "int"),
            snap("builtins.str", "str"),
        ]);
        let t = i(
            "typing.Generator",
            vec![i("builtins.int", vec![]), any(), i("builtins.str", vec![])],
        );
        assert_eq!(
            get_generator_yield_type_inner(&t, false, true, &r),
            Some(i("builtins.int", vec![]))
        );
    }

    #[test]
    fn yield_type_generator_of_any_yields_any() {
        // Generator[Any,Any,Any] is decidable on the nominal path
        // (same-ref exact match, 3 mapped == 3 right args), cleared as a
        // generator, and its args[0] = Any(special_form) comes back through

        // the args-not-empty branch.
        let r = make_resolver(vec![generator_snap()]);
        assert_eq!(
            get_generator_yield_type_inner(&generator_any(), false, true, &r),
            Some(any())
        );
    }

    #[test]
    fn yield_type_zero_arg_generator_is_subtype() {
        // Python's zip truncates to 0 iterations on empty right args, so
        // is_subtype returns True and the yield type is Any(special_form).
        let r = make_resolver(vec![generator_snap()]);
        let noargs = i("typing.Generator", vec![]);
        assert_eq!(
            get_generator_yield_type_inner(&noargs, false, true, &r),
            Some(any_type(ANY_SPECIAL_FORM, None))
        );
    }

    #[test]
    fn yield_type_awaitable_returns_special_any() {
        let r = make_resolver(vec![awaitable_snap()]);
        let t = awaitable_any();
        assert_eq!(
            get_generator_yield_type_inner(&t, true, true, &r),
            Some(any())
        );
    }

    #[test]
    fn yield_type_union_recurses() {
        // Yield's else-branch runs BOTH the Generator and the
        // AsyncGenerator probe, so the resolver must carry the async
        // snapshot for the str item to decide instead of defer.
        let r = make_resolver(vec![
            generator_snap(),
            async_generator_snap(),
            snap("builtins.int", "int"),
            snap("builtins.str", "str"),
        ]);
        // Union[Generator[int,Any,Any], str]: the Generator branch -> int,
        // the str branch -> Any(from_error). make_simplified_union([int,
        // Any]) runs the Python two-pass dedup (a single reverse between

        // passes). Pass 1 keeps [int, Any], reverses to [Any, int]; pass 2:
        // is_proper_subtype(Any, int) is False (left-Any, right not Any) and
        // is_proper_subtype(int, Any) is False (any-right under proper), so

        // both survive as [Any, int], reversed back to [int, Any]. Expected
        // order is int first, mirrored below.
        let t = union(vec![
            i(
                "typing.Generator",
                vec![i("builtins.int", vec![]), any(), any()],
            ),
            i("builtins.str", vec![]),
        ]);
        let out = get_generator_yield_type_inner(&t, false, true, &r);
        assert_eq!(out, Some(union(vec![i("builtins.int", vec![]), err_any()])));
    }

    #[test]
    fn yield_type_union_all_generator_items() {
        let r = make_resolver(vec![
            generator_snap(),
            snap("builtins.int", "int"),
            snap("builtins.str", "str"),
        ]);
        // Both items are generators, neither is redundant, union stays
        // Union[int, str] in input order.
        let t = union(vec![
            i(
                "typing.Generator",
                vec![i("builtins.int", vec![]), any(), any()],
            ),
            i(
                "typing.Generator",
                vec![i("builtins.str", vec![]), any(), any()],
            ),
        ]);
        let out = get_generator_yield_type_inner(&t, false, true, &r);
        assert_eq!(
            out,
            Some(union(vec![
                i("builtins.int", vec![]),
                i("builtins.str", vec![])
            ]))
        );
    }

    #[test]
    fn yield_type_non_generator_from_error() {
        // int is not a generator/async-generator return. Both probes must
        // be decidable: left Generator/AsyncGenerator operands need their
        // snapshots, and the right int needs its own.
        let r = make_resolver(vec![
            generator_snap(),
            async_generator_snap(),
            snap("builtins.int", "int"),
        ]);
        let t = i("builtins.int", vec![]);
        assert_eq!(
            get_generator_yield_type_inner(&t, false, true, &r),
            Some(err_any())
        );
    }

    // -- get_generator_receive_type --

    #[test]
    fn receive_type_generator_args() {
        let r = make_resolver(vec![generator_snap(), snap("builtins.str", "str")]);
        let t = i(
            "typing.Generator",
            vec![any(), i("builtins.str", vec![]), any()],
        );
        assert_eq!(
            get_generator_receive_type_inner(&t, false, true, &r),
            Some(i("builtins.str", vec![]))
        );
    }

    #[test]
    fn receive_type_async_generator() {
        // AsyncGenerator[Any, str]: the first is_generator_return_type probe
        // (Generator <: AsyncGenerator) needs the Generator left snapshot;
        // the AsyncGenerator exact check uses both snapshots plus str.
        let r = make_resolver(vec![
            generator_snap(),
            async_generator_snap(),
            snap("builtins.str", "str"),
        ]);
        let t = i(
            "typing.AsyncGenerator",
            vec![any(), i("builtins.str", vec![])],
        );
        assert_eq!(
            get_generator_receive_type_inner(&t, false, true, &r),
            Some(i("builtins.str", vec![]))
        );
    }

    #[test]
    fn receive_type_awaitable_any() {
        let r = make_resolver(vec![awaitable_snap()]);
        let t = awaitable_any();
        assert_eq!(
            get_generator_receive_type_inner(&t, true, true, &r),
            Some(any())
        );
    }

    #[test]
    fn receive_type_supertype_none() {
        // An Iterator return: is_generator is true (supertype of
        // Generator[Any,...] requires a bases derivation we do not model in
        // tests), so this specific case is not exercised here. Instead use

        // AwaitableGenerator with fewer than 3 args: the Generator <:
        // AwaitableGenerator probe decides False with the left snapshot, the
        // exact AwaitableGenerator match is a generator, then the receive

        // branches pass (args < 3) to NoneType.
        let r = make_resolver(vec![generator_snap()]);
        let t = i("typing.AwaitableGenerator", vec![any(), any()]);
        assert_eq!(
            get_generator_receive_type_inner(&t, false, true, &r),
            Some(Type::NoneType)
        );
    }

    // -- get_coroutine_return_type --

    #[test]
    fn coroutine_return_type() {
        let t = i(
            "typing.Coroutine",
            vec![any(), any(), i("builtins.int", vec![])],
        );
        assert_eq!(
            get_coroutine_return_type_inner(&t),
            Some(i("builtins.int", vec![]))
        );
    }

    #[test]
    fn coroutine_return_type_any_passthrough() {
        assert_eq!(
            get_coroutine_return_type_inner(&any()),
            Some(any_type(ANY_FROM_ANOTHER_ANY, Some(Box::new(any()))))
        );
    }

    #[test]
    fn coroutine_return_type_short_args_defers() {
        // Fewer than 3 args: Python would IndexError (asserts only that the
        // type is Instance); Rust defers to Python.
        let t = i("typing.Coroutine", vec![any(), any()]);
        assert_eq!(get_coroutine_return_type_inner(&t), None);
    }

    #[test]
    fn coroutine_return_type_alias_defers() {
        assert_eq!(get_coroutine_return_type_inner(&alias()), None);
    }

    // -- get_generator_return_type --

    #[test]
    fn generator_return_type() {
        let r = make_resolver(vec![generator_snap(), snap("builtins.str", "str")]);
        let t = i(
            "typing.Generator",
            vec![any(), any(), i("builtins.str", vec![])],
        );
        assert_eq!(
            get_generator_return_type_inner(&t, false, true, &r),
            Some(i("builtins.str", vec![]))
        );
    }

    #[test]
    fn generator_return_type_awaitable() {
        let r = make_resolver(vec![awaitable_snap(), snap("builtins.int", "int")]);
        let t = i("typing.Awaitable", vec![i("builtins.int", vec![])]);
        assert_eq!(
            get_generator_return_type_inner(&t, true, true, &r),
            Some(i("builtins.int", vec![]))
        );
    }

    #[test]
    fn generator_return_type_async_generator_none() {
        // AsyncGenerator[Any, Any] is not a generator return (non-coroutine
        // is_generator_return_type: the Generator <: AsyncGenerator probe
        // is False, and AsyncGenerator != AwaitableGenerator). The one-sided

        // check has NO is_async_generator alternative, so it yields
        // Any(from_error). The Generator/AsyncGenerator snapshots let the
        // first probe decide instead of defer.
        let r = make_resolver(vec![
            generator_snap(),
            async_generator_snap(),
            snap("builtins.int", "int"),
        ]);
        let t = i("typing.AsyncGenerator", vec![any(), any()]);
        assert_eq!(
            get_generator_return_type_inner(&t, false, true, &r),
            Some(err_any())
        );
    }

    #[test]
    fn generator_return_type_supertype_none() {
        // An AwaitableGenerator with fewer than 3 args passes the
        // one-sided is_generator check (exact AwaitableGenerator match, the
        // Generator <: AwaitableGenerator probe needs the Generator left

        // snapshot) and falls past Awaitable/Generator to NoneType — the
        // Rust-coverable instance of the commented-out Iterator case.
        let r = make_resolver(vec![generator_snap()]);
        let t = i("typing.AwaitableGenerator", vec![any(), any()]);
        assert_eq!(
            get_generator_return_type_inner(&t, false, true, &r),
            Some(Type::NoneType)
        );
    }

    #[test]
    fn generator_return_type_union_recurses() {
        let r = make_resolver(vec![
            generator_snap(),
            async_generator_snap(),
            snap("builtins.int", "int"),
            snap("builtins.str", "str"),
        ]);
        // Union[Generator[int,Any,Any], str]: the Generator branch -> int
        // (tr=args[2]), the str branch -> Any(from_error). The two-pass
        // dedup (single reverse between passes) yields int first, then Any

        // (see yield_type_union_recurses for the ordering proof).
        let t = union(vec![
            i(
                "typing.Generator",
                vec![any(), any(), i("builtins.int", vec![])],
            ),
            i("builtins.str", vec![]),
        ]);
        assert_eq!(
            get_generator_return_type_inner(&t, false, true, &r),
            Some(union(vec![i("builtins.int", vec![]), err_any()]))
        );
    }
}
