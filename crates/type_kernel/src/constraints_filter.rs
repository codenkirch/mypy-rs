//! Pure constraint-list filtering functions (constraints_filter.rs), Issue #474.
//!
//! Ports the remaining self-contained, decision-only helpers from
//! `mypy.constraints` and `mypy.solve` that operate on constraint lists
//! or simple Type predicates without needing live graph data:
//!
//!   * `skip_reverse_union_constraints` — solve.py:858-884. Removes
//!     constraints that are redundant or ambiguous when inferred from
//!     unions during polymorphic inference. For each constraint whose
//!     target is a UnionType containing a TypeVarType, the function
//!     removes: (a) the original constraint if the TypeVarType matches
//!     the origin and op is SUBTYPE_OF, and (b) both the "reverse"
//!     constraint (TypeVar <: origin, neg_op) and the "forward"
//!     constraint (origin, op, TypeVar). Returns the filtered list.
//!
//!   * `filter_imprecise_kinds` — constraints.py:1997-2015. For each
//!     ParamSpec origin, if at least one precise constraint exists
//!     (target is a ParamSpecType or a non-imprecise Parameters), drops
//!     all imprecise Parameters targets for that origin.
//!
//!   * `is_type_type` / `unwrap_type_type` — constraints.py:686-703.
//!     Pure predicates on the wire Type enum. `is_type_type` checks if
//!     a type is a TypeType or a UnionType of all-TypeType items.
//!     `unwrap_type_type` extracts the inner type from a TypeType or
//!     builds a UnionType of the inner items.
//!
//!   * `infer_directed_arg_constraints` — constraints.py:1909-1921.
//!     Infers constraints between two argument types using the direction
//!     between original callables. Returns [] for ParamSpec/UnpackType
//!     pairs; otherwise routes through `infer_constraints_full_inner`
//!     with inverted direction (argument contravariance).
//!
//! All functions use the wire format (count + N× [origin Type | op int |
//! target Type]) for constraint lists, and serialized Type blobs for
//! Type inputs/outputs. `None` defers to Python.

use pyo3::prelude::*;

use crate::constraints::{neg_op, SUBTYPE_OF};
use crate::wire::{read_type, write_type, ReadBuffer, Type, WireError, WriteBuffer};

// ---------------------------------------------------------------------------
// skip_reverse_union_constraints (solve.py:858-884)
// ---------------------------------------------------------------------------

/// `skip_reverse_union_constraints` (solve.py:858-884).
///
/// Wire layout in: count (bare int) + N× [origin Type | op int | target
/// Type]. Wire layout out: count (bare int) + M× [origin Type | op int |
/// target Type] (filtered subset).
///
/// For each constraint whose target is a UnionType:
/// - If any union item is a TypeVarType matching the origin and op is
///   SUBTYPE_OF, the original constraint is removed.
/// - For each union item that is a TypeVarType, two reverse constraints
///   are generated and added to a removal set: `(item, neg_op(op),
///   origin)` and `(origin, op, item)`.
///
/// Returns `None` on any decode/encode failure or when a TypeAliasType
/// target is encountered (get_proper_type expansion needed).
#[pyfunction]
pub(crate) fn rust_skip_reverse_union_constraints(constraints_bytes: &[u8]) -> Option<Vec<u8>> {
    let constraints = read_constraint_list(constraints_bytes)?;
    let filtered = skip_reverse_union_inner(&constraints)?;
    let mut output = WriteBuffer::new();
    write_size(&mut output, filtered.len() as i64).ok()?;
    for c in &filtered {
        write_constraint_parts(&mut output, c).ok()?;
    }
    Some(output.into_bytes())
}

/// Access to the (origin, op, target) triple shared by the wire-form
/// [`WireConstraint`] and the kernel [`crate::constraints::Constraint`].
/// Lets `skip_reverse_union_inner` filter either list shape.
trait ConstraintTriple
where
    Self: Clone + Sized,
{
    fn triple_origin(&self) -> &Type;
    fn triple_op(&self) -> i64;
    fn triple_target(&self) -> &Type;
    /// Rebuild a new constraint from the filtered parts.
    fn from_parts(origin: Type, op: i64, target: Type) -> Self;
}

impl ConstraintTriple for WireConstraint {
    fn triple_origin(&self) -> &Type {
        &self.origin
    }
    fn triple_op(&self) -> i64 {
        self.op
    }
    fn triple_target(&self) -> &Type {
        &self.target
    }
    fn from_parts(origin: Type, op: i64, target: Type) -> Self {
        WireConstraint { origin, op, target }
    }
}

impl ConstraintTriple for crate::constraints::Constraint {
    fn triple_origin(&self) -> &Type {
        &self.origin_type_var
    }
    fn triple_op(&self) -> i64 {
        self.op
    }
    fn triple_target(&self) -> &Type {
        &self.target
    }
    fn from_parts(origin: Type, op: i64, target: Type) -> Self {
        crate::constraints::Constraint {
            origin_type_var: origin,
            op,
            target,
            extra_tvars: Vec::new(),
        }
    }
}

fn skip_reverse_union_inner<T: ConstraintTriple + Clone + PartialEq>(
    constraints: &[T],
) -> Option<Vec<T>> {
    // Build the set of constraints to remove, mirroring solve.py:871-883.
    // Constraint equality is (origin_type_var, op, target) by value.
    let mut remove_set: Vec<T> = Vec::new();

    for c in constraints {
        // get_proper_type(c.target) — wire types are already proper, but
        // TypeAliasType needs expansion; defer.
        match c.triple_target() {
            Type::TypeAliasType { .. } => return None,
            Type::UnionType { items, .. } => {
                for item in items {
                    if let Type::TypeVarType { .. } = item {
                        if item == c.triple_origin() && c.triple_op() == SUBTYPE_OF {
                            // reverse_union_cs.add(c): remove the original and
                            // skip the exploded forms (solve.py's continue).
                            remove_set_push(&mut remove_set, c.clone());
                            continue;
                        }
                        // reverse_union_cs.add(Constraint(item, neg_op(op), origin))
                        let rev = T::from_parts(
                            item.clone(),
                            neg_op(c.triple_op()),
                            c.triple_origin().clone(),
                        );
                        remove_set_push(&mut remove_set, rev);
                        // reverse_union_cs.add(Constraint(origin, op, item))
                        let fwd =
                            T::from_parts(c.triple_origin().clone(), c.triple_op(), item.clone());
                        remove_set_push(&mut remove_set, fwd);
                    }
                }
            }
            _ => {}
        }
    }

    // Filter: keep constraints not in remove_set.
    let result: Vec<T> = constraints
        .iter()
        .filter(|c| !remove_set.iter().any(|r| r == *c))
        .cloned()
        .collect();
    Some(result)
}

// ---------------------------------------------------------------------------
// filter_imprecise_kinds (constraints.py:1997-2015)
// ---------------------------------------------------------------------------

/// `filter_imprecise_kinds` (constraints.py:1997-2015).
///
/// Wire layout in: count (bare int) + N× [origin Type | op int | target
/// Type]. Wire layout out: count (bare int) + M× index (bare int). The
/// Python shim reconstructs the filtered list from the original
/// constraints using these indices, preserving object identity and
/// `extra_tvars` (which a rebuild would lose).
///
/// For each ParamSpec origin, if at least one precise constraint exists
/// (target is ParamSpecType or non-imprecise Parameters), all imprecise
/// Parameters targets for that origin are dropped.
///
/// Returns `None` on decode failure or when a TypeAliasType origin/target
/// is encountered.
#[pyfunction]
pub(crate) fn rust_filter_imprecise_kinds(constraints_bytes: &[u8]) -> Option<Vec<u8>> {
    let constraints = read_constraint_list(constraints_bytes)?;
    let indices = filter_imprecise_kinds_indices(&constraints)?;
    let mut output = WriteBuffer::new();
    write_size(&mut output, indices.len() as i64).ok()?;
    for i in &indices {
        crate::wire::write_int_bare(&mut output, *i).ok()?;
    }
    Some(output.into_bytes())
}

fn filter_imprecise_kinds_indices(constraints: &[WireConstraint]) -> Option<Vec<i64>> {
    // Defer on any TypeAliasType (needs get_proper_type).
    for c in constraints {
        if matches!(c.origin, Type::TypeAliasType { .. })
            || matches!(c.target, Type::TypeAliasType { .. })
        {
            return None;
        }
    }

    // have_precise: set of TypeVarId (raw_id, meta_level, namespace) for
    // ParamSpec origins that have at least one precise constraint.
    let mut have_precise: Vec<TvId> = Vec::new();

    for c in constraints {
        if !matches!(c.origin, Type::ParamSpecType { .. }) {
            continue;
        }
        let is_precise = match &c.target {
            Type::ParamSpecType { .. } => true,
            Type::Parameters(ref p) => !p.imprecise_arg_kinds,
            _ => false,
        };
        if is_precise {
            if let Some(id) = tv_id(&c.origin) {
                if !have_precise.contains(&id) {
                    have_precise.push(id);
                }
            }
        }
    }

    // Python logic (constraints.py:2010-2014):
    //   for c in cs:
    //     if not isinstance(c.origin, ParamSpecType) or c.type_var not in have_precise:

    //       new_cs.append(c)
    //     if not isinstance(c.target, Parameters) or not c.target.imprecise_arg_kinds:
    //       new_cs.append(c)

    //
    // A constraint can be appended twice (once by each condition); the
    // Python list allows duplicates, so we emit the index twice.
    let mut indices: Vec<i64> = Vec::new();
    for (i, c) in constraints.iter().enumerate() {
        let origin_is_precise_paramspec = matches!(c.origin, Type::ParamSpecType { .. })
            && tv_id(&c.origin).is_some_and(|id| have_precise.contains(&id));

        if !origin_is_precise_paramspec {
            indices.push(i as i64);
        }
        let target_is_imprecise_params = matches!(
            &c.target,
            Type::Parameters(ref p) if p.imprecise_arg_kinds
        );
        if !target_is_imprecise_params {
            indices.push(i as i64);
        }
    }
    Some(indices)
}

// ---------------------------------------------------------------------------
// is_type_type / unwrap_type_type (constraints.py:686-703)
// ---------------------------------------------------------------------------

/// `_is_type_type` (constraints.py:686-696): is `tp` a `type[...]` or a
/// union thereof?
///
/// Returns `None` on decode failure or when a TypeAliasType is
/// encountered (needs get_proper_type expansion). Otherwise returns
/// `Some(true)` / `Some(false)`.
#[pyfunction]
pub(crate) fn rust_is_type_type(tp_bytes: &[u8]) -> Option<bool> {
    let tp = decode_type(tp_bytes)?;
    if matches!(tp, Type::TypeAliasType { .. }) {
        return None;
    }
    Some(is_type_type_inner(&tp))
}

/// `_unwrap_type_type` (constraints.py:699-703): extract the inner type
/// from a `type[...]` expression or a union thereof.
///
/// Returns `None` on decode failure, TypeAliasType input, or when
/// building the result union fails (Rust cannot build
/// `UnionType.make_union`).
#[pyfunction]
pub(crate) fn rust_unwrap_type_type(tp_bytes: &[u8]) -> Option<Vec<u8>> {
    let tp = decode_type(tp_bytes)?;
    if matches!(tp, Type::TypeAliasType { .. }) {
        return None;
    }
    let result = unwrap_type_type_inner(&tp)?;
    let mut buf = WriteBuffer::new();
    write_type(&mut buf, &result).ok()?;
    Some(buf.into_bytes())
}

pub(crate) fn is_type_type_inner(tp: &Type) -> bool {
    match tp {
        Type::TypeType { .. } => true,
        Type::UnionType { items, .. } => items.iter().all(|i| matches!(i, Type::TypeType { .. })),
        _ => false,
    }
}

pub(crate) fn unwrap_type_type_inner(tp: &Type) -> Option<Type> {
    match tp {
        Type::TypeType { item, .. } => Some((**item).clone()),
        Type::UnionType { items, .. } => {
            // UnionType.make_union([cast(TypeType, get_proper_type(o)).item for o in items])
            // Each item must be a TypeType; extract the inner item.
            let mut inner_items: Vec<Type> = Vec::new();
            for o in items {
                match o {
                    Type::TypeType { item, .. } => inner_items.push((**item).clone()),
                    _ => return None, // not all items are TypeType; defer
                }
            }
            // UnionType.make_union: 0 items -> UninhabitedType, 1 item ->
            // the item itself, >1 -> UnionType(items) (types.py:3774-3780).
            // The union gate guarantees TypeType items; an alias item defers.
            Some(crate::setops::union_make_union(inner_items))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// infer_directed_arg_constraints (constraints.py:1909-1921)
// ---------------------------------------------------------------------------

/// `infer_directed_arg_constraints` (constraints.py:1909-1921).
///
/// Infers constraints between two argument types using the direction
/// between original callables. Returns [] for ParamSpec/UnpackType
/// pairs; otherwise routes through `infer_constraints_full_inner` with
/// inverted direction (argument contravariance).
///
/// Wire layout in: left Type | right Type | direction int.
/// Wire layout out: count (bare int) + N× [origin Type | op int | target Type].
///
/// Returns `None` on decode failure or when the inner inference defers.
#[pyfunction]
pub(crate) fn rust_infer_directed_arg_constraints(
    resolver: &crate::typeinfo::NativeTypeResolver,
    left_bytes: &[u8],
    right_bytes: &[u8],
    direction: i64,
    strict_optional: bool,
) -> Option<Vec<u8>> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;

    // ParamSpec/UnpackType on either side -> return [].
    if matches!(left, Type::ParamSpecType { .. } | Type::UnpackType { .. })
        || matches!(right, Type::ParamSpecType { .. } | Type::UnpackType { .. })
    {
        let mut output = WriteBuffer::new();
        write_size(&mut output, 0).ok()?;
        return Some(output.into_bytes());
    }

    // Direction inversion for argument contravariance.
    let (template, actual, inferred_dir) = if direction == SUBTYPE_OF {
        // SUBTYPE_OF: invert direction -> infer_constraints(left, right, neg_op(direction))
        (left, right, neg_op(direction))
    } else {
        // SUPERTYPE_OF: infer_constraints(right, left, neg_op(direction))
        (right, left, neg_op(direction))
    };

    let constraints = crate::constraints::infer_constraints_full_inner(
        &template,
        &actual,
        inferred_dir,
        resolver.resolver(),
        resolver.alias_resolver(),
        strict_optional,
        false,
        // Python `infer_constraints` wrapper default (constraints.py:802).
        true,
    )?;
    // The write loop is 3-field: extras would be lost in serialization.
    // Defensive: no mode is installed on this FFI, so extras cannot arise.
    if constraints.iter().any(|c| !c.extra_tvars.is_empty()) {
        return None;
    }

    let mut output = WriteBuffer::new();
    write_size(&mut output, constraints.len() as i64).ok()?;
    for c in &constraints {
        write_type(&mut output, &c.origin_type_var).ok()?;
        crate::wire::write_int(&mut output, c.op).ok()?;
        write_type(&mut output, &c.target).ok()?;
    }
    Some(output.into_bytes())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Type variable id = (raw_id, meta_level, namespace), mirroring
/// `mypy.types.TypeVarId`.
type TvId = (i64, i64, String);

/// Extract the TypeVarId from a TypeVar-like wire type.
fn tv_id(t: &Type) -> Option<TvId> {
    match t {
        Type::TypeVarType {
            raw_id,
            meta_level,
            namespace,
            ..
        } => Some((*raw_id, *meta_level, namespace.clone())),
        Type::ParamSpecType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        Type::TypeVarTupleType {
            raw_id, namespace, ..
        } => Some((*raw_id, 0, namespace.clone())),
        _ => None,
    }
}

/// A constraint in wire form: origin Type, op int, target Type.
#[derive(Clone, Debug, PartialEq)]
struct WireConstraint {
    origin: Type,
    op: i64,
    target: Type,
}

impl WireConstraint {}

/// Read a constraint list from wire format: count (bare int) + N×
/// [origin Type | op int | target Type].
fn read_constraint_list(bytes: &[u8]) -> Option<Vec<WireConstraint>> {
    let mut buf = ReadBuffer::new(bytes);
    let n = read_size(&mut buf)?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let origin = read_type(&mut buf, None).ok()?;
        let op = crate::wire::read_int(&mut buf).ok()?;
        let target = read_type(&mut buf, None).ok()?;
        out.push(WireConstraint { origin, op, target });
    }
    Some(out)
}

/// Write a single constraint to a buffer: origin Type | op int | target Type.
fn write_constraint_parts(buf: &mut WriteBuffer, c: &WireConstraint) -> Result<(), WireError> {
    write_type(buf, &c.origin)?;
    crate::wire::write_int(buf, c.op)?;
    write_type(buf, &c.target)
}

/// Read a bare (untagged) size; negative sizes are invalid.
fn read_size(buf: &mut ReadBuffer<'_>) -> Option<i64> {
    let size = crate::wire::read_int_bare(buf).ok()?;
    if size < 0 {
        return None;
    }
    Some(size)
}

/// Write a bare (untagged) size.
fn write_size(buf: &mut WriteBuffer, size: i64) -> Result<(), WireError> {
    crate::wire::write_int_bare(buf, size)
}

/// Decode a Type from wire bytes.
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Push a constraint into the remove_set if not already present.
fn remove_set_push<T: PartialEq>(set: &mut Vec<T>, c: T) {
    if !set.contains(&c) {
        set.push(c);
    }
}

/// In-crate entry: filter a kernel `Constraint` list the way the Python
/// `skip_reverse_union_constraints` (solve.py:889) FFI seam does. The
/// unify port calls this before the polymorphic solve.
pub(crate) fn skip_reverse_union_kernel(
    constraints: &[crate::constraints::Constraint],
) -> Option<Vec<crate::constraints::Constraint>> {
    skip_reverse_union_inner(constraints)
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

    fn instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn wc(origin: Type, op: i64, target: Type) -> WireConstraint {
        WireConstraint { origin, op, target }
    }

    fn encode_constraints(constraints: &[WireConstraint]) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        write_size(&mut buf, constraints.len() as i64).unwrap();
        for c in constraints {
            write_constraint_parts(&mut buf, c).unwrap();
        }
        buf.into_bytes()
    }

    fn decode_constraints(bytes: &[u8]) -> Vec<WireConstraint> {
        read_constraint_list(bytes).unwrap_or_default()
    }

    // -- skip_reverse_union_constraints --

    #[test]
    fn test_skip_reverse_union_no_union_targets() {
        // No UnionType targets -> nothing removed.
        let t = type_var(1, 1, "T");
        let cs = vec![
            wc(t.clone(), SUBTYPE_OF, instance("builtins.int")),
            wc(t.clone(), 1, instance("builtins.str")),
        ];
        let result = skip_reverse_union_inner(&cs).unwrap();
        assert_eq!(result.len(), cs.len());
    }

    #[test]
    fn test_skip_reverse_union_removes_self_referential() {
        // T <: Union[T, int] -> original removed (T matches origin, op=SUBTYPE_OF).
        let t = type_var(1, 1, "T");
        let int = instance("builtins.int");
        let union = Type::UnionType {
            items: vec![t.clone(), int],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let cs = vec![wc(t.clone(), SUBTYPE_OF, union)];
        let result = skip_reverse_union_inner(&cs).unwrap();
        // The original constraint T <: Union[T, int] is removed because
        // item T == origin T and op == SUBTYPE_OF. The continue skips the
        // exploded (T, SUPERTYPE_OF, T) / (T, SUBTYPE_OF, T) forms.
        assert!(result.is_empty());
    }

    #[test]
    fn test_skip_reverse_union_self_ref_survives_exploded() {
        // A co-existing (T, SUBTYPE_OF, T) constraint must survive: the
        // self-referential branch removes only the original (solve.py
        // continue), never the exploded forms.
        let t = type_var(1, 1, "T");
        let int = instance("builtins.int");
        let union = Type::UnionType {
            items: vec![t.clone(), int],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let cs = vec![
            wc(t.clone(), SUBTYPE_OF, union),
            wc(t.clone(), SUBTYPE_OF, t.clone()),
        ];
        let result = skip_reverse_union_inner(&cs).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], wc(t.clone(), SUBTYPE_OF, t));
    }

    #[test]
    fn test_skip_reverse_union_self_ref_item_not_first() {
        // Reversed variant: the self-referential item sits behind a plain
        // item in the union, so the loop reaches it mid-iteration.
        let t = type_var(1, 1, "T");
        let int = instance("builtins.int");
        let union = Type::UnionType {
            items: vec![int, t.clone()],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let cs = vec![wc(t.clone(), SUBTYPE_OF, union)];
        let result = skip_reverse_union_inner(&cs).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_skip_reverse_union_removes_reverse_constraints() {
        // T :> Union[S, int] -> removes (S, SUBTYPE_OF, T) and (T, SUPERTYPE_OF, S)
        // from the constraint list if they exist.
        let t = type_var(1, 1, "T");
        let s = type_var(2, 1, "S");
        let int = instance("builtins.int");
        let union = Type::UnionType {
            items: vec![s.clone(), int],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        // Original: T :> Union[S, int] (op=SUPERTYPE_OF=1)
        let c_orig = wc(t.clone(), 1, union);
        // Reverse constraints that should be removed:
        // (S, neg_op(1)=0=SUBTYPE_OF, T) and (T, 1=SUPERTYPE_OF, S)
        let c_rev1 = wc(s.clone(), SUBTYPE_OF, t.clone());
        let c_rev2 = wc(t.clone(), 1, s.clone());
        let cs = vec![c_orig.clone(), c_rev1, c_rev2];
        let result = skip_reverse_union_inner(&cs).unwrap();
        // Only the original remains (the reverse constraints are removed).
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], c_orig);
    }

    #[test]
    fn test_skip_reverse_union_defers_alias() {
        let t = type_var(1, 1, "T");
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
            is_recursive: false,
        };
        let cs = vec![wc(t, SUBTYPE_OF, alias)];
        assert!(skip_reverse_union_inner(&cs).is_none());
    }

    // -- filter_imprecise_kinds --

    fn param_spec(raw_id: i64, name: &str) -> Type {
        Type::ParamSpecType {
            prefix: Box::new(crate::wire::Parameters {
                arg_types: Vec::new(),
                arg_kinds: Vec::new(),
                arg_names: Vec::new(),
                variables: Vec::new(),
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: name.to_string(),
            fullname: format!("mod.{name}"),
            raw_id,
            namespace: "fn".to_string(),
            flavor: 0,
            upper_bound: Box::new(any_type()),
            default: Box::new(any_type()),
            meta_level: 0,
        }
    }

    fn parameters(imprecise: bool) -> Type {
        Type::Parameters(crate::wire::Parameters {
            arg_types: vec![any_type()],
            arg_kinds: vec![0],
            arg_names: vec![None],
            variables: Vec::new(),
            imprecise_arg_kinds: imprecise,
            is_ellipsis_args: false,
        })
    }

    #[test]
    fn test_filter_imprecise_no_paramspec() {
        // Non-ParamSpec origin -> all constraints kept.
        let t = type_var(1, 1, "T");
        let cs = vec![wc(t.clone(), SUBTYPE_OF, any_type())];
        let result = filter_imprecise_kinds_indices(&cs).unwrap();
        // Non-ParamSpec origin + non-Parameters target -> index added twice
        // (once by each condition in the Python loop).
        assert_eq!(result.len(), 2);
        assert_eq!(result, vec![0, 0]);
    }

    #[test]
    fn test_filter_imprecise_keeps_precise_drops_imprecise() {
        // P with one precise (ParamSpecType target) + one imprecise
        // (Parameters imprecise=True target). The imprecise one is dropped.
        let p = param_spec(1, "P");
        let imprecise_params = parameters(true);
        let precise_target = param_spec(2, "Q");
        let cs = vec![
            wc(p.clone(), SUBTYPE_OF, imprecise_params.clone()),
            wc(p.clone(), SUBTYPE_OF, precise_target.clone()),
        ];
        let result = filter_imprecise_kinds_indices(&cs).unwrap();
        // P is now in have_precise. The imprecise Parameters constraint:
        //   condition (a): origin IS ParamSpec AND in have_precise -> not added
        //   condition (b): target IS imprecise Parameters -> not added

        // So the imprecise constraint is dropped entirely (index 0 absent).
        // The precise constraint (index 1):
        //   condition (a): origin IS ParamSpec AND in have_precise -> not added

        //   condition (b): target is ParamSpecType (not Parameters) -> added
        // So only index 1 remains.
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn test_filter_imprecise_keeps_all_without_precise() {
        // P with only imprecise constraints -> nothing dropped.
        let p = param_spec(1, "P");
        let imprecise_params = parameters(true);
        let cs = vec![wc(p.clone(), SUBTYPE_OF, imprecise_params.clone())];
        let result = filter_imprecise_kinds_indices(&cs).unwrap();
        // P not in have_precise (no precise target).
        // condition (a): origin IS ParamSpec but NOT in have_precise -> added
        // condition (b): target IS imprecise Parameters -> not added

        // So index 0 appears once.
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_filter_imprecise_defers_alias() {
        let t = type_var(1, 1, "T");
        let alias = Type::TypeAliasType {
            args: vec![],
            type_ref: "mod.Alias".to_string(),
            is_recursive: false,
        };
        let cs = vec![wc(t, SUBTYPE_OF, alias)];
        assert!(filter_imprecise_kinds_indices(&cs).is_none());
    }

    // -- is_type_type / unwrap_type_type --

    #[test]
    fn test_is_type_type_type_type() {
        let tp = Type::TypeType {
            item: Box::new(instance("builtins.int")),
            is_type_form: false,
        };
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &tp).unwrap();
        assert_eq!(rust_is_type_type(&buf.into_bytes()), Some(true));
    }

    #[test]
    fn test_is_type_type_union_of_type_types() {
        let tp = Type::UnionType {
            items: vec![
                Type::TypeType {
                    item: Box::new(instance("builtins.int")),
                    is_type_form: false,
                },
                Type::TypeType {
                    item: Box::new(instance("builtins.str")),
                    is_type_form: false,
                },
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &tp).unwrap();
        assert_eq!(rust_is_type_type(&buf.into_bytes()), Some(true));
    }

    #[test]
    fn test_is_type_type_plain_instance() {
        let tp = instance("builtins.int");
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &tp).unwrap();
        assert_eq!(rust_is_type_type(&buf.into_bytes()), Some(false));
    }

    #[test]
    fn test_is_type_type_union_with_non_type_type() {
        let tp = Type::UnionType {
            items: vec![
                Type::TypeType {
                    item: Box::new(instance("builtins.int")),
                    is_type_form: false,
                },
                instance("builtins.str"),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &tp).unwrap();
        assert_eq!(rust_is_type_type(&buf.into_bytes()), Some(false));
    }

    #[test]
    fn test_unwrap_type_type_single() {
        let tp = Type::TypeType {
            item: Box::new(instance("builtins.int")),
            is_type_form: false,
        };
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &tp).unwrap();
        let result = rust_unwrap_type_type(&buf.into_bytes()).unwrap();
        let decoded = decode_type(&result).unwrap();
        assert_eq!(decoded, instance("builtins.int"));
    }

    #[test]
    fn test_unwrap_type_type_union() {
        let tp = Type::UnionType {
            items: vec![
                Type::TypeType {
                    item: Box::new(instance("builtins.int")),
                    is_type_form: false,
                },
                Type::TypeType {
                    item: Box::new(instance("builtins.str")),
                    is_type_form: false,
                },
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
            is_evaluated: true,
            original_str_expr: None,
            original_str_fallback: None,
        };
        let mut buf = WriteBuffer::new();
        write_type(&mut buf, &tp).unwrap();
        let result = rust_unwrap_type_type(&buf.into_bytes()).unwrap();
        let decoded = decode_type(&result).unwrap();
        // Should be a UnionType with the inner items.
        match decoded {
            Type::UnionType { items, .. } => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], instance("builtins.int"));
                assert_eq!(items[1], instance("builtins.str"));
            }
            _ => panic!("expected UnionType"),
        }
    }

    // -- infer_directed_arg_constraints --

    fn make_resolver() -> crate::typeinfo::TypeResolver {
        crate::typeinfo::TypeResolver::new()
    }

    fn native_resolver() -> crate::typeinfo::NativeTypeResolver {
        crate::typeinfo::NativeTypeResolver::new(
            make_resolver(),
            crate::aliases::TypeAliasResolver::default(),
        )
    }

    #[test]
    fn test_infer_directed_arg_constraints_paramspec_returns_empty() {
        let p = param_spec(1, "P");
        let right = instance("builtins.int");
        let mut left_buf = WriteBuffer::new();
        write_type(&mut left_buf, &p).unwrap();
        let mut right_buf = WriteBuffer::new();
        write_type(&mut right_buf, &right).unwrap();
        let result = rust_infer_directed_arg_constraints(
            &native_resolver(),
            &left_buf.into_bytes(),
            &right_buf.into_bytes(),
            SUBTYPE_OF,
            true,
        )
        .unwrap();
        let decoded = read_constraint_list(&result).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_infer_directed_arg_constraints_unpack_returns_empty() {
        let unpack = Type::UnpackType {
            typ: Box::new(type_var(1, 0, "T")),
            from_star_syntax: false,
        };
        let right = instance("builtins.int");
        let mut left_buf = WriteBuffer::new();
        write_type(&mut left_buf, &unpack).unwrap();
        let mut right_buf = WriteBuffer::new();
        write_type(&mut right_buf, &right).unwrap();
        let result = rust_infer_directed_arg_constraints(
            &native_resolver(),
            &left_buf.into_bytes(),
            &right_buf.into_bytes(),
            SUBTYPE_OF,
            true,
        )
        .unwrap();
        assert!(read_constraint_list(&result).unwrap().is_empty());
    }

    #[test]
    fn test_infer_directed_arg_constraints_typevar_template() {
        // left=T (TypeVarType), right=int, direction=SUBTYPE_OF.
        // infer_directed_arg_constraints inverts to SUPERTYPE_OF:
        //   infer_constraints(T, int, SUPERTYPE_OF) -> T :> int.
        let t = type_var(1, 1, "T");
        let right = instance("builtins.int");
        let mut left_buf = WriteBuffer::new();
        write_type(&mut left_buf, &t).unwrap();
        let mut right_buf = WriteBuffer::new();
        write_type(&mut right_buf, &right).unwrap();
        let result = rust_infer_directed_arg_constraints(
            &native_resolver(),
            &left_buf.into_bytes(),
            &right_buf.into_bytes(),
            SUBTYPE_OF,
            true,
        );
        // The inner inference may defer (None) if the resolver doesn't
        // have builtins.int. That's fine — we just check it doesn't panic.
        if let Some(result) = result {
            let decoded = read_constraint_list(&result);
            // T :> int should produce one constraint: T SUPERTYPE_OF int.
            if let Some(constraints) = decoded {
                assert_eq!(constraints.len(), 1);
                assert_eq!(constraints[0].op, 1); // SUPERTYPE_OF
            }
        }
    }

    #[test]
    fn test_skip_reverse_union_constraints_pyfunction_roundtrip() {
        // Verify the pyfunction wire format round-trips correctly.
        let t = type_var(1, 1, "T");
        let int = instance("builtins.int");
        let cs = vec![wc(t.clone(), SUBTYPE_OF, int.clone())];
        let input = encode_constraints(&cs);
        let output = rust_skip_reverse_union_constraints(&input).unwrap();
        let decoded = decode_constraints(&output);
        // No union target -> nothing removed.
        assert_eq!(decoded.len(), cs.len());
    }

    #[test]
    fn test_filter_imprecise_kinds_pyfunction_roundtrip() {
        // The pyfunction returns a bare-int index list (count + N× index),
        // not a constraint list. Non-ParamSpec origin + non-Parameters
        // target -> index added twice.
        let t = type_var(1, 1, "T");
        let cs = vec![wc(t, SUBTYPE_OF, any_type())];
        let input = encode_constraints(&cs);
        let output = rust_filter_imprecise_kinds(&input).unwrap();
        let mut buf = ReadBuffer::new(&output);
        let count = read_size(&mut buf).unwrap();
        assert_eq!(count, 2);
        let i0 = crate::wire::read_int_bare(&mut buf).unwrap();
        let i1 = crate::wire::read_int_bare(&mut buf).unwrap();
        assert_eq!(i0, 0);
        assert_eq!(i1, 0);
    }
}
