//! Stage 4b constraint solver (constraints.rs) for Issue #84.
//!
//! Ports core constraint creation (`infer_constraints`, `Constraint`)
//! and single-variable constraint solving (`solve_one`, `solve_constraints`).

use pyo3::prelude::*;

use crate::wire::{
    read_int, read_type, write_int, write_type, ReadBuffer, Type, WireError, WriteBuffer,
};

pub(crate) const SUPERTYPE_OF: i64 = 1;

/// A representation of a type constraint (T <: type or T :> type).
///
/// Unlike the earlier wire format (which dropped `origin_type_var`, the
/// full `TypeVarType`), this carries the origin so the Python solver can
/// use `values`/`upper_bound`/`variance`/`meta_level` when grouping and
/// solving. The TypeVarType wire round-trip is complete (see #177).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Constraint {
    pub origin_type_var: Type,
    pub op: i64, // SUBTYPE_OF or SUPERTYPE_OF
    pub target: Type,
}

impl Constraint {
    pub(crate) fn write(&self, buf: &mut WriteBuffer) -> Result<(), WireError> {
        write_type(buf, &self.origin_type_var)?;
        write_int(buf, self.op)?;
        write_type(buf, &self.target)?;
        Ok(())
    }

    pub(crate) fn read(buf: &mut ReadBuffer<'_>) -> Result<Self, WireError> {
        let origin_type_var = read_type(buf, None)?;
        let op = read_int(buf)?;
        let target = read_type(buf, None)?;
        Ok(Self {
            origin_type_var,
            op,
            target,
        })
    }
}

/// PyO3 entry point for `infer_constraints`.
///
/// Only the top-level TypeVarType template case is handled (matching the
/// Python O(1) branch in `_infer_constraints`); everything else defers to
/// Python with `None`. Serializes the resulting `Constraint` list
/// (origin + op + target) so the shim can reconstruct full objects.
#[pyfunction]
pub(crate) fn rust_infer_constraints(
    template_bytes: &[u8],
    actual_bytes: &[u8],
    direction: i64,
) -> Option<Vec<Vec<u8>>> {
    let mut template_buf = ReadBuffer::new(template_bytes);
    let template = read_type(&mut template_buf, None).ok()?;
    let mut actual_buf = ReadBuffer::new(actual_bytes);
    let actual = read_type(&mut actual_buf, None).ok()?;

    let constraint = infer_constraints_inner(&template, &actual, direction)?;
    let mut buf = WriteBuffer::new();
    constraint.write(&mut buf).ok()?;
    Some(vec![buf.into_bytes()])
}

/// Emit a constraint for the top-level `TypeVarType` template, mirroring
/// `_infer_constraints`'s first branch. Defer (None) on anything else.
fn infer_constraints_inner(template: &Type, actual: &Type, direction: i64) -> Option<Constraint> {
    match template {
        Type::TypeVarType { .. } => Some(Constraint {
            origin_type_var: template.clone(),
            op: direction,
            target: actual.clone(),
        }),
        _ => None,
    }
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

    fn type_var(raw_id: i64, name: &str) -> Type {
        Type::TypeVarType {
            name: name.to_string(),
            fullname: format!("mod.{}", name),
            raw_id: raw_id,
            namespace: "fn".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            variance: 1, // COVARIANT
            meta_level: 1,
        }
    }

    #[test]
    fn test_constraint_roundtrip_full_typevar() {
        let tv = type_var(42, "T");
        let c = Constraint {
            origin_type_var: tv.clone(),
            op: SUPERTYPE_OF,
            target: any_type(),
        };
        let mut buf = WriteBuffer::new();
        c.write(&mut buf).unwrap();

        let binding = buf.into_bytes();
        let mut read_buf = ReadBuffer::new(&binding);
        let c2 = Constraint::read(&mut read_buf).unwrap();
        assert_eq!(c, c2);

        // The round-tripped origin must still be a TypeVarType with the
        // same identity fields.
        if let Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } = &c2.origin_type_var
        {
            assert_eq!(*raw_id, 42);
            assert_eq!(*meta_level, 1);
            assert_eq!(namespace, "fn");
        } else {
            panic!("origin_type_var not a TypeVarType");
        }
    }

    #[test]
    fn test_infer_constraints_top_level_typevar() {
        let tv = type_var(7, "T");
        let actual = any_type();
        let c = infer_constraints_inner(&tv, &actual, SUPERTYPE_OF).unwrap();
        assert_eq!(c.op, SUPERTYPE_OF);
        assert_eq!(c.origin_type_var, tv);
        assert_eq!(c.target, actual);
    }

    #[test]
    fn test_constraint_wire_bytes_ready() {
        // Round-trip through the pyfunction-shaped path: the emitted
        // blob can be read back to a Constraint.
        let tv = type_var(9, "U");
        let c = infer_constraints_inner(&tv, &any_type(), 0 /* SUBTYPE_OF */).unwrap();
        let mut buf = WriteBuffer::new();
        c.write(&mut buf).unwrap();
        let binding = buf.into_bytes();
        let mut read_buf = ReadBuffer::new(&binding);
        let c2 = Constraint::read(&mut read_buf).unwrap();
        assert_eq!(c2.origin_type_var, tv);
        assert_eq!(c2.target, any_type());
    }
}
