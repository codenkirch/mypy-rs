//! Callable parameter compatibility on the Rust `Type` enum (Stage 3c / M8c).
//!
//! Ports `is_callable_compatible` + `are_parameters_compatible` +
//! `are_args_compatible` from `mypy.subtypes` (subtypes.py:1732-2162): the
//! engine behind `SubtypeVisitor.visit_callable_type`, answering "can a callable
//! stand in for another callable at a call site". It sees the majority of
//! function-to-function subtype checks.
//!
//! Strangler-fig contract (same as `subtypes::rust_is_subtype`): returns
//! `None` for any case Rust does not handle, and the Python shim in
//! `mypy/subtypes.py` falls through to pure-Python `is_callable_compatible`.
//! Deferred shapes:
//!
//! * either side is a `Parameters` (wire format drops `Parameters.is_ellipsis_args`,
//!   which `are_parameters_compatible` reads);
//! * `left.variables` non-empty and `unify_generic_callable` (wave 37
//!   kernel port, issue #1426) defers — freshening needed, an exotic
//!   constraint shape, or any unified result the kernel cannot rebuild;
//! * either side `unpack_kwargs` (`with_unpacked_kwargs` expands a trailing
//!   `TypedDictType` into named args);
//! * any arg is an `UnpackType` (`with_normalized_var_args` would unfold);
//! * the `mypy.meet.meet_types` merge case in
//!   `mypy.typeops.callable_corresponding_argument`
//!   (`SetOpResult` carries only input markers, not the merged type);
//! * any nested `subtypes::is_subtype` returns `None` (all-or-nothing: Rust
//!   cannot enrich one comparison with Python's answer while deciding the rest);
//! * `is_type_obj()` needs `builtins.type`-base info and the resolver cannot
//!   see the `CallableType.fallback` `TypeInfo` (defer, don't guess).

use pyo3::prelude::*;

use crate::subtypes::SubtypeContext;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::unify::UnifyOutcome;
use crate::wire::{self, ReadBuffer, Type};

/// ArgKind constants mirroring `mypy.nodes` (nodes.py:2480-2507).
const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

/// Mirrors `mypy.types.FormalArgument` (types.py:252-270): one callable
/// parameter, either by name or by position, in one of the lookup results.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FormalArgument {
    pub(crate) name: Option<String>,
    pub(crate) pos: Option<usize>,
    pub(crate) typ: Type,
    pub(crate) required: bool,
}

/// `mypy.nodes.ArgKind.is_positional` (nodes.py:2480-2484).
pub(crate) fn kind_is_positional(kind: i64, star: bool) -> bool {
    kind == ARG_POS || kind == ARG_OPT || (star && kind == ARG_STAR)
}

/// `mypy.nodes.ArgKind.is_named` (nodes.py:2486-2490).
pub(crate) fn kind_is_named(kind: i64, star: bool) -> bool {
    kind == ARG_NAMED || kind == ARG_NAMED_OPT || (star && kind == ARG_STAR2)
}

/// `mypy.nodes.ArgKind.is_required` (nodes.py:2492-2494).
fn kind_is_required(kind: i64) -> bool {
    kind == ARG_POS || kind == ARG_NAMED
}

/// `mypy.nodes.ArgKind.is_optional` (nodes.py:2496-2498).
fn kind_is_optional(kind: i64) -> bool {
    kind == ARG_OPT || kind == ARG_NAMED_OPT
}

/// `mypy.nodes.ArgKind.is_star` (nodes.py:2506-2508).
pub(crate) fn kind_is_star(kind: i64) -> bool {
    kind == ARG_STAR || kind == ARG_STAR2
}

/// Decode a wire-format `Type` blob via `wire::read_type`. Returns `None` on
/// any read failure — duplicates `subtypes::decode_type` (private there).
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// `get_proper_type(t)` resolves a fully-constructed `TypeAliasType` only;
/// our wire types almost never carry one, so `is_any` checks the variant
/// directly (mirrors `isinstance(get_proper_type(x), AnyType)`).
fn is_any(t: &Type) -> bool {
    matches!(t, Type::AnyType { .. })
}

/// `CallableType.var_arg` (types.py:2314-2320): the first `*args` arg.
pub(crate) fn var_arg(arg_types: &[Type], arg_kinds: &[i64]) -> Option<FormalArgument> {
    arg_kinds
        .iter()
        .position(|&k| k == ARG_STAR)
        .map(|i| FormalArgument {
            name: None,
            pos: Some(i),
            typ: arg_types[i].clone(),
            required: false,
        })
}

/// `CallableType.kw_arg` (types.py:2322-2327): the first `**kwargs` arg.
pub(crate) fn kw_arg(arg_types: &[Type], arg_kinds: &[i64]) -> Option<FormalArgument> {
    arg_kinds
        .iter()
        .position(|&k| k == ARG_STAR2)
        .map(|i| FormalArgument {
            name: None,
            pos: Some(i),
            typ: arg_types[i].clone(),
            required: false,
        })
}

/// `CallableType/Parameters.formal_arguments` (types.py:2398-2419):
/// positional-and-named non-star args in order, skipping `*args`/`**kwargs`.
/// Python subtlety mirrored exactly: an arg whose kind is named-or-star flips
/// `done_with_positional` *before* the star-continue, so a star arg still
/// terminates the positional section.
pub(crate) fn formal_arguments(
    arg_types: &[Type],
    arg_kinds: &[i64],
    arg_names: &[Option<String>],
) -> Vec<FormalArgument> {
    let mut args = Vec::new();
    let mut done_with_positional = false;
    for i in 0..arg_types.len() {
        let kind = arg_kinds[i];
        if kind_is_named(kind, false) || kind_is_star(kind) {
            done_with_positional = true;
        }
        if kind_is_star(kind) {
            continue;
        }
        let required = kind_is_required(kind);
        let pos = if done_with_positional { None } else { Some(i) };
        args.push(FormalArgument {
            name: arg_names[i].clone(),
            pos,
            typ: arg_types[i].clone(),
            required,
        });
    }
    args
}

/// `CallableType.argument_by_name` (types.py:2421-2436).
pub(crate) fn argument_by_name(
    arg_types: &[Type],
    arg_kinds: &[i64],
    arg_names: &[Option<String>],
    name: &str,
) -> Option<FormalArgument> {
    let mut seen_star = false;
    for i in 0..arg_types.len() {
        let kind = arg_kinds[i];
        if kind_is_named(kind, false) || kind_is_star(kind) {
            seen_star = true;
        }
        if kind_is_star(kind) {
            continue;
        }
        if arg_names[i].as_deref() == Some(name) {
            let position = if seen_star { None } else { Some(i) };
            return Some(FormalArgument {
                name: Some(name.to_string()),
                pos: position,
                typ: arg_types[i].clone(),
                required: kind_is_required(kind),
            });
        }
    }
    // Not found by name: fall back to synthesizing from `**kwargs`.
    try_synthesizing_arg_from_kwarg(arg_types, arg_kinds, Some(name.to_string()))
}

/// `CallableType.argument_by_position` (types.py:2438-2451).
pub(crate) fn argument_by_position(
    arg_types: &[Type],
    arg_kinds: &[i64],
    arg_names: &[Option<String>],
    position: usize,
) -> Option<FormalArgument> {
    if position >= arg_names.len() {
        return try_synthesizing_arg_from_vararg(arg_types, arg_kinds, Some(position));
    }
    let name = arg_names[position].clone();
    let kind = arg_kinds[position];
    let typ = arg_types[position].clone();
    if kind_is_positional(kind, false) {
        Some(FormalArgument {
            name,
            pos: Some(position),
            typ,
            required: kind == ARG_POS,
        })
    } else {
        try_synthesizing_arg_from_vararg(arg_types, arg_kinds, Some(position))
    }
}

/// `CallableType.try_synthesizing_arg_from_kwarg` (types.py:2453-2458):
/// if the callable has a `**kwargs`, any name maps to its (optional) type.
pub(crate) fn try_synthesizing_arg_from_kwarg(
    arg_types: &[Type],
    arg_kinds: &[i64],
    name: Option<String>,
) -> Option<FormalArgument> {
    let kw = kw_arg(arg_types, arg_kinds)?;
    Some(FormalArgument {
        name,
        pos: None,
        typ: kw.typ,
        required: false,
    })
}

/// `CallableType.try_synthesizing_arg_from_vararg` (types.py:2460-2465):
/// if the callable has a `*args`, any out-of-range position maps to its type.
pub(crate) fn try_synthesizing_arg_from_vararg(
    arg_types: &[Type],
    arg_kinds: &[i64],
    position: Option<usize>,
) -> Option<FormalArgument> {
    let var = var_arg(arg_types, arg_kinds)?;
    Some(FormalArgument {
        name: None,
        pos: position,
        typ: var.typ,
        required: false,
    })
}

/// `mypy.subtypes.are_trivial_parameters` (subtypes.py:1888-1897): the
/// callable is `def _(*args: Any, **kwargs: Any)` — effectively
/// `Callable[..., Any]`.
fn are_trivial_parameters(arg_types: &[Type], arg_kinds: &[i64]) -> bool {
    let star = var_arg(arg_types, arg_kinds);
    let star2 = kw_arg(arg_types, arg_kinds);
    arg_kinds == [ARG_STAR, ARG_STAR2]
        && star.is_some()
        && is_any(&star.as_ref().unwrap().typ)
        && star2.is_some()
        && is_any(&star2.as_ref().unwrap().typ)
}

/// `mypy.subtypes.is_trivial_suffix` (subtypes.py:1900-1909).
fn is_trivial_suffix(arg_types: &[Type], arg_kinds: &[i64]) -> bool {
    if arg_kinds.len() < 2 {
        return false;
    }
    let star = var_arg(arg_types, arg_kinds);
    let star2 = kw_arg(arg_types, arg_kinds);
    arg_kinds[arg_kinds.len() - 2..] == [ARG_STAR, ARG_STAR2]
        && star.is_some()
        && is_any(&star.as_ref().unwrap().typ)
        && star2.is_some()
        && is_any(&star2.as_ref().unwrap().typ)
}

/// The bundle of `CallableType` fields the compatibility engine reads.
pub(crate) struct CallableFields<'a> {
    pub(crate) arg_types: &'a [Type],
    pub(crate) arg_kinds: &'a [i64],
    pub(crate) arg_names: &'a [Option<String>],
    pub(crate) ret_type: &'a Type,
    pub(crate) is_ellipsis_args: bool,
    pub(crate) implicit: bool,
    pub(crate) from_concatenate: bool,
    pub(crate) imprecise_arg_kinds: bool,
    pub(crate) unpack_kwargs: bool,
    /// The `type_guard` / `type_is` refinement payloads (types.py
    /// CallableType): needed for the both-only / one-only incompatibility
    /// pre-checks in the C1 visitor port (subtypes.py:910-929).
    pub(crate) type_guard: Option<&'a Type>,
    pub(crate) type_is: Option<&'a Type>,
}

impl<'a> CallableFields<'a> {
    fn ret_type(&self) -> &'a Type {
        self.ret_type
    }
}

/// Extract the `Callable` fields. `None` means the type is not a
/// `CallableType` (e.g. a `Parameters`), which defers to Python.
pub(crate) fn callable_fields(t: &Type) -> Option<CallableFields<'_>> {
    match t {
        Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            is_ellipsis_args,
            implicit,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            type_guard,
            type_is,
            ..
        } => Some(CallableFields {
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            is_ellipsis_args: *is_ellipsis_args,
            implicit: *implicit,
            from_concatenate: *from_concatenate,
            imprecise_arg_kinds: *imprecise_arg_kinds,
            unpack_kwargs: *unpack_kwargs,
            type_guard: type_guard.as_deref(),
            type_is: type_is.as_deref(),
        }),
        _ => None,
    }
}

/// `mypy.typeops.callable_corresponding_argument` (typeops.py:635-669).
/// Returns `None` both for "no corresponding arg" and — via the exact merge
/// case — "Rust cannot resolve the by-name/by-pos disagreement". The caller
/// (Phase 1b) treats a `None` as "no corresponding argument", so to distinguish
/// deferral we signal through a sentinel: `Deferred` is only produced by the
/// merge case, and `None` remains "no arg". The Python code returns the merged
/// `meet_types` arg there; it defers until meet_types support lands.
pub(crate) fn callable_corresponding_argument(
    arg_types: &[Type],
    arg_kinds: &[i64],
    arg_names: &[Option<String>],
    model: &FormalArgument,
) -> Result<Option<FormalArgument>, Defer> {
    let by_name = model
        .name
        .as_deref()
        .and_then(|n| argument_by_name(arg_types, arg_kinds, arg_names, n));
    let by_pos = model
        .pos
        .and_then(|p| argument_by_position(arg_types, arg_kinds, arg_names, p));
    match (&by_name, &by_pos) {
        (None, None) => Ok(None),
        (Some(a), Some(b)) if a == b => Ok(Some(a.clone())),
        (Some(a), Some(b)) => {
            // Distinct by-name and by-pos: Python merges only when both are
            // optional, by_name pos-only, by_pos name-only, and neither typ is
            // an UnpackType. The merged type is `meet_types(by_name.typ,

            // by_pos.typ)` — unreconstructible from `SetOpResult`, so defer.
            let _ = (a, b);
            Err(Defer)
        }
        (Some(a), None) => Ok(Some(a.clone())),
        (None, Some(b)) => Ok(Some(b.clone())),
    }
}

/// Marker for "Rust cannot produce a decision; fall through to Python".
#[derive(Debug, Clone, Copy)]
pub(crate) struct Defer;

/// Helper node for Phase 1a's `_incompatible` logic. Mirrors the Python
/// `None`-propagation: a nested `is_compat` that returns `None` defers the
/// whole call (the caller `?`s this), so "incompatible" is only `Some(true)`.
fn is_compat_pair(
    is_compat: &dyn Fn(&Type, &Type) -> Option<bool>,
    left: &Option<FormalArgument>,
    right: &Option<FormalArgument>,
    allow_partial_overlap: bool,
    trivial_suffix: bool,
) -> Option<bool> {
    match (left, right) {
        (_, None) => Some(false),
        (None, Some(_)) => Some(!allow_partial_overlap && !trivial_suffix),
        // Mirror `_incompatible`: incompatible iff left does NOT accept
        // right's type. A `None` from the nested `is_compat` defers.
        (Some(l), Some(r)) => Some(!is_compat(&r.typ, &l.typ)?),
    }
}

/// `mypy.subtypes.are_args_compatible` (subtypes.py:2108-2162).
fn are_args_compatible(
    left: &FormalArgument,
    right: &FormalArgument,
    is_compat: &dyn Fn(&Type, &Type) -> Option<bool>,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    allow_imprecise_kinds: bool,
) -> Option<bool> {
    let mut allow_partial_overlap = allow_partial_overlap;
    if left.required && right.required {
        allow_partial_overlap = false;
    }
    // name mismatch (`is_different` with allow_overlap=allow_partial_overlap,
    // subtypes.py:2272-2286). A missing right name never differs; a missing
    // left name differs when partial overlap is off, else matches any name.
    let name_diff = match (&left.name, &right.name) {
        (_, None) => false,
        (None, Some(_)) => !allow_partial_overlap,
        (Some(l), Some(r)) => l != r,
    };
    if name_diff && (!ignore_pos_arg_names || right.pos.is_none()) {
        return Some(false);
    }
    // position mismatch (`is_different` with allow_overlap=False): Some !=
    // None counts as a mismatch, so `right.pos != left.pos` handles a
    // missing left position.
    if right.pos.is_some() && right.pos != left.pos && !allow_imprecise_kinds {
        return Some(false);
    }
    // Right optional, left required → False unless partial overlap.
    if !allow_partial_overlap && !right.required && left.required {
        return Some(false);
    }
    if allow_partial_overlap && !left.required && !right.required {
        return Some(true);
    }
    // Left must have a more general type: is_compat(right.typ, left.typ).
    is_compat(&right.typ, &left.typ)
}

/// Extract the arg list + parameter-list flags from either a `Parameters`
/// or a `CallableType` wire type (both carry `arg_types`/`arg_kinds`/
/// `arg_names`). Used by the `rust_are_parameters_compatible` seam, which
/// serves the Python `Parameters`-`Parameters` paths (`visit_parameters`,
/// the meet overlap branch) that previously were pure-Python.
pub(crate) struct AnyArgList<'a> {
    pub(crate) arg_types: &'a [Type],
    pub(crate) arg_kinds: &'a [i64],
    pub(crate) arg_names: &'a [Option<String>],
    pub(crate) from_concatenate: bool,
    pub(crate) is_ellipsis_args: bool,
    pub(crate) imprecise_arg_kinds: bool,
    pub(crate) variables_empty: bool,
}

pub(crate) fn arg_list_from_type(t: &Type) -> Option<AnyArgList<'_>> {
    match t {
        Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            from_concatenate,
            is_ellipsis_args,
            imprecise_arg_kinds,
            variables,
            ..
        } => Some(AnyArgList {
            arg_types,
            arg_kinds,
            arg_names,
            from_concatenate: *from_concatenate,
            is_ellipsis_args: *is_ellipsis_args,
            imprecise_arg_kinds: *imprecise_arg_kinds,
            variables_empty: variables.is_empty(),
        }),
        Type::Parameters(p) => Some(AnyArgList {
            arg_types: &p.arg_types,
            arg_kinds: &p.arg_kinds,
            arg_names: &p.arg_names,
            // Wire drops Parameters.is_ellipsis_args/from_concatenate;
            // is_ellipsis_args=True only coincides with Any star types
            // (typeanal.py:2105), which are_trivial_parameters detects.
            from_concatenate: false,
            is_ellipsis_args: false,
            imprecise_arg_kinds: p.imprecise_arg_kinds,
            variables_empty: p.variables.is_empty(),
        }),
        _ => None,
    }
}

/// `#[pyfunction]` seam mirroring `mypy.subtypes.are_parameters_compatible`
/// (subtypes.py:1912-2105) for `Parameters`-vs-`Parameters` (or mixed
/// `CallableType`) comparison. The Python shim uses this in
/// `SubtypeVisitor.visit_parameters` (subtypes.py:962-971) and the meet
/// overlap branch (meet.py:708-716): both currently run pure-Python because
/// `rust_callables_compatible` requires both sides to be `CallableType`.
/// Returns `None` (defer to Python) for anything the engine cannot decide.
#[pyfunction]
#[pyo3(signature = (left_bytes, right_bytes, is_proper_subtype, ignore_pos_arg_names, allow_partial_overlap, strict_concatenate_check, strict_optional, nested_proper_subtype, resolver, infer_unions = false))]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_are_parameters_compatible(
    left_bytes: &[u8],
    right_bytes: &[u8],
    is_proper_subtype: bool,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    strict_concatenate_check: bool,
    strict_optional: bool,
    nested_proper_subtype: bool,
    resolver: &mut NativeTypeResolver,
    infer_unions: bool,
) -> Option<bool> {
    // Ambient `type_state.infer_unions` for kernel-expect unify (#1426).
    crate::unify::set_infer_unions(infer_unions);
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let lf = arg_list_from_type(&left)?;
    let rf = arg_list_from_type(&right)?;
    // Generic `Parameters`/`CallableType` (variables non-empty) defer: the
    // Python visit would unify via type inference first.
    if !lf.variables_empty || !rf.variables_empty {
        return None;
    }
    // Nested is_compat re-enters is_subtype through the caller's context,
    // which carries the visitor's proper_subtype even though the top call
    // hardcodes is_proper_subtype=False (subtypes.py:968-971).
    let ctx = SubtypeContext::with_callable_flags(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        false, // ignore_promotions
        nested_proper_subtype,
        strict_optional,
        ignore_pos_arg_names,
        strict_concatenate_check,
    );
    let is_compat: &dyn Fn(&Type, &Type) -> Option<bool> =
        &|l, r| crate::subtypes::is_subtype(l, r, &ctx, resolver.resolver());
    are_parameters_compatible(
        lf.arg_types,
        lf.arg_kinds,
        lf.arg_names,
        lf.from_concatenate,
        rf.arg_types,
        rf.arg_kinds,
        rf.arg_names,
        rf.imprecise_arg_kinds,
        rf.is_ellipsis_args,
        is_compat,
        is_proper_subtype,
        ignore_pos_arg_names,
        allow_partial_overlap,
        strict_concatenate_check,
    )
}

/// `mypy.subtypes.are_parameters_compatible` (subtypes.py:1912-2105).
#[allow(clippy::too_many_arguments)]
pub(crate) fn are_parameters_compatible(
    left_types: &[Type],
    left_kinds: &[i64],
    left_names: &[Option<String>],
    _left_from_concatenate: bool,
    right_types: &[Type],
    right_kinds: &[i64],
    right_names: &[Option<String>],
    right_imprecise: bool,
    right_ellipsis: bool,
    is_compat: &dyn Fn(&Type, &Type) -> Option<bool>,
    is_proper_subtype: bool,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    strict_concatenate_check: bool,
) -> Option<bool> {
    // subtypes.py:1923-1924.
    if right_ellipsis && !is_proper_subtype {
        return Some(true);
    }

    let left_star = var_arg(left_types, left_kinds);
    let left_star2 = kw_arg(left_types, left_kinds);
    let right_star = var_arg(right_types, right_kinds);
    let right_star2 = kw_arg(right_types, right_kinds);

    // subtypes.py:1931-1934.
    if are_trivial_parameters(right_types, right_kinds) && !is_proper_subtype {
        return Some(true);
    }
    let trivial_suffix = is_trivial_suffix(right_types, right_kinds) && !is_proper_subtype;

    // subtypes.py:1936-1947.
    let trivial_vararg_suffix = !right_kinds.is_empty()
        && right_kinds[right_kinds.len() - 1] == ARG_STAR
        && is_any(&right_types[right_types.len() - 1])
        && !is_proper_subtype
        && left_kinds.iter().all(|k| kind_is_positional(*k, true));

    // Phase 1a (subtypes.py:1981-1986).
    let star_incompat = is_compat_pair(
        is_compat,
        &left_star,
        &right_star,
        allow_partial_overlap,
        trivial_suffix,
    )?;
    let star2_incompat = is_compat_pair(
        is_compat,
        &left_star2,
        &right_star2,
        allow_partial_overlap,
        trivial_suffix,
    )?;
    if (star_incompat && !trivial_vararg_suffix) || star2_incompat {
        return Some(false);
    }

    // Phase 1b (subtypes.py:1991-2005).
    for right_arg in formal_arguments(right_types, right_kinds, right_names) {
        let left_arg =
            match callable_corresponding_argument(left_types, left_kinds, left_names, &right_arg) {
                Err(Defer) => return None,
                Ok(x) => x,
            };
        let Some(left_arg) = left_arg else {
            if allow_partial_overlap && !right_arg.required {
                continue;
            }
            return Some(false);
        };
        let ok = are_args_compatible(
            &left_arg,
            &right_arg,
            is_compat,
            ignore_pos_arg_names,
            allow_partial_overlap,
            right_imprecise,
        )?;
        if !ok {
            return Some(false);
        }
    }

    if trivial_suffix {
        // For trivial right suffix we *only* check that every non-star right
        // argument has a valid match on the left.
        return Some(true);
    }

    // Phase 1c (subtypes.py:2016-2038).
    if right_star.is_some() && !trivial_vararg_suffix {
        let right_star_pos = right_star.as_ref().and_then(|a| a.pos).unwrap_or(0);
        let right_by_position = try_synthesizing_arg_from_vararg(right_types, right_kinds, None)?;
        let mut i = right_star_pos;
        while i < left_kinds.len() && kind_is_positional(left_kinds[i], false) {
            if allow_partial_overlap && kind_is_optional(left_kinds[i]) {
                break;
            }
            let left_by_position = argument_by_position(left_types, left_kinds, left_names, i)?;
            let ok = are_args_compatible(
                &left_by_position,
                &right_by_position,
                is_compat,
                ignore_pos_arg_names,
                allow_partial_overlap,
                false,
            )?;
            if !ok {
                return Some(false);
            }
            i += 1;
        }
    }

    // Phase 1d (subtypes.py:2043-2074).
    if right_star2.is_some() {
        let right_names_set: std::collections::HashSet<&str> =
            right_names.iter().filter_map(|n| n.as_deref()).collect();
        let mut left_only_names: Vec<String> = Vec::new();
        if strict_concatenate_check {
            for (name, kind) in left_names.iter().zip(left_kinds.iter()) {
                if let Some(name) = name {
                    if !kind_is_star(*kind) && !right_names_set.contains(name.as_str()) {
                        left_only_names.push(name.clone());
                    }
                }
            }
        }
        if !left_only_names.is_empty() {
            let right_by_name = try_synthesizing_arg_from_kwarg(right_types, right_kinds, None)?;
            for name in &left_only_names {
                let left_by_name = argument_by_name(left_types, left_kinds, left_names, name)?;
                if allow_partial_overlap && !left_by_name.required {
                    continue;
                }
                let ok = are_args_compatible(
                    &left_by_name,
                    &right_by_name,
                    is_compat,
                    ignore_pos_arg_names,
                    allow_partial_overlap,
                    false,
                )?;
                if !ok {
                    return Some(false);
                }
            }
        }
    }

    // Phase 2 (subtypes.py:2079-2103).
    for left_arg in formal_arguments(left_types, left_kinds, left_names) {
        let right_by_name = left_arg
            .name
            .as_deref()
            .and_then(|n| argument_by_name(right_types, right_kinds, right_names, n));
        let right_by_pos = left_arg
            .pos
            .and_then(|p| argument_by_position(right_types, right_kinds, right_names, p));
        // subtypes.py:2090-2098.
        if let (Some(rn), Some(rp)) = (&right_by_name, &right_by_pos) {
            if rn != rp
                && (rp.required || rn.required)
                && strict_concatenate_check
                && !right_imprecise
            {
                return Some(false);
            }
        }
        // subtypes.py:2102-2103: all required left args must have a
        // corresponding right arg.
        if left_arg.required && right_by_pos.is_none() && right_by_name.is_none() {
            return Some(false);
        }
    }

    Some(true)
}

/// Does this callable have any `UnpackType` in its arg types? The native path
/// defers those (out-of-line `*args: *Ts` / `**kwargs: **Ts` shapes).
pub(crate) fn any_unpack_anywhere(t: &Type) -> bool {
    match t {
        Type::CallableType {
            arg_types,
            ret_type,
            ..
        } => {
            arg_types
                .iter()
                .any(|a| matches!(a, Type::UnpackType { .. }))
                || matches!(&**ret_type, Type::UnpackType { .. })
        }
        _ => false,
    }
}

/// `mypy.types.CallableType.is_type_obj` (types.py:2343-2346):
/// `fallback.type.is_metaclass() and not isinstance(get_proper_type(ret_type),
/// UninhabitedType)`. `is_metaclass` (nodes.py:4128-4133):
/// `has_base("builtins.type") or fullname == "abc.ABCMeta" or fallback_to_any`.
/// We can only see the fallback as an `Instance` type_ref; when the resolver
/// cannot resolve it, return `None` (unknown → defer).
pub(crate) fn is_type_obj(t: &Type, resolver: &TypeResolver) -> Option<bool> {
    let Type::CallableType {
        fallback, ret_type, ..
    } = t
    else {
        return None;
    };
    let Type::Instance { type_ref, .. } = &**fallback else {
        return None;
    };
    let info = resolver.get(type_ref)?;
    if is_uninhabited_proper(ret_type) {
        return Some(false);
    }
    Some(info.has_base("builtins.type") || info.fullname == "abc.ABCMeta" || info.fallback_to_any)
}

fn is_uninhabited_proper(t: &Type) -> bool {
    matches!(t, Type::UninhabitedType { .. })
}

/// `#[pyfunction]` entry: the Python shim in `mypy/subtypes.py` serializes the
/// two `CallableType` objects and passes the flags it would have passed to
/// `is_callable_compatible`, plus `strict_optional` so nested `is_subtype`
/// calls match the running state. `ignore_return`, `check_args_covariantly`,
/// and `allow_partial_overlap` are always False at the `visit_callable_type`
/// call site, so they are not parameters.
///
/// Returns `None` (Python `None`) when Rust doesn't handle the case; the shim
/// then falls through to the pure-Python `is_callable_compatible`.
#[pyfunction]
#[pyo3(signature = (left_bytes, right_bytes, is_proper_subtype, ignore_pos_arg_names, strict_concatenate, strict_optional, resolver, infer_unions = false))]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_callables_compatible(
    left_bytes: &[u8],
    right_bytes: &[u8],
    is_proper_subtype: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
    infer_unions: bool,
) -> Option<bool> {
    // Ambient `type_state.infer_unions` for kernel-expect unify (#1426).
    crate::unify::set_infer_unions(infer_unions);
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;

    let ctx = SubtypeContext::with_callable_flags(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        false, // ignore_promotions
        is_proper_subtype,
        strict_optional,
        ignore_pos_arg_names,
        strict_concatenate,
    );
    let res = resolver.resolver();
    callables_compatible(
        &left,
        &right,
        ctx.ignore_pos_arg_names,
        ctx.strict_concatenate,
        &ctx,
        res,
    )
}

/// Entry used by both the `#[pyfunction]` seam and the `subtypes::is_subtype`
/// visitor (Stage C1, issue #719). The `ignore_pos_arg_names` and
/// `strict_concatenate` flags are explicit because the wire seam passes them
/// as serialized parameters, while the visitor derives them from its
/// `SubtypeContext`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn callables_compatible(
    left: &Type,
    right: &Type,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    callables_compatible_with_ignore_return(
        left,
        right,
        ignore_pos_arg_names,
        strict_concatenate,
        ctx,
        resolver,
        false, // ignore_return
    )
}

/// Extended entry: same as `callables_compatible`, plus `ignore_return` for
/// the `find_matching_overload_items` seam (`ignore_return=True` mirrors the
/// Python `is_callable_compatible(...)` call in constraints.py:1950, where
/// the template's return type is indeterminate). The other call sites keep
/// `ignore_return=False`, matching `visit_callable_type` and the wire seam.
#[allow(clippy::too_many_arguments)]
pub(crate) fn callables_compatible_with_ignore_return(
    left: &Type,
    right: &Type,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    ignore_return: bool,
) -> Option<bool> {
    // Both sides must be plain CallableType. `Parameters` (either side) defers:
    // the wire format drops `Parameters.is_ellipsis_args`, which the Python
    // `are_parameters_compatible` reads.
    let (lf, rf) = match (callable_fields(left), callable_fields(right)) {
        (Some(l), Some(r)) => (l, r),
        _ => return None,
    };

    // type_guard / type_is mismatch pre-checks
    // (subtypes.py:910-929): both-only TypeGuard (covariant on the guarded
    // type), both-only TypeIs (checked both ways), or a guard/is on one side

    // only → False. These run before the general engine; the guarded
    // sub-type comparison goes through the same SubtypeContext as the
    // overall check (`self._is_subtype` in the Python visitor). Only when

    // that comparison itself defers do we defer the whole check.
    if let (Some(lg), Some(rg)) = (lf.type_guard, rf.type_guard) {
        match ctx_compat_is_subtype(ctx, resolver, lg, rg) {
            Some(false) => return Some(false),
            None => return None,
            Some(true) => {}
        }
    } else if let (Some(li), Some(ri)) = (lf.type_is, rf.type_is) {
        match ctx_compat_is_subtype(ctx, resolver, li, ri) {
            Some(false) => return Some(false),
            None => return None,
            Some(true) => {}
        }
        match ctx_compat_is_subtype(ctx, resolver, ri, li) {
            Some(false) => return Some(false),
            None => return None,
            Some(true) => {}
        }
    } else if (rf.type_guard.is_some() && lf.type_guard.is_none())
        || (rf.type_is.is_some() && lf.type_is.is_none())
    {
        // One side has a refinement the other lacks: incompatible
        // (subtypes.py:922-929).
        return Some(false);
    }

    // Everything exotic defers to Python.
    if lf.unpack_kwargs || rf.unpack_kwargs {
        return None;
    }
    if any_unpack_anywhere(left) || any_unpack_anywhere(right) {
        return None;
    }

    // subtypes.py:2590-2595: a generic `left` unifies via
    // `unify_generic_callable` (issue #1426): `NoUnify` returns False,
    // `Defer` keeps the blanket `None` so the Python shim re-runs.

    // The unpack gates above keep their pre-unify order: when they defer,
    // Python is_callable_compatible itself decides. Non-generic left keeps
    // the original (unnormalized) operands, preserving pre-#1426 parity.
    let mut unified_right: Option<Type> = None;
    let unified;
    let left: &Type = match left_variables(left) {
        Some(vars) if !vars.is_empty() => {
            let left_norm = match crate::checkcall::normalize_callable(left) {
                Ok(t) => t,
                Err(_) => return None,
            };
            let right_norm = match crate::checkcall::normalize_callable(right) {
                Ok(t) => t,
                Err(_) => return None,
            };
            unified_right = Some(right_norm);
            let aliases = match resolver.aliases() {
                Some(shared) => crate::aliases::TypeAliasResolver::from_shared_view(shared),
                None => crate::aliases::TypeAliasResolver::new(),
            };
            unified = match crate::unify::unify_generic_callable_core(
                &left_norm,
                unified_right.as_ref().unwrap(),
                ignore_return,
                ctx.strict_optional,
                resolver,
                &aliases,
            ) {
                UnifyOutcome::Unified(t) => t,
                UnifyOutcome::NoUnify => return Some(false),
                UnifyOutcome::Defer => return None,
            };
            &unified
        }
        _ => left,
    };
    let right: &Type = unified_right.as_ref().map_or(right, |t| t as &Type);

    let is_compat: &dyn Fn(&Type, &Type) -> Option<bool> =
        &|l, r| crate::subtypes::is_subtype(l, r, ctx, resolver);

    is_callable_compatible(
        left,
        right,
        is_compat,
        ctx.proper_subtype,
        ignore_pos_arg_names,
        strict_concatenate,
        ignore_return,
        false, // check_args_covariantly
        false, // allow_partial_overlap
        resolver,
    )
}

fn left_variables(t: &Type) -> Option<&[Type]> {
    match t {
        Type::CallableType { variables, .. } => Some(variables),
        _ => None,
    }
}

/// `is_subtype` through the same `SubtypeContext` the caller is using,
/// mirroring the Python visitor's `self._is_subtype(l, r)` — which
/// re-enters `is_subtype` with the current `subtype_context` (and the
/// same `proper_subtype`). Used by the type_guard / type_is refinement
/// pre-checks. `None` (defer on an unsupported nested shape) propagates.
fn ctx_compat_is_subtype(
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    left: &Type,
    right: &Type,
) -> Option<bool> {
    crate::subtypes::is_subtype(left, right, ctx, resolver)
}

/// `mypy.subtypes.is_callable_compatible` (subtypes.py:1883-2036), the subset
/// the wire format + resolver can answer. The caller supplies the `is_compat`
/// closure (e.g. `is_subtype` / `is_proper_subtype` / `is_more_precise` /
/// `is_same_type`), `ignore_return` (skip the return-type check), and
/// `check_args_covariantly` (flip `is_compat` for the argument path, keeping
/// the return path unflipped). `is_compat_return = is_compat`. Returns
/// `None` to defer.
///
/// The caller is responsible for the `left.variables` unify gate (Python
/// unifies a generic `left` via `unify_generic_callable` before this check);
/// this function assumes `left` is non-generic.
#[allow(clippy::too_many_arguments)]
pub(crate) fn is_callable_compatible(
    left: &Type,
    right: &Type,
    is_compat: &dyn Fn(&Type, &Type) -> Option<bool>,
    is_proper_subtype: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    ignore_return: bool,
    check_args_covariantly: bool,
    allow_partial_overlap: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    let lf = callable_fields(left)?;
    let rf = callable_fields(right)?;

    // ignore_pos_arg_names |= left.implicit or right.implicit
    // (subtypes.py:1841-1842).
    let ignore_pos_arg_names = ignore_pos_arg_names || lf.implicit || rf.implicit;

    // right.is_type_obj() and not left.is_type_obj() and not
    // allow_partial_overlap → False (subtypes.py:1845-1846); skipped under
    // allow_partial_overlap (existential overlap). Resolver misses defer.
    if !allow_partial_overlap {
        let right_is_type_obj = is_type_obj(right, resolver)?;
        let left_is_type_obj = is_type_obj(left, resolver)?;
        if right_is_type_obj && !left_is_type_obj {
            return Some(false);
        }
    }

    // Check return types (subtypes.py:1866-1867), covariant:
    // is_compat_return(left.ret_type, right.ret_type); skipped when
    // ignore_return, and always uses the unflipped is_compat.
    if !ignore_return && !is_compat(lf.ret_type(), rf.ret_type())? {
        return Some(false);
    }

    // check_args_covariantly flips is_compat for the argument path
    // (subtypes.py:1949-1951), leaving the return path untouched.
    let eff_is_compat: &dyn Fn(&Type, &Type) -> Option<bool> = if check_args_covariantly {
        &|l, r| is_compat(r, l)
    } else {
        is_compat
    };

    // strict_concatenate_check (subtypes.py:1872-1875).
    let strict_concatenate_check =
        strict_concatenate || !(lf.from_concatenate || rf.from_concatenate);

    // is_ellipsis_args on the right is read inside are_parameters_compatible.
    are_parameters_compatible(
        lf.arg_types,
        lf.arg_kinds,
        lf.arg_names,
        lf.from_concatenate,
        rf.arg_types,
        rf.arg_kinds,
        rf.arg_names,
        rf.imprecise_arg_kinds,
        rf.is_ellipsis_args,
        eff_is_compat,
        is_proper_subtype,
        ignore_pos_arg_names,
        allow_partial_overlap,
        strict_concatenate_check,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn class_type(name: &str) -> Type {
        Type::Instance {
            type_ref: format!("builtins.{name}"),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn compat_ok(l: &Type, r: &Type) -> Option<bool> {
        let _ = (l, r);
        Some(true)
    }

    #[test]
    fn trivial_right_parameters_accept_any_left() {
        // right = (*Any, **Any) stands in for Callable[..., Any]: a left with
        // a required positional arg is compatible without touching is_compat.
        let left_types = vec![class_type("int")];
        let left_kinds = [ARG_POS];
        let right_types = vec![any_type(), any_type()];
        let right_kinds = [ARG_STAR, ARG_STAR2];
        let names: Vec<Option<String>> = vec![None, None];
        let ok = are_parameters_compatible(
            &left_types,
            &left_kinds,
            &[],
            false,
            &right_types,
            &right_kinds,
            &names,
            false,
            false,
            &compat_ok,
            false,
            false,
            false,
            false,
        );
        assert_eq!(ok, Some(true));
    }

    #[test]
    fn trivial_right_parameters_off_under_proper_subtype() {
        // With is_proper_subtype the trivial-right shortcut is off, and left
        // has no match for right's star args.
        let left_types = vec![class_type("int")];
        let left_kinds = [ARG_POS];
        let right_types = vec![any_type(), any_type()];
        let right_kinds = [ARG_STAR, ARG_STAR2];
        let names: Vec<Option<String>> = vec![None, None];
        let left_names: Vec<Option<String>> = vec![None];
        let ok = are_parameters_compatible(
            &left_types,
            &left_kinds,
            &left_names,
            false,
            &right_types,
            &right_kinds,
            &names,
            false,
            false,
            &compat_ok,
            true,
            false,
            false,
            false,
        );
        assert_eq!(ok, Some(false));
    }

    #[test]
    fn trivial_suffix_only_checks_non_star_args() {
        // right = (int, *Any, **Any): the trivial suffix means only the
        // non-star right arg must match; left's extra str arg is unchecked.
        let left_types = vec![class_type("int"), class_type("str")];
        let left_kinds = [ARG_POS, ARG_POS];
        let right_types = vec![class_type("int"), any_type(), any_type()];
        let right_kinds = [ARG_POS, ARG_STAR, ARG_STAR2];
        let names: Vec<Option<String>> = vec![None, None, None];
        let left_names: Vec<Option<String>> = vec![None, None];
        let ok = are_parameters_compatible(
            &left_types,
            &left_kinds,
            &left_names,
            false,
            &right_types,
            &right_kinds,
            &names,
            false,
            false,
            &compat_ok,
            false,
            false,
            false,
            false,
        );
        assert_eq!(ok, Some(true));
    }

    #[test]
    fn trivial_vararg_suffix_covers_star_pair_mismatch() {
        // right = (*Any) with an all-positional left: (*Any) is a supertype
        // of positional callables, so the star-pair mismatch is waived.
        let left_types = vec![class_type("int")];
        let left_kinds = [ARG_POS];
        let right_types = vec![any_type()];
        let right_kinds = [ARG_STAR];
        let names: Vec<Option<String>> = vec![None];
        let left_names: Vec<Option<String>> = vec![None];
        let ok = are_parameters_compatible(
            &left_types,
            &left_kinds,
            &left_names,
            false,
            &right_types,
            &right_kinds,
            &names,
            false,
            false,
            &compat_ok,
            false,
            false,
            false,
            false,
        );
        assert_eq!(ok, Some(true));
    }

    #[test]
    fn trivial_vararg_suffix_needs_positional_left() {
        // The (*Any) waiver requires every left kind to be positional; a
        // kw-only left arg falls through to the star-pair check and fails.
        let left_types = vec![class_type("int")];
        let left_kinds = [ARG_NAMED];
        let left_names = vec![Some("x".to_string())];
        let right_types = vec![any_type()];
        let right_kinds = [ARG_STAR];
        let right_names: Vec<Option<String>> = vec![None];
        let ok = are_parameters_compatible(
            &left_types,
            &left_kinds,
            &left_names,
            false,
            &right_types,
            &right_kinds,
            &right_names,
            false,
            false,
            &compat_ok,
            false,
            false,
            false,
            false,
        );
        assert_eq!(ok, Some(false));
    }

    #[test]
    fn trivial_helpers_reject_non_any_star() {
        let int = class_type("int");
        assert!(!are_trivial_parameters(
            &[int.clone(), int.clone()],
            &[ARG_STAR, ARG_STAR2]
        ));
        assert!(!is_trivial_suffix(std::slice::from_ref(&int), &[ARG_STAR]));
        assert!(!is_trivial_suffix(&[], &[]));
        assert!(is_trivial_suffix(
            &[int.clone(), any_type(), any_type()],
            &[ARG_POS, ARG_STAR, ARG_STAR2]
        ));
    }
}
