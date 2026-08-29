//! `any_constraints` and `repack_callable_args` (constraints.py), Rust port.
//!
//! Ports the two top-level constraint-selection helpers behind the
//! `_native_constraints_active` gate:
//!
//!   * `any_constraints` (constraints.py:853-908) — recursive option-merge
//!     that deduces what a collection of constraint lists implies. The Rust
//!     port re-implements the helpers it calls (`is_same_constraints`,
//!     `is_similar_constraints`, `select_trivial`, `merge_with_any`,
//!     `filter_satisfiable`, `exclude_non_meta_vars`) internally instead of
//!     going through the Python seams.
//!   * `repack_callable_args` (constraints.py:1871-1894) — normalizes a
//!     callable with a star argument into a flat arg-type list with the
//!     unpack in the middle.
//!
//! Deferral policy mirrors the other constraint seams: any wire form the
//! port cannot decide exactly (`TypeAliasType` targets, `TypeVarType`
//! origins whose meta_level the wire round-trip cannot verify, subtype
//! checks the engine defers) returns `None` so the Python shim runs the
//! original body. Results are returned as wire bytes of the selected
//! constraint indices; the Python shim re-indexes its live `Constraint`
//! objects, preserving identity the solver relies on.

use pyo3::prelude::*;

use crate::subtypes::{is_same_type, is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type, WireError, WriteBuffer};

/// Each constraint on the wire is `origin Type | op int | target Type`
/// (mirrors constraints_helpers.rs).
#[derive(Clone)]
pub(crate) struct ConstraintRep {
    pub(crate) origin: Type,
    pub(crate) op: i64,
    pub(crate) target: Type,
}

fn read_constraint(buf: &mut ReadBuffer<'_>) -> Option<ConstraintRep> {
    let origin = wire::read_type(buf, None).ok()?;
    let op = wire::read_int(buf).ok()?;
    let target = wire::read_type(buf, None).ok()?;
    Some(ConstraintRep { origin, op, target })
}

/// The options wire format: bare count + N option blobs. A None option is
/// a bare `-1`; a present option is a bare count + M× constraint (mirrors
/// `_try_native_any_constraints` in constraints.py).
fn read_options(bytes: &[u8]) -> Option<Vec<Option<Vec<ConstraintRep>>>> {
    let mut buf = ReadBuffer::new(bytes);
    let n = read_size(&mut buf)?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let m = read_size(&mut buf)?;
        if m < 0 {
            out.push(None);
            continue;
        }
        let mut option = Vec::with_capacity(m as usize);
        for _ in 0..m {
            option.push(read_constraint(&mut buf)?);
        }
        out.push(Some(option));
    }
    Some(out)
}

/// Read a bare (untagged) size; negative sizes are invalid.
fn read_size(buf: &mut ReadBuffer<'_>) -> Option<i64> {
    let size = wire::read_int_bare(buf).ok()?;
    if size < 0 {
        return None;
    }
    Some(size)
}

/// `Constraint.type_var` — `TypeVarId.__eq__` is `(raw_id, meta_level,
/// namespace)`. Mirrors constraints_helpers.rs `typevar_ids_equal`.
fn typevar_ids_equal(a: &Type, b: &Type) -> Option<bool> {
    let key = |t: &Type| match t {
        Type::TypeVarType {
            raw_id,
            namespace,
            meta_level,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        _ => None,
    };
    match (key(a), key(b)) {
        (Some(a), Some(b)) => Some(a == b),
        _ => None, // defer on alias / ParamSpec / TypeVarTuple origins
    }
}

/// `isinstance(get_proper_type(c.target), AnyType)`. `TypeAliasType`
/// targets defer because `get_proper_type` cannot expand them on the wire.
fn is_any_target(constraint: &ConstraintRep) -> Option<bool> {
    match &constraint.target {
        Type::AnyType { .. } => Some(true),
        Type::TypeAliasType { .. } => None,
        _ => Some(false),
    }
}

/// `all(isinstance(get_proper_type(c.target), AnyType))` (select_trivial).
fn all_any_targets(constraints: &[ConstraintRep]) -> Option<bool> {
    for constraint in constraints {
        if !is_any_target(constraint)? {
            return Some(false);
        }
    }
    Some(true)
}

/// `constraint.target` unioned with Any (merge_with_any, constraints.py:
/// 822-834): keep the target unless it already contains Any, else build
/// `Union(target, Any)` with `type_of_any=implementation_artifact` (8).
fn merge_with_any(constraint: &ConstraintRep) -> Option<ConstraintRep> {
    if is_union_with_any(&constraint.target)? {
        return Some(constraint.clone());
    }
    let any_type = Type::AnyType {
        type_of_any: 8, // TypeOfAny.implementation_artifact
        source_any: None,
        missing_import_name: None,
    };
    let target = make_union(vec![constraint.target.clone(), any_type]);
    Some(ConstraintRep {
        origin: constraint.origin.clone(),
        op: constraint.op,
        target,
    })
}

/// `types_utils.is_union_with_any` (types_utils.py:110-119): a plain Any
/// or a union with an Any item. Recursive. `TypeAliasType` anywhere in the
/// tree defers (`None`) because `get_proper_type` cannot expand aliases on
/// the wire (mirrors `is_any_target`).
fn is_union_with_any(tp: &Type) -> Option<bool> {
    match tp {
        Type::AnyType { .. } => Some(true),
        Type::TypeAliasType { .. } => None,
        Type::UnionType { items, .. } => {
            for item in items {
                match is_union_with_any(item) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => return None,
                }
            }
            Some(false)
        }
        _ => Some(false),
    }
}

/// `UnionType.make_union(items, line, column)` (types.py:3749-3756):
/// 1 item -> the item, 0 items -> UninhabitedType, else UnionType.
///
/// can_be_true/can_be_false are hardcoded True. The only callers feed an
/// Any item (merge_with_any) or fresh unions, and Python computes these
/// lazily via `any(item.can_be_true)`; a union containing Any is both.
fn make_union(items: Vec<Type>) -> Type {
    match items.len() {
        1 => items.into_iter().next().unwrap(),
        0 => Type::UninhabitedType { ambiguous: false },
        _ => {
            let mut flattened = Vec::new();
            flatten_union_items(items.into_iter(), &mut flattened);
            Type::UnionType {
                items: flattened,
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            }
        }
    }
}

/// `flatten_nested_unions(items, handle_type_alias_type=False)`
/// (types.py:4956): a union flattens nested unions into its item list.
fn flatten_union_items(iter: impl Iterator<Item = Type>, out: &mut Vec<Type>) {
    for item in iter {
        if let Type::UnionType { items, .. } = item {
            flatten_union_items(items.into_iter(), out);
        } else {
            out.push(item);
        }
    }
}

/// `is_same_constraint` (constraints.py:961-970): same type var and
/// (op or both targets Any) and `is_same_type` on targets.
fn is_same_constraint(
    c1: &ConstraintRep,
    c2: &ConstraintRep,
    resolver: &TypeResolver,
) -> Option<bool> {
    let skip_op_check = match (is_any_target(c1), is_any_target(c2)) {
        (Some(true), Some(true)) => true,
        (Some(_), Some(_)) => false,
        _ => return None,
    };
    if !typevar_ids_equal(&c1.origin, &c2.origin)? {
        return Some(false);
    }
    if !skip_op_check && c1.op != c2.op {
        return Some(false);
    }
    is_same_type(&c1.target, &c2.target, true, true, resolver)
}

/// `is_same_constraints` (constraints.py:951-958): every constraint in
/// each list has a same one in the other.
fn is_same_constraints(
    x: &[ConstraintRep],
    y: &[ConstraintRep],
    resolver: &TypeResolver,
) -> Option<bool> {
    for c1 in x {
        let mut any_same = false;
        for c2 in y {
            match is_same_constraint(c1, c2, resolver) {
                Some(true) => {
                    any_same = true;
                    break;
                }
                Some(false) => {}
                None => return None,
            }
        }
        if !any_same {
            return Some(false);
        }
    }
    for c1 in y {
        let mut any_same = false;
        for c2 in x {
            match is_same_constraint(c1, c2, resolver) {
                Some(true) => {
                    any_same = true;
                    break;
                }
                Some(false) => {}
                None => return None,
            }
        }
        if !any_same {
            return Some(false);
        }
    }
    Some(true)
}

/// `is_similar_constraints` one direction (constraints.py:990-1007):
/// every constraint in `x` has a similar one in `y` — same type var and
/// (op or either target Any). TypeAlias targets / non-TypeVar origins defer.
fn is_similar_inner(x: &[ConstraintRep], y: &[ConstraintRep]) -> Option<bool> {
    'outer: for c1 in x {
        for c2 in y {
            if !typevar_ids_equal(&c1.origin, &c2.origin)? {
                continue;
            }
            let skip_op = is_any_target(c1)? || is_any_target(c2)?;
            if skip_op || c1.op == c2.op {
                continue 'outer;
            }
        }
        return Some(false);
    }
    Some(true)
}

/// `is_similar_constraints` (constraints.py:973-987): both directions.
fn is_similar_constraints(x: &[ConstraintRep], y: &[ConstraintRep]) -> Option<bool> {
    let fwd = is_similar_inner(x, y)?;
    if !fwd {
        return Some(false);
    }
    is_similar_inner(y, x)
}

/// `select_trivial` (constraints.py:804-819): keep options whose every
/// constraint is against Any (empty options count as trivial in the
/// `all` sense; they are handled by the eager filter before this call).
fn select_trivial(options: &[Option<Vec<ConstraintRep>>]) -> Option<Vec<bool>> {
    let mut selected = Vec::with_capacity(options.len());
    for option in options {
        match option {
            Some(cs) => selected.push(all_any_targets(cs)?),
            None => selected.push(false), // None options were filtered out upstream
        }
    }
    Some(selected)
}

/// `filter_satisfiable` (constraints.py:911-932): keep only constraints
/// that can possibly be satisfied given the origin's values / upper bound.
/// `strict_optional=True` matches the wire check (the Python body reads
/// the global state).
fn filter_satisfiable(
    option: &Option<Vec<ConstraintRep>>,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<Option<Vec<ConstraintRep>>> {
    let Some(cs) = option else {
        return Some(None);
    };
    if cs.is_empty() {
        return Some(option.clone());
    }
    let mut satisfiable: Vec<ConstraintRep> = Vec::new();
    for c in cs {
        let Type::TypeVarType {
            values,
            upper_bound,
            ..
        } = &c.origin
        else {
            return None; // no TypeVarType origin (ParamSpec etc.) for this path
        };
        if !values.is_empty() {
            let mut any_value = false;
            for value in values {
                match is_subtype(&c.target, value, ctx, resolver) {
                    Some(true) => {
                        any_value = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            if any_value {
                satisfiable.push(c.clone());
            }
        } else {
            match is_subtype(&c.target, upper_bound, ctx, resolver) {
                Some(true) => satisfiable.push(c.clone()),
                Some(false) => {}
                None => return None,
            }
        }
    }
    if satisfiable.is_empty() {
        return Some(None);
    }
    Some(Some(satisfiable))
}

/// `exclude_non_meta_vars` (constraints.py:935-948): drop constraints
/// whose origin is not a meta var; keep an empty option intact.
fn exclude_non_meta_vars(
    option: &Option<Vec<ConstraintRep>>,
) -> Option<Option<Vec<ConstraintRep>>> {
    let Some(cs) = option else {
        return Some(None);
    };
    if cs.is_empty() {
        return Some(option.clone());
    }
    let mut kept: Vec<ConstraintRep> = Vec::new();
    for c in cs {
        let Type::TypeVarType { meta_level, .. } = &c.origin else {
            return None; // defer on ParamSpec / TypeVarTuple origins
        };
        if *meta_level > 0 {
            kept.push(c.clone());
        }
    }
    if kept.is_empty() {
        return Some(None);
    }
    Some(Some(kept))
}

/// The recursive body of `any_constraints` (constraints.py:853-908),
/// returning the resulting constraints in order (wire `ConstraintRep`s),
/// or `None` to defer the whole call to Python.
pub(crate) fn any_constraints_inner(
    options: &[Option<Vec<ConstraintRep>>],
    eager: bool,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<Vec<ConstraintRep>> {
    // valid_options: eager drops empty lists, otherwise drops None.
    let mut valid: Vec<usize> = Vec::new();
    for (i, option) in options.iter().enumerate() {
        let keep = match option {
            Some(cs) => {
                if eager {
                    !cs.is_empty()
                } else {
                    true
                }
            }
            None => false,
        };
        if keep {
            valid.push(i);
        }
    }
    if valid.is_empty() {
        return Some(Vec::new());
    }
    if valid.len() == 1 {
        return Some(options[valid[0]].as_ref()?.clone());
    }
    // All the same: pick the first.
    let first = options[valid[0]].as_ref()?;
    let mut all_same = true;
    for &i in &valid[1..] {
        if !is_same_constraints(first, options[i].as_ref()?, resolver)? {
            all_same = false;
            break;
        }
    }
    if all_same {
        return Some(first.clone());
    }
    // All similar: merge-in trivial options and recurse.
    let mut all_similar = true;
    for &i in &valid[1..] {
        if !is_similar_constraints(first, options[i].as_ref()?)? {
            all_similar = false;
            break;
        }
    }
    if all_similar {
        let trivial = select_trivial(options)?;
        let mut trivial_count = 0;
        let mut any_trivial = false;
        for (i, option) in options.iter().enumerate() {
            if trivial[i] && option.is_some() {
                trivial_count += 1;
                any_trivial = true;
            }
        }
        if any_trivial && (trivial_count as usize) < valid.len() {
            let mut merged_options: Vec<Option<Vec<ConstraintRep>>> = Vec::new();
            for (i, option) in options.iter().enumerate() {
                if trivial[i] || option.is_none() {
                    continue;
                }
                let mut merged = Vec::new();
                for c in option.as_ref()? {
                    merged.push(merge_with_any(c)?);
                }
                merged_options.push(Some(merged));
            }
            return any_constraints_inner(&merged_options, eager, ctx, resolver);
        }
    }
    // Filter satisfiable and retry.
    let mut filtered_options: Vec<Option<Vec<ConstraintRep>>> = Vec::with_capacity(options.len());
    for option in options {
        filtered_options.push(filter_satisfiable(option, ctx, resolver)?);
    }
    if !options_equal(&filtered_options, options) {
        return any_constraints_inner(&filtered_options, eager, ctx, resolver);
    }
    // Exclude non-meta vars and retry.
    let mut filtered_options: Vec<Option<Vec<ConstraintRep>>> = Vec::with_capacity(options.len());
    for option in options {
        filtered_options.push(exclude_non_meta_vars(option)?);
    }
    if !options_equal(&filtered_options, options) {
        return any_constraints_inner(&filtered_options, eager, ctx, resolver);
    }
    Some(Vec::new())
}

fn options_equal(a: &[Option<Vec<ConstraintRep>>], b: &[Option<Vec<ConstraintRep>>]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
            (None, None) => true,
            (Some(x), Some(y)) => {
                x.len() == y.len()
                    && x.iter().zip(y.iter()).all(|(cx, cy)| {
                        cx.origin == cy.origin && cx.op == cy.op && cx.target == cy.target
                    })
            }
            _ => false,
        })
}

/// `#[pyfunction]` entry for `any_constraints`. Wire in: bare count + N
/// option blobs (each: bare count + M× constraint; a bare `-1` marker for a
/// None option). Wire out: `list[bytes]`, one blob per result constraint
/// (`origin Type | op int | target Type`), or `None` (defer).
#[pyfunction]
pub(crate) fn rust_any_constraints(
    options_bytes: &[u8],
    eager: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let options = read_options(options_bytes)?;
    // Matches the pure-Python `is_subtype` defaults (subtypes.py:260-270):
    // all flags False except strict_optional, which follows the state.
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    let result = any_constraints_inner(&options, eager, &ctx, resolver.resolver())?;
    let mut out = Vec::with_capacity(result.len());
    for constraint in result {
        let mut blob = WriteBuffer::new();
        wire::write_type(&mut blob, &constraint.origin).ok()?;
        write_constraint_op(&mut blob, constraint.op).ok()?;
        wire::write_type(&mut blob, &constraint.target).ok()?;
        out.push(blob.into_bytes());
    }
    Some(out)
}

fn write_constraint_op(buf: &mut WriteBuffer, op: i64) -> Result<(), WireError> {
    wire::write_int(buf, op)
}

/// Read a single option blob: bare count + M× constraint.
fn read_option_list(bytes: &[u8]) -> Option<Vec<ConstraintRep>> {
    let mut buf = ReadBuffer::new(bytes);
    let n = read_size(&mut buf)?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        out.push(read_constraint(&mut buf)?);
    }
    Some(out)
}

/// Standalone seam for `merge_with_any` (constraints.py:924-936), Issue
/// #1001. Rust only decides whether the target already contains Any:
/// `true` = a union with Any is needed, `false` = the target already
/// contains Any and the constraint is kept intact. Python applies the
/// decision so the live target and origin type var keep their identity.
/// Wire in: one constraint (origin Type | op int | target Type). Defers
/// (`None`) on a `TypeAliasType` target, which `get_proper_type` cannot
/// expand on the wire.
#[pyfunction]
pub(crate) fn rust_merge_with_any(constraint_bytes: &[u8]) -> Option<bool> {
    let mut buf = ReadBuffer::new(constraint_bytes);
    let constraint = read_constraint(&mut buf)?;
    is_union_with_any(&constraint.target).map(|has_any| !has_any)
}

/// Standalone seam for `filter_satisfiable` (constraints.py:1020-1041),
/// Issue #1001: keep only constraints that can possibly be satisfied
/// given the origin's values / upper bound. Wire out: kept constraint
/// indices as bare ints (empty = all filtered, Python returns `None`).
/// Defers (`None`) on non-`TypeVarType` origins (ParamSpec /
/// TypeVarTuple) and subtype checks the engine cannot decide.
#[pyfunction]
pub(crate) fn rust_filter_satisfiable(
    option_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let option = read_option_list(option_bytes)?;
    // Matches the pure-Python `is_subtype` defaults (subtypes.py:260-270):
    // all flags False except strict_optional, which follows the state.
    let ctx = SubtypeContext::new(false, false, false, false, false, strict_optional);
    let mut kept = Vec::with_capacity(option.len());
    for (i, c) in option.iter().enumerate() {
        let Type::TypeVarType {
            values,
            upper_bound,
            ..
        } = &c.origin
        else {
            return None; // no TypeVarType origin (ParamSpec etc.) for this path
        };
        let satisfiable = if values.is_empty() {
            is_subtype(&c.target, upper_bound, &ctx, resolver.resolver())?
        } else {
            let mut any_value = false;
            for value in values {
                match is_subtype(&c.target, value, &ctx, resolver.resolver()) {
                    Some(true) => {
                        any_value = true;
                        break;
                    }
                    Some(false) => {}
                    None => return None,
                }
            }
            any_value
        };
        if satisfiable {
            kept.push(i as i64);
        }
    }
    let mut output = WriteBuffer::new();
    crate::wire::write_int_bare(&mut output, kept.len() as i64).ok()?;
    for index in &kept {
        crate::wire::write_int_bare(&mut output, *index).ok()?;
    }
    Some(output.into_bytes())
}

/// Standalone seam for `is_same_constraints` (constraints.py:1060-1067),
/// Issue #1001: two lists are the same when every constraint in each has
/// a same one in the other (same type var, op unless both targets are
/// Any, and `is_same_type` on targets). Wire in: two option blobs. Defers
/// (`None`) on any undecidable pairwise check (alias targets, non-TypeVar
/// origins, subtype deferrals).
#[pyfunction]
pub(crate) fn rust_is_same_constraints(
    x_bytes: &[u8],
    y_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let x = read_option_list(x_bytes)?;
    let y = read_option_list(y_bytes)?;
    is_same_constraints(&x, &y, resolver.resolver())
}

/// `repack_callable_args` (constraints.py:1871-1894): present a callable's
/// arg list in normalized form — prefix args, then the unpack in the
/// middle, then any suffix — as if they appeared in a TupleType.
///
/// The Python body builds `UnpackType(Instance(tuple_type, [star_type]))`
/// when `star_type` is not already an UnpackType; the wire form cannot
/// re-read the caller's `tuple_type` TypeInfo, so in that branch Rust
/// emits `UnpackType(Instance("builtins.tuple", [star_type]))` and the
/// Python shim re-wraps it with the live `tuple_type`.
#[pyfunction]
pub(crate) fn rust_repack_callable_args(
    callable_bytes: &[u8],
    _resolver: &mut NativeTypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let mut callable_buf = ReadBuffer::new(callable_bytes);
    let callable = wire::read_type(&mut callable_buf, None).ok()?;
    let Type::CallableType {
        arg_types,
        arg_kinds,
        ..
    } = callable
    else {
        return None;
    };
    // ARG_STAR == 2 (mypy/nodes.py ArgKind.ARG_STAR).
    let star_index = arg_kinds.iter().position(|k| *k == 2)?;
    let prefix = &arg_types[..star_index];
    let star_type = &arg_types[star_index];
    let (middle, suffix_types): (Type, Vec<Type>) = match star_type {
        Type::UnpackType { typ } => match typ.as_ref() {
            Type::TupleType { items, .. } => {
                // Python: `assert isinstance(tp.items[0], UnpackType);
                // star_type = tp.items[0]` — the first item itself (an
                // UnpackType) becomes the spliced middle.
                if !matches!(items[0], Type::UnpackType { .. }) {
                    return None; // assert isinstance(tp.items[0], UnpackType)
                }
                (items[0].clone(), items[1..].to_vec())
            }
            _ => (star_type.clone(), Vec::new()),
        },
        _ => (
            // Re-normalize *args: X -> *args: *tuple[X, ...]. The wire
            // carries only fullnames; the shim re-wraps with live TypeInfo.
            Type::UnpackType {
                typ: Box::new(Type::Instance {
                    type_ref: "builtins.tuple".to_string(),
                    args: vec![star_type.clone()],
                    last_known_value: None,
                    extra_attrs: None,
                }),
            },
            Vec::new(),
        ),
    };
    let mut out: Vec<Type> = Vec::with_capacity(prefix.len() + 1 + suffix_types.len());
    out.extend_from_slice(prefix);
    out.push(middle);
    out.extend(suffix_types);
    // One blob per repacked type so the Python shim can fix up each
    // Instance ref against the live TypeInfo (fullname -> TypeInfo).
    let mut result = Vec::with_capacity(out.len());
    for item in out {
        let mut blob = WriteBuffer::new();
        wire::write_type(&mut blob, &item).ok()?;
        result.push(blob.into_bytes());
    }
    Some(result)
}
