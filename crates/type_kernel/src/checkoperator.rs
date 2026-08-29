//! Native port of `mypy.checkexpr.check_op_reversible` STEP 2a (Stage M2x).
//!
//! STEP 2a (checkexpr.py:4695-4735) decides the *order* in which Python
//! calls `__op__` and `__rop__` for a binary operator expression:
//!
//!   * shortcut: `op` in `op_methods_that_shortcut` and left and right are
//!     the same type -> only `__op__` is called (single variant).
//!   * reverse-first: right is a subclass of left (or an `alt_promote`
//!     special case) -> try `__rop__` first, then `__op__`.
//!   * normal: try `__op__` first, then `__rop__`.
//!
//! This module ports ONLY the variant-ordering decision. It returns an
//! `Option<u8>`: 0 = shortcut-single, 1 = reverse-first, 2 = normal, or
//! `None` to defer to the pure-Python chain. Code 1 rides the
//! parity-tested `covers_at_runtime_inner` port (#1131): when the kernel
//! decides `covers_at_runtime(right, left)`, the Python fold would return
//! the same bool, so building the reverse-first variants from it is exact.
//! Deferred (None) where the covers port defers (tuple-shaped operands,
//! nested aliases, missing snapshots) or where any decision is layered on
//! an undecided same-type check.
//!
//! Deferred (return None):
//!   * Any operand — Python returns early before ordering (checkexpr.py:4660).
//!   * `TypeAliasType` operand — the wire format carries no resolved alias
//!     target, so `get_proper_type` cannot expand it.
//!   * A subtype/covers check that returns `None` (Rust unsupported subtyping,
//!     tuple-shaped or alias-carrying operands).
//!   * A snapshot missing from the resolver (cannot distinguish "absent"
//!     from "unknown").

use pyo3::prelude::*;

use crate::operators::get_reverse_op_method;
use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{read_type, ReadBuffer, Type};

/// Operator method names in `mypy.operators.op_methods_that_shortcut`
/// (operators.py:83-84, operators.rs:122-137). For these, Python only
/// calls `__op__` when left and right are the same type.
const SHORTCUT_OPS: &[&str] = &[
    "__add__",
    "__sub__",
    "__mul__",
    "__truediv__",
    "__mod__",
    "__divmod__",
    "__floordiv__",
    "__pow__",
    "__matmul__",
    "__and__",
    "__or__",
    "__xor__",
    "__lshift__",
    "__rshift__",
];

/// Return codes for `rust_check_operator`. Mirror the `variants_raw`
/// construction branches in `check_op_reversible` (checkexpr.py:4702-4732).
pub(crate) const OP_VARIANT_SHORTCUT_SINGLE: u8 = 0;
pub(crate) const OP_VARIANT_REVERSE_FIRST: u8 = 1;
pub(crate) const OP_VARIANT_NORMAL: u8 = 2;

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    read_type(&mut buf, None).ok()
}

/// Whether `op_name` is an operator method that shortcuts same-type args.
/// Mirrors membership in `operators.op_methods_that_shortcut`.
fn is_shortcut_op(op_name: &str) -> bool {
    SHORTCUT_OPS.contains(&op_name)
}

/// `mypy.checkexpr.ExpressionChecker.lookup_definer` (checkexpr.py:4654-4665).
///
/// Returns `Some(Some(fullname))` of the first MRO class defining `name`,
/// `Some(None)` when no class defines it, and `None` (defer) when the
/// class or any MRO ancestor snapshot is missing from the resolver.
fn lookup_definer(resolver: &TypeResolver, type_ref: &str, name: &str) -> Option<Option<String>> {
    let snap = resolver.get(type_ref)?;
    for base in &snap.mro {
        let b = resolver.get(base)?;
        if b.member_info.contains_key(name) {
            return Some(Some(base.clone()));
        }
    }
    Some(None)
}

/// `mypy.subtypes.is_same_type` (subtypes.py:303-336) on the wire format.
///
/// Fast path: both `Instance`, same type_ref, equal arg length, both
/// `last_known_value` absent (Python compares `is` identity), then compare
/// args pairwise. `None` from any recursive `is_same_type` defers (via `?`).
///
/// The TypeVarType fast path is intentionally skipped: it falls through to
/// the two-way `is_proper_subtype` slow path, which resolves it correctly
/// for recurrences that matter at the top level here.
fn is_same_type(
    left: &Type,
    right: &Type,
    resolver: &TypeResolver,
    strict_optional: bool,
) -> Option<bool> {
    if let (
        Type::Instance {
            type_ref: l,
            args: la,
            last_known_value: lk,
            ..
        },
        Type::Instance {
            type_ref: r,
            args: ra,
            last_known_value: rk,
            ..
        },
    ) = (left, right)
    {
        if l == r && la.len() == ra.len() && lk.is_none() && rk.is_none() {
            let mut eq = true;
            for (x, y) in la.iter().zip(ra.iter()) {
                if !is_same_type(x, y, resolver, strict_optional)? {
                    eq = false;
                    break;
                }
            }
            return Some(eq);
        }
    }
    // Slow path: proper subtype both ways, ignore_promotions=True
    // (is_same_type default) via SubtypeContext.
    let ctx = SubtypeContext::new(false, false, false, true, true, strict_optional);
    let fwd = is_subtype(left, right, &ctx, resolver)?;
    let rev = is_subtype(right, left, &ctx, resolver)?;
    Some(fwd && rev)
}

/// Decide `check_op_reversible` STEP 2a variant ordering (see module doc).
///
/// `op_name` is the non-reversed operator method (`__add__`, ...). Returns
/// `Some(0)` for shortcut-single, `Some(1)` for reverse-first, `Some(2)`
/// for normal order, or `None` to defer to the pure-Python chain.
pub(crate) fn operator_plan_inner(
    op_name: &str,
    left: &Type,
    right: &Type,
    resolver: &TypeResolver,
    strict_optional: bool,
) -> Option<u8> {
    // checkexpr.py:4660-4666: if either operand is Any, Python returns any
    // before any ordering. Defer so Python's early return stands.
    if matches!(left, Type::AnyType { .. }) || matches!(right, Type::AnyType { .. }) {
        return None;
    }
    // TypeAliasType: needs get_proper_type expansion (no wire target). Defer.
    if matches!(left, Type::TypeAliasType { .. }) || matches!(right, Type::TypeAliasType { .. }) {
        return None;
    }

    if is_shortcut_op(op_name) {
        match is_same_type(left, right, resolver, strict_optional) {
            Some(true) => {
                return Some(OP_VARIANT_SHORTCUT_SINGLE);
            }
            Some(false) => {}
            None => {
                return None;
            }
        }
    }

    // checkexpr.py:4702-4732. For a non-Instance pair the code-2 elif's
    // `not (both instances)` disjunct provably fires, and the decision
    // reduces to covers_at_runtime(right, left) (#19006).

    // The same reduction applies for differing definers behind the
    // alt_promote check.
    if !matches!(left, Type::Instance { .. }) || !matches!(right, Type::Instance { .. }) {
        return match crate::covers_at_runtime::covers_at_runtime_inner(
            right,
            left,
            strict_optional,
            resolver,
        ) {
            Some(false) => Some(OP_VARIANT_NORMAL),
            Some(true) => Some(OP_VARIANT_REVERSE_FIRST),
            None => None,
        };
    }
    let l_ref = if let Type::Instance { type_ref, .. } = left {
        type_ref.as_str()
    } else {
        unreachable!("checked Instance above")
    };
    let r_ref = if let Type::Instance { type_ref, .. } = right {
        type_ref.as_str()
    } else {
        unreachable!("checked Instance above")
    };

    let rev_op = get_reverse_op_method(op_name).unwrap_or(op_name);

    let ldef = lookup_definer(resolver, l_ref, op_name);
    let rdef = lookup_definer(resolver, r_ref, rev_op);
    if ldef.is_none() || rdef.is_none() {
        return None;
    }
    if ldef == rdef {
        // checkexpr.py:4712-4713: definers equal -> normal order.
        return Some(OP_VARIANT_NORMAL);
    }
    // alt_promote special case (checkexpr.py:4714-4716): left's alt_promote
    // is the right's type_ref -> NOT subclass, even though definers differ.
    let left_alt = match resolver.get(l_ref) {
        Some(s) => s.alt_promote_fullname.as_deref(),
        None => {
            return None;
        }
    };
    if left_alt == Some(r_ref) {
        return Some(OP_VARIANT_NORMAL);
    }
    // Differing definers and no alt_promote equality -> Python evaluates
    // covers_at_runtime (reverse-first candidate), #19006.
    match crate::covers_at_runtime::covers_at_runtime_inner(right, left, strict_optional, resolver)
    {
        Some(false) => Some(OP_VARIANT_NORMAL),
        Some(true) => Some(OP_VARIANT_REVERSE_FIRST),
        None => None,
    }
}

/// PyO3 seam for the `check_op_reversible` STEP 2a ordering decision.
///
/// Returns an `int | None` order code: 0 = shortcut-single, 1 = reverse-first,
/// 2 = normal. `None` defers to the pure-Python chain.
#[pyfunction]
pub(crate) fn rust_check_operator(
    resolver: &NativeTypeResolver,
    op_name: &str,
    left_bytes: &[u8],
    right_bytes: &[u8],
    strict_optional: bool,
) -> PyResult<Option<u8>> {
    let left = match decode_type(left_bytes) {
        Some(t) => t,
        None => {
            return Ok(None);
        }
    };
    let right = match decode_type(right_bytes) {
        Some(t) => t,
        None => {
            return Ok(None);
        }
    };
    Ok(operator_plan_inner(
        op_name,
        &left,
        &right,
        resolver.resolver(),
        strict_optional,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    fn make_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn any_type() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn snap(fullname: &str, name: &str) -> TypeInfoSnapshot {
        // Real TypeInfo always has its own fullname in mro and has_base.
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        s
    }

    fn plan(op: &str, l: &Type, r: &Type, resolver: &TypeResolver) -> Option<u8> {
        operator_plan_inner(op, l, r, resolver, true)
    }

    #[test]
    fn shortcut_same_instance_is_single() {
        // "A() + A()" -> only __add__ is called (code 0).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let a = make_instance("a.A", vec![]);
        assert_eq!(plan("__add__", &a, &a, &r), Some(0));
    }

    #[test]
    fn shortcut_same_generic_instance_with_equal_args_is_single() {
        // "A[int]() + A[int]()" (fast path: equal type_ref, equal args).
        let mut gen = snap("a.Gen", "Gen");
        gen.member_info
            .entry("__add__".to_string())
            .or_insert((false, true));
        let r = make_resolver(vec![gen, snap("builtins.int", "int")]);
        let i = make_instance("builtins.int", vec![]);
        let l = make_instance("a.Gen", vec![i.clone()]);
        let rr = make_instance("a.Gen", vec![i]);
        assert_eq!(plan("__add__", &l, &rr, &r), Some(0));
    }

    #[test]
    fn shortcut_missing_definer_still_single_same_type() {
        // Even if __add__ is missing entirely, same-type shortcut holds.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let a = make_instance("a.A", vec![]);
        assert_eq!(plan("__add__", &a, &a, &r), Some(0));
    }

    #[test]
    fn shortcut_different_instances_normal_order() {
        // A + B, not same type, definers differ (B's __radd__ undefined in
        // the snapshots), and covers_at_runtime(B, A) is False -> the else
        // branch: normal order (code 2). Pre-#1131 the seam deferred here.
        let mut a = snap("a.A", "A");
        a.member_info
            .entry("__add__".to_string())
            .or_insert((false, true));
        let r = make_resolver(vec![a, snap("a.B", "B")]);
        let la = make_instance("a.A", vec![]);
        let rb = make_instance("a.B", vec![]);
        assert_eq!(plan("__add__", &la, &rb, &r), Some(2));
    }

    #[test]
    fn definer_equal_both_instances_normal() {
        // A.__add__ == B.__radd__ (both defined in builtins.object) ->
        // normal order (code 2).
        let mut obj = snap("builtins.object", "object");
        obj.member_info = {
            let mut m = std::collections::HashMap::new();
            m.insert("__add__".to_string(), (false, true));
            m.insert("__radd__".to_string(), (false, true));
            m
        };
        let mut a = snap("a.A", "A");
        a.mro.push("builtins.object".to_string());
        let mut b = snap("a.B", "B");
        b.mro.push("builtins.object".to_string());
        let r = make_resolver(vec![obj, a, b]);
        let la = make_instance("a.A", vec![]);
        let rb = make_instance("a.B", vec![]);
        assert_eq!(plan("__add__", &la, &rb, &r), Some(2));
    }

    #[test]
    fn alt_promote_equals_right_normal() {
        // left's alt_promote is right's type_ref -> NOT subclass -> code 2
        // (checkexpr.py:4714-4716).
        let mut int = snap("builtins.int", "int");
        int.alt_promote_fullname = Some("builtins.something".to_string());
        let mut thing = snap("builtins.something", "something");
        thing
            .member_info
            .entry("__add__".to_string())
            .or_insert((false, true));
        let r = make_resolver(vec![int, thing]);
        let l = make_instance("builtins.int", vec![]);
        let rr = make_instance("builtins.something", vec![]);
        assert_eq!(plan("__add__", &l, &rr, &r), Some(2));
    }

    #[test]
    fn non_instance_operand_normal_order() {
        // None + A: __add__ is a shortcut op but the operands differ, and
        // covers_at_runtime(A, None) is False -> normal order (code 2).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let n = Type::NoneType;
        let a = make_instance("a.A", vec![]);
        assert_eq!(plan("__add__", &n, &a, &r), Some(2));
    }

    #[test]
    fn any_operand_defers() {
        let r = make_resolver(vec![snap("a.A", "A")]);
        let a = make_instance("a.A", vec![]);
        assert_eq!(plan("__add__", &any_type(), &a, &r), None);
        assert_eq!(plan("__add__", &a, &any_type(), &r), None);
    }

    #[test]
    fn missing_snapshot_defers() {
        // Right type_ref has no snapshot -> lookup_definer defers.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let la = make_instance("a.A", vec![]);
        let rb = make_instance("a.B", vec![]);
        assert_eq!(plan("__add__", &la, &rb, &r), None);
    }

    #[test]
    fn non_shortcut_op_still_normal_path() {
        // __eq__ is not a shortcut op: skips code 0, goes both-instance.
        // Equivalent definers -> code 2.
        let mut obj = snap("builtins.object", "object");
        obj.member_info = {
            let mut m = std::collections::HashMap::new();
            m.insert("__eq__".to_string(), (false, true));
            m.insert("__ne__".to_string(), (false, true));
            m
        };
        let mut a = snap("a.A", "A");
        a.mro.push("builtins.object".to_string());
        let mut b = snap("a.B", "B");
        b.mro.push("builtins.object".to_string());
        let r = make_resolver(vec![obj, a, b]);
        let la = make_instance("a.A", vec![]);
        let rb = make_instance("a.B", vec![]);
        assert_eq!(plan("__eq__", &la, &rb, &r), Some(2));
    }

    #[test]
    fn lookup_definer_walks_mro() {
        // B(->A) defines __add__ in A, B.__radd__ in A too -> equal.
        let mut a = snap("a.A", "A");
        a.member_info = {
            let mut m = std::collections::HashMap::new();
            m.insert("__add__".to_string(), (false, true));
            m.insert("__radd__".to_string(), (false, true));
            m
        };
        let mut b = snap("a.B", "B");
        b.mro.push("a.A".to_string());
        let r = make_resolver(vec![a, b]);
        let la = make_instance("a.A", vec![]);
        let rb = make_instance("a.B", vec![]);
        assert_eq!(plan("__add__", &la, &rb, &r), Some(2));
    }

    #[test]
    fn non_instance_covers_reverse_first() {
        // TypedDict == dict: __eq__ is not a shortcut op, neither operand
        // deferral applies, and covers_at_runtime(right, left) is True
        // (typed dict instances are always dicts at runtime) -> code 1.
        let res = make_resolver(vec![
            snap("builtins.dict", "dict"),
            snap("builtins.str", "str"),
        ]);
        let dict = make_instance("builtins.dict", vec![]);
        let td = Type::TypedDictType {
            fallback: Box::new(dict.clone()),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        assert_eq!(plan("__eq__", &dict, &td, &res), Some(1));
    }

    #[test]
    fn non_instance_not_covers_normal() {
        // TypedDict == str: covers_at_runtime(TypedDict, str) is False
        // (isinstance(td_value, str) never holds) -> code 2.
        let res = make_resolver(vec![
            snap("builtins.dict", "dict"),
            snap("builtins.str", "str"),
        ]);
        let td = Type::TypedDictType {
            fallback: Box::new(make_instance("builtins.dict", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: true,
        };
        let str_inst = make_instance("builtins.str", vec![]);
        assert_eq!(plan("__eq__", &str_inst, &td, &res), Some(2));
    }

    #[test]
    fn differing_definers_right_subclass_reverse_first() {
        // A.__add__ and B.__radd__ are defined in different classes and B
        // is a subclass of A -> Python evaluates covers_at_runtime(B, A),
        // which is True -> reverse-first (code 1).
        let mut a = snap("a.A", "A");
        a.member_info
            .entry("__add__".to_string())
            .or_insert((false, true));
        let mut b = snap("b.B", "B");
        b.member_info
            .entry("__radd__".to_string())
            .or_insert((false, true));
        b.mro.push("a.A".to_string());
        b.mro.push("builtins.object".to_string());
        a.mro.push("builtins.object".to_string());
        b.has_base.insert("a.A".to_string());
        b.has_base.insert("builtins.object".to_string());
        a.has_base.insert("builtins.object".to_string());
        let r = make_resolver(vec![a, b, snap("builtins.object", "object")]);
        let la = make_instance("a.A", vec![]);
        let rb = make_instance("b.B", vec![]);
        assert_eq!(plan("__add__", &la, &rb, &r), Some(1));
    }

    #[test]
    fn differing_definers_not_covers_normal() {
        // Same definer setup as above but B is unrelated to A:
        // covers_at_runtime(B, A) is False -> normal order (code 2). The
        // pre-#1131 seam deferred here.
        let mut a = snap("a.A", "A");
        a.member_info
            .entry("__add__".to_string())
            .or_insert((false, true));
        let mut b = snap("b.B", "B");
        b.member_info
            .entry("__radd__".to_string())
            .or_insert((false, true));
        b.mro.push("builtins.object".to_string());
        a.mro.push("builtins.object".to_string());
        b.has_base.insert("builtins.object".to_string());
        a.has_base.insert("builtins.object".to_string());
        let r = make_resolver(vec![a, b, snap("builtins.object", "object")]);
        let la = make_instance("a.A", vec![]);
        let rb = make_instance("b.B", vec![]);
        assert_eq!(plan("__add__", &la, &rb, &r), Some(2));
    }
}
