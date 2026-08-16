//! Stage 3c (M8b): nominal-instance `is_subtype` on the Rust `Type` enum.
//!
//! Ports the `visit_instance` `isinstance(right, Instance)` branch of
//! `mypy.subtypes.SubtypeVisitor` (subtypes.py:531-626) plus the shared
//! `_is_subtype` worker entry (subtypes.py:295-376) for the subset each
//! handles. Returns `None` (fall through to Python) for every variant the
//! nominal path does not cover: `TypeAliasType`, `TypeVarTupleType`
//! variadic, protocol right, `find_member` path, `TupleType` right,
//! `TypeType` right, `LiteralType` right with lkv, `FunctionLike` right,
//! `PartialType` left, and the generic `map_instance_to_supertype` path
//! (which needs `expand_type_by_instance`, deferred to M8c).
//!
//! The strangler-fig contract mirrors `erase::erase_type`
//! (erasetype.py:80-86): `None` means "Rust doesn't handle this, let
//! Python decide". No production code calls this until
//! `Options.native_type_kernel` is on AND `mypy/subtypes.py` dispatches
//! to it (the shim is added in this same milestone).

use pyo3::prelude::*;
use std::collections::HashSet;

use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

/// Variance constants mirroring `mypy.nodes` (nodes.py:3146).
pub(crate) const INVARIANT: i64 = 0;
pub(crate) const COVARIANT: i64 = 1;
pub(crate) const CONTRAVARIANT: i64 = 2;
pub(crate) const VARIANCE_NOT_READY: i64 = 3;

/// `TypeOfAny.special_form` (types.py) — used when synthesizing an
/// `AnyType` for a tuple-like right without an explicit iter type.
const ANY_SPECIAL_FORM: i64 = 6;

/// `mypy.typeops.is_named_instance` (typeops.py:636-640).
pub(crate) fn is_named_instance(t: &Type, name: &str) -> bool {
    matches!(t, Type::Instance { type_ref, .. } if type_ref == name)
}

/// `get_proper_type` for wire `Type`s: TypeAliasType is the only variant
/// that needs expansion (which the Rust side cannot do); every other
/// variant is already proper. Returns None (defer) for aliases.
fn get_proper_type_or_defer<'a>(typ: &'a Type, _resolver: &'a TypeResolver) -> Option<&'a Type> {
    match typ {
        Type::TypeAliasType { .. } => None,
        t => Some(t),
    }
}

fn any_type(type_of_any: i64) -> Type {
    Type::AnyType {
        type_of_any,
        source_any: None,
        missing_import_name: None,
    }
}

/// Mirrors `mypy.subtypes.SubtypeContext` (subtypes.py:90-122). Only the
/// flags the nominal-instance path reads are carried; the rest stay
/// Python-side (the shim passes them through unchanged when Rust
/// returns `None`).
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct SubtypeContext {
    pub ignore_type_params: bool,
    pub ignore_declared_variance: bool,
    pub always_covariant: bool,
    pub ignore_promotions: bool,
    pub proper_subtype: bool,
    pub strict_optional: bool,
}

impl SubtypeContext {
    pub(crate) fn new(
        ignore_type_params: bool,
        ignore_declared_variance: bool,
        always_covariant: bool,
        ignore_promotions: bool,
        proper_subtype: bool,
        strict_optional: bool,
    ) -> Self {
        Self {
            ignore_type_params,
            ignore_declared_variance,
            always_covariant,
            ignore_promotions,
            proper_subtype,
            strict_optional,
        }
    }
}

/// Entry point mirroring `mypy.subtypes._is_subtype` for the nominal path.
///
/// Returns `Some(bool)` when Rust decided the check; `None` when the
/// variant is not handled (Python falls through). The Python shim is
/// responsible for `get_proper_type` expansion, the `AnyType`/`UnboundType`/
/// `ErasedType` right short-circuit (subtypes.py:306-313), the `UnionType`
/// right dispatch (subtypes.py:317-364), the `TypeVarType`-with-values
/// right (subtypes.py:366-374), and the `assuming` recursion guard
/// (subtypes.py:167-189) BEFORE calling this.
#[allow(dead_code)]
pub(crate) fn is_subtype(
    left: &Type,
    right: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    // subtypes.py:352-359: non-proper subtype of Any/Unbound/Erased is
    // always True (unless left is UnpackType, which the wire format
    // doesn't produce in this recursive path). The Python shim handles
    // this at the top-level entry, but recursive calls from
    // check_type_parameter and the internal visit_* recursions
    // (e.g. TypeType-vs-TypeType recursing on items, subtypes.py:1323)
    // bypass the shim, so we must mirror it here. UnboundType right is
    // produced by forward references ("A?") that survive get_proper_type;
    // ErasedType has no wire representation (left/right args are never
    // Erased after the shim filters them).
    if matches!(left, Type::TypeAliasType { .. }) || matches!(right, Type::TypeAliasType { .. }) {
        // Python expands both operands with get_proper_type before every
        // comparison (subtypes.py:346-347); recursive check_type_parameter
        // and callable-compat paths bypass that, so defer unexpanded aliases.
        return None;
    }
    // TupleType left: handled by the visit_tuple_type port below, which
    // returns None for the variadic cases (Unpack items, TypeVarTuple)
    // that still defer to Python's SubtypeVisitor (subtypes.py:950-1037).
    if !ctx.proper_subtype && matches!(right, Type::AnyType { .. } | Type::UnboundType { .. }) {
        return Some(true);
    }
    // _is_subtype (subtypes.py:363-410): when right is UnionType and
    // left is not, left <: right iff left <: some item. Python handles
    // this BEFORE the visitor dispatch; mirror it here so recursive
    // calls from check_type_parameter (which bypass the Python shim)
    // get the right answer for union-typed type arguments. Must fire
    // before the NoneType handler (visit_none_type returns False for
    // UnionType right, but Python's _is_subtype short-circuit would
    // have already found None <: some union item).
    if let Type::UnionType { items, .. } = right {
        if !matches!(left, Type::UnionType { .. }) {
            if matches!(left, Type::TypeVarType { .. }) {
                // TypeVarType left: Python falls through to the visitor
                // (may match via upper_bound). Defer to preserve that.
                return None;
            }
            let mut all_decided_false = true;
            for item in items {
                match is_subtype(left, item, ctx, resolver) {
                    Some(true) => return Some(true),
                    None => {
                        all_decided_false = false;
                    }
                    Some(false) => {}
                }
            }
            if all_decided_false {
                return Some(false);
            }
            return None;
        }
    }
    // visit_uninhabited_type (subtypes.py:555-556): UninhabitedType is
    // a subtype of everything (bottom type). Fires before any right-side
    // dispatch because the Python visitor's `accept` lands on
    // `visit_uninhabited_type` regardless of `self.right`.
    if matches!(left, Type::UninhabitedType { .. }) {
        return Some(true);
    }
    // visit_deleted_type (subtypes.py:564): DeletedType is a subtype of
    // everything (same rationale as UninhabitedType).
    if matches!(left, Type::DeletedType { .. }) {
        return Some(true);
    }
    // visit_none_type (subtypes.py:539-554). The shim already passes
    // state.strict_optional as ctx.strict_optional.
    if matches!(left, Type::NoneType) {
        if !ctx.strict_optional {
            // subtypes.py:553-554: when strict_optional is off, None
            // is a subtype of everything.
            return Some(true);
        }
        return match right {
            // subtypes.py:539-541: right is NoneType or builtins.object
            // -> True.
            Type::NoneType => Some(true),
            Type::Instance { type_ref, .. } if type_ref == "builtins.object" => Some(true),
            Type::Instance { type_ref, .. } => {
                // subtypes.py:543-549: right is a protocol Instance.
                // When all protocol_members are __hash__/__str__ (or
                // members is empty), Python returns True; else False.
                // Non-protocol Instance -> False (subtypes.py:551).
                let snap = resolver.get(type_ref)?;
                if snap.is_protocol {
                    let ok = snap.protocol_members.is_empty()
                        || snap
                            .protocol_members
                            .iter()
                            .all(|m| m == "__hash__" || m == "__str__");
                    Some(ok)
                } else {
                    Some(false)
                }
            }
            // Any other right (CallableType, TupleType, UnionType, etc.)
            // falls to the `return False` at subtypes.py:551.
            _ => Some(false),
        };
    }
    // visit_literal_type (subtypes.py:1068-1072): when both sides are
    // LiteralType, subtype is structural equality. Needed by the
    // `_remove_redundant_union_items` dedup pass for unions like
    // [Literal[True], Literal[False]] (neither is a subtype of the
    // other, so dedup keeps both before literal contraction collapses
    // them to `bool`).
    if let (Type::LiteralType { .. }, Type::LiteralType { .. }) = (left, right) {
        return Some(left == right);
    }
    // visit_literal_type else-branch (subtypes.py:1072):
    // is_subtype(LiteralType, right) = is_subtype(lit.fallback, right)
    // for any non-LiteralType right. Python's SubtypeVisitor dispatches
    // on left (LiteralType) and recurses into the fallback Instance for
    // every right class except LiteralType (handled above by
    // structural equality), so mirror the full else-branch, not just
    // the Instance-right case.
    if let Type::LiteralType { fallback, .. } = left {
        return is_subtype(fallback, right, ctx, resolver);
    }
    // visit_instance vs LiteralType right (subtypes.py:724-728): only
    // fires when left.last_known_value is Some, recursing into
    // is_subtype(left.last_known_value, right). When lkv is None,
    // Instance is NOT a subtype of LiteralType (falls to else: False).
    if let Type::LiteralType { .. } = right {
        if let Type::Instance {
            last_known_value: Some(lkv),
            ..
        } = left
        {
            return is_subtype(lkv.as_ref(), right, ctx, resolver);
        }
        if let Type::Instance {
            last_known_value: None,
            ..
        } = left
        {
            return Some(false);
        }
    }
    // visit_type_var (subtypes.py:735-748), fast path only. When both
    // sides are TypeVarType with the same id (raw_id + namespace, per
    // TypeVarId.__eq__ types.py:567-577; meta_level is not in the wire
    // format) and the same upper_bound, Python returns True. The
    // values-with-upper_bound and upper_bound-recursion branches
    // produce results that need a deeper walker; defer those.
    //
    // This fast path is what makes is_equivalent_callable return
    // Some(true) for `def f[T](x: T) -> T` vs `def g[T](x: T) -> T`
    // after match_generic_callables renumbers both T's to the same id.
    if let Type::TypeVarType {
        raw_id: l_raw,
        namespace: l_ns,
        upper_bound: l_ub,
        values: l_values,
        ..
    } = left
    {
        if let Type::TypeVarType {
            raw_id: r_raw,
            namespace: r_ns,
            upper_bound: r_ub,
            ..
        } = right
        {
            if l_raw == r_raw && l_ns == r_ns {
                if l_ub == r_ub {
                    return Some(true);
                }
                // is_self (raw_id == 0, typing.Self): subtypes.py:858.
                if *l_raw == 0 {
                    return Some(true);
                }
                // Recurse on upper bounds (subtypes.py:860).
                return is_subtype(l_ub.as_ref(), r_ub.as_ref(), ctx, resolver);
            }
            // Different id: Python checks `left.values` then falls back
            // to `_is_subtype(left.upper_bound, right)`. Convert values
            // to a UnionType and try it; if that decides True, done.
            // Otherwise fall through to the upper-bound comparison.
            if !l_values.is_empty() {
                let union = Type::UnionType {
                    items: l_values.clone(),
                    uses_pep604_syntax: false,
                    can_be_true: true,
                    can_be_false: true,
                };
                if is_subtype(&union, right, ctx, resolver) == Some(true) {
                    return Some(true);
                }
            }
            return is_subtype(l_ub.as_ref(), right, ctx, resolver);
        }
        // right not TypeVarType: Python checks `left.values` then
        // `_is_subtype(left.upper_bound, right)`. Same UnionType
        // approach as the different-id branch above.
        if !l_values.is_empty() {
            let union = Type::UnionType {
                items: l_values.clone(),
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            };
            if is_subtype(&union, right, ctx, resolver) == Some(true) {
                return Some(true);
            }
        }
        return is_subtype(l_ub.as_ref(), right, ctx, resolver);
    }
    // visit_instance (subtypes.py:567-710) when right is TypeVarType:
    // Python falls through to `return False` (line 710) since right is
    // not Instance/TupleType/TypeVarTupleType/TypeType. Mirror that
    // for the common case (left=Instance, right=TypeVarType). The
    // protocol/TypeType branches are not reachable here (right is
    // TypeVarType, not those).
    // visit_typeddict_type (subtypes.py:1039-1096): structural TypedDict
    // subtyping. Previously used `left == right` (nominal equality including
    // fallback), which rejected two structurally identical TypedDicts with
    // different fallback type_refs (e.g. ast_serialize.ParseError vs
    // mypy.nodes.ParseError). Python's structural check ignores fallbacks
    // (subtypes.py:1093) and compares items, required_keys, readonly_keys,
    // and is_closed.
    if let Type::TypedDictType { .. } = left {
        return visit_typeddict_subtype(left, right, ctx, resolver);
    }
    if let Type::Instance { .. } = left {
        if let Type::TypeVarType { .. } = right {
            return Some(false);
        }
    }
    // visit_unbound_type (subtypes.py:528-530): unbound types are always
    // subtypes (bad annotation). Fires regardless of self.right because
    // the visitor's `accept` dispatches on left type.
    if matches!(left, Type::UnboundType { .. }) {
        return Some(true);
    }
    // visit_any (subtypes.py:534-535): proper subtype of Any only if
    // right is also Any. Non-proper subtype of Any is always True.
    if matches!(left, Type::AnyType { .. }) {
        if ctx.proper_subtype {
            return Some(matches!(right, Type::AnyType { .. }));
        }
        return Some(true);
    }
    // visit_union_type (subtypes.py:1175-1227): each item of left must be
    // a subtype of right. Python first optimises for right being Instance
    // or UnionType with literal-pruning; the general case is
    // `all(self._is_subtype(item, self.orig_right) for item in left.items)`.
    // We always fire the general case — Python's shim has already handled
    // the UnionType-right case at the top-level entry (subtypes.py:362-408),
    // so this only fires when left is UnionType and right is something else
    // (or when the shim returned None).
    if let Type::UnionType { items, .. } = left {
        for item in items {
            match is_subtype(item, right, ctx, resolver) {
                Some(true) => {}
                Some(false) => return Some(false),
                None => return None,
            }
        }
        return Some(true);
    }
    // visit_tuple_type (subtypes.py:950-1037): TupleType left vs right.
    // Handles right=Instance (Sized, TUPLE_LIKE, structural fallback,
    // protocol) and right=TupleType (variadic via `variadic_tuple_subtype`,
    // length, item-by-item, fallback matching).
    if let Type::TupleType {
        partial_fallback,
        items: left_items,
        ..
    } = left
    {
        // TupleType vs Instance right (subtypes.py:951-996).
        if let Type::Instance {
            type_ref: right_ref,
            args: right_args,
            ..
        } = right
        {
            // subtypes.py:953-954: typing.Sized is always a supertype.
            if is_named_instance(right, "typing.Sized") {
                return Some(true);
            }
            // subtypes.py:955-979: TUPLE_LIKE_INSTANCE_NAMES = builtins.tuple,
            // typing.Iterable, typing.Container, typing.Sequence,
            // typing.Reversible (types.py:166-171).
            let is_tuple_like = matches!(
                right_ref.as_str(),
                "builtins.tuple"
                    | "typing.Iterable"
                    | "typing.Container"
                    | "typing.Sequence"
                    | "typing.Reversible"
            );
            if is_tuple_like {
                // subtypes.py:956-958: args[0] is the iterator type; if
                // missing, Any under non-proper (False under proper).
                let iter_type = match right_args.first() {
                    Some(it) => it.clone(),
                    None => {
                        if ctx.proper_subtype {
                            return Some(false);
                        }
                        any_type(ANY_SPECIAL_FORM)
                    }
                };
                // subtypes.py:962-966: for builtins.tuple with Any iter
                // type, always True (isinstance(x, tuple) special case).
                if right_ref == "builtins.tuple" && matches!(iter_type, Type::AnyType { .. }) {
                    return Some(true);
                }
                // subtypes.py:968-978: each left item must be a subtype of
                // iter_type; UnpackType items unwrap to their element type.
                for li in left_items {
                    let li = if let Type::UnpackType { typ: inner } = li {
                        // get_proper_type(li.type): aliases defer;
                        // TypeVarTuple unwraps to upper_bound.
                        let unpacked = get_proper_type_or_defer(inner.as_ref(), resolver)?;
                        let unpacked = match unpacked {
                            Type::TypeVarTupleType { upper_bound, .. } => {
                                get_proper_type_or_defer(upper_bound.as_ref(), resolver)?
                            }
                            t => t,
                        };
                        // Python asserts the unpack is a builtins.tuple
                        // Instance; args[0] is the element type.
                        let Type::Instance { args, .. } = unpacked else {
                            return None;
                        };
                        args.first()?
                    } else {
                        li
                    };
                    match is_subtype(li, &iter_type, ctx, resolver) {
                        Some(true) => {}
                        Some(false) => return Some(false),
                        None => return None,
                    }
                }
                return Some(true);
            }
            // subtypes.py:980-982: structural fallback check. Python checks
            // BOTH the partial fallback and the full tuple_fallback.
            if is_subtype(partial_fallback, right, ctx, resolver)? {
                return Some(true);
            }
            // tuple_fallback (typeops.py:194-220) — the builtins.tuple
            // instance built from the items' union.
            if let Some(tf) = crate::typeops::tuple_fallback(left, resolver) {
                if is_subtype(&tf, right, ctx, resolver)? {
                    return Some(true);
                }
            }
            // subtypes.py:983-994: protocol branch. is_protocol_implementation
            // is Python-only (member-wise protocol checks); defer those to
            // Python. For non-protocol Instance right we return False here.
            if resolver.get(right_ref).is_some_and(|s| s.is_protocol) {
                return None;
            }
            return Some(false);
        }
        // TupleType vs TupleType right (subtypes.py:997-1036).
        if let Type::TupleType {
            items: right_items,
            partial_fallback: right_fallback,
            ..
        } = right
        {
            // subtypes.py:1004-1005: variadic unpack handling.
            // Defer to Python when variadic (TypeVarTuple or *tuple[X, ...]
            // unpacks present).
            if left_items
                .iter()
                .any(|t| matches!(t, Type::UnpackType { .. }))
                || right_items
                    .iter()
                    .any(|t| matches!(t, Type::UnpackType { .. }))
            {
                return None;
            }
            // Length check (subtypes.py:1006).
            if left_items.len() != right_items.len() {
                return Some(false);
            }
            // Item-by-item subtype check (subtypes.py:1008).
            for (l, r) in left_items.iter().zip(right_items.iter()) {
                match is_subtype(l, r, ctx, resolver) {
                    Some(true) => {}
                    Some(false) => return Some(false),
                    None => return None,
                }
            }
            // Fallback check: if right.fallback is builtins.tuple, no need
            // to verify (subtypes.py:1010-1013).
            if let Type::Instance {
                type_ref: rfb_ref, ..
            } = right_fallback.as_ref()
            {
                if rfb_ref == "builtins.tuple" {
                    return Some(true);
                }
            }
            // If left.fallback is builtins.tuple, it's NOT a subtype
            // (subtypes.py:1015-1018).
            if let Type::Instance {
                type_ref: lfb_ref, ..
            } = partial_fallback.as_ref()
            {
                if lfb_ref == "builtins.tuple" {
                    return Some(false);
                }
            }
            // Structural fallback check (subtypes.py:1020).
            return is_subtype(partial_fallback, right_fallback.as_ref(), ctx, resolver);
        }
        // TupleType vs anything else (CallableType, etc.): False
        // (subtypes.py:1037).
        return Some(false);
    }
    // visit_type_type (subtypes.py:1220-1280). Handles TypeType left.
    if let Type::TypeType {
        item: left_item,
        is_type_form: left_is_type_form,
    } = left
    {
        // left.is_type_form path (subtypes.py:1230-1244).
        if *left_is_type_form {
            // right is TypeType: must also be is_type_form, then recurse on items.
            if let Type::TypeType {
                item: right_item,
                is_type_form: right_is_type_form,
            } = right
            {
                if !*right_is_type_form {
                    return Some(false);
                }
                return is_subtype(left_item, right_item, ctx, resolver);
            }
            // right is Instance: only true if builtins.object.
            if let Type::Instance {
                type_ref: right_ref,
                ..
            } = right
            {
                if right_ref == "builtins.object" {
                    return Some(true);
                }
                return Some(false);
            }
            return Some(false);
        }
        // not left.is_type_form (subtypes.py:1239-1276).
        // right is TypeType: recurse on items.
        if let Type::TypeType {
            item: right_item, ..
        } = right
        {
            return is_subtype(left_item, right_item, ctx, resolver);
        }
        // right is CallableType: Type[X] <: Callable is unsound but done.
        // We don't check __init__ signature.
        if matches!(right, Type::CallableType { .. } | Type::Overloaded { .. }) {
            // Check if left.item has a __call__ equivalent.
            // Simplified: if left_item is Instance, get its type_object_type
            // and check return type match. For full parity, we defer to Python
            // the complex callable matching.
            return None;
        }
        // right is Instance.
        if let Type::Instance {
            type_ref: right_ref,
            ..
        } = right
        {
            // builtins.object and builtins.type are always True.
            if right_ref == "builtins.object" || right_ref == "builtins.type" {
                return Some(true);
            }
            // For other instances: check metaclass of left.item.
            // Simplified: if left_item is Instance, check if it has a metaclass
            // that is a subtype of right. Defer to Python for full accuracy.
            return None;
        }
        return Some(false);
    }
    // visit_overloaded (subtypes.py:1104-1169): Overloaded left.
    if let Type::Overloaded { items: left_items } = left {
        // right is Instance (subtypes.py:1105-1119): for a protocol Instance
        // the check is `find_member("__call__", right, right)` then a
        // subtyping check plus `is_protocol_implementation`; for a plain
        // Instance it recurses on `left.fallback`. Neither is ported
        // (find_member / is_protocol_implementation stay on the Python
        // side), so defer — deciding here produced a false "incompatible
        // type" for an Overloaded passed to a __call__-Protocol.
        if matches!(right, Type::Instance { .. }) {
            return None;
        }
        // right is CallableType: at least one overload item must match.
        if let Type::CallableType { .. } = right {
            for item in left_items {
                match is_subtype(item, right, ctx, resolver) {
                    Some(true) => return Some(true),
                    Some(false) => {}
                    None => return None,
                }
            }
            return Some(false);
        }
        // right is Overloaded: structural overload matching (order-sensitive).
        // Simplified: if left == right, True. Otherwise defer complex matching.
        if left == right {
            return Some(true);
        }
        return None;
    }
    // visit_callable_type (subtypes.py:807-889): CallableType left. The
    // Callable-vs-Callable case is handled by the separate callable_compat
    // engine (called from the Python shim), so here we only handle the
    // non-Callable right cases that the Python visitor dispatches after the
    // Callable right check. Each path that needs a Python-only helper
    // (find_member, is_protocol_implementation) or the callable_compat
    // engine defers to Python.
    if let Type::CallableType {
        fallback: left_fallback,
        instance_type: left_instance_type,
        ..
    } = left
    {
        // right is Overloaded (subtypes.py:866-867): left must be a subtype
        // of every overload item. Each recursion is CallableType-vs-
        // CallableType, which the callable_compat engine handles (not
        // this function), so each item defers; defer the whole check.
        if matches!(right, Type::Overloaded { .. }) {
            return None;
        }
        // right is Instance (subtypes.py:868-884). Protocol Instance with
        // "__call__" in protocol_members needs find_member and
        // is_protocol_implementation (Python-only); defer those. Protocol
        // Instance with left.is_type_obj() also needs
        // is_protocol_implementation; defer. Non-protocol Instance falls
        // through to `is_subtype(left.fallback, right)` (subtypes.py:884).
        if let Type::Instance {
            type_ref: right_ref,
            ..
        } = right
        {
            let right_snap = resolver.get(right_ref);
            let right_is_protocol = right_snap.is_some_and(|s| s.is_protocol);
            if right_is_protocol {
                let has_call =
                    right_snap.is_some_and(|s| s.protocol_members.iter().any(|m| m == "__call__"));
                if has_call {
                    // Protocol with __call__: needs find_member +
                    // is_protocol_implementation. Defer.
                    return None;
                }
                // Protocol without __call__: Python checks
                // is_protocol_implementation(left.fallback, right) only if
                // left.is_type_obj() (subtypes.py:878-883). Defer that too;
                // the non-type-obj path falls to is_subtype(fallback, right).
                // We can't distinguish is_type_obj here without the
                // fallback.is_metaclass() check, so defer all protocol
                // Instance right to be safe.
                return None;
            }
            // Non-protocol Instance: is_subtype(left.fallback, right)
            // (subtypes.py:884).
            return is_subtype(left_fallback.as_ref(), right, ctx, resolver);
        }
        // right is TypeType (subtypes.py:885-887): unsound, only checks
        // `left.is_type_obj() and is_subtype(left.get_instance_type(),
        // right.item)`. is_type_obj() is `fallback.type.is_metaclass() and
        // ret_type is not UninhabitedType`; we can't read is_metaclass from
        // the snapshot, but instance_type being Some is a strong signal
        // (it's only set for type objects). Defer when instance_type is
        // None (can't decide is_type_obj); otherwise recurse on it.
        if let Type::TypeType {
            item: right_item, ..
        } = right
        {
            if let Some(inst_type) = left_instance_type {
                return is_subtype(inst_type.as_ref(), right_item.as_ref(), ctx, resolver);
            }
            // instance_type is None: may still be a type object via the
            // fallback.is_metaclass() historic path. Defer to Python.
            return None;
        }
        // right is anything else (subtypes.py:888-889): False.
        // Note: CallableType-vs-CallableType is handled by callable_compat,
        // not here; if we reach this point with right=CallableType it
        // means the Python shim's callable_compat gate returned None and
        // fell through to the visitor, which then calls is_callable_compatible
        // (Python-only). Defer that case too.
        if matches!(right, Type::CallableType { .. }) {
            return None;
        }
        return Some(false);
    }
    let (left_ref, left_args) = match left {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => return None,
    };
    // Python's visit_instance falls through to `return False` when right
    // is Any under a proper-subtype check (the _is_subtype Any
    // short-circuit at subtypes.py:348-355 is non-proper-only; visit_any
    // handles left=Any; visit_union_type handles left-Union). Deciding
    // here instead of deferring is required: is_proper_subtype(X, Any)
    // is pervasive in thank-you paths like _remove_redundant_union_items,
    // and a `None` deferral there would push the whole simplified union to
    // Python, defeating the port.
    if ctx.proper_subtype && matches!(right, Type::AnyType { .. }) {
        return Some(false);
    }
    let (right_ref, right_args) = match right {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        // Python's `visit_instance` (subtypes.py:567-710) continues past
        // the Instance-right case for several non-Instance right types,
        // returning a definite `False` for most. Port that tail here so
        // the huge Instance-left / non-Instance-right population stops
        // deferring to Python (issue #591, ~124k deferrals on the
        // self-check corpus). Cases needing Python-only helpers
        // (find_member, is_metaclass, is_protocol_implementation,
        // callable_compat) still defer.
        _ => {
            let r =
                visit_instance_noninstance_right(left, right, ctx, resolver, left_ref, left_args);
            return r;
        }
    };
    visit_instance_nominal(
        left_ref, left_args, right, right_ref, right_args, ctx, resolver,
    )
}

/// `SubtypeVisitor.visit_instance` tail (subtypes.py:680-710), Rust port:
/// `left` is an Instance, `right` is NOT an Instance.
///
/// Python's decision table, in order:
/// * TupleType right:
///   - `right.partial_fallback.type.is_enum` -> `is_subtype(left,
///     tuple_fallback(right))`.
///   - single-item tuple whose item is an UnpackType wrapping an Instance
///     -> `is_subtype(left, that Instance)`.
///   - `left.type.has_base(right.partial_fallback.type.fullname)`:
///     non-proper + `map_instance_to_supertype` erased -> True when the
///     mapped target is `builtins.tuple` or variadic, else False.
///   - otherwise False.
/// * TypeVarTupleType right: needs variadic-slice reconstruction; rare
///   for Instance-left on the self-check corpus -> defer to Python.
/// * TypeType right: needs `left.type.is_metaclass()` (not in snapshot)
///   -> defer to Python.
/// * FunctionLike right (CallableType / Overloaded):
///   `find_member("__call__", left, ...)` not ported -> defer to Python.
/// * LiteralType right: handled earlier (last_known_value recursion);
///   a LiteralType reaching here has `last_known_value` None on the left
///   -> `False`.
/// * All other right types (NoneType, UninhabitedType, DeletedType,
///   TypedDictType, ParamSpecType, UnpackType): `else: return False`.
fn visit_instance_noninstance_right(
    left: &Type,
    right: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    left_ref: &str,
    left_args: &[Type],
) -> Option<bool> {
    // Left snapshot missing (synthesized instance, e.g. from isinstance
    // narrowing): matching Python needs the live TypeInfo, which the
    // nominal path also treats as missing-snapshot. Defer rather than
    // risk a wrong result for FakeInfo / fallback_to_any left types.
    let left_snap = resolver.get(left_ref)?;
    // fallback_to_any short-circuit (subtypes.py:638-643): a class with
    // dynamic bases is a subtype of everything except NoneType. Matches
    // the nominal-path guard (subtypes.py:493-498 equivalent).
    if left_snap.fallback_to_any && !ctx.proper_subtype {
        return Some(!matches!(right, Type::NoneType));
    }
    // TupleType right (subtypes.py:595-616).
    if let Type::TupleType {
        partial_fallback,
        items,
        ..
    } = right
    {
        let Type::Instance {
            type_ref: pf_ref, ..
        } = partial_fallback.as_ref()
        else {
            return Some(false);
        };
        // Snapshot missing (synthesized fallback): can't confirm the
        // `is_enum` / `has_base` conditions, so defer like the nominal
        // path does on a missing left snapshot rather than risk a wrong
        // `False`.
        let pf_snap = resolver.get(pf_ref)?;
        // subtypes.py:595-596: partial_fallback.type.is_enum.
        if pf_snap.is_enum {
            return match crate::typeops::tuple_fallback(right, resolver) {
                Some(fb) => is_subtype(left, &fb, ctx, resolver),
                None => None,
            };
        }
        // subtypes.py:597-604: single-item non-normalized tuple with an
        // UnpackType item wrapping an Instance.
        if items.len() == 1 {
            if let Type::UnpackType { typ } = &items[0] {
                let unpacked = get_proper_type_or_defer(typ.as_ref(), resolver)?;
                if matches!(unpacked, Type::Instance { .. }) {
                    return is_subtype(left, unpacked, ctx, resolver);
                }
            }
        }
        // subtypes.py:605-616: left has_base on the tuple's fallback.
        match resolver.get(left_ref) {
            None => return None,
            Some(left_snap) if left_snap.has_base(pf_ref) => {
                if !ctx.proper_subtype {
                    let mapped = map_instance_to_supertype(left_ref, left_args, pf_ref, resolver)?;
                    let mapped_instance = Type::Instance {
                        type_ref: pf_ref.to_string(),
                        args: mapped,
                        last_known_value: None,
                        extra_attrs: None,
                    };
                    if is_erased_instance(&mapped_instance)? {
                        let mapped_is_tuple_or_variadic =
                            pf_ref == "builtins.tuple" || pf_snap.has_type_var_tuple_type;
                        if mapped_is_tuple_or_variadic {
                            return Some(true);
                        }
                    }
                }
                return Some(false);
            }
            Some(_) => return Some(false),
        }
    }
    // TypeVarTupleType right (subtypes.py:617-620) needs
    // `map_instance_to_supertype` against the typevar's variadic
    // tuple_fallback and a first-arg check; rare for Instance-left on
    // production corpora and not exercised by the deferred ports' parity
    // suite. Defer to Python.
    if matches!(right, Type::TypeVarTupleType { .. }) {
        return None;
    }
    // TypeType right (subtypes.py:784-795): when left is `builtins.type`
    // (non-proper) recurse against Any; when left's type is a metaclass
    // AND the right item is `builtins.object` (a class can accept any
    // metaclass instance as its "object"-typed slot) return True.
    // Otherwise fall through to the FunctionLike / literal / False tail
    // below, matching Python exactly.
    if let Type::TypeType { item, .. } = right {
        // subtypes.py:788-793: item may be a TupleType -> tuple_fallback.
        let item = if let Type::TupleType { .. } = item.as_ref() {
            crate::typeops::tuple_fallback(item.as_ref(), resolver)?
        } else {
            (**item).clone()
        };
        if !ctx.proper_subtype {
            // subtypes.py:789-792: `left` is `builtins.type`: recurse
            // against `TypeType(Any)`. (TypeType is reached for
            // `type[builtins.type]`, i.e. a `type` object passed where a
            // class object is expected.)
            if left_ref == "builtins.type" {
                let special_any = Type::AnyType {
                    type_of_any: 0, // TypeOfAny.special_form
                    source_any: None,
                    missing_import_name: None,
                };
                let wrapped = Type::TypeType {
                    item: Box::new(special_any),
                    // TypeType(AnyType(special_form)); not a metaclass
                    // self-type. Python's `TypeType(AnyType(...))` has
                    // `is_metaclass_self_type=False`.
                    is_type_form: false,
                };
                return is_subtype(&wrapped, right, ctx, resolver);
            }
            if let Some(ls) = resolver.get(left_ref) {
                // TypeInfo.is_metaclass (nodes.py:4192-4198), precise=false.
                let is_metaclass = ls.has_base.contains("builtins.type")
                    || ls.fullname == "abc.ABCMeta"
                    || ls.fallback_to_any;
                if is_metaclass {
                    match item {
                        Type::AnyType { .. } => return Some(true),
                        Type::Instance { type_ref: r, .. } if r == "builtins.object" => {
                            return Some(true)
                        }
                        _ => {}
                    }
                }
            }
        }
        // Fall through to the FunctionLike/literal/False tail.
    }
    // FunctionLike right needs `find_member("__call__", ...)`
    // (subtypes.py:678-682) -> defer.
    if matches!(right, Type::CallableType { .. } | Type::Overloaded { .. }) {
        return None;
    }
    // Anything else: Python's `else: return False` (subtypes.py:683).
    // Includes NoneType, UninhabitedType, DeletedType, TypedDictType,
    // ParamSpecType, UnpackType, LiteralType (no lkv).
    Some(false)
}

/// `visit_typeddict_type` (subtypes.py:1039-1096), Rust port.
///
/// Returns `Some(bool)` when Rust decided; `None` when a recursive
/// `is_subtype` hit an unsupported variant (caller defers to Python).
///
/// Two cases:
/// - right is Instance: `is_subtype(left.fallback, right)`.
/// - right is TypedDictType: structural check over items, required_keys,
///   readonly_keys, and is_closed. Fallbacks don't matter
///   (subtypes.py:1093).
fn visit_typeddict_subtype(
    left: &Type,
    right: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    let (left_fallback, left_items, left_required, left_readonly, left_closed) = match left {
        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => (
            fallback.as_ref(),
            items.as_slice(),
            required_keys,
            readonly_keys,
            *is_closed,
        ),
        _ => return None,
    };
    // right is Instance: is_subtype(left.fallback, right)
    // (subtypes.py:1041-1042).
    if let Type::Instance { .. } = right {
        return is_subtype(left_fallback, right, ctx, resolver);
    }
    let (right_items, right_required, right_readonly, right_closed) = match right {
        Type::TypedDictType {
            items,
            required_keys,
            readonly_keys,
            is_closed,
            ..
        } => (items.as_slice(), required_keys, readonly_keys, *is_closed),
        // Any other right is not a subtype (subtypes.py:1095-1096).
        _ => return Some(false),
    };
    // Equal types are always subtypes (subtypes.py:1044-1045 fast path).
    if left == right {
        return Some(true);
    }
    // A closed type must remain closed (subtypes.py:1048-1049).
    if right_closed && !left_closed {
        return Some(false);
    }
    // Collect all unique key names from both dicts (zipall,
    // types.py:3232-3239).
    let mut all_keys: Vec<&str> = Vec::new();
    for (name, _) in left_items {
        if !all_keys.contains(&name.as_str()) {
            all_keys.push(name.as_str());
        }
    }
    for (name, _) in right_items {
        if !all_keys.contains(&name.as_str()) {
            all_keys.push(name.as_str());
        }
    }
    // Key-based checks (subtypes.py:1051-1062).
    for name in &all_keys {
        let (_, l_required, l_readonly) =
            td_item(left_items, left_required, left_readonly, left_closed, name);
        let (_, r_required, r_readonly) = td_item(
            right_items,
            right_required,
            right_readonly,
            right_closed,
            name,
        );
        // Required keys must remain required.
        if r_required && !l_required {
            return Some(false);
        }
        // Mutable keys must remain mutable.
        if !r_readonly && l_readonly {
            return Some(false);
        }
        // Mutable optional keys must also remain optional.
        if !r_readonly && !r_required && l_required {
            return Some(false);
        }
    }
    // Value type checks (subtypes.py:1064-1092).
    for name in &all_keys {
        let (l_typ, _, _l_readonly) =
            td_item(left_items, left_required, left_readonly, left_closed, name);
        let (r_typ, _, r_readonly) = td_item(
            right_items,
            right_required,
            right_readonly,
            right_closed,
            name,
        );
        let check = if !r_readonly {
            // Mutable items: invariant (is_equivalent / is_same_type).
            // Both typ must be Some (guaranteed by key-based checks
            // for mutable items).
            match (l_typ.as_ref(), r_typ.as_ref()) {
                (Some(lt), Some(rt)) => {
                    let fwd = is_subtype(lt, rt, ctx, resolver)?;
                    let bwd = is_subtype(rt, lt, ctx, resolver)?;
                    Some(fwd && bwd)
                }
                _ => return None,
            }
        } else {
            // Read-only items: covariant.
            match (l_typ.as_ref(), r_typ.as_ref()) {
                (_, None) => Some(true),
                (None, Some(_)) => Some(false),
                (Some(lt), Some(rt)) => is_subtype(lt, rt, ctx, resolver),
            }
        };
        match check {
            Some(true) => {}
            Some(false) => return Some(false),
            None => return None,
        }
    }
    // (NOTE: Fallbacks don't matter. — subtypes.py:1093)
    Some(true)
}

/// Look up a key in a TypedDictType, mirroring `TypedDictType.item`
/// (types.py:3218-3230). Returns `(typ, required, readonly)`.
///
/// For a missing key in a closed dict: `(Some(UninhabitedType), false, false)`.
/// For a missing key in an open dict: `(None, false, true)`.
fn td_item(
    items: &[(String, Type)],
    required_keys: &HashSet<String>,
    readonly_keys: &HashSet<String>,
    is_closed: bool,
    name: &str,
) -> (Option<Type>, bool, bool) {
    if let Some((_, typ)) = items.iter().find(|(n, _)| n == name) {
        (
            Some(typ.clone()),
            required_keys.contains(name),
            readonly_keys.contains(name),
        )
    } else if is_closed {
        (
            Some(Type::UninhabitedType { ambiguous: false }),
            false,
            false,
        )
    } else {
        (None, false, true)
    }
}

/// The `visit_instance` `isinstance(right, Instance)` branch
/// `expand_type_by_instance` (expandtype.py:85-115), Rust subset.
///
/// Substitutes TypeVarType nodes in `typ` whose `namespace` matches
/// `left_ref` and whose `raw_id` is a 1-based position into `left_args`.
/// Mirrors the Python env build:
///   `variables[binder.id] = arg` for each (defn.type_vars[i], args[i]).
///
/// Class type vars have `raw_id = i+1` and `namespace = class.fullname`,
/// so we match `(namespace == left_ref, raw_id == i+1)` and substitute
/// `left_args[i]`. Returns `None` for Type variants the subset walker
/// does not handle (CallableType, ParamSpec, UnpackType, etc. inside
/// the tree); the caller falls through to Python for those.
fn expand_type_by_instance(typ: &Type, left_ref: &str, left_args: &[Type]) -> Option<Type> {
    match typ {
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let mut new_args = Vec::with_capacity(args.len());
            for arg in args {
                new_args.push(expand_type_by_instance(arg, left_ref, left_args)?);
            }
            // builtins.tuple normalization (expandtype.py:228-237) is
            // deferred; the common nominal case doesn't need it.
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: new_args,
                last_known_value: last_known_value.clone(),
                extra_attrs: extra_attrs.clone(),
            })
        }
        Type::TypeVarType {
            raw_id, namespace, ..
        } => {
            // Match class type vars: namespace == left_ref, raw_id is
            // 1-based position into defn.type_vars (== left_args).
            if namespace == left_ref && *raw_id >= 1 {
                let idx = (*raw_id - 1) as usize;
                if idx < left_args.len() {
                    // Python clears last_known_value on Instance
                    // replacements (expandtype.py:246-249); we clone as-is
                    // since lkv handling is the LiteralType path (M8c+).
                    return Some(left_args[idx].clone());
                }
            }
            // Unmatched TypeVar: namespace mismatch or raw_id out of
            // range. Python leaves it as-is and visit_typevar (not
            // ported) handles it. Return None to fall through.
            None
        }
        Type::UnionType {
            items,
            uses_pep604_syntax,
            ..
        } => {
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(expand_type_by_instance(item, left_ref, left_args)?);
            }
            // Python's visit_union_type rebuilds via UnionType.make_union,
            // so truthiness is recomputed from the (expanded) items.
            let can_be_true = new_items.iter().any(crate::setops::union_item_can_be_true);
            let can_be_false = new_items.iter().any(crate::setops::union_item_can_be_false);
            Some(Type::UnionType {
                items: new_items,
                uses_pep604_syntax: *uses_pep604_syntax,
                can_be_true,
                can_be_false,
            })
        }
        Type::NoneType | Type::UninhabitedType { .. } => Some(typ.clone()),
        Type::AnyType { .. } | Type::DeletedType { .. } | Type::LiteralType { .. } => {
            Some(typ.clone())
        }
        // Unsupported variants in the tree: fall through to Python.
        _ => None,
    }
}

/// `map_instance_to_supertype` (maptype.py:8-23), Rust subset.
///
/// Walks `class_derivation_paths` (maptype.py:46-67) over the snapshot's
/// `bases` blobs, mapping `left` up to `right_ref`'s frame step by step
/// via `expand_type_by_instance`. Returns the mapped args (to compare
/// against `right_args` in `check_type_parameter`).
///
/// Handles direct bases (path length 1) and multi-level paths. Returns
/// `None` when any step hits an unsupported Type variant in
/// `expand_type_by_instance`, or when no derivation path is found (the
/// snapshot may be stale mid-build; Python handles the Any fallback).
pub(crate) fn map_instance_to_supertype(
    left_ref: &str,
    left_args: &[Type],
    right_ref: &str,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let _left_snap = resolver.get(left_ref)?;
    // Fast path: left.type == right.type (maptype.py:15-17).
    if left_ref == right_ref {
        return Some(left_args.to_vec());
    }
    // Walk class_derivation_paths via the snapshot's bases blobs.
    // Each base is a serialized Instance; decode and recurse.
    map_derivation_path(left_ref, left_args, right_ref, resolver)
}

/// Recursive step of `map_instance_to_supertypes` (maptype.py:26-43).
/// Finds a base whose type_ref == right_ref (direct) or recurses through
/// a base whose own bases lead to right_ref (multi-level path).
fn map_derivation_path(
    left_ref: &str,
    left_args: &[Type],
    right_ref: &str,
    resolver: &TypeResolver,
) -> Option<Vec<Type>> {
    let left_snap = resolver.get(left_ref)?;
    // Variadic left: expand_type_by_instance would need the
    // split_with_prefix_and_suffix logic to substitute the TypeVarTuple
    // middle. Not ported; defer to Python. Also guards mid-path bases
    // that are variadic even when the original left isn't.
    if left_snap.has_type_var_tuple_type {
        return None;
    }
    for base_blob in &left_snap.bases {
        let base = decode_type(base_blob)?;
        if let Type::Instance {
            type_ref: base_ref,
            args: _base_args,
            ..
        } = &base
        {
            if base_ref == right_ref {
                // Direct base: expand base's args by left's frame.
                let expanded = expand_type_by_instance(&base, left_ref, left_args)?;
                if let Type::Instance { args, .. } = expanded {
                    return Some(args);
                }
                return None;
            }
            // Multi-level: recurse through this base. First map left to
            // this base's frame, then continue from there.
            let mapped = expand_type_by_instance(&base, left_ref, left_args)?;
            if let Type::Instance {
                type_ref: mid_ref,
                args: mid_args,
                ..
            } = mapped
            {
                if let Some(result) = map_derivation_path(&mid_ref, &mid_args, right_ref, resolver)
                {
                    return Some(result);
                }
            }
        }
    }
    None
}

fn visit_instance_nominal(
    left_ref: &str,
    left_args: &[Type],
    right: &Type,
    right_ref: &str,
    right_args: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    let left_snap = resolver.get(left_ref);
    let right_snap = resolver.get(right_ref);

    // If left's TypeInfo is not in the resolver, it may be a synthesized
    // type (e.g. ad-hoc intersection from isinstance narrowing) whose
    // MRO and bases are only available on the live Python TypeInfo.
    // Defer rather than returning a wrong Some(false).
    #[allow(clippy::question_mark)]
    if left_snap.is_none() {
        return None;
    }

    // fallback_to_any short-circuit (subtypes.py:493-498): a class with
    // dynamic bases is a subtype of everything except None. We only
    // detect NoneType by tag; right is Instance here, so it never fires.
    if let Some(snap) = &left_snap {
        if snap.fallback_to_any && !ctx.proper_subtype {
            // Python handled NoneType before us (right is Instance here).
            return Some(true);
        }
    }

    // promote loop (subtypes.py:536-542): walk left.type.mro, check each
    // base's _promote against right. Skip on ignore_promotions or when
    // right is a protocol (snapshot missing means "assume not protocol").
    let right_not_protocol = right_snap.is_none_or(|s| !s.is_protocol);
    if !ctx.ignore_promotions && right_not_protocol {
        if let Some(snap) = &left_snap {
            for base_fullname in &snap.mro {
                if let Some(base_snap) = resolver.get(base_fullname) {
                    for promote_blob in &base_snap.promote_bytes {
                        if let Some(promote) = decode_type(promote_blob) {
                            if is_subtype(&promote, right, ctx, resolver) == Some(true) {
                                return Some(true);
                            }
                        }
                    }
                }
            }
            // alt_promote (subtypes.py:546-547): left.type.alt_promote
            // whose target type is right.type.
            if let Some(alt) = &snap.alt_promote_fullname {
                if alt == right_ref {
                    return Some(true);
                }
            }
        }
    }

    // Nominal check (subtypes.py:554-561). NamedTuple special case and
    // builtins.object fast-path mirror the Python condition.
    let has_base = left_snap.is_some_and(|s| s.has_base(right_ref));
    let is_object = right_ref == "builtins.object";
    let right_is_protocol = right_snap.is_some_and(|s| s.is_protocol);
    // Python's NamedTuple clause (subtypes.py:632-635) fires only when
    // `rname in TYPED_NAMEDTUPLE_NAMES` (right is typing.NamedTuple or
    // typing_extensions.NamedTuple literally) AND some class in left's
    // mro is a NamedTuple. The snapshot's `is_named_tuple` flag is True
    // for ANY NamedTuple subclass (e.g. __main__.A), not just the
    // typing.NamedTuple base, so checking `right_snap.is_named_tuple`
    // would wrongly apply the nominal branch to two unrelated
    // NamedTuples (e.g. is_subtype(A, B) -> Some(true)). Rust can't read
    // `rname in TYPED_NAMEDTUPLE_NAMES` from the snapshot alone without
    // also special-casing the two base fullnames; defer the whole
    // NamedTuple-right case so Python's exact condition decides.
    // Python's NamedTuple clause (subtypes.py:632-637) fires when right
    // is literally typing.NamedTuple or typing_extensions.NamedTuple (the
    // only two names in TYPED_NAMEDTUPLE_NAMES) AND some class in left's
    // mro is_named_tuple. Checking right_snap.is_named_tuple would be
    // wrong because that flag is True for ANY NamedTuple subclass.
    let is_named_tuple_right = matches!(
        right_ref,
        "typing.NamedTuple" | "typing_extensions.NamedTuple"
    ) && left_snap.is_some_and(|s| {
        s.mro
            .iter()
            .any(|m| resolver.get(m).is_some_and(|n| n.is_named_tuple))
    });
    let nominal_applies =
        (has_base || is_object || is_named_tuple_right) && !ctx.ignore_declared_variance;
    if !nominal_applies {
        // Nominal branch skipped. If right is a protocol, defer to the
        // Python protocol-implementation path (M8c). Otherwise Python
        // records a negative cache entry and returns False.
        if right_is_protocol {
            return None;
        }
        return Some(false);
    }

    let right_snap = right_snap?;

    // Variadic right (subtypes.py:644-670): Python takes a special path
    // using split_with_prefix_and_suffix to splice the TypeVarTuple
    // middle into left/right args. Not ported; defer to Python.
    if right_snap.has_type_var_tuple_type {
        return None;
    }
    // Variadic left when left != right: map_instance_to_supertype would
    // need the same split logic to substitute the variadic tvar. Defer.
    if left_ref != right_ref && left_snap.is_some_and(|s| s.has_type_var_tuple_type) {
        return None;
    }

    // Map left to right's type. Fast path: left.type == right.type (no
    // substitution needed). Slow path calls map_instance_to_supertype
    // to walk the bases blobs and substitute TypeVars.
    let mapped_args: Vec<Type> = if left_ref == right_ref {
        left_args.to_vec()
    } else if right_snap.type_vars_with_variance.is_empty() {
        // right has no type vars: map_instance_to_supertype returns
        // Instance(right, []) (no args to substitute).
        Vec::new()
    } else {
        // Generic substitution path: map_instance_to_supertype walks
        // class_derivation_paths over the snapshot's bases blobs,
        // substituting TypeVars via expand_type_by_instance. Returns
        // None when an unsupported Type variant is in the tree (e.g.
        // UnpackType, ParamSpec), in which case Python falls through.
        map_instance_to_supertype(left_ref, left_args, right_ref, resolver)?
    };

    if ctx.ignore_type_params {
        return Some(true);
    }

    // check_type_parameter over (lefta, righta, tvar) triples
    // (subtypes.py:598-621). VARIANCE_NOT_READY returns None (Python
    // handles infer_class_variances; mutating live defn, deferred).
    let right_tvars = &right_snap.type_vars_with_variance;
    if mapped_args.len() != right_args.len() || mapped_args.len() != right_tvars.len() {
        // Arity mismatch. Python would assert; we fall through rather
        // than panic, since the snapshot may be stale mid-build.
        return None;
    }
    let mut nominal = true;
    for (i, (_tvar_name, variance, kind)) in right_tvars.iter().enumerate() {
        // ParamSpec (kind=1) / TypeVarTuple (kind=2): Python's else
        // branch (subtypes.py:691-696) treats them as COVARIANT, but
        // the arg shapes (CallableType with ParamSpec prefix, TupleType
        // for variadic middle) hit unsupported variants in the
        // recursive is_subtype. Defer to Python.
        if *kind != 0 {
            return None;
        }
        let lefta = &mapped_args[i];
        let righta = &right_args[i];
        let effective_variance = if ctx.always_covariant && *variance == INVARIANT {
            COVARIANT
        } else {
            *variance
        };
        if *variance == VARIANCE_NOT_READY {
            // infer_class_variances mutates live defn.type_vars; the
            // snapshot can't mirror that without re-reading. Fall through.
            return None;
        }
        match check_type_parameter(lefta, righta, effective_variance, ctx, resolver) {
            Some(true) => {}
            Some(false) => {
                nominal = false;
                break;
            }
            // Recursive is_subtype hit an unsupported variant. Don't
            // assume not-subtype (would give wrong answers); defer.
            None => return None,
        }
    }
    Some(nominal)
}

/// `check_type_parameter` (subtypes.py:379-410), Rust subset.
///
/// Returns `Some(bool)` when Rust decided; `None` when a recursive
/// `is_subtype` hit an unsupported variant. Propagating `None` (rather
/// than swallowing it as `false` via `unwrap_or`) prevents wrong
/// answers: if Rust can't decide `is_subtype(lefta, righta)`, the whole
/// `visit_instance_nominal` must defer to Python, not assume not-subtype.
///
/// COVARIANT / VARIANCE_NOT_READY: `is_subtype(left, right)`.
/// CONTRAVARIANT: `is_subtype(right, left)`.
/// INVARIANT: `is_equivalent(left, right)` — a two-way subtype check
/// (both `is_subtype(left, right)` and `is_subtype(right, left)` must
/// hold). This mirrors Python's `is_equivalent` / `is_same_type` for
/// both proper and non-proper subtype checks. The `proper_subtype` flag
/// flows through `ctx.proper_subtype` into the recursive `is_subtype`
/// calls, so the two-way check respects properness at every depth.
fn check_type_parameter(
    left: &Type,
    right: &Type,
    variance: i64,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    // subtypes.py:522-526: an `ambiguous` UninhabitedType (empty
    // collection literal / partial state) is safe to treat as COVARIANT
    // even under INVARIANT, since such a type can't be stored in a
    // variable. Without this, the invariant two-way check would demand
    // `right <: Never` and reject empty literals against dict/list
    // contexts. Mirrors `checker.is_valid_inferred_type()`.
    let effective_variance = if variance == INVARIANT {
        if matches!(left, Type::UninhabitedType { ambiguous: true }) {
            COVARIANT
        } else {
            variance
        }
    } else {
        variance
    };
    match effective_variance {
        COVARIANT | VARIANCE_NOT_READY => is_subtype(left, right, ctx, resolver),
        CONTRAVARIANT => is_subtype(right, left, ctx, resolver),
        _ => {
            let fwd = is_subtype(left, right, ctx, resolver)?;
            let bwd = is_subtype(right, left, ctx, resolver)?;
            Some(fwd && bwd)
        }
    }
}

/// Decode a wire-format `Type` blob via `wire::read_type`. Returns
/// `None` on any read failure (truncated input, unknown tag).
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

// =====================================================================
// Issue #465: pure-computation helpers from subtypes.py
// =====================================================================

/// `has_underscore_prefix` (subtypes.py:2453-2454): true when a name
/// starts with `_` but is NOT a dunder (`__name__`).
///
/// Pure string function; no Type object or resolver needed.
fn has_underscore_prefix(name: &str) -> bool {
    name.starts_with('_') && !(name.starts_with("__") && name.ends_with("__"))
}

/// `#[pyfunction]` entry for `has_underscore_prefix`.
#[pyfunction]
pub(crate) fn rust_has_underscore_prefix(name: &str) -> bool {
    has_underscore_prefix(name)
}

/// `is_erased_instance` (subtypes.py:2486-2500): true when an Instance
/// has at least one arg and every arg is Any (after `get_proper_type`).
/// `UnpackType` args are unpacked: the inner type must be
/// `builtins.tuple` with `args[0]` being Any.
///
/// Returns `Some(bool)` when the wire form fully decodes; `None` when
/// a `TypeAliasType` or other unhandled variant is in the tree (Python
/// falls through with `get_proper_type` already applied).
fn is_erased_instance(t: &Type) -> Option<bool> {
    let Type::Instance { args, .. } = t else {
        return Some(false);
    };
    if args.is_empty() {
        return Some(false);
    }
    for arg in args {
        if !is_erased_arg(arg)? {
            return Some(false);
        }
    }
    Some(true)
}

/// Check a single arg of `is_erased_instance`. Returns `None` when the
/// arg is a `TypeAliasType` (can't expand in the wire form); `Some(bool)`
/// otherwise.
fn is_erased_arg(arg: &Type) -> Option<bool> {
    match arg {
        Type::UnpackType { typ } => {
            // get_proper_type(arg.type) — UnpackType wraps an Instance.
            let inner = typ.as_ref();
            if !matches!(inner, Type::Instance { .. }) {
                return Some(false);
            }
            let Type::Instance {
                type_ref,
                args: inner_args,
                ..
            } = inner
            else {
                return Some(false);
            };
            if type_ref != "builtins.tuple" {
                return Some(false);
            }
            // get_proper_type(unpacked.args[0]) must be AnyType.
            let first = inner_args.first()?;
            match first {
                Type::AnyType { .. } => Some(true),
                Type::TypeAliasType { .. } => None,
                _ => Some(false),
            }
        }
        Type::AnyType { .. } => Some(true),
        Type::TypeAliasType { .. } => None,
        _ => Some(false),
    }
}

/// `#[pyfunction]` entry for `is_erased_instance`.
#[pyfunction]
pub(crate) fn rust_is_erased_instance(t_bytes: &[u8]) -> Option<bool> {
    let t = decode_type(t_bytes)?;
    is_erased_instance(&t)
}

/// `try_restrict_literal_union` (subtypes.py:2264-2282): return the
/// items of `t` (a UnionType) excluding any occurrence of `s`, iff
/// every item and `s` are simple literals. Otherwise return `None`.
///
/// Returns `Some(Vec<Vec<u8>>)` (serialized remaining items) or `None`
/// (not all simple literals, or a decode failure). An empty `Vec` means
/// all items matched `s` (the union reduces to `Never`).
fn try_restrict_literal_union(
    t: &Type,
    s: &Type,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let Type::UnionType { items, .. } = t else {
        return None;
    };
    let s_is_simple = crate::typeops::is_simple_literal(s, resolver)?;
    if !s_is_simple {
        return None;
    }
    let mut remaining: Vec<Vec<u8>> = Vec::new();
    for item in items {
        // relevant_items() (types.py:3517-3522): when strict_optional is
        // off, NoneType items are skipped entirely.
        if !strict_optional && matches!(item, Type::NoneType) {
            continue;
        }
        let is_simple = crate::typeops::is_simple_literal(item, resolver)?;
        if !is_simple {
            return None;
        }
        // Compare by structural equality (get_proper_type already applied
        // by the wire format — both are proper types here).
        if item != s {
            let mut buf = WriteBuffer::new();
            wire::write_type(&mut buf, item).ok()?;
            remaining.push(buf.into_bytes());
        }
    }
    Some(remaining)
}

/// `#[pyfunction]` entry for `try_restrict_literal_union`.
///
/// Returns `Some(Vec<Vec<u8>>)` (serialized remaining items) or `None`
/// (not all simple literals, or decode failure). Python deserializes
/// each item back to a `Type` via the wire format.
#[pyfunction]
pub(crate) fn rust_try_restrict_literal_union(
    t_bytes: &[u8],
    s_bytes: &[u8],
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<Vec<u8>>> {
    let t = decode_type(t_bytes)?;
    let s = decode_type(s_bytes)?;
    try_restrict_literal_union(&t, &s, strict_optional, resolver.resolver())
}

/// `is_more_precise` (subtypes.py:2355-2366): left is more precise than
/// right when right is Any (any left qualifies), or left is a proper
/// subtype of right.
///
/// Mirrors the Python: `right = get_proper_type(right); if isinstance(right,
/// AnyType): return True; return is_proper_subtype(left, right, ...)`.
/// The `get_proper_type` expansion is handled by the wire format (the
/// Python shim serializes after expansion). `ignore_promotions` flows
/// into the `SubtypeContext`.
fn is_more_precise(
    left: &Type,
    right: &Type,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    // right is AnyType (after get_proper_type) -> True.
    if matches!(right, Type::AnyType { .. }) {
        return Some(true);
    }
    // is_proper_subtype(left, right, ignore_promotions=...)
    let ctx = SubtypeContext::new(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        ignore_promotions,
        true, // proper_subtype
        strict_optional,
    );
    is_subtype(left, right, &ctx, resolver)
}

/// `#[pyfunction]` entry for `is_more_precise`.
#[pyfunction]
pub(crate) fn rust_is_more_precise(
    left_bytes: &[u8],
    right_bytes: &[u8],
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    is_more_precise(
        &left,
        &right,
        ignore_promotions,
        strict_optional,
        resolver.resolver(),
    )
}

/// `is_equivalent` (subtypes.py:277-300): a <: b AND b <: a (non-proper
/// subtype both ways). The Python shim handles `get_proper_type` and
/// the `left == right` fast path before calling Rust.
fn is_equivalent(
    a: &Type,
    b: &Type,
    ignore_type_params: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    let ctx = SubtypeContext::new(
        ignore_type_params,
        false, // ignore_declared_variance
        false, // always_covariant
        false, // ignore_promotions
        false, // proper_subtype
        strict_optional,
    );
    let fwd = is_subtype(a, b, &ctx, resolver)?;
    if !fwd {
        return Some(false);
    }
    let bwd = is_subtype(b, a, &ctx, resolver)?;
    Some(fwd && bwd)
}

/// `#[pyfunction]` entry for `is_equivalent`.
#[pyfunction]
pub(crate) fn rust_is_equivalent(
    a_bytes: &[u8],
    b_bytes: &[u8],
    ignore_type_params: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let a = decode_type(a_bytes)?;
    let b = decode_type(b_bytes)?;
    is_equivalent(
        &a,
        &b,
        ignore_type_params,
        strict_optional,
        resolver.resolver(),
    )
}

/// `is_same_type` (subtypes.py:303-335): a and b are proper subtypes of
/// each other, with fast paths for common Instance and TypeVarType pairs.
///
/// Fast path 1 (Instance): same `type_ref`, same arg count, same
/// `last_known_value` identity -> recurse on args.
/// Fast path 2 (TypeVarType): same `raw_id`+`namespace` and same
/// `upper_bound` -> True.
/// General: `is_proper_subtype(a, b) and is_proper_subtype(b, a)`.
fn is_same_type(
    a: &Type,
    b: &Type,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &TypeResolver,
) -> Option<bool> {
    // Fast path 1: both Instance, same type_ref, same arg count, same lkv.
    if let (
        Type::Instance {
            type_ref: ra,
            args: aa,
            last_known_value: la,
            ..
        },
        Type::Instance {
            type_ref: rb,
            args: ab,
            last_known_value: lb,
            ..
        },
    ) = (a, b)
    {
        if ra == rb && aa.len() == ab.len() && la == lb {
            for (x, y) in aa.iter().zip(ab.iter()) {
                let same = is_same_type(x, y, ignore_promotions, strict_optional, resolver)?;
                if !same {
                    return Some(false);
                }
            }
            return Some(true);
        }
    }
    // Fast path 2: both TypeVarType, same id (raw_id + namespace), same upper_bound.
    if let (
        Type::TypeVarType {
            raw_id: ia,
            namespace: na,
            upper_bound: ua,
            ..
        },
        Type::TypeVarType {
            raw_id: ib,
            namespace: nb,
            upper_bound: ub,
            ..
        },
    ) = (a, b)
    {
        if ia == ib && na == nb && ua == ub {
            return Some(true);
        }
    }
    // General: is_proper_subtype both ways.
    let ctx = SubtypeContext::new(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        ignore_promotions,
        true, // proper_subtype
        strict_optional,
    );
    let fwd = is_subtype(a, b, &ctx, resolver)?;
    if !fwd {
        return Some(false);
    }
    let bwd = is_subtype(b, a, &ctx, resolver)?;
    Some(fwd && bwd)
}

/// `#[pyfunction]` entry for `mypy.checkexpr.all_same_types`.
///
/// Mirrors checkexpr.py:8278-8281: empty list yields True; otherwise every
/// item after the first must be `is_same_type` against the first. Defer
/// (return `Option::None`) on any decoding or subtype-resolution defer,
/// matching `rust_is_same_type` per-pair so the Python fallback can run.
#[pyfunction]
pub(crate) fn rust_all_same_types(
    items_bytes: Vec<&[u8]>,
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    if items_bytes.is_empty() {
        return Some(true);
    }
    let first = decode_type(items_bytes[0])?;
    for b_bytes in items_bytes.iter().skip(1) {
        let b = decode_type(b_bytes)?;
        match is_same_type(
            &first,
            &b,
            ignore_promotions,
            strict_optional,
            resolver.resolver(),
        ) {
            Some(true) => {}
            Some(false) => return Some(false),
            None => return None,
        }
    }
    Some(true)
}

/// `#[pyfunction]` entry for `is_same_type`.
#[pyfunction]
pub(crate) fn rust_is_same_type(
    a_bytes: &[u8],
    b_bytes: &[u8],
    ignore_promotions: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let a = decode_type(a_bytes)?;
    let b = decode_type(b_bytes)?;
    is_same_type(
        &a,
        &b,
        ignore_promotions,
        strict_optional,
        resolver.resolver(),
    )
}

/// `#[pyfunction]` entry: the Python-side shim calls this with the
/// serialized `left` and `right` Type blobs plus the
/// `NativeTypeResolver` pyclass. Returns `None` (Python `None`) when
/// Rust doesn't handle the case; `Some(bool)` otherwise.
///
/// The shim in `mypy/subtypes.py` is responsible for `get_proper_type`,
/// the `AnyType`/`UnboundType`/`ErasedType` right short-circuit, the
/// `UnionType` right dispatch, the `TypeVarType`-with-values right,
/// and the `assuming` recursion guard BEFORE calling this.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_is_subtype(
    left_bytes: &[u8],
    right_bytes: &[u8],
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let ctx = SubtypeContext::new(
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        proper_subtype,
        strict_optional,
    );
    is_subtype(&left, &right, &ctx, resolver.resolver())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;

    fn make_resolver(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
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

    fn tuple_type(items: Vec<Type>) -> Type {
        Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items,
            implicit: true,
        }
    }

    fn callable_type(arg_types: Vec<Type>, ret_type: Type, instance_type: Option<Type>) -> Type {
        Type::CallableType {
            fallback: Box::new(instance("builtins.function", vec![])),
            instance_type: instance_type.map(Box::new),
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            arg_types,
            arg_kinds: vec![],
            arg_names: vec![],
            ret_type: Box::new(ret_type),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    fn ctx_nominal() -> SubtypeContext {
        SubtypeContext::new(false, false, false, false, false, true)
    }

    fn snap(fullname: &str, name: &str) -> TypeInfoSnapshot {
        // Real TypeInfo always has its own fullname in mro and has_base.
        // Tests that need a different mro should overwrite these fields.
        let mut s = TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        s
    }

    #[test]
    fn same_instance_no_args_is_subtype() {
        // A <: A when both have no args (subtypes.py:554 has_base + map
        // to self + no type_params to check).
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.A", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn instance_is_subtype_of_object() {
        // Any Instance is a subtype of builtins.object (subtypes.py:556).
        // object has no type vars, so the non-generic path applies.
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = instance("a.A", vec![]);
        let right = instance("builtins.object", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn derived_is_subtype_of_base_when_has_base() {
        // a.B has_base("a.A") -> B <: A.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let r = make_resolver(vec![snap("a.A", "A"), b]);
        let left = instance("a.B", vec![]);
        let right = instance("a.A", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn unrelated_instances_are_not_subtypes() {
        // a.A does not has_base("a.B") -> not a subtype, not object.
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn fallback_to_any_short_circuits_non_proper() {
        // fallback_to_any=True, non-proper -> True (subtypes.py:493-498).
        let mut base = snap("a.AnyBase", "AnyBase");
        base.fallback_to_any = true;
        let r = make_resolver(vec![base, snap("a.Other", "Other")]);
        let left = instance("a.AnyBase", vec![]);
        let right = instance("a.Other", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn fallback_to_any_does_not_short_circuit_proper() {
        // proper_subtype=True: the fallback_to_any branch is skipped
        // (subtypes.py:493 `not self.proper_subtype`). a.AnyBase is not
        // a nominal base of a.Other and a.Other is not a protocol, so
        // Python records a negative cache and returns False
        // (subtypes.py:634-635).
        let mut base = snap("a.AnyBase", "AnyBase");
        base.fallback_to_any = true;
        let r = make_resolver(vec![base, snap("a.Other", "Other")]);
        let left = instance("a.AnyBase", vec![]);
        let right = instance("a.Other", vec![]);
        let ctx = SubtypeContext::new(false, false, false, false, true, true);
        assert_eq!(is_subtype(&left, &right, &ctx, &r), Some(false));
    }

    #[test]
    fn instance_left_none_right_is_false() {
        // Instance <: NoneType -> False (subtypes.py:683 else tail).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = instance("a.A", vec![]);
        let right = Type::NoneType;
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn fallback_to_any_left_none_right_is_false() {
        // fallback_to_any=True vs NoneType: the short-circuit excludes
        // NoneType (subtypes.py:638-643), so even a dynamic-base class is
        // not a subtype of None -> False (subtypes.py:683 else tail).
        let mut base = snap("a.AnyBase", "AnyBase");
        base.fallback_to_any = true;
        let r = make_resolver(vec![base]);
        let left = instance("a.AnyBase", vec![]);
        let right = Type::NoneType;
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn instance_left_typevartuple_right_defers() {
        // Instance <: TypeVarTupleType needs variadic-slice
        // reconstruction, not ported -> defer (subtypes.py:617-620).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = instance("a.A", vec![]);
        let right = Type::TypeVarTupleType {
            tuple_fallback: Box::new(instance("builtins.tuple", vec![])),
            name: "Ts".to_string(),
            fullname: "a.Ts".to_string(),
            raw_id: 0,
            namespace: "a".to_string(),
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            min_len: 0,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn instance_left_single_unpack_tuple_item_recurses() {
        // Instance <: (Unpack[Other],) -> recurse against Other
        // (subtypes.py:597-604). Other has no base relation to A, so the
        // nominal recursion records a negative result -> False.
        let r = make_resolver(vec![
            snap("a.A", "A"),
            snap("a.Other", "Other"),
            snap("builtins.tuple", "tuple"),
        ]);
        let left = instance("a.A", vec![]);
        let unpack = Type::UnpackType {
            typ: Box::new(instance("a.Other", vec![])),
        };
        let right = tuple_type(vec![unpack]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn alt_promote_matches_right() {
        // left.alt_promote_fullname == right.type_ref -> True
        // (subtypes.py:546-547).
        let mut s = snap("builtins.int", "int");
        s.alt_promote_fullname = Some("builtins.something".to_string());
        let r = make_resolver(vec![s, snap("builtins.something", "something")]);
        let left = instance("builtins.int", vec![]);
        let right = instance("builtins.something", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn generic_substitution_without_bases_returns_none() {
        // right has type_vars_with_variance and left.type != right.type,
        // but left has no bases blobs (snapshot not populated). The
        // map_instance_to_supertype walker returns None, falling through.
        let mut base = snap("a.Gen", "Gen");
        base.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let mut derived = snap("a.Sub", "Sub");
        derived.has_base.insert("a.Gen".to_string());
        derived.mro.push("a.Gen".to_string());
        let r = make_resolver(vec![base, derived]);
        let left = instance("a.Sub", vec![]);
        let right = instance("a.Gen", vec![any_type()]);
        // No bases blobs -> map_instance_to_supertype returns None.
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn expand_type_by_instance_substitutes_typevar() {
        // Instance[TypeVarType(T`1, ns="a.Sub")] with left = a.Sub[A]
        // -> Instance[A]. The TypeVar's (namespace, raw_id) matches
        // (left_ref, 1), so it's replaced by left_args[0].
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "a.Sub.T".to_string(),
            raw_id: 1,
            namespace: "a.Sub".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: COVARIANT,
            meta_level: 0,
        };
        let base = instance("a.Gen", vec![tvar]);
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&base, "a.Sub", &[left_arg.clone()]);
        assert_eq!(expanded, Some(instance("a.Gen", vec![left_arg])));
    }

    #[test]
    fn expand_type_by_instance_no_match_returns_none() {
        // TypeVar with a different namespace is not substituted. Python
        // leaves it as-is, but visit_typevar (the unmatched-tvar path)
        // is not ported, so Rust returns None to fall through.
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "a.Other.T".to_string(),
            raw_id: 1,
            namespace: "a.Other".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: COVARIANT,
            meta_level: 0,
        };
        let base = instance("a.Gen", vec![tvar]);
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&base, "a.Sub", &[left_arg]);
        assert_eq!(expanded, None);
    }

    #[test]
    fn expand_type_by_instance_recurses_into_instance_args() {
        // Instance[a.Gen[Instance[a.Gen[T]]]] with left = a.Sub[A]:
        // both outer and inner TypeVars get substituted.
        let tvar = || Type::TypeVarType {
            name: "T".to_string(),
            fullname: "a.Sub.T".to_string(),
            raw_id: 1,
            namespace: "a.Sub".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: COVARIANT,
            meta_level: 0,
        };
        let inner = instance("a.Gen", vec![tvar()]);
        let outer = instance("a.Gen", vec![inner]);
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&outer, "a.Sub", &[left_arg.clone()]);
        let expected = instance("a.Gen", vec![instance("a.Gen", vec![left_arg])]);
        assert_eq!(expanded, Some(expected));
    }

    #[test]
    fn expand_type_by_instance_passthrough_for_leaf_types() {
        // NoneType, AnyType, LiteralType pass through unchanged.
        assert_eq!(
            expand_type_by_instance(&Type::NoneType, "a.Sub", &[]),
            Some(Type::NoneType)
        );
        let any = any_type();
        assert_eq!(expand_type_by_instance(&any, "a.Sub", &[]), Some(any));
    }

    #[test]
    fn expand_type_by_instance_returns_none_for_unsupported() {
        // TupleType inside the tree is not handled by the subset walker.
        let t = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![])),
            items: vec![any_type()],
            implicit: false,
        };
        let expanded = expand_type_by_instance(&t, "a.Sub", &[any_type()]);
        assert_eq!(expanded, None);
    }

    #[test]
    fn same_type_covariant_args_subtype() {
        // A[A] <: A[A] when A's T is covariant. Args are Instance so the
        // Rust recurse handles them (AnyType right is handled Python-side
        // by the shim before calling into Rust).
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let r = make_resolver(vec![gen, snap("a.A", "A")]);
        let arg = instance("a.A", vec![]);
        let left = instance("a.Gen", vec![arg.clone()]);
        let right = instance("a.Gen", vec![arg]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn tuple_left_defers_when_right_is_unrelated_instance() {
        // TupleType <: a.A: fallback (builtins.tuple) is not a subtype of
        // a.A, and a.A is not a protocol -> False (subtypes.py:983-996).
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.tuple", "tuple")]);
        let tuple = tuple_type(vec![instance("a.A", vec![])]);
        let right = instance("a.A", vec![]);
        assert_eq!(is_subtype(&tuple, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn tuple_left_subtype_of_builtins_tuple() {
        // (A,) <: tuple[A] -> True (subtypes.py:955-979).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let tuple = tuple_type(vec![instance("a.A", vec![])]);
        let right = instance("builtins.tuple", vec![instance("a.A", vec![])]);
        assert_eq!(is_subtype(&tuple, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn tuple_left_subtype_of_sequence() {
        // (A,) <: Sequence[A] -> True (TUPLE_LIKE in
        // TUPLE_LIKE_INSTANCE_NAMES).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let tuple = tuple_type(vec![instance("a.A", vec![])]);
        let right = instance("typing.Sequence", vec![instance("a.A", vec![])]);
        assert_eq!(is_subtype(&tuple, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn tuple_left_subtype_of_sized() {
        // Any tuple <: Sized -> True (subtypes.py:953-954).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let tuple = tuple_type(vec![instance("a.A", vec![])]);
        let right = instance("typing.Sized", vec![]);
        assert_eq!(is_subtype(&tuple, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn tuple_left_subtype_of_tuple_right() {
        // (A, B) <: (A, B) -> True (length + item-wise + fallback).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let left = tuple_type(vec![instance("a.A", vec![]), instance("a.B", vec![])]);
        let right = tuple_type(vec![instance("a.A", vec![]), instance("a.B", vec![])]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn tuple_left_not_subtype_of_different_length_tuple() {
        // (A,) !<: (A, B) -> False (length mismatch, subtypes.py:1006).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let left = tuple_type(vec![instance("a.A", vec![])]);
        let right = tuple_type(vec![instance("a.A", vec![]), instance("a.B", vec![])]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn tuple_left_defer_on_variadic_unpack() {
        // *tuple[X, ...] in the items: variadic path not ported, so defer
        // to Python (subtypes.py:1004-1005).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let unpack = Type::UnpackType {
            typ: Box::new(instance("builtins.tuple", vec![instance("a.A", vec![])])),
        };
        let left = tuple_type(vec![unpack]);
        let right = tuple_type(vec![instance("a.A", vec![])]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn tuple_left_unpacks_unpack_item_for_tuple_like() {
        // (*tuple[A, ...],) <: Iterable[A] -> True: the UnpackType item's
        // inner element type is checked (subtypes.py:968-978).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let unpack = Type::UnpackType {
            typ: Box::new(instance("builtins.tuple", vec![instance("a.A", vec![])])),
        };
        let left = tuple_type(vec![unpack]);
        let right = instance("typing.Iterable", vec![instance("a.A", vec![])]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn variance_not_ready_returns_none() {
        // When a tvar has VARIANCE_NOT_READY, Python calls
        // infer_class_variances (mutates live defn); we return None.
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars_with_variance = vec![("T".to_string(), VARIANCE_NOT_READY, 0)];
        let r = make_resolver(vec![gen]);
        let left = instance("a.Gen", vec![any_type()]);
        let right = instance("a.Gen", vec![any_type()]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn ignore_type_params_short_circuits_to_true() {
        // When ignore_type_params is set, nominal base -> True.
        let mut b = snap("a.B", "B");
        b.has_base.insert("a.A".to_string());
        b.mro.push("a.A".to_string());
        let r = make_resolver(vec![snap("a.A", "A"), b]);
        let ctx = SubtypeContext::new(true, false, false, false, false, true);
        let left = instance("a.B", vec![any_type()]);
        let right = instance("a.A", vec![any_type()]);
        assert_eq!(is_subtype(&left, &right, &ctx, &r), Some(true));
    }

    #[test]
    fn invariant_args_equivalent_when_same() {
        // A[A] <: A[A] when A's T is invariant: is_equivalent =
        // is_subtype both ways, each direction is A <: A = True.
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars_with_variance = vec![("T".to_string(), INVARIANT, 0)];
        let r = make_resolver(vec![gen, snap("a.A", "A")]);
        let arg = instance("a.A", vec![]);
        let left = instance("a.Gen", vec![arg.clone()]);
        let right = instance("a.Gen", vec![arg]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    // ---- visit_none_type / visit_uninhabited_type / visit_deleted_type (M8aa) ----

    fn ctx_strict_optional(strict_optional: bool) -> SubtypeContext {
        SubtypeContext::new(false, false, false, false, false, strict_optional)
    }

    #[test]
    fn none_subtype_of_none_strict_optional() {
        // visit_none_type (subtypes.py:539-541): right is NoneType -> True.
        let r = make_resolver(vec![]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &Type::NoneType,
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_subtype_of_object_strict_optional() {
        // visit_none_type (subtypes.py:539-541): right is builtins.object
        // -> True (is_named_instance check).
        let r = make_resolver(vec![snap("builtins.object", "object")]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("builtins.object", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_not_subtype_of_instance_strict_optional() {
        // visit_none_type (subtypes.py:551): strict_optional + right is
        // Instance (non-protocol) -> False. Protocol detection needs the
        // snapshot's is_protocol field; this test uses a non-protocol
        // Instance, so we return False.
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("a.A", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(false)
        );
    }

    #[test]
    fn none_subtype_of_anything_when_optional_disabled() {
        // visit_none_type (subtypes.py:553-554): strict_optional=False
        // -> True for any right.
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("a.A", vec![]),
                &ctx_strict_optional(false),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_subtype_of_protocol_with_hashable_members() {
        // visit_none_type (subtypes.py:543-549): right is a protocol
        // Instance. When all protocol_members are __hash__/__str__
        // (or members is empty), Python returns True.
        let mut proto = snap("typing.Hashable", "Hashable");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__hash__".to_string()];
        let r = make_resolver(vec![proto]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("typing.Hashable", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn none_not_subtype_of_protocol_with_other_members() {
        // visit_none_type (subtypes.py:543-549): right is a protocol
        // Instance but members include something other than
        // __hash__/__str__ -> False.
        let mut proto = snap("typing.Iterable", "Iterable");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__iter__".to_string()];
        let r = make_resolver(vec![proto]);
        assert_eq!(
            is_subtype(
                &Type::NoneType,
                &instance("typing.Iterable", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(false)
        );
    }

    #[test]
    fn uninhabited_subtype_of_anything() {
        // visit_uninhabited_type (subtypes.py:555-556): UninhabitedType
        // is a subtype of everything.
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::UninhabitedType { ambiguous: false },
                &instance("a.A", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn deleted_subtype_of_anything() {
        let r = make_resolver(vec![snap("a.A", "A")]);
        assert_eq!(
            is_subtype(
                &Type::DeletedType { source: None },
                &instance("a.A", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_subtyping_structure() {
        let t1 = Type::TypedDictType {
            fallback: Box::new(instance("builtins.dict", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        let t2 = Type::TypedDictType {
            fallback: Box::new(instance("builtins.dict", vec![])),
            items: vec![],
            required_keys: Default::default(),
            readonly_keys: Default::default(),
            is_closed: false,
        };
        let r = make_resolver(vec![]);
        assert_eq!(
            is_subtype(&t1, &t2, &ctx_strict_optional(true), &r),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_structural_subtype_different_fallbacks() {
        // M18 parity: two TypedDicts with identical items but different
        // fallback type_refs (e.g. ast_serialize.ParseError vs
        // mypy.nodes.ParseError) must be compatible. Python ignores
        // fallbacks (subtypes.py:1093); the old Rust `left == right`
        // check rejected this.
        let mk_typeddict = |type_ref: &str| Type::TypedDictType {
            fallback: Box::new(instance(type_ref, vec![])),
            items: vec![("code".to_string(), instance("builtins.int", vec![]))],
            required_keys: ["code".to_string()].into_iter().collect(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        let t1 = mk_typeddict("ast_serialize.ParseError");
        let t2 = mk_typeddict("mypy.nodes.ParseError");
        // builtins.int snap needed for the invariant value-type check
        // on the shared "code" item.
        let r = make_resolver(vec![snap("builtins.int", "int")]);
        assert_eq!(
            is_subtype(&t1, &t2, &ctx_strict_optional(true), &r),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_structural_subtype_same_items_different_fallback() {
        // Both directions should be True (symmetric structural match).
        let mk_typeddict = |type_ref: &str| Type::TypedDictType {
            fallback: Box::new(instance(type_ref, vec![])),
            items: vec![
                ("msg".to_string(), instance("builtins.str", vec![])),
                ("code".to_string(), instance("builtins.int", vec![])),
            ],
            required_keys: ["msg".to_string(), "code".to_string()]
                .into_iter()
                .collect(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        let t1 = mk_typeddict("mypy.ast_serialize.ParseError");
        let t2 = mk_typeddict("mypy.nodes.ParseError");
        let r = make_resolver(vec![
            snap("builtins.str", "str"),
            snap("builtins.int", "int"),
        ]);
        assert_eq!(
            is_subtype(&t1, &t2, &ctx_strict_optional(true), &r),
            Some(true)
        );
        assert_eq!(
            is_subtype(&t2, &t1, &ctx_strict_optional(true), &r),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_required_key_must_remain_required() {
        // left has "x" as optional, right has "x" as required -> not a
        // subtype (subtypes.py:1053-1055).
        let mk = |required: bool| Type::TypedDictType {
            fallback: Box::new(instance("builtins.dict", vec![])),
            items: vec![("x".to_string(), instance("builtins.int", vec![]))],
            required_keys: if required {
                ["x".to_string()].into_iter().collect()
            } else {
                HashSet::new()
            },
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        let r = make_resolver(vec![]);
        assert_eq!(
            is_subtype(&mk(false), &mk(true), &ctx_strict_optional(true), &r),
            Some(false)
        );
        // required -> optional is NOT OK: right is mutable+optional, so
        // delete-ability must be preserved (subtypes.py:1058-1059);
        // left still requires the key.
        assert_eq!(
            is_subtype(&mk(true), &mk(false), &ctx_strict_optional(true), &r),
            Some(false)
        );
    }

    #[test]
    fn test_typeddict_closed_must_remain_closed() {
        // right is closed, left is not -> not a subtype
        // (subtypes.py:1048-1049).
        let mk = |is_closed: bool| Type::TypedDictType {
            fallback: Box::new(instance("builtins.dict", vec![])),
            items: vec![("x".to_string(), instance("builtins.int", vec![]))],
            required_keys: ["x".to_string()].into_iter().collect(),
            readonly_keys: HashSet::new(),
            is_closed,
        };
        let r = make_resolver(vec![snap("builtins.int", "int")]);
        assert_eq!(
            is_subtype(&mk(false), &mk(true), &ctx_strict_optional(true), &r),
            Some(false)
        );
        // closed -> closed is OK.
        assert_eq!(
            is_subtype(&mk(true), &mk(true), &ctx_strict_optional(true), &r),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_readonly_item_covariant() {
        // Read-only items are covariant: int <: object -> TD(ro x: int)
        // <: TD(ro x: object).
        let mk = |item_type: &str| {
            let readonly_keys: HashSet<String> = ["x".to_string()].into_iter().collect();
            Type::TypedDictType {
                fallback: Box::new(instance("builtins.dict", vec![])),
                items: vec![("x".to_string(), instance(item_type, vec![]))],
                required_keys: ["x".to_string()].into_iter().collect(),
                readonly_keys,
                is_closed: false,
            }
        };
        let r = make_resolver(vec![
            snap("builtins.int", "int"),
            snap("builtins.object", "object"),
        ]);
        assert_eq!(
            is_subtype(
                &mk("builtins.int"),
                &mk("builtins.object"),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_mutable_item_invariant() {
        // Mutable (non-readonly) items are invariant: int is not
        // equivalent to object -> not a subtype.
        let mk = |item_type: &str| Type::TypedDictType {
            fallback: Box::new(instance("builtins.dict", vec![])),
            items: vec![("x".to_string(), instance(item_type, vec![]))],
            required_keys: ["x".to_string()].into_iter().collect(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        let r = make_resolver(vec![
            snap("builtins.int", "int"),
            snap("builtins.object", "object"),
        ]);
        assert_eq!(
            is_subtype(
                &mk("builtins.int"),
                &mk("builtins.object"),
                &ctx_strict_optional(true),
                &r
            ),
            Some(false)
        );
        // int <: int is trivially equivalent.
        assert_eq!(
            is_subtype(
                &mk("builtins.int"),
                &mk("builtins.int"),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_subtype_instance_fallback() {
        // right is Instance: is_subtype(left.fallback, right).
        let td = Type::TypedDictType {
            fallback: Box::new(instance("mypy.nodes.ParseError", vec![])),
            items: vec![],
            required_keys: HashSet::new(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        let mut base = snap("mypy.nodes.ParseError", "ParseError");
        base.has_base.insert("builtins.object".to_string());
        base.mro.push("builtins.object".to_string());
        let r = make_resolver(vec![base, snap("builtins.object", "object")]);
        assert_eq!(
            is_subtype(
                &td,
                &instance("builtins.object", vec![]),
                &ctx_strict_optional(true),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn test_typeddict_not_subtype_unrelated_type() {
        // TypedDict vs non-Instance/non-TypedDict right -> False
        // (subtypes.py:1095-1096).
        let td = Type::TypedDictType {
            fallback: Box::new(instance("builtins.dict", vec![])),
            items: vec![],
            required_keys: HashSet::new(),
            readonly_keys: HashSet::new(),
            is_closed: false,
        };
        let r = make_resolver(vec![]);
        assert_eq!(
            is_subtype(&td, &Type::NoneType, &ctx_strict_optional(true), &r),
            Some(false)
        );
    }

    // ---- visit_type_type (issue #443, subtypes.py:1251-1311) ----

    #[test]
    fn type_type_type_form_subtype_of_type_type_type_form() {
        // left.is_type_form and right.is_type_form: recurse on items.
        // subtypes.py:1253-1257.
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: true,
        };
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: true,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn type_type_type_form_not_subtype_when_right_not_type_form() {
        // left.is_type_form, right.is_type_form=False -> False
        // (subtypes.py:1255-1256).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: true,
        };
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn type_type_type_form_subtype_of_object_instance() {
        // left.is_type_form, right=builtins.object -> True
        // (subtypes.py:1259-1260).
        let r = make_resolver(vec![snap("builtins.object", "object")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: true,
        };
        let right = instance("builtins.object", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn type_type_type_form_not_subtype_of_non_object_instance() {
        // left.is_type_form, right=Instance (not object) -> False
        // (subtypes.py:1261).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: true,
        };
        let right = instance("a.A", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn type_type_not_type_form_subtype_of_type_type() {
        // not left.is_type_form, right is TypeType: recurse on items
        // (subtypes.py:1264-1265).
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn type_type_not_type_form_subtype_of_object_instance() {
        // not left.is_type_form, right=builtins.object -> True
        // (subtypes.py:1294-1295).
        let r = make_resolver(vec![snap("builtins.object", "object")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        let right = instance("builtins.object", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn type_type_not_type_form_subtype_of_type_instance() {
        // not left.is_type_form, right=builtins.type -> True
        // (subtypes.py:1294-1295).
        let r = make_resolver(vec![snap("builtins.type", "type")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        let right = instance("builtins.type", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn type_type_not_type_form_defers_callable_right() {
        // right is CallableType: needs constructor matching (Python-only).
        // Defer (subtypes.py:1271-1292).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        let right = callable_type(vec![], Type::NoneType, None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn type_type_not_type_form_defers_non_object_instance() {
        // right is Instance (not object/type): needs metaclass check
        // (Python-only). Defer (subtypes.py:1294-1310).
        let r = make_resolver(vec![snap("a.A", "A"), snap("a.B", "B")]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        let right = instance("a.B", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn type_type_not_type_form_not_subtype_of_none() {
        // right is NoneType: falls to else -> False (subtypes.py:1311).
        let r = make_resolver(vec![]);
        let left = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(
            is_subtype(&left, &Type::NoneType, &ctx_nominal(), &r),
            Some(false)
        );
    }

    // ---- visit_overloaded (issue #443, subtypes.py:1113-1194) ----

    #[test]
    fn overloaded_subtype_of_callable_when_one_matches() {
        // right is CallableType: at least one overload item must match
        // (subtypes.py:1126-1130). Each item is CallableType vs
        // CallableType, which defers to the callable_compat engine (not
        // called from recursive is_subtype), so None propagates.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let item = callable_type(vec![], Type::NoneType, None);
        let left = Type::Overloaded {
            items: vec![item.clone()],
        };
        assert_eq!(is_subtype(&left, &item, &ctx_nominal(), &r), None);
    }

    #[test]
    fn overloaded_not_subtype_of_callable_when_none_match() {
        // right is CallableType: no item matches -> False.
        // The items differ (ret_type None vs int instance), and
        // Callable-vs-Callable defers (callable_compat not called from
        // recursive is_subtype), so None propagates.
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap("builtins.int", "int"),
        ]);
        let item1 = callable_type(vec![], Type::NoneType, None);
        let item2 = callable_type(vec![], instance("builtins.int", vec![]), None);
        let left = Type::Overloaded {
            items: vec![item1.clone()],
        };
        // item1 vs item2: Callable-vs-Callable defers -> None.
        assert_eq!(is_subtype(&left, &item2, &ctx_nominal(), &r), None);
    }

    #[test]
    fn overloaded_subtype_of_same_overloaded() {
        // right is Overloaded: left == right -> True (subtypes.py:1132-1134).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let item = callable_type(vec![], Type::NoneType, None);
        let ov = Type::Overloaded { items: vec![item] };
        assert_eq!(is_subtype(&ov, &ov, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn overloaded_defers_instance_right() {
        // right is Instance: needs find_member/is_protocol_implementation
        // (Python-only). Defer (subtypes.py:1115-1125).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let item = callable_type(vec![], Type::NoneType, None);
        let left = Type::Overloaded { items: vec![item] };
        let right = instance("builtins.function", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    // ---- visit_callable_type (issue #443, subtypes.py:807-889) ----

    #[test]
    fn callable_subtype_of_non_protocol_instance_via_fallback() {
        // right is non-protocol Instance: is_subtype(left.fallback, right)
        // (subtypes.py:884). builtins.function <: builtins.object.
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap("builtins.object", "object"),
        ]);
        let mut func_snap = snap("builtins.function", "function");
        func_snap.has_base.insert("builtins.object".to_string());
        func_snap.mro.push("builtins.object".to_string());
        let r = make_resolver(vec![func_snap, snap("builtins.object", "object")]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = instance("builtins.object", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn callable_not_subtype_of_unrelated_non_protocol_instance() {
        // right is non-protocol Instance, fallback not a subtype -> False.
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap("a.A", "A"),
        ]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = instance("a.A", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn callable_defers_protocol_instance_right() {
        // right is protocol Instance: needs find_member /
        // is_protocol_implementation (Python-only). Defer
        // (subtypes.py:869-883).
        let mut proto = snap("proto.P", "P");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__call__".to_string()];
        let r = make_resolver(vec![proto, snap("builtins.function", "function")]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = instance("proto.P", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn callable_defers_callable_right() {
        // right is CallableType: handled by callable_compat (Python shim),
        // not this function. Defer (subtypes.py:809-865).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = callable_type(vec![], Type::NoneType, None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn callable_defers_overloaded_right() {
        // right is Overloaded: each item is Callable-vs-Callable (defers).
        // Defer the whole check (subtypes.py:866-867).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let item = callable_type(vec![], Type::NoneType, None);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = Type::Overloaded { items: vec![item] };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn callable_subtype_of_type_type_with_instance_type() {
        // right is TypeType, left has instance_type set: recurse on
        // instance_type <: right.item (subtypes.py:885-887).
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = callable_type(vec![], Type::NoneType, Some(instance("a.A", vec![])));
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn callable_defers_type_type_without_instance_type() {
        // right is TypeType, left.instance_type is None: can't decide
        // is_type_obj() without fallback.is_metaclass(). Defer
        // (subtypes.py:885-887).
        let r = make_resolver(vec![]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn callable_not_subtype_of_none() {
        // right is NoneType: falls to else -> False (subtypes.py:888-889).
        let r = make_resolver(vec![]);
        let left = callable_type(vec![], Type::NoneType, None);
        assert_eq!(
            is_subtype(&left, &Type::NoneType, &ctx_nominal(), &r),
            Some(false)
        );
    }
}
