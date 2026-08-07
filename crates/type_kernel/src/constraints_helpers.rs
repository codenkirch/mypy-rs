//! Pure constraint-list helper predicates (constraints_helpers.rs), Issue #259.
//!
//! Ports the self-contained, decision-only helpers from
//! `mypy.constraints` (constraints.py:554-732) that require neither
//! `UnionType` construction nor live-type callbacks:
//!
//!   * `select_trivial` — keep options whose every target is `Any`
//!     (returns the selected option *indices* so the Python shim can
//!     return the original `Constraint` objects, preserving identity that
//!     the solver relies on).
//!   * `exclude_non_meta_vars` — drop constraints whose type var is not a
//!     metavariable (returns the kept constraint *indices*).
//!   * `is_similar_constraints` — structural comparison that ignores
//!     targets (and direction against `Any`).
//!
//! `merge_with_any`, `any_constraints`, `filter_satisfiable`, and the
//! `is_same_constraint(s)` family stay in Python: the former three need
//! `UnionType` construction / full `is_subtype`, and the `is_same` family
//! depends on a complete `is_same_type` that the current Rust subtype path
//! does not provide (see #259).
//!
//! Anything that would require `get_proper_type` expansion (a
//! `TypeAliasType` target) or a `meta_level` the wire format does not
//! carry (`ParamSpecType` / `TypeVarTupleType` origins) defers (`None`),
//! letting the shim fall back to the Python implementation (mirroring
//! `read_type_to_str`).

use pyo3::prelude::*;

use crate::wire::{read_type, ReadBuffer, Type, WireError, WriteBuffer};

/// `select_trivial` (constraints.py:554-562): keep only options whose
/// every constraint is against `Any`, returning the selected option
/// indices as bare ints.
///
/// Wire layout in: count (bare int) + N option blobs, each blob = count
/// (bare int) + M× [origin Type | op int | target Type]. Wire layout out:
/// count (bare int) + N× index (bare int). Defers (`None`) on any
/// `TypeAliasType` target, because `get_proper_type` cannot expand it on
/// the wire.
#[pyfunction]
pub(crate) fn rust_select_trivial(options_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut input = ReadBuffer::new(options_bytes);
    let n = read_size(&mut input)?;
    let mut blobs = Vec::with_capacity(n as usize);
    for _ in 0..n {
        blobs.push(read_option(&mut input)?);
    }
    let mut selected = Vec::with_capacity(blobs.len());
    for (i, option) in blobs.iter().enumerate() {
        if all_any_targets(option)? {
            selected.push(i as i64);
        }
    }
    let mut output = WriteBuffer::new();
    write_size(&mut output, selected.len() as i64).ok()?;
    for index in &selected {
        crate::wire::write_int_bare(&mut output, *index).ok()?;
    }
    Some(output.into_bytes())
}

/// `exclude_non_meta_vars` (constraints.py:673-679): drop constraints
/// whose type var is not a metavariable, returning the kept constraint
/// indices as bare ints.
///
/// Wire layout in: count (bare int) + N× [origin Type | op int | target
/// Type]. Wire layout out: count (bare int) + N× index (bare int). An
/// empty source list round-trips as an empty index list (the Python shim
/// keeps it intact). Defers (`None`) when any origin is a
/// `ParamSpecType`/`TypeVarTupleType` (no `meta_level` on the wire) or a
/// `TypeAliasType`.
#[pyfunction]
pub(crate) fn rust_exclude_non_meta_vars(option_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut input = ReadBuffer::new(option_bytes);
    let n = read_size(&mut input)?;
    let mut kept = Vec::with_capacity(n as usize);
    for i in 0..n {
        let constraint = read_constraint(&mut input)?;
        if origin_is_meta_var(&constraint.origin)? {
            kept.push(i);
        }
    }
    let mut output = WriteBuffer::new();
    write_size(&mut output, kept.len() as i64).ok()?;
    for index in &kept {
        crate::wire::write_int_bare(&mut output, *index).ok()?;
    }
    Some(output.into_bytes())
}

/// `is_similar_constraints` (constraints.py:704-711): checks that both
/// lists have the same type-var/direction pairs, ignoring targets, and
/// ignoring direction when either target is `Any`.
#[pyfunction]
pub(crate) fn rust_is_similar_constraints(x_bytes: &[u8], y_bytes: &[u8]) -> Option<bool> {
    let x = read_constraint_list(x_bytes)?;
    let y = read_constraint_list(y_bytes)?;
    if is_similar_inner(&x, &y)? && is_similar_inner(&y, &x)? {
        Some(true)
    } else {
        Some(false)
    }
}

/// Each constraint on the wire is `origin Type | op int | target Type`.
#[derive(Clone)]
struct ConstraintParts {
    origin: Type,
    op: i64,
    target: Type,
}

fn read_constraint(buf: &mut ReadBuffer<'_>) -> Option<ConstraintParts> {
    let origin = read_type(buf, None).ok()?;
    let op = crate::wire::read_int(buf).ok()?;
    let target = read_type(buf, None).ok()?;
    Some(ConstraintParts { origin, op, target })
}

#[cfg(test)]
use crate::wire::write_type;

#[cfg(test)]
fn write_constraint(buf: &mut WriteBuffer, constraint: &ConstraintParts) -> Result<(), WireError> {
    write_type(buf, &constraint.origin)?;
    crate::wire::write_int(buf, constraint.op)?;
    write_type(buf, &constraint.target)
}

fn read_option(buf: &mut ReadBuffer<'_>) -> Option<Vec<ConstraintParts>> {
    read_constraint_list_inner(buf)
}

fn read_constraint_list(bytes: &[u8]) -> Option<Vec<ConstraintParts>> {
    let mut buf = ReadBuffer::new(bytes);
    read_constraint_list_inner(&mut buf)
}

fn read_constraint_list_inner(buf: &mut ReadBuffer<'_>) -> Option<Vec<ConstraintParts>> {
    let n = read_size(buf)?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        out.push(read_constraint(buf)?);
    }
    Some(out)
}

/// `all(isinstance(get_proper_type(c.target), AnyType))` (constraints.py:560).
fn all_any_targets(constraints: &[ConstraintParts]) -> Option<bool> {
    for constraint in constraints {
        if !is_any_target_opt(constraint)? {
            return Some(false);
        }
    }
    Some(true)
}

/// `isinstance(get_proper_type(c.target), AnyType)`, with `TypeAliasType`
/// targets deferred because `get_proper_type` cannot expand them on the
/// wire.
fn is_any_target_opt(constraint: &ConstraintParts) -> Option<bool> {
    match &constraint.target {
        Type::AnyType { .. } => Some(true),
        Type::TypeAliasType { .. } => None, // cannot expand on the wire
        _ => Some(false),
    }
}

/// `is_similar_constraints`'s one direction (constraints.py:714-731): every
/// constraint in `x` has a "similar" one in `y`. Similarity is equal
/// type-var identity plus equal direction, unless either target is `Any`,
/// in which case direction is ignored. `TypeAliasType` targets and
/// `ParamSpec`/`TypeVarTuple` origins (no `meta_level`) make the check
/// undecidable, so it defers.
fn is_similar_inner(x: &[ConstraintParts], y: &[ConstraintParts]) -> Option<bool> {
    'outer: for c1 in x {
        for c2 in y {
            if !typevar_ids_equal(&c1.origin, &c2.origin)? {
                continue;
            }
            let skip_op = is_any_target_opt(c1)? || is_any_target_opt(c2)?;
            if skip_op || c1.op == c2.op {
                continue 'outer;
            }
        }
        return Some(false);
    }
    Some(true)
}

/// `c.type_var == c2.type_var` — `TypeVarId.__eq__` is
/// `(raw_id, meta_level, namespace)`. Mirrors the wire identity fields.
/// Defers on `TypeAliasType` origins and `ParamSpecType`/`TypeVarTupleType`
/// (no `meta_level` on the wire, so equality against them is undecidable).
fn typevar_ids_equal(a: &Type, b: &Type) -> Option<bool> {
    // Owned namespace avoids borrow-lifetime coupling between the two reads.
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

/// `c.type_var.is_meta_var()` (constraints.py:679) — `TypeVarId.is_meta_var`
/// is `meta_level > 0`. `ParamSpecType`/`TypeVarTupleType` origins do not
/// serialize their `meta_level`, so whether they are meta vars is
/// undecidable, and defers. `TypeAliasType` origins also defer.
fn origin_is_meta_var(origin: &Type) -> Option<bool> {
    match origin {
        Type::TypeVarType { meta_level, .. } => Some(*meta_level > 0),
        _ => None, // defer on ParamSpecType / TypeVarTupleType / TypeAliasType
    }
}

/// Read a bare (untagged) size; negative sizes are invalid.
fn read_size(buf: &mut ReadBuffer<'_>) -> Option<i64> {
    let size = crate::wire::read_int_bare(buf).ok()?;
    if size < 0 {
        return None;
    }
    Some(size)
}

fn write_size(buf: &mut WriteBuffer, size: i64) -> Result<(), WireError> {
    crate::wire::write_int_bare(buf, size)
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

    #[allow(dead_code)]
    fn type_var(raw_id: i64, meta_level: i64, name: &str) -> Type {
        Type::TypeVarType {
            name: name.to_string(),
            fullname: format!("mod.{name}"),
            raw_id,
            namespace: "fn".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            variance: 1,
            meta_level,
        }
    }

    /// Nested options format (select_trivial): outer count + inner counts.
    fn option_bytes(options: Vec<Vec<ConstraintParts>>) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_size(&mut buf, options.len() as i64).unwrap();
        for option in &options {
            write_size(&mut buf, option.len() as i64).unwrap();
            for constraint in option {
                write_constraint(&mut buf, constraint).unwrap();
            }
        }
        buf.into_bytes()
    }

    /// Single-option format (exclude_non_meta_vars / is_similar): one
    /// leading count, then the constraints.
    fn single_option(option: Vec<ConstraintParts>) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_size(&mut buf, option.len() as i64).unwrap();
        for constraint in &option {
            write_constraint(&mut buf, constraint).unwrap();
        }
        buf.into_bytes()
    }

    fn read_indices(bytes: &[u8]) -> Vec<i64> {
        let mut buf = ReadBuffer::new(bytes);
        let n = read_size(&mut buf).unwrap();
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            out.push(crate::wire::read_int_bare(&mut buf).unwrap());
        }
        out
    }

    fn c(tv: Type, op: i64, target: Type) -> ConstraintParts {
        ConstraintParts {
            origin: tv,
            op,
            target,
        }
    }

    #[test]
    fn test_select_trivial_picks_all_any_options() {
        let t_meta = type_var(1, 1, "T");
        let options = vec![
            vec![c(t_meta.clone(), 0, any_type())],
            vec![c(t_meta.clone(), 0, type_var(2, 0, "U"))],
        ];
        let result = rust_select_trivial(&option_bytes(options)).unwrap();
        assert_eq!(read_indices(&result), vec![0]);
    }

    #[test]
    fn test_select_trivial_picks_multiple() {
        let t_meta = type_var(1, 1, "T");
        let options = vec![
            vec![c(t_meta.clone(), 0, any_type())],
            vec![c(t_meta.clone(), 0, type_var(2, 0, "U"))],
            vec![c(t_meta.clone(), 1, any_type())],
        ];
        let result = rust_select_trivial(&option_bytes(options)).unwrap();
        assert_eq!(read_indices(&result), vec![0, 2]);
    }

    #[test]
    fn test_select_trivial_none_any_drops_everything() {
        let t_meta = type_var(1, 1, "T");
        let u_meta = type_var(2, 1, "U");
        let options = vec![vec![c(t_meta, 0, u_meta)]];
        let result = rust_select_trivial(&option_bytes(options)).unwrap();
        assert_eq!(read_indices(&result), Vec::<i64>::new());
    }

    #[test]
    fn test_exclude_non_meta_vars_filters_to_meta() {
        let t_meta = type_var(1, 1, "T");
        let t_nonmeta = type_var(2, 0, "U");
        let option = vec![
            c(t_meta.clone(), 0, any_type()),
            c(t_nonmeta.clone(), 0, any_type()),
        ];
        let result = rust_exclude_non_meta_vars(&single_option(option)).unwrap();
        assert_eq!(read_indices(&result), vec![0]);
    }

    #[test]
    fn test_exclude_non_meta_vars_empty_list_kept() {
        let result = rust_exclude_non_meta_vars(&single_option(Vec::new())).unwrap();
        assert_eq!(read_indices(&result), Vec::<i64>::new());
    }

    #[test]
    fn test_exclude_non_meta_vars_all_filtered_empty() {
        let t_nonmeta = type_var(2, 0, "U");
        let option = vec![c(t_nonmeta, 0, any_type())];
        let result = rust_exclude_non_meta_vars(&single_option(option)).unwrap();
        assert_eq!(read_indices(&result), Vec::<i64>::new());
    }

    #[test]
    fn test_is_similar_constraints_matches_by_structure() {
        let t_meta = type_var(1, 1, "T");
        let x = vec![c(t_meta.clone(), 0, any_type())];
        let y = vec![c(t_meta.clone(), 0, type_var(9, 0, "W"))];
        assert_eq!(
            rust_is_similar_constraints(&single_option(x), &single_option(y)),
            Some(true)
        );
    }

    #[test]
    fn test_is_similar_constraints_direction_ignored_when_any() {
        let t_meta = type_var(1, 1, "T");
        // c1 target Any => skip_op, so differing op still matches.
        let x = vec![c(t_meta.clone(), 0, any_type())];
        let y = vec![c(t_meta.clone(), 1, any_type())];
        assert_eq!(
            rust_is_similar_constraints(&single_option(x), &single_option(y)),
            Some(true)
        );
    }

    #[test]
    fn test_is_similar_constraints_different_var_not_similar() {
        let t_meta = type_var(1, 1, "T");
        let u_meta = type_var(2, 1, "U");
        let x = vec![c(t_meta, 0, any_type())];
        let y = vec![c(u_meta, 0, any_type())];
        assert_eq!(
            rust_is_similar_constraints(&single_option(x), &single_option(y)),
            Some(false)
        );
    }
}
