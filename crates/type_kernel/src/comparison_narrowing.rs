//! Native port of the operand-classification front of
//! `mypy.checker.TypeChecker.comparison_type_narrowing_helper`
//! (checker.py:8579-8613), the Step-1 loop deciding which comparison
//! operands are narrowable.
//!
//! The Python shim computes the cheap AST literal facts (`literal(expr)`
//! kind, `is_literal_none` / `is_literal_not_implemented` /
//! `is_false_literal` / `is_true_literal` / `is_literal_enum`) and
//! serializes each operand type to the wire format; Rust owns the rest of
//! the conjunction: the `LITERAL_TYPE` gate, the five literal-kind
//! suppressions, and the two non-narrowable proper-type tests
//! (`FunctionLike.is_type_obj()`, `TypeType` over a `TypeVarType`).
//!
//! Returns one narrowability bool per operand, or `None` (defer) when any
//! operand cannot be classified: an undecodable wire blob, a `TypeAliasType`
//! operand whose alias snapshot is missing or whose target/substitution the
//! kernel cannot expand (chain cycle, ParamSpec/TypeVarTuple env, arg-count
//! mismatch), a length mismatch, or a type-object fact the resolver snapshot
//! cannot decide (fallback class not snapshotted yet). Alias operands with a
//! snapshot expand exactly like Python's `get_proper_type` (issue #1235).
//! The Python shim then re-runs the original pure-Python loop. Everything
//! downstream (literal-hash bookkeeping, chain grouping via
//! `rust_group_comparison_operands`, the narrowing arm bodies, TypeMap
//! returns) stays Python-side.

use pyo3::prelude::*;

use crate::checkmember::decode_type;
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::Type;

/// `mypy.nodes.LITERAL_TYPE` (nodes.py:207): the `literal(expr)` value
/// marking an operand whose type can be narrowed.
const LITERAL_TYPE_KIND: i64 = 1;

/// Pure decision core over already-decoded operand types. `operand_flags`
/// is one `(is_none, is_not_impl, is_false, is_true, is_enum)` tuple per
/// operand, precomputed by the Python shim; flags for operands whose kind
/// is not `LITERAL_TYPE` are placeholders (Rust never reads them, mirroring
/// the Python short-circuit).
pub(crate) fn classify_comparison_operands_inner(
    literal_kinds: &[i64],
    operand_flags: &[(bool, bool, bool, bool, bool)],
    operand_types: &[Type],
    aliases: &crate::aliases::TypeAliasResolver,
    resolver: &TypeResolver,
) -> Option<Vec<bool>> {
    if literal_kinds.len() != operand_flags.len() || literal_kinds.len() != operand_types.len() {
        return None;
    }
    let mut out = Vec::with_capacity(literal_kinds.len());
    for i in 0..literal_kinds.len() {
        // literal(expr) == LITERAL_TYPE gate: everything else is a no-op.
        if literal_kinds[i] != LITERAL_TYPE_KIND {
            out.push(false);
            continue;
        }
        let (is_none, is_not_impl, is_false, is_true, is_enum) = operand_flags[i];
        if is_none || is_not_impl || is_false || is_true || is_enum {
            out.push(false);
            continue;
        }
        let proper = crate::checkexpr_functions::get_proper_or_expand(&operand_types[i], aliases)?;
        match &proper {
            // CallableType type objects are usually already maximally
            // specific, so they are not narrowable.
            Type::CallableType { .. } | Type::Overloaded { .. } => {
                match function_like_is_type_obj(&proper, resolver, aliases) {
                    Some(false) => out.push(true),
                    Some(true) => out.push(false),
                    None => return None,
                }
            }
            // TypeType over a TypeVar is not narrowable without
            // intersection types (checker.py:8607-8608).
            Type::TypeType { item, .. } => {
                out.push(!matches!(&**item, Type::TypeVarType { .. }));
            }
            _ => out.push(true),
        }
    }
    Some(out)
}

/// `FunctionLike.is_type_obj()`: `CallableType` via
/// `callable_compat::is_type_obj` (fallback.type.is_metaclass() and the
/// ret-type not uninhabited); `Overloaded` via `items[0]`. A `TypeAliasType`
/// ret-type expands through the alias snapshot like Python's
/// `get_proper_type` (issue #1235): expanding to `UninhabitedType` decides
/// `is_type_obj == False` (not narrowable), otherwise the decision falls
/// through to `is_type_obj` on the callable. Defers (`None`) when the
/// resolver has no snapshot for the fallback class, when an alias ret-type
/// cannot expand, or on a malformed shape.
fn function_like_is_type_obj(
    t: &Type,
    resolver: &TypeResolver,
    aliases: &crate::aliases::TypeAliasResolver,
) -> Option<bool> {
    let callable = match t {
        Type::CallableType { .. } => t,
        Type::Overloaded { items } => items.first()?,
        _ => return None,
    };
    let Type::CallableType { ret_type, .. } = callable else {
        return None;
    };
    if let Type::TypeAliasType { .. } = &**ret_type {
        let proper = crate::checkexpr_functions::get_proper_or_expand(ret_type, aliases)?;
        if matches!(proper, Type::UninhabitedType { .. }) {
            return Some(false);
        }
    }
    crate::callable_compat::is_type_obj(callable, resolver)
}

/// `#[pyfunction]` entry for the shim in `mypy/checker.py`. Decodes each
/// operand type from the wire format; any undecodable blob defers the whole
/// call (`None`), mirroring the Python `try/except` shim fallback.
#[pyfunction]
pub(crate) fn rust_classify_comparison_operands(
    literal_kinds: Vec<i64>,
    operand_flags: Vec<(bool, bool, bool, bool, bool)>,
    operand_wires: Vec<Vec<u8>>,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<bool>> {
    if literal_kinds.len() != operand_wires.len() {
        return None;
    }
    let mut operand_types = Vec::with_capacity(operand_wires.len());
    for bytes in &operand_wires {
        operand_types.push(decode_type(bytes)?);
    }
    let aliases = resolver.alias_resolver();
    classify_comparison_operands_inner(
        &literal_kinds,
        &operand_flags,
        &operand_types,
        aliases,
        resolver.resolver(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::aliases::{TypeAliasResolver, TypeAliasSnapshot};
    use crate::typeinfo::TypeInfoSnapshot;

    fn instance(type_ref: &str) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: Vec::new(),
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn tvar() -> Type {
        Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 0,
            namespace: "".to_string(),
            values: Vec::new(),
            upper_bound: Box::new(instance("builtins.object")),
            default: Box::new(instance("builtins.object")),
            variance: 0,
            meta_level: 0,
        }
    }

    fn callable(ret: Type, fallback_ref: &str) -> Type {
        Type::CallableType {
            fallback: Box::new(instance(fallback_ref)),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types: Vec::new(),
            arg_kinds: Vec::new(),
            arg_names: Vec::new(),
            ret_type: Box::new(ret),
            name: None,
            variables: Vec::new(),
            type_guard: None,
            type_is: None,
            special_sig: None,
        }
    }

    fn flags(lit: bool) -> (bool, bool, bool, bool, bool) {
        if lit {
            (false, false, false, false, false)
        } else {
            (false, false, false, false, true)
        }
    }

    fn run(
        kinds: &[i64],
        flags: &[(bool, bool, bool, bool, bool)],
        types: &[Type],
        aliases: &crate::aliases::TypeAliasResolver,
        resolver: &TypeResolver,
    ) -> Option<Vec<bool>> {
        classify_comparison_operands_inner(kinds, flags, types, aliases, resolver)
    }

    fn no_aliases() -> TypeAliasResolver {
        TypeAliasResolver::new()
    }

    fn alias_resolver(fullname: &str, target: &Type) -> TypeAliasResolver {
        let mut r = TypeAliasResolver::new();
        r.insert(
            fullname.to_string(),
            TypeAliasSnapshot {
                fullname: fullname.to_string(),
                target: crate::checkmember::encode_type(target).unwrap(),
                ..Default::default()
            },
        );
        r
    }

    #[test]
    fn test_plain_instance_narrowable() {
        let types = vec![instance("builtins.int"), instance("builtins.str")];
        assert_eq!(
            run(
                &[1, 1],
                &[flags(true), flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            Some(vec![true, true])
        );
    }

    #[test]
    fn test_kind_gate_suppresses() {
        // literal(expr) != LITERAL_TYPE: not narrowable, other facts never
        // consulted (flags are placeholders).
        let types = vec![instance("builtins.int"), instance("builtins.str")];
        assert_eq!(
            run(
                &[0, 2],
                &[flags(true), flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            Some(vec![false, false])
        );
    }

    #[test]
    fn test_literal_kind_flags_suppress() {
        let types = vec![instance("builtins.int")];
        for f in [
            (true, false, false, false, false),
            (false, true, false, false, false),
            (false, false, true, false, false),
            (false, false, false, true, false),
            (false, false, false, false, true),
        ] {
            assert_eq!(
                run(&[1], &[f], &types, &no_aliases(), &TypeResolver::new()),
                Some(vec![false]),
                "flags {f:?} must suppress narrowing"
            );
        }
    }

    #[test]
    fn test_type_object_not_narrowable() {
        // callable with a builtins.type fallback = a type object.
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "builtins.type".to_string(),
            name: "type".to_string(),
            ..Default::default()
        };
        snap.has_base.insert("builtins.type".to_string());
        resolver.insert("builtins.type".to_string(), snap);
        let types = vec![callable(instance("builtins.object"), "builtins.type")];
        assert_eq!(
            run(&[1], &[flags(true)], &types, &no_aliases(), &resolver),
            Some(vec![false])
        );
    }

    #[test]
    fn test_non_typeobj_callable_narrowable() {
        // A plain function (fallback builtins.function) is not a type
        // object, so the operand stays narrowable.
        let mut resolver = TypeResolver::new();
        let snap = TypeInfoSnapshot {
            fullname: "builtins.function".to_string(),
            name: "function".to_string(),
            ..Default::default()
        };
        resolver.insert("builtins.function".to_string(), snap);
        let types = vec![callable(instance("builtins.object"), "builtins.function")];
        assert_eq!(
            run(&[1], &[flags(true)], &types, &no_aliases(), &resolver),
            Some(vec![true])
        );
    }

    #[test]
    fn test_typeobj_uninhabited_ret_narrowable() {
        // is_type_obj() is False when ret_type is UninhabitedType.
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "builtins.type".to_string(),
            name: "type".to_string(),
            ..Default::default()
        };
        snap.has_base.insert("builtins.type".to_string());
        resolver.insert("builtins.type".to_string(), snap);
        let types = vec![callable(
            Type::UninhabitedType { ambiguous: false },
            "builtins.type",
        )];
        assert_eq!(
            run(&[1], &[flags(true)], &types, &no_aliases(), &resolver),
            Some(vec![true])
        );
    }

    #[test]
    fn test_typeobj_unresolved_fallback_defers() {
        // No snapshot for the fallback class: Python's is_type_obj cannot
        // decide either; the whole call defers.
        let types = vec![callable(instance("builtins.object"), "builtins.type")];
        assert_eq!(
            run(
                &[1],
                &[flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            None
        );
    }

    #[test]
    fn test_overloaded_typeobj_not_narrowable() {
        // Overloaded.is_type_obj() = items[0].is_type_obj().
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "builtins.type".to_string(),
            name: "type".to_string(),
            ..Default::default()
        };
        snap.has_base.insert("builtins.type".to_string());
        resolver.insert("builtins.type".to_string(), snap);
        let types = vec![Type::Overloaded {
            items: vec![callable(instance("builtins.object"), "builtins.type")],
        }];
        assert_eq!(
            run(&[1], &[flags(true)], &types, &no_aliases(), &resolver),
            Some(vec![false])
        );
    }

    #[test]
    fn test_type_type_over_typevar_not_narrowable() {
        let types = vec![Type::TypeType {
            item: Box::new(tvar()),
            is_type_form: false,
        }];
        assert_eq!(
            run(
                &[1],
                &[flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            Some(vec![false])
        );
    }

    #[test]
    fn test_type_type_over_instance_narrowable() {
        let types = vec![Type::TypeType {
            item: Box::new(instance("builtins.int")),
            is_type_form: false,
        }];
        assert_eq!(
            run(
                &[1],
                &[flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            Some(vec![true])
        );
    }

    #[test]
    fn test_alias_operand_defers() {
        // No alias snapshot: Python's get_proper_type would expand from the
        // live node; the kernel cannot decide, so the call defers.
        let types = vec![Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.Alias".to_string(),
        }];
        assert_eq!(
            run(
                &[1],
                &[flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            None
        );
    }

    #[test]
    fn test_alias_operand_expands_to_instance() {
        // A snapshotted alias operand expands like get_proper_type (issue
        // #1235); expanding to a plain Instance is narrowable.
        let aliases = alias_resolver("mod.Alias", &instance("builtins.int"));
        let types = vec![Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.Alias".to_string(),
        }];
        assert_eq!(
            run(&[1], &[flags(true)], &types, &aliases, &TypeResolver::new()),
            Some(vec![true])
        );
    }

    #[test]
    fn test_alias_operand_expands_to_type_object() {
        // Expanding to a type-object callable with a builtins.type fallback
        // is not narrowable.
        let mut resolver = TypeResolver::new();
        let mut snap = TypeInfoSnapshot {
            fullname: "builtins.type".to_string(),
            name: "type".to_string(),
            ..Default::default()
        };
        snap.has_base.insert("builtins.type".to_string());
        resolver.insert("builtins.type".to_string(), snap);
        let aliases = alias_resolver(
            "mod.Alias",
            &callable(instance("builtins.object"), "builtins.type"),
        );
        let types = vec![Type::TypeAliasType {
            args: Vec::new(),
            type_ref: "mod.Alias".to_string(),
        }];
        assert_eq!(
            run(&[1], &[flags(true)], &types, &aliases, &resolver),
            Some(vec![false])
        );
    }

    #[test]
    fn test_alias_ret_expands_to_uninhabited() {
        // A snapshotted alias ret-type expanding to UninhabitedType decides
        // is_type_obj == False (via the expansion fallback) without needing
        // a fallback-class snapshot.
        let aliases = alias_resolver("mod.Evil", &Type::UninhabitedType { ambiguous: false });
        let types = vec![callable(
            Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "mod.Evil".to_string(),
            },
            "builtins.function",
        )];
        assert_eq!(
            run(&[1], &[flags(true)], &types, &aliases, &TypeResolver::new()),
            Some(vec![true])
        );
    }

    #[test]
    fn test_alias_ret_missing_snapshot_defers() {
        // A ret-type alias without a snapshot still defers: Python expands
        // it from the live alias node.
        let types = vec![callable(
            Type::TypeAliasType {
                args: Vec::new(),
                type_ref: "mod.Missing".to_string(),
            },
            "builtins.function",
        )];
        assert_eq!(
            run(
                &[1],
                &[flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            None
        );
    }

    #[test]
    fn test_length_mismatch_defers() {
        let types = vec![instance("builtins.int")];
        assert_eq!(
            run(
                &[1, 1],
                &[flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            None
        );
        assert_eq!(
            run(
                &[1],
                &[flags(true), flags(true)],
                &types,
                &no_aliases(),
                &TypeResolver::new()
            ),
            None
        );
    }

    #[test]
    fn test_empty_operands() {
        assert_eq!(
            run(&[], &[], &[], &no_aliases(), &TypeResolver::new()),
            Some(vec![])
        );
    }

    #[test]
    fn test_wire_round_trip() {
        // The #[pyfunction] path decodes wire bytes before classifying.
        let types = [instance("builtins.int"), instance("builtins.str")];
        let wires: Vec<Vec<u8>> = types
            .iter()
            .map(|t| crate::checkmember::encode_type(t).unwrap())
            .collect();
        let mut resolver = NativeTypeResolver::from_resolver(TypeResolver::new());
        assert_eq!(
            rust_classify_comparison_operands(
                vec![1, 1],
                vec![flags(true), flags(true)],
                wires,
                &mut resolver
            ),
            Some(vec![true, true])
        );
    }
}
