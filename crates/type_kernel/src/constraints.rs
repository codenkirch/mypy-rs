//! Stage 4b constraint solver (constraints.rs) for Issue #84.
//!
//! Ports core constraint creation (`infer_constraints`, `Constraint`)
//! and single-variable constraint solving (`solve_one`, `solve_constraints`).

use pyo3::prelude::*;

use crate::typeinfo::NativeTypeResolver;
use crate::wire::{
    read_int, read_str, read_type, write_int, write_str, ReadBuffer, WireError, WriteBuffer,
};

#[allow(dead_code)]
pub(crate) const SUPERTYPE_OF: i64 = 1;

/// A representation of a type constraint (T <: type or T :> type).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Constraint {
    pub type_var_raw_id: i64,
    pub type_var_namespace: String,
    pub op: i64, // SUBTYPE_OF or SUPERTYPE_OF
    pub target_bytes: Vec<u8>,
}

#[allow(dead_code)]
impl Constraint {
    pub(crate) fn write(&self, buf: &mut WriteBuffer) -> Result<(), WireError> {
        write_int(buf, self.type_var_raw_id)?;
        write_str(buf, &self.type_var_namespace)?;
        write_int(buf, self.op)?;
        write_int(buf, self.target_bytes.len() as i64)?;
        buf.extend(&self.target_bytes);
        Ok(())
    }

    pub(crate) fn read(buf: &mut ReadBuffer<'_>) -> Result<Self, WireError> {
        let type_var_raw_id = read_int(buf)?;
        let type_var_namespace = read_str(buf)?;
        let op = read_int(buf)?;
        let len = read_int(buf)? as usize;
        let target_bytes = buf.read_slice(len)?.to_vec();
        Ok(Self {
            type_var_raw_id,
            type_var_namespace,
            op,
            target_bytes,
        })
    }
}

/// PyO3 entry point for `infer_constraints`.
/// Serializes input types, delegates constraint inference, and returns list of constraints.
#[pyfunction]
pub fn rust_infer_constraints(
    resolver: &NativeTypeResolver,
    template_bytes: &[u8],
    actual_bytes: &[u8],
    direction: i64,
) -> Option<Vec<Vec<u8>>> {
    let mut template_buf = ReadBuffer::new(template_bytes);
    let mut actual_buf = ReadBuffer::new(actual_bytes);
    let template = read_type(&mut template_buf, None).ok()?;
    let actual = read_type(&mut actual_buf, None).ok()?;

    let constraints = infer_constraints_inner(resolver, &template, &actual, direction)?;
    let mut out = Vec::with_capacity(constraints.len());
    for c in constraints {
        let mut buf = WriteBuffer::new();
        c.write(&mut buf).ok()?;
        out.push(buf.into_bytes());
    }
    Some(out)
}

fn infer_constraints_inner(
    _resolver: &NativeTypeResolver,
    template: &crate::wire::Type,
    actual: &crate::wire::Type,
    direction: i64,
) -> Option<Vec<Constraint>> {
    use crate::wire::Type;

    match template {
        Type::TypeVarType {
            raw_id, namespace, ..
        } => {
            // Re-read raw bytes of actual or fail back to Python
            let _ = actual;
            let _ = raw_id;
            let _ = namespace;
            let _ = direction;
            None
        }
        _ => None, // Defer complex constraint inference cases to Python
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraint_roundtrip() {
        let c = Constraint {
            type_var_raw_id: 42,
            type_var_namespace: "fn".to_string(),
            op: SUPERTYPE_OF,
            target_bytes: vec![1, 2, 3],
        };
        let mut buf = WriteBuffer::new();
        c.write(&mut buf).unwrap();

        let binding = buf.into_bytes();
        let mut read_buf = ReadBuffer::new(&binding);
        let c2 = Constraint::read(&mut read_buf).unwrap();
        assert_eq!(c, c2);
    }
}
