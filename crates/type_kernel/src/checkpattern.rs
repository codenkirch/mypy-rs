//! Native port of pure helper functions from `mypy/checkpattern.py`
//! (M22, issue #300).
//!
//! `PatternChecker` itself is a visitor deeply intertwined with the type
//! checker (`chk.expr_checker.accept`, `conditional_types_with_intersection`,
//! `msg.fail`), so the full visitor cannot be ported as standalone Rust
//! functions without porting the entire checker. Instead, this module ports
//! the standalone pure-logic helpers that `PatternChecker` calls:
//!
//! * `rust_is_uninhabited` — mirrors `checkpattern.is_uninhabited`.
//! * `rust_get_match_arg_names` — mirrors
//!   `checkpattern.get_match_arg_names`.
//! * `rust_get_type_range` — mirrors `checkpattern.get_type_range`: whether a
//!   bool `last_known_value` should unwrap before wrapping in a `TypeRange`.
//! * `rust_should_self_match` — mirrors `PatternChecker.should_self_match`.
//! * `rust_can_match_sequence` — mirrors `PatternChecker.can_match_sequence`.
//!
//! * `rust_contract_starred_pattern_types` — mirrors
//!   `PatternChecker.contract_starred_pattern_types`, re-shaping a list of
//!   types around a starred capture so a sequence pattern can be matched
//!   against a fixed-length type list.
//! * `rust_expand_starred_pattern_types` — mirrors
//!   `PatternChecker.expand_starred_pattern_types`, the inverse operation
//!   that restores the star item before a match.
//! * `rust_construct_sequence_child` — mirrors
//!   `PatternChecker.construct_sequence_child`, producing the inner sequence
//!   type used to recurse into a sequence pattern's items.
//! * `rust_classify_class_pattern_ranges` — mirrors the dispatch of
//!   `PatternChecker.get_class_pattern_type_ranges` (issue #987): one branch
//!   tag per leaf item, union recursion on the wire.
//!
//! Each function takes wire-format bytes (serialized `Type` objects) and a
//! `NativeTypeResolver` for subtyping checks. Returns `None` to defer to
//! Python when the wire form cannot be fully decoded, a subtyping check
//! is undecided, or the type shape differs from the mechanical subset this
//! module implements (the strangler-fig per-call gate).

use pyo3::prelude::*;
use pyo3::types::PyType;

use crate::setops::make_simplified_union;
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::{find_unpack_in_list_inner, split_with_prefix_and_suffix_inner};
use crate::wire::{self, LiteralValue, ReadBuffer, Type};

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

/// Fetch a class from `mypy.nodes`. Mirrors the private helper in
/// `semanal_bases.rs` / `checker_functions.rs`.
fn nodes_class<'py>(py: Python<'py>, name: &str) -> PyResult<&'py PyType> {
    py.import("mypy.nodes")?
        .getattr(name)?
        .downcast::<PyType>()
        .map_err(Into::into)
}

/// Encode a `Type` via `write_type`. Returns `None` if the variant is not
/// writable (the caller defers to Python).
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = crate::wire::WriteBuffer::new();
    crate::wire::write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
}

/// Encode a list of types, dropping any that cannot be serialized.
fn encode_type_list(types: &[Type]) -> Vec<Vec<u8>> {
    types.iter().filter_map(encode_type).collect()
}

/// Decode a list of type blobs, returning `None` on any read failure.
fn decode_type_list(blobs: &[Vec<u8>]) -> Option<Vec<Type>> {
    let mut out = Vec::with_capacity(blobs.len());
    for b in blobs {
        out.push(decode_type(b)?);
    }
    Some(out)
}

/// `get_proper_type` on the wire: a `ProperType` that may sit behind
/// nothing at all. A `TypeAliasType` cannot be expanded (Python resolves
/// it from live `TypeInfo`), so it defers. This mirrors the private
/// `get_proper` helper used by `setops` and isolates the alias rejection.
fn proper_wire(t: &Type) -> Option<&Type> {
    match t {
        Type::TypeAliasType { .. } => None,
        other => Some(other),
    }
}

/// `checkpattern.is_uninhabited(typ)` — whether `get_proper_type(typ)` is an
/// `UninhabitedType`.
///
/// Mirrors checkpattern.py:884-885. The shim passes the live type directly
/// (possibly a `TypeAliasType`), so a top-level alias expands through the
/// resolver before the `UninhabitedType` match. Returns `None` (defer) when
/// the wire form cannot be decoded or the alias cannot be resolved.
#[pyfunction]
pub(crate) fn rust_is_uninhabited(
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    // Python: isinstance(get_proper_type(typ), UninhabitedType). A top-level
    // TypeAliasType must expand through the resolver before the check; a
    // missing snapshot or unresolvable substitution defers to Python.
    let t = crate::checkexpr_functions::get_proper_or_expand(&t, resolver.alias_resolver())?;
    Some(matches!(t, Type::UninhabitedType { .. }))
}

/// `checkpattern.get_match_arg_names(typ)` — extract match-arg names from a
/// `TupleType`'s items.
///
/// Mirrors checkpattern.py:851-859. For each item in `typ.items`, calls
/// `try_getting_str_literals_from_type`. If the result is `None` or has
/// length != 1, appends `None`; otherwise appends the single str value.
///
/// Returns `None` (defer to Python) when the wire form cannot be decoded,
/// the type is not a `TupleType`, or an item `TypeAliasType` cannot be
/// resolved (which changes the answer). Returns `Some(list)` where each
/// element is a string or `None`.
#[pyfunction]
pub(crate) fn rust_get_match_arg_names(
    py: Python<'_>,
    t_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<PyObject> {
    let t = decode_type(t_bytes)?;
    let items = match &t {
        Type::TupleType { items, .. } => items,
        _ => return None,
    };
    let aliases = resolver.alias_resolver();
    let mut names: Vec<PyObject> = Vec::with_capacity(items.len());
    for item in items {
        // An unresolvable TypeAliasType item must defer the whole call:
        // Python's try_getting_str_literals_from_type resolves it via
        // get_proper_type, and a missing snapshot would change the answer.
        if matches!(item, Type::TypeAliasType { .. })
            && crate::checkexpr_functions::get_proper_or_expand(item, aliases).is_none()
        {
            return None;
        }
        match extract_single_str_literal(item, aliases) {
            Some(s) => names.push(s.into_py(py)),
            None => names.push(py.None()),
        }
    }
    Some(pyo3::types::PyList::new(py, names).into())
}

/// Extract a single string literal from a wire `Type`, mirroring
/// `try_getting_str_literals_from_type` returning exactly one value.
///
/// Handles: Instance with str LKV, LiteralType with str fallback, UnionType
/// of str literals. Returns `None` if no single str literal can be extracted
/// (matching Python returning `None` or length != 1) or an unresolvable
/// `TypeAliasType` item is reached.
fn extract_single_str_literal(
    t: &Type,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<String> {
    // An item-level TypeAliasType expands through the resolver, mirroring the
    // get_proper_type call inside try_getting_str_literals_from_type.
    let t = crate::checkexpr_functions::get_proper_or_expand(t, aliases)?;
    let candidates: Vec<&Type> = match &t {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => vec![lkv.as_ref()],
        Type::UnionType { items, .. } => items.iter().collect(),
        _ => vec![&t],
    };
    let mut found: Option<String> = None;
    for c in candidates {
        match c {
            Type::LiteralType { fallback, value } => {
                let Type::Instance { type_ref, .. } = fallback.as_ref() else {
                    return None;
                };
                if type_ref != "builtins.str" {
                    return None;
                }
                match value {
                    LiteralValue::Str(s) => {
                        if found.is_some() {
                            return None;
                        }
                        found = Some(s.clone());
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
    }
    found
}

/// `checkpattern.get_type_range(typ)` — determine whether a type's
/// `last_known_value` (if it's a bool) should be unwrapped before wrapping in
/// a `TypeRange`.
///
/// Mirrors checkpattern.py:873-881. The Python function:
///   ```text
///   typ = get_proper_type(typ)
///   if isinstance(typ, Instance) and typ.last_known_value
///       and isinstance(typ.last_known_value.value, bool):
///       typ = typ.last_known_value
///   return TypeRange(typ, is_upper_bound=False)
///   ```
///
/// Returns `Some(true)` when the type is an Instance with a bool LKV (the
/// caller should unwrap to `typ.last_known_value` before building the
/// TypeRange). Returns `Some(false)` when the type does not need unwrapping.
/// Returns `None` (defer) when the wire form is a `TypeAliasType`.
#[pyfunction]
pub(crate) fn rust_get_type_range(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    match &t {
        Type::Instance {
            last_known_value: Some(lkv),
            ..
        } => {
            // Check if the LKV is a LiteralType with a bool value and a
            // builtins.bool fallback.
            match lkv.as_ref() {
                Type::LiteralType { fallback, value } => {
                    let is_bool = matches!(
                        fallback.as_ref(),
                        Type::Instance { type_ref, .. } if type_ref == "builtins.bool"
                    ) && matches!(value, LiteralValue::Bool(_));
                    Some(is_bool)
                }
                _ => Some(false),
            }
        }
        _ => Some(false),
    }
}

/// `PatternChecker.should_self_match(typ)` — whether a class pattern should
/// match against the type itself rather than its `__match_args__`.
///
/// Mirrors checkpattern.py:756-769. The Python method:
///   ```text
///   typ = get_proper_type(typ)
///   if isinstance(typ, TupleType):
///       typ = typ.partial_fallback
///   if isinstance(typ, AnyType):
///       return False
///   if isinstance(typ, Instance) and typ.type.get("__match_args__") is not None:
///       return False
///   for other in self.self_match_types:
///       if is_subtype(typ, other):
///           return True
///   return False
///   ```
///
/// The `__match_args__` check needs live `TypeInfo` (not on the wire), so it
/// is handled by the Python shim before calling this function. The shim
/// passes `has_match_args: bool` so Rust can short-circuit.
///
/// Returns `None` (defer) when the type is a `TypeAliasType` or any subtype
/// check returns `None`.
#[pyfunction]
pub(crate) fn rust_should_self_match(
    t_bytes: &[u8],
    has_match_args: bool,
    self_match_types_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    let self_match_types = decode_type(self_match_types_bytes)?;

    // Unwrap TupleType to its partial_fallback.
    let typ = match &t {
        Type::TupleType {
            partial_fallback, ..
        } => partial_fallback.as_ref(),
        _ => &t,
    };

    // AnyType -> False.
    if matches!(typ, Type::AnyType { .. }) {
        return Some(false);
    }

    // Instance with __match_args__ -> False (checked by Python shim).
    if has_match_args && matches!(typ, Type::Instance { .. }) {
        return Some(false);
    }

    // Check is_subtype(typ, other) for each self_match_type.
    let items = match &self_match_types {
        Type::UnionType { items, .. } => items.clone(),
        // Single type wrapped in a list.
        _ => vec![self_match_types.clone()],
    };

    let ctx = SubtypeContext::new(false, false, false, false, false, true);
    for other in &items {
        match is_subtype(typ, other, &ctx, resolver.resolver()) {
            Some(true) => return Some(true),
            None => return None,
            Some(false) => {}
        }
    }
    Some(false)
}

/// `PatternChecker.can_match_sequence(typ)` — whether a type can match a
/// sequence pattern.
///
/// Mirrors checkpattern.py:771-783. The Python method:
///   ```text
///   if isinstance(typ, AnyType): return True
///   if isinstance(typ, UnionType):
///       return any(self.can_match_sequence(item) for item in typ.items)
///   for other in self.non_sequence_match_types:
///       if is_subtype(typ, other, ignore_promotions=True): return False
///   sequence = self.chk.named_type("typing.Sequence")
///   return is_subtype(typ, sequence) or is_subtype(sequence, typ)
///   ```
///
/// `self.non_sequence_match_types` and `typing.Sequence` are serialized as
/// wire bytes by the Python shim. Returns `None` (defer) when any subtype
/// check is undecided.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_can_match_sequence(
    t_bytes: &[u8],
    non_seq_types_bytes: &[u8],
    sequence_type_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    let non_seq_types = decode_type(non_seq_types_bytes)?;
    let sequence_type = decode_type(sequence_type_bytes)?;

    can_match_sequence_inner(&t, &non_seq_types, &sequence_type, resolver)
}

/// Recursive inner: mirrors the UnionType recursion in the Python method.
fn can_match_sequence_inner(
    typ: &Type,
    non_seq_types: &Type,
    sequence_type: &Type,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    // AnyType -> True.
    if matches!(typ, Type::AnyType { .. }) {
        return Some(true);
    }
    // UnionType -> any(item can match sequence).
    if let Type::UnionType { items, .. } = typ {
        for item in items {
            match can_match_sequence_inner(item, non_seq_types, sequence_type, resolver) {
                Some(true) => return Some(true),
                None => return None,
                Some(false) => {}
            }
        }
        return Some(false);
    }
    // non_sequence_match_types: if is_subtype(typ, other, ignore_promotions)
    // -> return False.
    let non_seq_items = match non_seq_types {
        Type::UnionType { items, .. } => items.clone(),
        _ => vec![non_seq_types.clone()],
    };
    // ignore_promotions=True, proper_subtype=False, strict_optional=True.
    let ctx = SubtypeContext::new(false, false, false, true, false, true);
    for other in &non_seq_items {
        match is_subtype(typ, other, &ctx, resolver.resolver()) {
            Some(true) => return Some(false),
            None => return None,
            Some(false) => {}
        }
    }
    // sequence check: is_subtype(typ, sequence) or is_subtype(sequence, typ).
    match is_subtype(typ, sequence_type, &ctx, resolver.resolver()) {
        Some(true) => Some(true),
        None => None,
        Some(false) => match is_subtype(sequence_type, typ, &ctx, resolver.resolver()) {
            Some(true) => Some(true),
            None => None,
            Some(false) => Some(false),
        },
    }
}

/// `PatternChecker.contract_starred_pattern_types(types, star_pos,
/// num_patterns)` — contract a list of types in a sequence pattern around a
/// starred capture position.
///
/// Mirrors checkpattern.py:434-481. Two regimes:
/// 1. A variadic `UnpackType` is present (unaligned tuple): re-shape the
///    list around the unpack so the requested pattern length fits.
/// 2. A fixed-length list with `star_pos` (starred capture, no unpack):
///    collapse the `star_length` middle items into a simplified union.
///
/// The unpack branch requires the type-level `find_unpack_in_list`,
/// `split_with_prefix_and_suffix`, and `make_simplified_union` (whose
/// subtyping steps need the resolver). Returns `None` (defer to Python)
/// when any type cannot be decoded, the unpack is not `Instance[builtins.
/// tuple]`, or a union cannot be simplified.
#[pyfunction]
#[pyo3(signature = (types_bytes, star_pos, num_patterns, resolver))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_contract_starred_pattern_types(
    types_bytes: Vec<Vec<u8>>,
    star_pos: Option<i64>,
    num_patterns: i64,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let star_pos = match star_pos {
        Some(p) if p < 0 => return None,
        Some(p) => Some(p as usize),
        None => None,
    };
    let num_patterns: usize = match num_patterns.try_into() {
        Ok(v) => v,
        Err(_) => return None,
    };
    let types = decode_type_list(&types_bytes)?;
    let unpack_index = find_unpack_in_list_inner(&types);
    if unpack_index >= 0 {
        contract_with_unpack(
            types,
            unpack_index as usize,
            star_pos,
            num_patterns,
            Some(resolver.resolver()),
            Some(resolver.alias_resolver()),
        )
    } else {
        contract_no_unpack(types, star_pos, num_patterns, Some(resolver.resolver()))
    }
}

/// Unpack branch: re-shape a variadic list around the requested pattern
/// length. `star_pos == None` broadens the unpack item to cover `missing`
/// pattern slots; `star_pos == Some` normalizes via prefix/suffix split and
/// collapses the middle into a single union.
///
/// The resolver is only used by the `Some(star_pos)` branch (to simplify the
/// middle union); the alias resolver expands a top-level `TypeAliasType` on
/// the unpack inner type before the `builtins.tuple` check. Both are
/// `Option` so the pure logic is unit-testable offline.
fn contract_with_unpack(
    types: Vec<Type>,
    unpack_index: usize,
    star_pos: Option<usize>,
    num_patterns: usize,
    resolver: Option<&TypeResolver>,
    aliases: Option<&crate::aliases::TypeAliasResolver>,
) -> Option<Vec<Vec<u8>>> {
    let Type::UnpackType {
        typ: unpack_typ, ..
    } = &types[unpack_index]
    else {
        return None;
    };
    // Python: unpacked = get_proper_type(unpack.type). A top-level alias
    // expands through the resolver (unresolvable -> defer); a non-alias
    // passes through whether or not a resolver is present.
    let unpacked = if matches!(unpack_typ.as_ref(), Type::TypeAliasType { .. }) {
        crate::checkexpr_functions::get_proper_or_expand(unpack_typ, aliases?)?
    } else {
        (**unpack_typ).clone()
    };
    let Type::Instance {
        type_ref,
        args: unpacked_args,
        ..
    } = &unpacked
    else {
        return None;
    };
    if type_ref != "builtins.tuple" {
        return None;
    }
    match star_pos {
        None => {
            // missing = num_patterns - len(types) + 1.
            let missing = num_patterns
                .checked_sub(types.len())
                .and_then(|d| d.checked_add(1))?;
            let mut new_types = Vec::with_capacity(types.len() + missing);
            new_types.extend(types[..unpack_index].iter().cloned());
            if let Some(t) = unpacked_args.first() {
                new_types.extend(std::iter::repeat_n(t.clone(), missing));
            }
            new_types.extend(types[unpack_index + 1..].iter().cloned());
            Some(encode_type_list(&new_types))
        }
        Some(pos) => {
            // The Python split only reads `args[0]` of any UnpackType-wrapped
            // builtins.tuple, so passing `&types` directly is equivalent.
            let suffix = num_patterns.checked_sub(pos)?;
            let (prefix, middle, suffix_types) =
                split_with_prefix_and_suffix_inner(&types, pos, suffix);
            let unpack_item = unpacked_args.first().cloned()?;
            let new_middle: Vec<Type> = middle
                .iter()
                .map(|m| {
                    if matches!(m, Type::UnpackType { .. }) {
                        unpack_item.clone()
                    } else {
                        m.clone()
                    }
                })
                .collect();
            let res = resolver?;
            let ctx = SubtypeContext::new(false, false, false, true, true, true);
            let merged = make_simplified_union(&new_middle, &ctx, res, true, false)?;
            let mut out = Vec::with_capacity(prefix.len() + 1 + suffix_types.len());
            out.extend(prefix);
            out.push(merged);
            out.extend(suffix_types);
            Some(encode_type_list(&out))
        }
    }
}

/// Fixed-length branch: with `star_pos` collapse the `star_length` middle
/// items into a single simplified union; with `star_pos == None` return the
/// list unchanged.
fn contract_no_unpack(
    types: Vec<Type>,
    star_pos: Option<usize>,
    num_patterns: usize,
    resolver: Option<&TypeResolver>,
) -> Option<Vec<Vec<u8>>> {
    let Some(pos) = star_pos else {
        return Some(encode_type_list(&types));
    };
    // star_length = len(types) - num_patterns.
    let star_length = types.len().checked_sub(num_patterns)?;
    let slice_end = pos.checked_add(star_length)?;
    if slice_end < pos || slice_end > types.len() {
        return None;
    }
    let res = resolver?;
    let ctx = SubtypeContext::new(false, false, false, true, true, true);
    let merged = make_simplified_union(&types[pos..slice_end], &ctx, res, true, false)?;
    let mut new_types = Vec::with_capacity(pos + 1 + (types.len() - slice_end));
    new_types.extend(types[..pos].iter().cloned());
    new_types.push(merged);
    new_types.extend(types[slice_end..].iter().cloned());
    Some(encode_type_list(&new_types))
}

/// `PatternChecker.expand_starred_pattern_types(types, star_pos,
/// num_types, original_unpack)` — undo the contraction done by
/// `contract_starred_pattern_types`.
///
/// Mirrors checkpattern.py:483-509. With `star_pos == None` the list is
/// returned unchanged. With `original_unpack`, the star item is re-wrapped
/// in `UnpackType[builtins.tuple[t]]` (only when the type is not
/// uninhabited, matching `is_uninhabited`); otherwise the star item is
/// duplicated `star_length = num_types - len(types) + 1` times.
///
/// Returns `None` (defer to Python) when the list cannot be decoded, the
/// star position is out of range, or the star item is an unresolvable
/// `TypeAliasType` (expansion is needed to decide `is_uninhabited` before
/// re-wrapping). Non-star aliases pass through unchanged and do not defer.
#[pyfunction]
#[pyo3(signature = (types_bytes, star_pos, num_types, original_unpack, resolver))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_expand_starred_pattern_types(
    types_bytes: Vec<Vec<u8>>,
    star_pos: Option<i64>,
    num_types: i64,
    original_unpack: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let Some(pos) = star_pos else {
        // star_pos is None: types returned unchanged, no decode needed.
        return Some(types_bytes);
    };
    if pos < 0 {
        return None;
    }
    let pos = pos as usize;
    let types = decode_type_list(&types_bytes)?;
    // Never rewrap an alias in the star slot: the caller's is_uninhabited
    // resolves it live. Non-star aliases pass through unchanged, so they are
    // not a deferral reason.
    if pos >= types.len() {
        // The star item must exist before we can expand it.
        return None;
    }
    let aliases = resolver.alias_resolver();
    if original_unpack {
        let mut res = Vec::with_capacity(types.len());
        for (i, t) in types.into_iter().enumerate() {
            if i != pos || is_uninhabited_wire(&t, aliases)? {
                res.push(t);
            } else {
                res.push(Type::UnpackType {
                    typ: Box::new(Type::Instance {
                        type_ref: "builtins.tuple".to_string(),
                        args: vec![t],
                        last_known_value: None,
                        extra_attrs: None,
                    }),
                    from_star_syntax: false,
                });
            }
        }
        Some(encode_type_list(&res))
    } else {
        // star_length = num_types - len(types) + 1.
        let star_length = match num_types
            .checked_sub(types.len() as i64)
            .and_then(|d| d.checked_add(1))
        {
            Some(d) if d > 0 => d as usize,
            _ => return None,
        };
        let mut new_types = Vec::with_capacity(pos + star_length + (types.len() - pos - 1));
        new_types.extend(types[..pos].iter().cloned());
        new_types.extend(std::iter::repeat_n(types[pos].clone(), star_length));
        new_types.extend(types[pos + 1..].iter().cloned());
        Some(encode_type_list(&new_types))
    }
}

/// `checkpattern.is_uninhabited(typ)` on the wire: expand any top-level
/// `TypeAliasType` operand through the alias resolver, then check for
/// `UninhabitedType`. Returns `None` to defer when the alias cannot be
/// resolved (missing snapshot / substitution), so the Python fallback runs.
fn is_uninhabited_wire(t: &Type, aliases: &crate::aliases::TypeAliasResolver) -> Option<bool> {
    let t = crate::checkexpr_functions::get_proper_or_expand(t, aliases)?;
    Some(matches!(t, Type::UninhabitedType { .. }))
}

/// `PatternChecker.construct_sequence_child(outer_type, inner_type)` — if
/// `outer_type` is a subtype of `typing.Sequence`, produce a new instance of
/// it whose type argument is `inner_type`; otherwise produce
/// `Sequence[inner_type]`.
///
/// Mirrors checkpattern.py:877-911 for the non-recursive subset (a `TupleType`
/// or `Instance` after `get_proper_type`, or a direct `AnyType`).
/// `TypeVarType`/`UnionType` inputs recurse into children that need
/// `copy_modified`/`can_match_sequence` and are deferred to Python (the shim
/// returns `None` for them before serializing).
///
/// The `empty_type` (`fill_typevars(proper_type.type)`) and the sequence
/// instance (`named_generic_type("typing.Sequence", [inner_type])`) are
/// computed on the Python side (they reference live `TypeInfo`), then passed
/// in for expansion. Returns `None` (defer to Python) when any input cannot
/// be decoded, the type is a recursive case, a subtype check is undecided,
/// or the by-instance expansion cannot bind every class typevar.
#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_construct_sequence_child(
    outer_bytes: &[u8],
    empty_type_bytes: &[u8],
    sequence_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<u8>> {
    let outer = decode_type(outer_bytes)?;
    // The shim serializes the original outer_type (possibly an alias) even
    // though it gates on get_proper_type(outer_type) being an Instance, so
    // expand a top-level alias here to mirror get_proper_type.
    let outer =
        crate::checkexpr_functions::get_proper_or_expand(&outer, resolver.alias_resolver())?;
    let empty_type = decode_type(empty_type_bytes)?;
    let sequence = decode_type(sequence_bytes)?;
    construct_sequence_child_inner(&outer, &empty_type, &sequence, Some(resolver.resolver()))
}

/// Inner logic: `AnyType` passes through, `TypeVarType`/`UnionType` defer
/// (recursive children live in Python), and the Instance branch checks
/// `is_subtype(outer, Sequence[Any])` to decide between returning the
/// expanded proper type or the passed-in `Sequence[inner]`.
fn construct_sequence_child_inner(
    outer: &Type,
    empty_type: &Type,
    sequence: &Type,
    resolver: Option<&TypeResolver>,
) -> Option<Vec<u8>> {
    let proper = proper_wire(outer)?;
    if matches!(proper, Type::AnyType { .. }) {
        return encode_type(outer);
    }
    // Recursive children (_copy_modified / _can_match_sequence_filtered):
    // handled by the Python recursion because they need live TypeInfo and
    // Rust does not model those transformations.
    if !matches!(proper, Type::Instance { .. }) {
        return None;
    }
    let res = resolver?;
    let ctx = SubtypeContext::new(false, false, false, false, false, true);
    // Python compares against a bare Sequence (type var filled with
    // TypeOfAny.special_form Any); the parametrized `sequence` would wrongly
    // reject e.g. List[int] <: Sequence[bool] over inner_type=bool.
    let sequence_any = Type::Instance {
        type_ref: "typing.Sequence".to_string(),
        args: vec![Type::AnyType {
            type_of_any: 6,
            source_any: None,
            missing_import_name: None,
        }],
        last_known_value: None,
        extra_attrs: None,
    };
    let can_seq = is_subtype(outer, &sequence_any, &ctx, res)?;
    if !can_seq {
        return encode_type(sequence);
    }
    // Mirror Python's two-step split (checkpattern.py:976-977): bind the
    // class tvars from the narrowed sequence args, then re-expand; a single
    // expand against `outer` would substitute object, not the narrowed int.
    let step1 = crate::expandtype::expand_type_by_instance_core(empty_type, sequence, res, true)?;
    let final_t = crate::expandtype::expand_type_by_instance_core(&step1, outer, res, true)?;
    encode_type(&final_t)
}

// ---------------------------------------------------------------------------
// rust_classify_class_pattern_ranges (issue #987)
// ---------------------------------------------------------------------------

/// Branch tags returned to the Python shim for
/// `PatternChecker.get_class_pattern_type_ranges` (checkpattern.py:794-832):
/// - `FAIL`: no arm matched; Python reports CLASS_PATTERN_TYPE_REQUIRED.
/// - `TYPE_OBJ`: FunctionLike type object; Python builds
///   `TypeRange(fill_typevars_with_any(p_typ.type_object()), False)`.
/// - `CALLABLE_VAR`: class_ref is a `typing.Callable` Var; Python builds a
///   `Callable[..., Any]` range via `callable_with_ellipsis`.
/// - `TYPE_TYPE`: `TypeRange(p_typ.item, True)`.
/// - `ANY`: `TypeRange(p_typ, False)`.
pub(crate) const CPR_FAIL: i64 = 0;
pub(crate) const CPR_TYPE_OBJ: i64 = 1;
pub(crate) const CPR_CALLABLE_VAR: i64 = 2;
pub(crate) const CPR_TYPE_TYPE: i64 = 3;
pub(crate) const CPR_ANY: i64 = 4;

/// `PatternChecker.get_class_pattern_type_ranges(typ, o)` — classify each
/// leaf item into a branch tag; union recursion happens here on the wire.
///
/// Python checks, per recursive call: UnionType (recurse per item),
/// FunctionLike + is_type_obj, then the scalar class-ref condition
/// (`isinstance(o.class_ref.node, Var)` + `node.type is not None` +
/// `node.fullname == "typing.Callable"`), then TypeType, AnyType, and the
/// fail tail. Rust mirrors the same order and returns one tag per leaf in
/// union pre-order; the Python shim zips them with the identically
/// flattened live items, builds the TypeRanges, and applies `self.msg.fail`
/// for FAIL tags. Returns `None` (defer) on any wire decode failure, a
/// `TypeAliasType` anywhere (get_proper_type would expand it from live
/// TypeInfo), or a type-object callable whose fallback is not provably
/// `builtins.type` (is_metaclass needs the live fallback TypeInfo).
#[pyfunction]
pub(crate) fn rust_classify_class_pattern_ranges(
    py: Python<'_>,
    typ_bytes: &[u8],
    class_ref_node: Option<&PyAny>,
) -> PyResult<Option<Vec<i64>>> {
    let typ = match decode_type(typ_bytes) {
        Some(t) => t,
        None => return Ok(None),
    };
    let (is_var, node_typed, is_callable_ref) = match class_ref_facts(py, class_ref_node)? {
        Some(facts) => facts,
        None => return Ok(None),
    };
    let callable_var = is_var && node_typed && is_callable_ref;
    let mut tags = Vec::new();
    if classify_class_pattern_inner(&typ, callable_var, &mut tags).is_none() {
        return Ok(None);
    }
    Ok(Some(tags))
}

/// Scalar facts of `o.class_ref.node` read via PyO3: is a `Var`, has a
/// non-None `type`, and has fullname `typing.Callable`. A missing/None node
/// is a plain non-Var; an unreadable attribute defers (`None`).
fn class_ref_facts(py: Python<'_>, node: Option<&PyAny>) -> PyResult<Option<(bool, bool, bool)>> {
    let node = match node {
        Some(n) if !n.is_none() => n,
        _ => return Ok(Some((false, false, false))),
    };
    let var_cls = match nodes_class(py, "Var") {
        Ok(cls) => cls,
        Err(_) => return Ok(None),
    };
    let is_var = match node.is_instance(var_cls) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if !is_var {
        return Ok(Some((false, false, false)));
    }
    let node_typed = match node.getattr("type") {
        Ok(t) => !t.is_none(),
        Err(_) => return Ok(None),
    };
    let fullname = match node.getattr("fullname") {
        Ok(f) => f,
        Err(_) => return Ok(None),
    };
    let is_callable_ref = match fullname.extract::<String>() {
        Ok(f) => f == "typing.Callable",
        Err(_) => return Ok(None),
    };
    Ok(Some((true, node_typed, is_callable_ref)))
}

/// Union pre-order walk mirroring the Python recursion; every leaf pushes
/// exactly one tag, or the whole call defers.
fn classify_class_pattern_inner(typ: &Type, callable_var: bool, tags: &mut Vec<i64>) -> Option<()> {
    // Python runs get_proper_type at each recursion level; a wire alias
    // cannot be expanded, so defer.
    if matches!(typ, Type::TypeAliasType { .. }) {
        return None;
    }
    if let Type::UnionType { items, .. } = typ {
        for item in items {
            classify_class_pattern_inner(item, callable_var, tags)?;
        }
        return Some(());
    }
    tags.push(classify_class_pattern_leaf(typ, callable_var)?);
    Some(())
}

fn classify_class_pattern_leaf(typ: &Type, callable_var: bool) -> Option<i64> {
    // Python: isinstance(p_typ, FunctionLike) and p_typ.is_type_obj().
    match typ {
        Type::CallableType {
            fallback, ret_type, ..
        } => match callable_is_type_obj(fallback, ret_type)? {
            true => Some(CPR_TYPE_OBJ),
            false if callable_var => Some(CPR_CALLABLE_VAR),
            false => Some(CPR_FAIL),
        },
        Type::Overloaded { items } => {
            // Overloaded.is_type_obj() queries only the first item.
            match items.first()? {
                Type::CallableType {
                    fallback, ret_type, ..
                } => match callable_is_type_obj(fallback, ret_type)? {
                    true => Some(CPR_TYPE_OBJ),
                    false if callable_var => Some(CPR_CALLABLE_VAR),
                    false => Some(CPR_FAIL),
                },
                _ => None,
            }
        }
        _ if callable_var => Some(CPR_CALLABLE_VAR),
        Type::TypeType { .. } => Some(CPR_TYPE_TYPE),
        Type::AnyType { .. } => Some(CPR_ANY),
        _ => Some(CPR_FAIL),
    }
}

/// `CallableType.is_type_obj()`: `fallback.type.is_metaclass()` and the
/// return type is not Uninhabited. From the wire only a `builtins.type`
/// fallback is provably a metaclass; any other fallback (a custom
/// metaclass Instance, an alias) defers. An alias ret_type also defers:
/// Python would expand it before the Uninhabited check.
fn callable_is_type_obj(fallback: &Type, ret_type: &Type) -> Option<bool> {
    if matches!(ret_type, Type::UninhabitedType { .. }) {
        return Some(false);
    }
    if matches!(ret_type, Type::TypeAliasType { .. }) {
        return None;
    }
    match fallback {
        Type::Instance { type_ref, .. } if type_ref == "builtins.type" => Some(true),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// #1633 driver-head classifiers (PatternChecker visit_* family)
// ---------------------------------------------------------------------------

// One branch-tag classifier per `visit_*` dispatch head; the Python shim
// applies every side effect and keeps the verbatim Python head as the
// fallback (`None` defers on undecodable/unresolvable/undecided input).

// Tags for `rust_classify_sequence_pattern_head`: the step 1-3 dispatch of
// `visit_sequence_pattern` (checkpattern.py:284-376). The four
// early-non-match tags all map to `early_non_match()` in the shim.
pub(crate) const SEQ_CANNOT_MATCH: i64 = 0;
pub(crate) const SEQ_UNION: i64 = 1;
pub(crate) const SEQ_FIXED_TOO_FEW: i64 = 2;
pub(crate) const SEQ_FIXED_TOO_MANY: i64 = 3;
pub(crate) const SEQ_FIXED_OK: i64 = 4;
pub(crate) const SEQ_VARIADIC_TOO_MANY: i64 = 5;
pub(crate) const SEQ_VARIADIC_OK: i64 = 6;
pub(crate) const SEQ_ANY: i64 = 7;
pub(crate) const SEQ_ITERABLE: i64 = 8;
pub(crate) const SEQ_OTHER: i64 = 9;

// Tags for the rest-type arbitration of `rust_classify_sequence_tuple_result`.
pub(crate) const SEQ_REST_ALL: i64 = 0;
pub(crate) const SEQ_REST_SINGLE: i64 = 1;
pub(crate) const SEQ_REST_KEEP: i64 = 2;

// Tags for `rust_classify_mapping_rest`: the `o.rest` branch of
// `visit_mapping_pattern` (checkpattern.py:588-600).
pub(crate) const MAP_INSTANCE: i64 = 0;
pub(crate) const MAP_DICT_FALLBACK: i64 = 1;

// Violation tags for `rust_classify_class_pattern_keywords`: the duplicate
// arbitration of `visit_class_pattern` (checkpattern.py:723-754).
pub(crate) const CLS_KW_TOO_MANY: i64 = 1;
pub(crate) const CLS_KW_MATCHES_POSITIONAL: i64 = 2;
pub(crate) const CLS_KW_DUPLICATE: i64 = 3;

/// `visit_sequence_pattern` steps 1-3 in one call (checkpattern.py:284-376).
///
/// Python: `can_match_sequence` gate, union-recurse branch, then the
/// inner-type source dispatch (fixed tuple with size checks, variadic
/// tuple with its size check, `AnyType`, iterable `Instance`, object
/// fallback). The star facts ride as scalars (`star_pos`, already `None`
/// when several starred patterns exist, plus `required_patterns`), so no
/// AST crosses the seam. The `can_match_sequence` check runs inline via
/// the shared `can_match_sequence_inner` (an undecided verdict defers the
/// whole head rather than answering `CANNOT_MATCH`). The iterable arm
/// mirrors `type_is_iterable` (checker.py:6083): the subject is already an
/// `Instance` here, so the type-object unwrap never fires and only
/// `is_subtype(t, Iterable[Any])` is needed; any other non-tuple shape
/// answers `OTHER` without consulting the resolver.
#[pyfunction]
#[pyo3(signature = (type_bytes, star_pos, required_patterns, non_seq_bytes, sequence_bytes, iterable_bytes, resolver))]
#[allow(clippy::too_many_arguments)]
pub(crate) fn rust_classify_sequence_pattern_head(
    type_bytes: &[u8],
    star_pos: Option<i64>,
    required_patterns: i64,
    non_seq_bytes: &[u8],
    sequence_bytes: &[u8],
    iterable_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<i64> {
    let star_pos = match star_pos {
        Some(p) if p < 0 => return None,
        Some(p) => Some(p as usize),
        None => None,
    };
    let required: usize = required_patterns.try_into().ok()?;
    let t = decode_type(type_bytes)?;
    let non_seq = decode_type(non_seq_bytes)?;
    let sequence = decode_type(sequence_bytes)?;
    let iterable = decode_type(iterable_bytes)?;
    // Step 1 entry: get_proper_type (checkpattern.py:288).
    let t = crate::checkexpr_functions::get_proper_or_expand(&t, resolver.alias_resolver())?;
    // Step 1 gate (checkpattern.py:289-290).
    if !can_match_sequence_inner(&t, &non_seq, &sequence, resolver)? {
        return Some(SEQ_CANNOT_MATCH);
    }
    // Step 2: union recursion (checkpattern.py:305).
    if matches!(t, Type::UnionType { .. }) {
        return Some(SEQ_UNION);
    }
    // Step 3: inner-type source dispatch (checkpattern.py:346-376).
    match &t {
        Type::TupleType { items, .. } => {
            if find_unpack_in_list_inner(items) < 0 {
                let size_diff = items.len() as i64 - required as i64;
                if size_diff < 0 {
                    Some(SEQ_FIXED_TOO_FEW)
                } else if size_diff > 0 && star_pos.is_none() {
                    Some(SEQ_FIXED_TOO_MANY)
                } else {
                    Some(SEQ_FIXED_OK)
                }
            } else if items.len() as i64 - 1 > required as i64 && star_pos.is_none() {
                Some(SEQ_VARIADIC_TOO_MANY)
            } else {
                Some(SEQ_VARIADIC_OK)
            }
        }
        Type::AnyType { .. } => Some(SEQ_ANY),
        Type::Instance { .. } => {
            let ctx = SubtypeContext::new(false, false, false, false, false, true);
            match is_subtype(&t, &iterable, &ctx, resolver.resolver()) {
                Some(true) => Some(SEQ_ITERABLE),
                Some(false) => Some(SEQ_OTHER),
                None => None,
            }
        }
        _ => Some(SEQ_OTHER),
    }
}

/// Step-5 arbitration for a fixed tuple in `visit_sequence_pattern`
/// (checkpattern.py:407-426): whether any new inner type is uninhabited,
/// and the rest-type fold over the rest inner types (all-always-match,
/// exactly-one-conditional with its index, or keep). Returns
/// `(new_uninhabited, rest_tag, rest_index, rest_uninhabited_mask)`; the
/// mask lets the shim rebuild the single-conditional tuple without
/// re-querying `is_uninhabited` per item. Every item expands a top-level
/// alias through the resolver first (`is_uninhabited_wire`); an
/// unresolvable alias defers the whole call.
#[pyfunction]
pub(crate) fn rust_classify_sequence_tuple_result(
    new_bytes: Vec<Vec<u8>>,
    rest_bytes: Vec<Vec<u8>>,
    resolver: &mut NativeTypeResolver,
) -> Option<(bool, i64, i64, Vec<bool>)> {
    let new_types = decode_type_list(&new_bytes)?;
    let rest_types = decode_type_list(&rest_bytes)?;
    let aliases = resolver.alias_resolver();
    let mut new_uninhabited = false;
    for t in &new_types {
        if is_uninhabited_wire(t, aliases)? {
            new_uninhabited = true;
            break;
        }
    }
    let mut mask = Vec::with_capacity(rest_types.len());
    let mut count: i64 = 0;
    let mut single_idx: i64 = -1;
    for (i, t) in rest_types.iter().enumerate() {
        let uninhabited = is_uninhabited_wire(t, aliases)?;
        mask.push(uninhabited);
        if uninhabited {
            count += 1;
        } else {
            single_idx = i as i64;
        }
    }
    let n = rest_types.len() as i64;
    if count == n {
        Some((new_uninhabited, SEQ_REST_ALL, -1, mask))
    } else if count == n - 1 {
        Some((new_uninhabited, SEQ_REST_SINGLE, single_idx, mask))
    } else {
        Some((new_uninhabited, SEQ_REST_KEEP, -1, mask))
    }
}

/// The `o.rest` branch of `visit_mapping_pattern` (checkpattern.py:588-598).
///
/// Python: `is_subtype(current_type, mapping) and
/// isinstance(current_type, Instance)` selects the `map_instance_to_supertype`
/// arm; anything else builds `dict[object, object]`. The subtype call uses
/// the default context; the alias at entry expands like the visit method's
/// own `get_proper_type` (checkpattern.py:574).
#[pyfunction]
pub(crate) fn rust_classify_mapping_rest(
    type_bytes: &[u8],
    mapping_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<i64> {
    let t = decode_type(type_bytes)?;
    let mapping = decode_type(mapping_bytes)?;
    let t = crate::checkexpr_functions::get_proper_or_expand(&t, resolver.alias_resolver())?;
    let ctx = SubtypeContext::new(false, false, false, false, false, true);
    let is_sub = is_subtype(&t, &mapping, &ctx, resolver.resolver())?;
    if is_sub && matches!(t, Type::Instance { .. }) {
        Some(MAP_INSTANCE)
    } else {
        Some(MAP_DICT_FALLBACK)
    }
}

/// The generic-alias gate of `visit_class_pattern` (checkpattern.py:658-661).
///
/// Python: `isinstance(type_info, TypeAlias) and not type_info.no_args`
/// reports `CLASS_PATTERN_GENERIC_TYPE_ALIAS` and takes `early_non_match`.
/// A missing/`None` node is a plain non-alias (`Some(false)`); only an
/// unreadable attribute defers (`None`), mirroring `class_ref_facts`.
#[pyfunction]
pub(crate) fn rust_classify_class_pattern_alias_gate(
    py: Python<'_>,
    class_ref_node: Option<&PyAny>,
) -> PyResult<Option<bool>> {
    let node = match class_ref_node {
        Some(n) if !n.is_none() => n,
        _ => return Ok(Some(false)),
    };
    let alias_cls = match nodes_class(py, "TypeAlias") {
        Ok(cls) => cls,
        Err(_) => return Ok(None),
    };
    let is_alias = match node.is_instance(alias_cls) {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    if !is_alias {
        return Ok(Some(false));
    }
    match node.getattr("no_args") {
        Ok(v) => match v.extract::<bool>() {
            Ok(no_args) => Ok(Some(!no_args)),
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

/// The positional-count and duplicate arbitration of `visit_class_pattern`
/// (checkpattern.py:723-754) as pure string-list logic.
///
/// The shim passes the already-resolved `match_arg_names` (the tuple arm's
/// `get_match_arg_names` result, or `[None] * num_positionals` for the
/// non-tuple arm, exactly as Python builds it), so `num_positionals >
/// names.len()` fires precisely for the tuple-arm overflow. Returns the
/// violations in emission order (`TOO_MANY` alone, else the per-key
/// `MATCHES_POSITIONAL` / `DUPLICATE` hits with the keyword index); an
/// empty vec means the shim proceeds to the keyword loop. Never defers on
/// well-formed input (`None` only on a negative count).
#[pyfunction]
pub(crate) fn rust_classify_class_pattern_keywords(
    match_arg_names: Vec<Option<String>>,
    num_positionals: i64,
    keyword_keys: Vec<String>,
) -> Option<Vec<(i64, i64)>> {
    let n: usize = num_positionals.try_into().ok()?;
    if n > match_arg_names.len() {
        return Some(vec![(CLS_KW_TOO_MANY, -1)]);
    }
    let match_arg_set: std::collections::HashSet<&str> = match_arg_names
        .iter()
        .take(n)
        .filter_map(|o| o.as_deref())
        .collect();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (i, key) in keyword_keys.iter().enumerate() {
        if match_arg_set.contains(key.as_str()) {
            out.push((CLS_KW_MATCHES_POSITIONAL, i as i64));
        } else if !seen.insert(key.as_str()) {
            out.push((CLS_KW_DUPLICATE, i as i64));
        }
    }
    Some(out)
}

/// The match-type filter tail of `visit_or_pattern` (checkpattern.py:221-224).
///
/// Python keeps `pattern_type.type for ... if not
/// is_uninhabited(pattern_type.type)` and unions the survivors with
/// `make_simplified_union` (which stays Python-side via the already-native
/// typeops seam, so no union path forks here). Returns the surviving
/// indices; the shim maps them back onto the live `PatternType`s, so no
/// decoded type crosses back. An unresolvable alias item defers the whole
/// call (`is_uninhabited_wire`), preserving the live-`get_proper_type`
/// fallback.
#[pyfunction]
pub(crate) fn rust_filter_or_match_types(
    match_bytes: Vec<Vec<u8>>,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<i64>> {
    let types = decode_type_list(&match_bytes)?;
    let aliases = resolver.alias_resolver();
    let mut out = Vec::new();
    for (i, t) in types.iter().enumerate() {
        if !is_uninhabited_wire(t, aliases)? {
            out.push(i as i64);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_resolver() -> NativeTypeResolver {
        crate::typeinfo::NativeTypeResolver::from_resolver(crate::typeinfo::TypeResolver::new())
    }

    fn alias_resolver_with_targets(targets: &[(&str, Type)]) -> crate::aliases::TypeAliasResolver {
        let mut r = crate::aliases::TypeAliasResolver::new();
        for (fullname, target) in targets {
            let mut buf = crate::wire::WriteBuffer::new();
            crate::wire::write_type(&mut buf, target).unwrap();
            r.insert(
                fullname.to_string(),
                crate::aliases::TypeAliasSnapshot {
                    fullname: fullname.to_string(),
                    target: buf.into_bytes(),
                    ..Default::default()
                },
            );
        }
        r
    }

    fn type_alias(type_ref: &str) -> Type {
        Type::TypeAliasType {
            type_ref: type_ref.to_string(),
            args: vec![],
            is_recursive: false,
        }
    }

    fn resolver_with_aliases(aliases: crate::aliases::TypeAliasResolver) -> NativeTypeResolver {
        crate::typeinfo::NativeTypeResolver::new(crate::typeinfo::TypeResolver::new(), aliases)
    }

    #[test]
    fn test_is_uninhabited_uninhabited() {
        let t = Type::UninhabitedType { ambiguous: false };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(
            rust_is_uninhabited(&bytes, &mut test_resolver()),
            Some(true)
        );
    }

    #[test]
    fn test_is_uninhabited_not_uninhabited() {
        let t = Type::NoneType;
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(
            rust_is_uninhabited(&bytes, &mut test_resolver()),
            Some(false)
        );
    }

    #[test]
    fn test_is_uninhabited_alias_to_uninhabited() {
        // A TypeAliasType resolving to UninhabitedType: Python's
        // get_proper_type turns it into UninhabitedType, so is_uninhabited
        // is True. The resolver must expand the alias.
        let aliases = alias_resolver_with_targets(&[(
            "mod.Never",
            Type::UninhabitedType { ambiguous: false },
        )]);
        let bytes = make_alias_blob("mod.Never");
        assert_eq!(
            rust_is_uninhabited(&bytes, &mut resolver_with_aliases(aliases)),
            Some(true)
        );
    }

    #[test]
    fn test_is_uninhabited_alias_missing_snapshot_defers() {
        // Missing resolver snapshot: the alias cannot expand, so the seam
        // defers (None) and Python falls back to get_proper_type.
        let bytes = make_alias_blob("mod.Never");
        assert_eq!(rust_is_uninhabited(&bytes, &mut test_resolver()), None);
    }

    #[test]
    fn test_is_uninhabited_alias_resolves_to_non_uninhabited() {
        // Alias resolving to a plain Instance: not uninhabited, Some(false).
        let aliases = alias_resolver_with_targets(&[("mod.A", instance("builtins.int", vec![]))]);
        let bytes = make_alias_blob("mod.A");
        assert_eq!(
            rust_is_uninhabited(&bytes, &mut resolver_with_aliases(aliases)),
            Some(false)
        );
    }

    fn make_alias_blob(type_ref: &str) -> Vec<u8> {
        let mut b = crate::wire::WriteBuffer::new();
        crate::wire::write_tag(&mut b, crate::wire::TYPE_ALIAS_TYPE);
        crate::wire::write_type_list(&mut b, &[]).expect("empty args encode");
        crate::wire::write_str(&mut b, type_ref).expect("ref encodes");
        crate::wire::write_tag(&mut b, crate::wire::END_TAG);
        b.into_bytes()
    }

    #[test]
    fn test_get_type_range_bool_lkv() {
        // Instance with bool LKV should return Some(true).
        let lkv = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.bool".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Bool(true),
        };
        let t = Type::Instance {
            type_ref: "builtins.bool".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv)),
            extra_attrs: None,
        };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_get_type_range(&bytes), Some(true));
    }

    #[test]
    fn test_get_type_range_no_lkv() {
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: None,
        };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_get_type_range(&bytes), Some(false));
    }

    #[test]
    fn test_get_type_range_int_lkv_not_bool() {
        // Instance with int LKV should return Some(false) (not a bool).
        let lkv = Type::LiteralType {
            fallback: Box::new(Type::Instance {
                type_ref: "builtins.int".to_string(),
                args: vec![],
                last_known_value: None,
                extra_attrs: None,
            }),
            value: LiteralValue::Int(42),
        };
        let t = Type::Instance {
            type_ref: "builtins.int".to_string(),
            args: vec![],
            last_known_value: Some(Box::new(lkv)),
            extra_attrs: None,
        };
        let mut buf = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut buf, &t).unwrap();
        let bytes = buf.into_bytes();
        assert_eq!(rust_get_type_range(&bytes), Some(false));
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn unpack(item: Type) -> Type {
        Type::UnpackType {
            typ: Box::new(instance("builtins.tuple", vec![item])),
            from_star_syntax: false,
        }
    }

    fn blobs(types: &[Type]) -> Vec<Vec<u8>> {
        types
            .iter()
            .map(|t| {
                let mut b = crate::wire::WriteBuffer::new();
                crate::wire::write_type(&mut b, t).unwrap();
                b.into_bytes()
            })
            .collect()
    }

    fn decode_one(blob: &[u8]) -> Type {
        let mut buf = crate::wire::ReadBuffer::new(blob);
        crate::wire::read_type(&mut buf, None).unwrap()
    }

    #[test]
    fn test_expand_star_pos_none_returns_unchanged() {
        let types = vec![
            instance("builtins.int", vec![]),
            instance("builtins.str", vec![]),
        ];
        let input = blobs(&types);
        let res =
            rust_expand_starred_pattern_types(input.clone(), None, 0, false, &mut test_resolver());
        assert!(res.is_some());
        // star_pos == None returns the input bytes verbatim, no decode.
        assert_eq!(res, Some(input));
    }

    #[test]
    fn test_expand_original_unpack_rewraps_star_item() {
        // original_unpack rewraps the star item as UnpackType[tuple[t]]
        // (checkpattern.py:527-533).
        let types = vec![
            instance("builtins.int", vec![]),
            instance("builtins.str", vec![]),
            instance("builtins.bool", vec![]),
        ];
        let res = rust_expand_starred_pattern_types(
            blobs(&types),
            Some(1),
            3,
            true,
            &mut test_resolver(),
        )
        .unwrap();
        assert_eq!(res.len(), 3);
        let star = decode_one(&res[1]);
        assert_eq!(
            star,
            Type::UnpackType {
                typ: Box::new(instance(
                    "builtins.tuple",
                    vec![instance("builtins.str", vec![])]
                )),
                from_star_syntax: false,
            }
        );
        // Non-star items pass through untouched.
        assert_eq!(decode_one(&res[0]), types[0]);
        assert_eq!(decode_one(&res[2]), types[2]);
    }

    #[test]
    fn test_expand_original_unpack_keeps_uninhabited_star_item() {
        // An uninhabited star item is not re-wrapped (is_uninhabited guard
        // in checkpattern.py:529).
        let types = vec![
            instance("builtins.int", vec![]),
            Type::UninhabitedType { ambiguous: false },
            instance("builtins.bool", vec![]),
        ];
        let res = rust_expand_starred_pattern_types(
            blobs(&types),
            Some(1),
            3,
            true,
            &mut test_resolver(),
        )
        .unwrap();
        assert_eq!(decode_one(&res[1]), types[1]);
    }

    #[test]
    fn test_expand_no_unpack_duplicates_star_item() {
        // star_length = num_types - len(types) + 1 (checkpattern.py:535-537).
        let types = vec![
            instance("builtins.int", vec![]),
            instance("builtins.str", vec![]),
        ];
        let res = rust_expand_starred_pattern_types(
            blobs(&types),
            Some(1),
            3,
            false,
            &mut test_resolver(),
        )
        .unwrap();
        assert_eq!(res.len(), 3);
        let expected = vec![
            instance("builtins.int", vec![]),
            instance("builtins.str", vec![]),
            instance("builtins.str", vec![]),
        ];
        let got: Vec<Type> = res.iter().map(|b| decode_one(b)).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_contract_with_unpack_no_star_broadens() {
        // unpack split with star_pos=None: missing = num_patterns -
        // len(types) + 1; the unpack item is duplicated to fill the slot
        // (checkpattern.py:484-489).
        let types = vec![
            instance("builtins.int", vec![]),
            unpack(instance("builtins.bool", vec![])),
            instance("builtins.str", vec![]),
        ];
        let res = contract_with_unpack(types, 1, None, 4, None, None).unwrap();
        let expected = vec![
            instance("builtins.int", vec![]),
            instance("builtins.bool", vec![]),
            instance("builtins.bool", vec![]),
            instance("builtins.str", vec![]),
        ];
        let got: Vec<Type> = res.iter().map(|b| decode_one(b)).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_contract_no_unpack_no_star_returns_unchanged() {
        let types = vec![
            instance("builtins.int", vec![]),
            instance("builtins.str", vec![]),
            instance("builtins.bool", vec![]),
        ];
        let res = contract_no_unpack(types.clone(), None, 4, None).unwrap();
        let got: Vec<Type> = res.iter().map(|b| decode_one(b)).collect();
        assert_eq!(got, types);
    }

    #[test]
    fn test_contract_star_branches_need_resolver() {
        // The union-simplification star branches require a resolver; without
        // one they defer (return None) rather than guess.
        let with_unpack = vec![
            instance("builtins.int", vec![]),
            unpack(instance("builtins.bool", vec![])),
            instance("builtins.str", vec![]),
        ];
        assert_eq!(
            contract_with_unpack(with_unpack, 1, Some(1), 4, None, None),
            None
        );
        let no_unpack = vec![
            instance("builtins.int", vec![]),
            instance("builtins.str", vec![]),
            instance("builtins.bool", vec![]),
            instance("builtins.bytes", vec![]),
        ];
        assert_eq!(contract_no_unpack(no_unpack, Some(1), 2, None), None);
    }

    #[test]
    fn test_construct_sequence_child_any_passthrough() {
        // AnyType outer passes through unchanged (checkpattern.py:923-924).
        let any = Type::AnyType {
            type_of_any: 1,
            source_any: None,
            missing_import_name: None,
        };
        let empty = instance(
            "builtins.list",
            vec![Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }],
        );
        let seq = instance("typing.Sequence", vec![instance("builtins.str", vec![])]);
        let res = construct_sequence_child_inner(&any, &empty, &seq, None).unwrap();
        assert_eq!(decode_one(&res), any);
    }

    #[test]
    fn test_construct_sequence_child_defers_non_instance() {
        // TypeVarType / UnionType / TypeAliasType outer defer to Python.
        let empty = instance(
            "builtins.list",
            vec![Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }],
        );
        let seq = instance("typing.Sequence", vec![instance("builtins.str", vec![])]);
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "T".to_string(),
            raw_id: 1,
            namespace: "m".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(Type::AnyType {
                type_of_any: 6,
                source_any: None,
                missing_import_name: None,
            }),
            variance: 0,
            meta_level: 0,
        };
        assert_eq!(
            construct_sequence_child_inner(&tvar, &empty, &seq, None),
            None
        );
        // Instance outer needs a resolver for the is_subtype check.
        let outer = instance("builtins.list", vec![instance("builtins.int", vec![])]);
        assert_eq!(
            construct_sequence_child_inner(&outer, &empty, &seq, None),
            None
        );
    }

    #[test]
    fn test_contract_with_unpack_alias_unpack_inner() {
        // star_pos=None broadens when the unpack inner is a TypeAliasType
        // resolving to builtins.tuple[bool]: the alias resolver expands it
        // (contract_with_unpack: Python unpacked = get_proper_type(u.type)).
        let aliases = alias_resolver_with_targets(&[(
            "mod.Tup",
            instance("builtins.tuple", vec![instance("builtins.bool", vec![])]),
        )]);
        let types = vec![
            instance("builtins.int", vec![]),
            Type::UnpackType {
                typ: Box::new(type_alias("mod.Tup")),
                from_star_syntax: false,
            },
            instance("builtins.str", vec![]),
        ];
        let res = contract_with_unpack(types, 1, None, 4, None, Some(&aliases)).unwrap();
        let expected = vec![
            instance("builtins.int", vec![]),
            instance("builtins.bool", vec![]),
            instance("builtins.bool", vec![]),
            instance("builtins.str", vec![]),
        ];
        let got: Vec<Type> = res.iter().map(|b| decode_one(b)).collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_contract_with_unpack_alias_unpack_missing_snapshot_defers() {
        // Missing alias snapshot: the resolver cannot expand the unpack
        // inner, so the seam defers (None) rather than guess.
        let types = vec![
            instance("builtins.int", vec![]),
            Type::UnpackType {
                typ: Box::new(type_alias("mod.Tup")),
                from_star_syntax: false,
            },
            instance("builtins.str", vec![]),
        ];
        assert_eq!(contract_with_unpack(types, 1, None, 4, None, None), None);
    }

    #[test]
    fn test_expand_original_unpack_star_alias_inhabited_rewraps() {
        // original_unpack with an inhabited alias star item: Python wraps the
        // ORIGINAL alias (not the expanded type) in UnpackType[tuple[t]]
        // (checkpattern.py:527-533). The blanket alias rejection is gone.
        let aliases = alias_resolver_with_targets(&[("mod.Str", instance("builtins.str", vec![]))]);
        let types = vec![
            instance("builtins.int", vec![]),
            type_alias("mod.Str"),
            instance("builtins.bool", vec![]),
        ];
        let res = rust_expand_starred_pattern_types(
            blobs(&types),
            Some(1),
            3,
            true,
            &mut resolver_with_aliases(aliases),
        )
        .unwrap();
        assert_eq!(res.len(), 3);
        let star = decode_one(&res[1]);
        assert_eq!(
            star,
            Type::UnpackType {
                typ: Box::new(instance("builtins.tuple", vec![type_alias("mod.Str")])),
                from_star_syntax: false,
            }
        );
        assert_eq!(decode_one(&res[0]), types[0]);
        assert_eq!(decode_one(&res[2]), types[2]);
    }

    #[test]
    fn test_expand_original_unpack_star_alias_uninhabited_keeps_alias() {
        // original_unpack with an uninhabited alias star item: is_uninhabited
        // resolves it, so the star item is kept untouched (not re-wrapped).
        let aliases = alias_resolver_with_targets(&[(
            "mod.Never",
            Type::UninhabitedType { ambiguous: false },
        )]);
        let types = vec![
            instance("builtins.int", vec![]),
            type_alias("mod.Never"),
            instance("builtins.bool", vec![]),
        ];
        let res = rust_expand_starred_pattern_types(
            blobs(&types),
            Some(1),
            3,
            true,
            &mut resolver_with_aliases(aliases),
        )
        .unwrap();
        assert_eq!(decode_one(&res[1]), types[1]);
    }

    #[test]
    fn test_expand_original_unpack_star_alias_missing_snapshot_defers() {
        // Missing snapshot for the star alias: is_uninhabited cannot be
        // decided, so the whole call defers to Python.
        let types = vec![
            instance("builtins.int", vec![]),
            type_alias("mod.Str"),
            instance("builtins.bool", vec![]),
        ];
        assert_eq!(
            rust_expand_starred_pattern_types(
                blobs(&types),
                Some(1),
                3,
                true,
                &mut test_resolver(),
            ),
            None
        );
    }

    #[test]
    fn test_expand_no_unpack_allows_non_star_alias() {
        // Non-original-unpack branch: a non-star alias passes through the
        // duplication unchanged instead of triggering the old blanket reject.
        let types = vec![
            type_alias("mod.A"),
            instance("builtins.int", vec![]),
            instance("builtins.bool", vec![]),
        ];
        let res = rust_expand_starred_pattern_types(
            blobs(&types),
            Some(1),
            4,
            false,
            &mut test_resolver(),
        )
        .unwrap();
        let expected = vec![
            type_alias("mod.A"),
            instance("builtins.int", vec![]),
            instance("builtins.int", vec![]),
            instance("builtins.bool", vec![]),
        ];
        let got: Vec<Type> = res.iter().map(|b| decode_one(b)).collect();
        assert_eq!(got, expected);
    }
    // ----- rust_classify_class_pattern_ranges pure decision tests -----

    fn cpr_callable(fallback: &str, ret: Type) -> Type {
        Type::CallableType {
            fallback: Box::new(instance(fallback, vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: vec![],
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(ret),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
            special_sig: None,
            definition_ref: None,
        }
    }

    #[expect(dead_code)]
    fn cpr_overloaded(first: Type) -> Type {
        Type::Overloaded { items: vec![first] }
    }

    fn cpr_union(items: Vec<Type>) -> Type {
        Type::UnionType {
            items,
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        }
    }

    #[test]
    fn test_cpr_leaf_type_obj() {
        let t = cpr_callable("builtins.type", instance("builtins.int", vec![]));
        assert_eq!(classify_class_pattern_leaf(&t, false), Some(CPR_TYPE_OBJ));
    }

    #[test]
    fn test_cpr_leaf_type_obj_uninhabited_ret_is_not_type_obj() {
        let t = cpr_callable("builtins.type", Type::UninhabitedType { ambiguous: false });
        assert_eq!(classify_class_pattern_leaf(&t, false), Some(CPR_FAIL));
    }

    #[test]
    fn test_cpr_leaf_callable_var_arm() {
        // is_type_obj is False when the ret type is Uninhabited, so the
        // scalar class-ref arm decides: CALLABLE_VAR or FAIL.
        let t = cpr_callable(
            "builtins.function",
            Type::UninhabitedType { ambiguous: false },
        );
        assert_eq!(
            classify_class_pattern_leaf(&t, true),
            Some(CPR_CALLABLE_VAR)
        );
        assert_eq!(classify_class_pattern_leaf(&t, false), Some(CPR_FAIL));
    }

    #[test]
    fn test_cpr_leaf_function_fallback_defers() {
        // fallback.type.is_metaclass() needs the live TypeInfo, so a
        // function-fallback callable cannot be decided on the wire.
        let t = cpr_callable("builtins.function", instance("builtins.int", vec![]));
        assert_eq!(classify_class_pattern_leaf(&t, true), None);
        assert_eq!(classify_class_pattern_leaf(&t, false), None);
    }

    #[test]
    fn test_cpr_leaf_custom_metaclass_fallback_defers() {
        // fallback.type.is_metaclass() needs the live TypeInfo: defer.
        let t = cpr_callable("mymeta.Meta", instance("builtins.int", vec![]));
        assert_eq!(classify_class_pattern_leaf(&t, false), None);
    }

    #[test]
    fn test_cpr_leaf_overloaded_first_item_decides() {
        let t = Type::Overloaded {
            items: vec![
                cpr_callable("builtins.type", instance("builtins.int", vec![])),
                cpr_callable("builtins.function", instance("builtins.int", vec![])),
            ],
        };
        assert_eq!(classify_class_pattern_leaf(&t, false), Some(CPR_TYPE_OBJ));
    }

    #[test]
    fn test_cpr_leaf_scalar_arms() {
        assert_eq!(
            classify_class_pattern_leaf(&instance("builtins.int", vec![]), false),
            Some(CPR_FAIL)
        );
        assert_eq!(
            classify_class_pattern_leaf(&instance("builtins.int", vec![]), true),
            Some(CPR_CALLABLE_VAR)
        );
        let tt = Type::TypeType {
            item: Box::new(instance("builtins.int", vec![])),
            is_type_form: false,
        };
        assert_eq!(classify_class_pattern_leaf(&tt, false), Some(CPR_TYPE_TYPE));
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(classify_class_pattern_leaf(&any, false), Some(CPR_ANY));
    }

    #[test]
    fn test_cpr_inner_union_preorder_and_alias_defer() {
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let tt = Type::TypeType {
            item: Box::new(instance("builtins.int", vec![])),
            is_type_form: false,
        };
        let union = cpr_union(vec![any.clone(), cpr_union(vec![tt, any.clone()])]);
        let mut tags = Vec::new();
        assert!(classify_class_pattern_inner(&union, false, &mut tags).is_some());
        assert_eq!(tags, vec![CPR_ANY, CPR_TYPE_TYPE, CPR_ANY]);
        // A nested alias defers the whole call.
        let union_alias = cpr_union(vec![any.clone(), type_alias("mod.A")]);
        let mut tags2 = Vec::new();
        assert!(classify_class_pattern_inner(&union_alias, false, &mut tags2).is_none());
    }

    #[test]
    fn test_cpr_alias_ret_type_defers() {
        let t = cpr_callable("builtins.type", type_alias("mod.A"));
        assert_eq!(classify_class_pattern_leaf(&t, false), None);
    }

    // ----- #1633 driver-head classifier tests -----

    fn seq_head(
        t: &Type,
        star_pos: Option<i64>,
        required: i64,
        resolver: &mut NativeTypeResolver,
    ) -> Option<i64> {
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let mut b = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut b, t).unwrap();
        let t_bytes = b.into_bytes();
        let mut nb = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut nb, &any).unwrap();
        let any_bytes = nb.into_bytes();
        rust_classify_sequence_pattern_head(
            &t_bytes, star_pos, required, &any_bytes, &any_bytes, &any_bytes, resolver,
        )
    }

    fn fixed_tuple(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance(
                "builtins.tuple",
                vec![instance("builtins.int", vec![])],
            )),
            items,
            implicit: false,
        }
    }

    #[test]
    fn test_seq_head_any_subject() {
        // AnyType short-circuits can_match_sequence without the resolver,
        // so the ANY tag decides with an empty snapshot.
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(seq_head(&any, None, 1, &mut test_resolver()), Some(SEQ_ANY));
    }

    #[test]
    fn test_seq_head_negative_star_defers() {
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(seq_head(&any, Some(-1), 1, &mut test_resolver()), None);
    }

    #[test]
    fn test_seq_head_negative_required_defers() {
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        assert_eq!(seq_head(&any, None, -2, &mut test_resolver()), None);
    }

    #[test]
    fn test_seq_head_garbage_bytes_defers() {
        let mut r = test_resolver();
        let any = vec![0u8; 4];
        assert_eq!(
            rust_classify_sequence_pattern_head(&[255u8; 8], None, 1, &any, &any, &any, &mut r),
            None
        );
    }

    #[test]
    fn test_seq_head_alias_subject_missing_snapshot_defers() {
        // Step-1 get_proper_type cannot expand without a snapshot.
        assert_eq!(
            seq_head(&type_alias("mod.Seq"), None, 1, &mut test_resolver()),
            None
        );
    }

    #[test]
    fn test_seq_head_tuple_needs_subtype_snapshot() {
        // A fixed tuple reaches can_match_sequence first, which needs
        // subtype snapshots; with an empty resolver the head defers.
        let t = fixed_tuple(vec![instance("builtins.int", vec![])]);
        let str_t = instance("builtins.str", vec![]);
        let seq = instance(
            "typing.Sequence",
            vec![Type::AnyType {
                type_of_any: 0,
                source_any: None,
                missing_import_name: None,
            }],
        );
        let mut b = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut b, &t).unwrap();
        let t_bytes = b.into_bytes();
        let mut nb = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut nb, &str_t).unwrap();
        let non_seq_bytes = nb.into_bytes();
        let mut sb = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut sb, &seq).unwrap();
        let seq_bytes = sb.into_bytes();
        assert_eq!(
            rust_classify_sequence_pattern_head(
                &t_bytes,
                None,
                1,
                &non_seq_bytes,
                &seq_bytes,
                &seq_bytes,
                &mut test_resolver(),
            ),
            None
        );
    }

    #[test]
    fn test_seq_tuple_result_fixed_arms() {
        let int = instance("builtins.int", vec![]);
        let str_t = instance("builtins.str", vec![]);
        let never = Type::UninhabitedType { ambiguous: false };
        // New has an uninhabited item; rest all always-match -> ALL.
        let (new_uninh, tag, idx, mask) = rust_classify_sequence_tuple_result(
            blobs(&[never.clone(), str_t.clone()]),
            blobs(&[never.clone(), never.clone()]),
            &mut test_resolver(),
        )
        .unwrap();
        assert!(new_uninh);
        assert_eq!((tag, idx), (SEQ_REST_ALL, -1));
        assert_eq!(mask, vec![true, true]);
        // Exactly one conditional -> SINGLE with its index.
        let (new_uninh, tag, idx, mask) = rust_classify_sequence_tuple_result(
            blobs(&[int.clone(), str_t.clone()]),
            blobs(&[never.clone(), str_t.clone()]),
            &mut test_resolver(),
        )
        .unwrap();
        assert!(!new_uninh);
        assert_eq!((tag, idx), (SEQ_REST_SINGLE, 1));
        assert_eq!(mask, vec![true, false]);
        // Two conditionals -> KEEP.
        let (_, tag, idx, _) = rust_classify_sequence_tuple_result(
            blobs(&[int.clone(), str_t.clone()]),
            blobs(&[int.clone(), str_t.clone()]),
            &mut test_resolver(),
        )
        .unwrap();
        assert_eq!((tag, idx), (SEQ_REST_KEEP, -1));
    }

    #[test]
    fn test_seq_tuple_result_empty_rest_is_all() {
        // Mirrors Python: 0 == len([]) takes the Uninhabited arm.
        let int = instance("builtins.int", vec![]);
        let (_, tag, _, mask) =
            rust_classify_sequence_tuple_result(blobs(&[int]), vec![], &mut test_resolver())
                .unwrap();
        assert_eq!(tag, SEQ_REST_ALL);
        assert!(mask.is_empty());
    }

    #[test]
    fn test_seq_tuple_result_alias_item_defers_without_snapshot() {
        let int = instance("builtins.int", vec![]);
        assert_eq!(
            rust_classify_sequence_tuple_result(
                blobs(&[int.clone()]),
                blobs(&[type_alias("mod.A")]),
                &mut test_resolver(),
            ),
            None
        );
    }

    #[test]
    fn test_seq_tuple_result_alias_item_decides_with_snapshot() {
        let aliases = alias_resolver_with_targets(&[("mod.A", instance("builtins.int", vec![]))]);
        let int = instance("builtins.int", vec![]);
        let (new_uninh, tag, idx, mask) = rust_classify_sequence_tuple_result(
            blobs(&[int.clone()]),
            blobs(&[type_alias("mod.A")]),
            &mut resolver_with_aliases(aliases),
        )
        .unwrap();
        assert!(!new_uninh);
        // One inhabited rest item: exactly-one-conditional -> SINGLE.
        assert_eq!((tag, idx), (SEQ_REST_SINGLE, 0));
        assert_eq!(mask, vec![false]);
    }

    #[test]
    fn test_mapping_rest_garbage_defers() {
        let mut r = test_resolver();
        assert_eq!(
            rust_classify_mapping_rest(&[255u8; 8], &[0u8; 4], &mut r),
            None
        );
    }

    #[test]
    fn test_mapping_rest_alias_subject_defers_without_snapshot() {
        let any = Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        };
        let mut b = crate::wire::WriteBuffer::new();
        crate::wire::write_type(&mut b, &any).unwrap();
        let mapping_bytes = b.into_bytes();
        assert_eq!(
            rust_classify_mapping_rest(
                &make_alias_blob("mod.M"),
                &mapping_bytes,
                &mut test_resolver()
            ),
            None
        );
    }

    #[test]
    fn test_class_keywords_too_many() {
        assert_eq!(
            rust_classify_class_pattern_keywords(
                vec![Some("x".to_string())],
                2,
                vec!["a".to_string()]
            ),
            Some(vec![(CLS_KW_TOO_MANY, -1)])
        );
    }

    #[test]
    fn test_class_keywords_positional_then_duplicates() {
        // "y" collides with a positional name, "a" repeats: both reported
        // in key order, mirroring the Python loop (no early break).
        assert_eq!(
            rust_classify_class_pattern_keywords(
                vec![Some("x".to_string()), None],
                2,
                vec!["y".to_string(), "a".to_string(), "a".to_string()],
            ),
            Some(vec![(CLS_KW_DUPLICATE, 2)])
        );
        assert_eq!(
            rust_classify_class_pattern_keywords(
                vec![Some("x".to_string()), Some("y".to_string())],
                2,
                vec!["y".to_string(), "a".to_string(), "a".to_string()],
            ),
            Some(vec![(CLS_KW_MATCHES_POSITIONAL, 0), (CLS_KW_DUPLICATE, 2)])
        );
    }

    #[test]
    fn test_class_keywords_none_names_never_collide() {
        // Non-tuple arm passes [None]*n: no positional collision possible.
        assert_eq!(
            rust_classify_class_pattern_keywords(
                vec![None, None],
                2,
                vec!["x".to_string(), "x".to_string()],
            ),
            Some(vec![(CLS_KW_DUPLICATE, 1)])
        );
    }

    #[test]
    fn test_class_keywords_ok_is_empty() {
        assert_eq!(
            rust_classify_class_pattern_keywords(
                vec![Some("x".to_string())],
                1,
                vec!["a".to_string(), "b".to_string()],
            ),
            Some(vec![])
        );
    }

    #[test]
    fn test_class_keywords_negative_count_defers() {
        assert_eq!(
            rust_classify_class_pattern_keywords(vec![], -1, vec![]),
            None
        );
    }

    #[test]
    fn test_or_filter_keeps_inhabited_indices() {
        let int = instance("builtins.int", vec![]);
        let never = Type::UninhabitedType { ambiguous: false };
        assert_eq!(
            rust_filter_or_match_types(blobs(&[int.clone(), never, int]), &mut test_resolver(),),
            Some(vec![0, 2])
        );
    }

    #[test]
    fn test_or_filter_all_uninhabited_is_empty() {
        let never = Type::UninhabitedType { ambiguous: false };
        assert_eq!(
            rust_filter_or_match_types(blobs(&[never]), &mut test_resolver(),),
            Some(vec![])
        );
    }

    #[test]
    fn test_or_filter_alias_item_defers_without_snapshot() {
        let int = instance("builtins.int", vec![]);
        assert_eq!(
            rust_filter_or_match_types(blobs(&[int, type_alias("mod.A")]), &mut test_resolver(),),
            None
        );
    }

    #[test]
    fn test_or_filter_alias_item_decides_with_snapshot() {
        let aliases = alias_resolver_with_targets(&[("mod.A", instance("builtins.int", vec![]))]);
        let int = instance("builtins.int", vec![]);
        assert_eq!(
            rust_filter_or_match_types(
                blobs(&[int, type_alias("mod.A")]),
                &mut resolver_with_aliases(aliases),
            ),
            Some(vec![0, 1])
        );
    }
}

/// Register this module's Python-facing seam surface (#1677).
pub(crate) fn register_registry(m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rust_is_uninhabited, m)?)?;

    m.add_function(wrap_pyfunction!(rust_get_match_arg_names, m)?)?;

    m.add_function(wrap_pyfunction!(rust_get_type_range, m)?)?;

    m.add_function(wrap_pyfunction!(rust_should_self_match, m)?)?;

    m.add_function(wrap_pyfunction!(rust_can_match_sequence, m)?)?;

    m.add_function(wrap_pyfunction!(rust_contract_starred_pattern_types, m)?)?;

    m.add_function(wrap_pyfunction!(rust_expand_starred_pattern_types, m)?)?;

    m.add_function(wrap_pyfunction!(rust_construct_sequence_child, m)?)?;

    m.add_function(wrap_pyfunction!(rust_classify_class_pattern_ranges, m)?)?;

    m.add_function(wrap_pyfunction!(rust_classify_sequence_pattern_head, m)?)?;

    m.add_function(wrap_pyfunction!(rust_classify_sequence_tuple_result, m)?)?;

    m.add_function(wrap_pyfunction!(rust_classify_mapping_rest, m)?)?;

    m.add_function(wrap_pyfunction!(rust_classify_class_pattern_alias_gate, m)?)?;

    m.add_function(wrap_pyfunction!(rust_classify_class_pattern_keywords, m)?)?;

    m.add_function(wrap_pyfunction!(rust_filter_or_match_types, m)?)?;
    Ok(())
}
