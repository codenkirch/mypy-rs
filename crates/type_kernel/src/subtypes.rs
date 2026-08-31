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

use pyo3::basic::CompareOp;
use pyo3::prelude::*;
use std::cell::RefCell;
use std::collections::HashSet;

use crate::typeinfo::{NativeTypeResolver, TypeInfoSnapshot, TypeResolver};
use crate::wire::{self, ReadBuffer, Type, WriteBuffer};

/// Variance constants mirroring `mypy.nodes` (nodes.py:3146).
pub(crate) const INVARIANT: i64 = 0;
pub(crate) const COVARIANT: i64 = 1;
pub(crate) const CONTRAVARIANT: i64 = 2;
pub(crate) const VARIANCE_NOT_READY: i64 = 3;

/// `ArgKind.ARG_STAR` (nodes.py:2563) — the `*args` formal kind. Used
/// by the expand_type_by_instance CallableType arm to detect a var-arg
/// typed `UnpackType` (deferred interpolation).
const ARG_STAR: i64 = 2;

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

/// Outcome of the snapshot MRO walk mirroring `TypeInfo.get(name)` +
/// `is_valid_constructor` for the visit_type_type constructor decision.
enum MroCtorDefiner {
    /// First MRO entry whose own names define `name` with a constructor
    /// node (FuncBase or Decorator); the value is `node.info.fullname`.
    Definer(String),
    /// The name is defined somewhere on the MRO but the node is not a valid
    /// constructor (Var or another kind): `type_object_type` returns the
    /// invalid-class-definition Any.
    Invalid,
    /// Not defined anywhere on the MRO.
    Missing,
    /// An MRO entry's snapshot is absent: cannot decide (defer).
    Defer,
}

/// MRO walk for `info.get(name)` with `is_valid_constructor` verdicts,
/// decided from each MRO entry's own `member_definers` / `member_info`
/// map. `member_definers` stores FuncBase/Decorator/Var nodes with each
/// node's `info.fullname`; a name present in the wider `member_info` map
/// but absent from `member_definers` is a node whose kind is neither, i.e.
/// invalid.
fn mro_constructor_definer(
    snap: &TypeInfoSnapshot,
    name: &str,
    resolver: &TypeResolver,
) -> MroCtorDefiner {
    for entry in &snap.mro {
        let Some(entry_snap) = resolver.get(entry) else {
            return MroCtorDefiner::Defer;
        };
        let Some((kind, definer)) = entry_snap.member_definers.get(name) else {
            if entry_snap.member_info.contains_key(name) {
                return MroCtorDefiner::Invalid;
            }
            continue;
        };
        if *kind == 0 || *kind == 1 {
            return MroCtorDefiner::Definer(definer.clone());
        }
        return MroCtorDefiner::Invalid;
    }
    MroCtorDefiner::Missing
}

/// The visit_type_type Instance-item decision head
/// (subtypes.py:1816-1832). Decides the constructor path when the chosen
/// `__init__`/`__new__` method is `builtins.object`'s `__init__`
/// (`ret_type` None; universal callable with ret Any on a fallback_to_any
/// tie). Everything else needs live `function_type` / `class_callable`
/// bytes and defers (`None`).
fn type_object_ret_decision(
    item_type_ref: &str,
    right_ret: &Type,
    left_item: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    let item_snap = resolver.get(item_type_ref)?;
    let init = mro_constructor_definer(item_snap, "__init__", resolver);
    let new = mro_constructor_definer(item_snap, "__new__", resolver);
    // Python: a missing or invalid __init__, or (when __new__ exists) an
    // invalid __new__, makes type_object_type return the
    // invalid-class-definition Any, which falls through to the unsound
    // tail. A missing __new__ is replaced by __init__ (an MRO tie).
    let init = match init {
        MroCtorDefiner::Definer(def) => Some(def),
        MroCtorDefiner::Invalid | MroCtorDefiner::Missing => None,
        MroCtorDefiner::Defer => return None,
    };
    let Some(init_definer) = init else {
        return is_subtype(left_item, right_ret, ctx, resolver);
    };
    let new = match new {
        MroCtorDefiner::Definer(def) => Some(def),
        MroCtorDefiner::Invalid => {
            return is_subtype(left_item, right_ret, ctx, resolver);
        }
        MroCtorDefiner::Missing => None,
        MroCtorDefiner::Defer => return None,
    };
    let init_idx = item_snap.mro.iter().position(|m| m == &init_definer);
    let Some(init_idx) = init_idx else {
        // `mro.index(node.info)` failure is unreachable for sane snapshots.
        return None;
    };
    let (_new_definer, new_idx) = match new {
        Some(def) => {
            let idx = item_snap.mro.iter().position(|m| m == &def);
            (Some(def), idx)
        }
        None => (None, Some(init_idx)),
    };
    let new_idx = new_idx?;
    if init_idx > new_idx {
        // __new__ wins the MRO race. object.__new__ carries a Self-typed
        // ret that only the live expansion can decide; everything else
        // needs live `function_type` bytes. Defer.
        return None;
    }
    if init_definer == "builtins.object" {
        // On the exact MRO tie for a bogus-base (fallback_to_any) class,
        // Python builds the universal callable with ret Any.
        if init_idx == new_idx && item_snap.fallback_to_any {
            return Some(true);
        }
        // Python keeps going: the method is object.__init__ and
        // type_object_type_from_function replaces a non-__new__ ret with
        // fill_typevars(info). expand_type_by_instance(constructor, item)
        // maps those class tvars onto item's args, so the compared ret is
        // structurally `item` itself for a plain non-generic class with
        // this type_ref. Generic classes (TVT/ParamSpec/unpack shapes)
        // defer; the unsound tail is not reachable from this arm.
        if let Type::Instance { type_ref, args, .. } = left_item {
            if *type_ref == item_type_ref
                && args.is_empty()
                && item_snap.type_vars_with_variance.is_empty()
            {
                return is_subtype(left_item, right_ret, ctx, resolver);
            }
        }
        return None;
    }
    None
}

fn any_type_of(type_of_any: i64) -> Type {
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
    /// `SubtypeContext.ignore_pos_arg_names` (subtypes.py:192): when true,
    /// positional-argument names are not required to match in callable
    /// compatibility. Read by the callable_compat engine (Stage C1).
    pub ignore_pos_arg_names: bool,
    /// `strict_concatenate` for the callable_compat engine: Python computes
    /// it as `options.extra_checks or options.strict_concatenate` at the
    /// subtype call site (subtypes.py:944-947) and passes it explicitly;
    /// stored on the context so the `visit_callable_type` port can forward
    /// it to the native engine without an options lookup.
    pub strict_concatenate: bool,
    /// `SubtypeContext.erase_instances` (subtypes.py:508): erase the left
    /// Instance *after* mapping it to the supertype (subtypes.py:1151-1155),
    /// so per-arg checks compare erased args against the right args. Only
    /// `covers_at_runtime` and `map_instance_to_supertype`-level
    /// `is_proper_subtype(..., erase_instances=True)` callers set it.
    pub erase_instances: bool,
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
        Self::with_callable_flags(
            ignore_type_params,
            ignore_declared_variance,
            always_covariant,
            ignore_promotions,
            proper_subtype,
            strict_optional,
            false, // ignore_pos_arg_names
            false, // strict_concatenate
        )
    }

    /// Full constructor including the callable-compat flags. `new` keeps the
    /// historical 6-arg shape (the callable flags default to False) and every
    /// pre-C1 call site is unchanged.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_callable_flags(
        ignore_type_params: bool,
        ignore_declared_variance: bool,
        always_covariant: bool,
        ignore_promotions: bool,
        proper_subtype: bool,
        strict_optional: bool,
        ignore_pos_arg_names: bool,
        strict_concatenate: bool,
    ) -> Self {
        Self {
            ignore_type_params,
            ignore_declared_variance,
            always_covariant,
            ignore_promotions,
            proper_subtype,
            strict_optional,
            ignore_pos_arg_names,
            strict_concatenate,
            erase_instances: false,
        }
    }
}

/// Entry point mirroring `mypy.subtypes._is_subtype` for the nominal path.
///
/// Returns `Some(bool)` when Rust decided the check; `None` when the
/// variant is not handled (Python falls through). The Python shim is
/// responsible for `get_proper_type` expansion, the `AnyType`/`UnboundType`/
/// `ErasedType` right short-circuit (subtypes.py:754-761; mirrored below
/// for the shim-bypassing recursive paths), the `UnionType`
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
    if matches!(left, Type::TypeAliasType { .. }) || matches!(right, Type::TypeAliasType { .. }) {
        // Python expands both operands with get_proper_type before every
        // comparison (subtypes.py:346-347); recursive check_type_parameter
        // and callable-compat paths bypass that, so defer unexpanded aliases.
        return None;
    }
    // subtypes.py:754-761: a non-proper subtype of an Any/Unbound/Erased
    // right is always True, unless left is UnpackType (defer mirroring
    // Python's `not isinstance(left, UnpackType)` guard).

    // The shim applies this at the top-level entry only, but recursive
    // calls from check_type_parameter and the internal visit_* recursions
    // (e.g. TypeType-vs-TypeType items, subtypes.py:1323) bypass it.

    // UnboundType right comes from forward references that survive
    // get_proper_type. Nested ErasedType (wire tag 122) still crosses
    // FFI: the shim gate filters only top-level Erased (subtypes.py:842).

    // left=ErasedType defers at the visit_* dispatch below (Python's
    // visit_erased_type answers `not keep_erased_types`, which native
    // contexts never set).
    if !ctx.proper_subtype
        && matches!(
            right,
            Type::AnyType { .. } | Type::UnboundType { .. } | Type::ErasedType
        )
        && !matches!(left, Type::UnpackType { .. })
    {
        return Some(true);
    }
    // TupleType left: the visit_tuple_type port below returns None for the
    // variadic cases (Unpack items, TypeVarTuple), which defer to Python's
    // SubtypeVisitor (subtypes.py:950-1037).

    // _is_subtype (subtypes.py:363-410): when right is UnionType and left is not,
    // left <: right iff left <: some item. Python checks this before the visitor
    // dispatch; it must fire before the NoneType handler (False for UnionType).

    // check_type_parameter recursions bypass the Python shim, so mirroring it
    // here is what gives union-typed type arguments the right answer.
    if let Type::UnionType { items, .. } = right {
        if !matches!(left, Type::UnionType { .. }) {
            if matches!(left, Type::TypeVarType { .. }) {
                // subtypes.py:829-833: tvar-left union-right only
                // short-circuits True per item; no-match falls to the
                // visitor (upper bound may itself be a union, #2314).
                let mut saw_defer = false;
                for item in items {
                    match is_subtype(left, item, ctx, resolver) {
                        Some(true) => return Some(true),
                        Some(false) => {}
                        None => {
                            saw_defer = true;
                        }
                    }
                }
                if saw_defer {
                    return None;
                }
            } else {
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
                        any_type_of(ANY_SPECIAL_FORM)
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
            // subtypes.py:1004-1005: variadic unpack handling first.
            // Decides when the right side has a variadic unpack; `None`
            // falls through to the fixed-length logic below.
            if let Some(decided) = variadic_tuple_subtype(left, right, ctx, resolver)? {
                return Some(decided);
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
        // right is Overloaded (subtypes.py:1240-1243): only a constructor
        // overload qualifies (items[0].is_type_obj() decides); then
        // `left <: right.items[0]`, a TypeType-vs-CallableType recursion.
        if let Type::Overloaded { items: right_items } = right {
            let first = right_items.first()?;
            return match crate::callable_compat::is_type_obj(first, resolver) {
                Some(true) => is_subtype(left, first, ctx, resolver),
                Some(false) => Some(false),
                None => None,
            };
        }
        // right is CallableType (subtypes.py:1245-1278). Type[X] <: Callable
        // is unsound (no __init__ check): the item is compared against
        // right.ret_type, except for an Instance item (type_object_type).
        if let Type::CallableType {
            ret_type: right_ret,
            ..
        } = right
        {
            // subtypes.py:1246-1250: a proper-subtype comparison of
            // Type[X] against a non-type-object Callable is False
            // (transitivity); a deferred is_type_obj defers.
            if ctx.proper_subtype {
                match crate::callable_compat::is_type_obj(right, resolver) {
                    Some(false) => return Some(false),
                    Some(true) => {}
                    None => return None,
                }
            }
            // subtypes.py:1251-1255: a TupleType item compares via its
            // tuple_fallback (always a tuple Instance, so the Instance
            // check below defers it); a fallback failure defers.
            let item_ref: &Type;
            let tuple_fb: Type;
            if let Type::TupleType { .. } = left_item.as_ref() {
                tuple_fb = crate::typeops::tuple_fallback(left_item.as_ref(), resolver)?;
                item_ref = &tuple_fb;
            } else {
                item_ref = left_item.as_ref();
            }
            // subtypes.py:1256-1268: an Instance item compares through the
            // constructor (`type_object_type(item.type)` after expansion).
            // The wire snapshot carries no method type bytes, so only the
            // object-defined subset is decidable: when the chosen method is
            // object's `__init__`, the constructor's `ret_type` is None
            // (or Any for a fallback_to_any tie, the universal callable).
            // A user-defined or semanal-incomplete constructor defers.
            if let Type::Instance {
                type_ref: item_type_ref,
                ..
            } = item_ref
            {
                return type_object_ret_decision(
                    item_type_ref,
                    right_ret,
                    left_item,
                    ctx,
                    resolver,
                );
            }
            // subtypes.py:1271: fallthrough — unsound, no __init__ check.
            return is_subtype(left_item, right_ret, ctx, resolver);
        }
        // right is Instance (subtypes.py:1244-1256).
        if let Type::Instance {
            type_ref: right_ref,
            ..
        } = right
        {
            // builtins.object and builtins.type are always True.
            if right_ref == "builtins.object" || right_ref == "builtins.type" {
                return Some(true);
            }
            // A protocol right needs the class_obj
            // `is_protocol_implementation` check; defer.
            if resolver.get(right_ref).is_some_and(|s| s.is_protocol) {
                return None;
            }
            // item unwrap (subtypes.py:1248-1249): a TypeVarType item
            // unwraps to its upper bound; a non-Instance item returns
            // False.
            let item = match left_item.as_ref() {
                Type::TypeVarType { upper_bound, .. } => {
                    get_proper_type_or_defer(upper_bound.as_ref(), resolver)?
                }
                t => t,
            };
            let Type::Instance {
                type_ref: item_ref, ..
            } = item
            else {
                return Some(false);
            };
            // metaclass check (subtypes.py:1253-1254): the item's metaclass
            // must be a subtype of right; the snapshot carries it only
            // for classes with an explicit or inherited metaclass.

            // An absent one defers: Python's live `metaclass_type` is
            // context-sensitive (the overlap path falls back to
            // `right.has_base("builtins.type")`).

            // A decided False here broke `issubclass(x, cls)` on plain
            // classes (issue #1121).
            let item_snap = resolver.get(item_ref)?;
            let Some(meta_fullname) = &item_snap.metaclass_fullname else {
                return None;
            };
            let meta_snap = resolver.get(meta_fullname);
            if meta_snap.is_some_and(|s| !s.type_vars_with_variance.is_empty()) {
                return None;
            }
            let metaclass = Type::Instance {
                type_ref: meta_fullname.clone(),
                args: Vec::new(),
                last_known_value: None,
                extra_attrs: None,
            };
            return is_subtype(&metaclass, right, ctx, resolver);
        }
        return Some(false);
    }
    // visit_overloaded (subtypes.py:1104-1169): Overloaded left.
    if let Type::Overloaded { items: left_items } = left {
        // right is Instance (subtypes.py:1105-1119).
        if let Type::Instance {
            type_ref: right_ref,
            ..
        } = right
        {
            // A protocol with an explicit `__call__` member needs
            // find_member + the live member loop
            // (is_protocol_implementation, skip=["__call__"]).

            // Both need live machinery; defer so Python decides.
            if resolver.get(right_ref).is_some_and(|s| {
                s.is_protocol && s.protocol_members.iter().any(|m| m == "__call__")
            }) {
                return None;
            }
            // subtypes.py:1118: a plain Instance right recurses on the
            // overload's fallback (`Overloaded.fallback` is the first
            // item's fallback).
            let fallback = match left_items.first().map(|i| match i {
                Type::CallableType { fallback, .. } => Some(fallback.as_ref()),
                _ => None,
            }) {
                Some(Some(f)) => f,
                _ => {
                    return None;
                }
            };
            return is_subtype(fallback, right, ctx, resolver);
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
        // right is Overloaded: structural overload matching mirroring
        // visit_overloaded (subtypes.py:1148-1170): each right item
        // matched in order; overlap-only is a fail.
        if left == right {
            return Some(true);
        }
        let Type::Overloaded { items: right_items } = right else {
            // subtypes.py:1169-1177 tail: right UnboundType -> True; right
            // TypeType -> `left.is_type_obj() and left.items[0] <: right`
            // (items share the type-object status); any other right: False.
            if matches!(right, Type::UnboundType { .. }) {
                return Some(true);
            }
            if let Type::TypeType { .. } = right {
                let first = left_items.first()?;
                return match crate::callable_compat::is_type_obj(first, resolver) {
                    Some(true) => is_subtype(first, right, ctx, resolver),
                    Some(false) => Some(false),
                    None => None,
                };
            }
            return Some(false);
        };
        let mut previous_match_left_index: i64 = -1;
        let mut matched_overloads: HashSet<usize> = HashSet::new();
        for right_item in right_items {
            let mut found_match = false;
            for (left_index, left_item) in left_items.iter().enumerate() {
                let subtype_match = is_subtype(left_item, right_item, ctx, resolver)?;
                if subtype_match && previous_match_left_index <= left_index as i64 {
                    previous_match_left_index = left_index as i64;
                    found_match = true;
                    matched_overloads.insert(left_index);
                    break;
                }
                // Not an exact in-order match: a potential-error probe
                // (subtypes.py:1160-1168) on the unmatched left index; the
                // entire comparison defers when a probe is undecidable.
                if !matched_overloads.contains(&left_index) {
                    let compat_lr = crate::callable_compat::callables_compatible_with_ignore_return(
                        left_item,
                        right_item,
                        ctx.ignore_pos_arg_names,
                        ctx.strict_concatenate,
                        ctx,
                        resolver,
                        true,
                    );
                    if compat_lr == Some(true) {
                        return Some(false);
                    }
                    let compat_rl = if compat_lr.is_some() {
                        crate::callable_compat::callables_compatible_with_ignore_return(
                            right_item,
                            left_item,
                            ctx.ignore_pos_arg_names,
                            ctx.strict_concatenate,
                            ctx,
                            resolver,
                            true,
                        )
                    } else {
                        None
                    };
                    match (compat_lr, compat_rl) {
                        (Some(true), _) | (_, Some(true)) => return Some(false),
                        (Some(false), Some(false)) => {}
                        _ => {
                            return None;
                        }
                    }
                }
            }
            if !found_match {
                return Some(false);
            }
        }
        return Some(true);
    }
    // visit_parameters (subtypes.py:1381-1418): Parameters left. The
    // right-Parameters case routes through `are_parameters_compatible` and
    // defers; right-Instance is the pure `builtins.object` check; else False.
    if let Type::Parameters(..) = left {
        return match right {
            Type::Instance { type_ref, .. } => Some(type_ref == "builtins.object"),
            Type::Parameters(..) => None,
            _ => Some(false),
        };
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
        // CallableType, which the callable_compat engine handles below, so

        // the whole check is answerable natively (C1). Defer only if an
        // item recursion defers (wire-unsupported shape).
        if matches!(right, Type::Overloaded { .. }) {
            let Type::Overloaded { items } = right else {
                return None;
            };
            for item in items {
                match is_subtype(left, item, ctx, resolver) {
                    Some(false) => return Some(false),
                    None => return None,
                    Some(true) => {}
                }
            }
            return Some(true);
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
                    // subtypes.py:1389-1398: a callable implements a protocol
                    // with a __call__ member when the fetched member accepts
                    // this callable and the remaining members agree (#1255).
                    match callable_protocol_call_check(
                        left,
                        left_fallback.as_ref(),
                        right,
                        right_ref,
                        right_snap,
                        ctx,
                        resolver,
                    ) {
                        CallCheck::Decide(b) => return Some(b),
                        CallCheck::Defer => return None,
                        CallCheck::FallThrough => {}
                    }
                }
                // Protocol without __call__: Python checks
                // is_protocol_implementation(class_obj) only when
                // left.is_type_obj(). The port below decides that.
                match crate::callable_compat::is_type_obj(left, resolver) {
                    Some(false) => return is_subtype(left_fallback.as_ref(), right, ctx, resolver),
                    _ => return None,
                }
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
        // right is CallableType (C1): route into the native callable_compat
        // engine (subtypes.py:909-965). The engine mirrors the Python
        // `is_callable_compatible` including the pre-checks for

        // type_guard/type_is incompatibility (defers on wire-unsupported
        // forms: Parameters, generic left with variables, unpack, resolver
        // misses on the type-obj check, and any nested is_subtype that

        // returns None). The flags the visitor passes (`ignore_pos_arg_names`
        // from the SubtypeContext, `strict_concatenate` from extra_checks/
        // strict_concatenate, proper_subtype, strict_optional) come from

        // this context; `ignore_return = False`, `check_args_covariantly =
        // False`, and `allow_partial_overlap = False`, matching the
        // `visit_callable_type` call site.
        if let Type::CallableType { .. } = right {
            let res = crate::callable_compat::callables_compatible(
                left,
                right,
                ctx.ignore_pos_arg_names,
                ctx.strict_concatenate,
                ctx,
                resolver,
            );
            // The engine already incorporates any nested subtype checks
            // through the `is_compat` closure; `None` means it could not
            // decide (all-or-nothing), so defer the whole comparison.
            return res;
        }
        return Some(false);
    }
    let (left_ref, left_args) = match left {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => {
            return None;
        }
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
        left, left_ref, left_args, right, right_ref, right_args, ctx, resolver,
    )
}

/// `variadic_tuple_subtype` (subtypes.py:1086-1166), Rust port.
///
/// Check subtyping between two potentially variadic tuples. The right
/// side must have an UnpackType (variadic unpack); returns `None` when
/// it cannot decide (unsupported shape or a recursive `is_subtype`
/// deferral), in which case the caller falls through to the fixed-length
/// logic.
fn variadic_tuple_subtype(
    left: &Type,
    right: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<Option<bool>> {
    let Type::TupleType {
        items: left_items, ..
    } = left
    else {
        return None;
    };
    let Type::TupleType {
        items: right_items, ..
    } = right
    else {
        return None;
    };
    // right_unpack_index = find_unpack_in_list(right.items).
    let Some(right_unpack_index) = find_unpack_in_list(right_items) else {
        return Some(None);
    };
    let right_unpack = right_items.get(right_unpack_index)?;
    let Type::UnpackType { typ: r_unpack } = right_unpack else {
        return None;
    };
    // get_proper_type(right_unpack.type) must be a builtins.tuple
    // Instance (subtypes.py:1095-1098); else this case is handled by the
    // caller and answers False.
    let right_unpacked = match get_proper_type_or_defer(r_unpack.as_ref(), resolver)? {
        Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
            args.first().cloned()?
        }
        _ => return Some(None),
    };
    let right_prefix = right_unpack_index;
    let right_suffix = right_items.len() - right_unpack_index - 1;
    let left_unpack_index = find_unpack_in_list(left_items);
    // Simple case: left is fixed. Find the mapping to the right
    // (subtypes.py:1109-1124).
    if left_unpack_index.is_none() {
        if left_items.len() < right_prefix + right_suffix {
            return Some(None);
        }
        let (prefix, middle, suffix) =
            split_with_prefix_and_suffix(left_items, right_prefix, right_suffix)?;
        let right_prefix_items = &right_items[..right_prefix];
        for (li, ri) in prefix.iter().zip(right_prefix_items.iter()) {
            match is_subtype(li, ri, ctx, resolver) {
                Some(true) => {}
                Some(false) => return Some(None),
                None => return None,
            }
        }
        if right_suffix > 0 {
            let right_suffix_items = &right_items[right_items.len() - right_suffix..];
            for (li, ri) in suffix.iter().zip(right_suffix_items.iter()) {
                match is_subtype(li, ri, ctx, resolver) {
                    Some(true) => {}
                    Some(false) => return Some(None),
                    None => return None,
                }
            }
        }
        for li in middle {
            match is_subtype(li, &right_unpacked, ctx, resolver) {
                Some(true) => {}
                Some(false) => return Some(None),
                None => return None,
            }
        }
        return Some(Some(true));
    }
    // Both sides have a variadic unpack.
    if left_items.len() < right_items.len() {
        return Some(None);
    }
    let left_prefix = left_unpack_index.unwrap();
    let left_suffix = left_items.len() - left_prefix - 1;
    let left_unpack = &left_items[left_prefix];
    let Type::UnpackType { typ: l_unpack } = left_unpack else {
        return None;
    };
    let left_unpacked = match get_proper_type_or_defer(l_unpack.as_ref(), resolver)? {
        Type::Instance { type_ref, args, .. } if type_ref == "builtins.tuple" => {
            args.first().cloned()?
        }
        _ => {
            // *Ts unpack can't be split except if all mapped to tops.
            if is_top_type(&right_unpacked, ctx)? {
                let (right_prefix_types, middle, right_suffix_types) =
                    split_with_prefix_and_suffix(right_items, left_prefix, left_suffix)?;
                for ri in middle {
                    if !is_top_type(ri, ctx)? && !matches!(ri, Type::UnpackType { .. }) {
                        return Some(None);
                    }
                }
                if !all_subtypes(
                    &left_items[..left_prefix],
                    right_prefix_types,
                    ctx,
                    resolver,
                )? {
                    return Some(None);
                }
                if !all_subtypes(
                    &left_items[left_items.len() - left_suffix..],
                    right_suffix_types,
                    ctx,
                    resolver,
                )? {
                    return Some(None);
                }
                return Some(Some(true));
            }
            return Some(None);
        }
    };
    // Asymptotic case: both unpacks must be subtypes.
    match is_subtype(&left_unpacked, &right_unpacked, ctx, resolver) {
        Some(true) => {}
        Some(false) => return Some(None),
        None => return None,
    }
    // Finite overlaps: for each overlap, build the left representation
    // with `overlap` copies of left_item in the middle and check it
    // against the right (subtypes.py:1152-1165).
    let max_overlap = {
        let rp = right_prefix as isize - left_prefix as isize;
        let rs = right_suffix as isize - left_suffix as isize;
        rp.max(0).max(rs.max(0)) as usize
    };
    for overlap in 0..=max_overlap {
        let mut repr_items: Vec<Type> = Vec::with_capacity(left_prefix + overlap + left_suffix);
        repr_items.extend_from_slice(&left_items[..left_prefix]);
        for _ in 0..overlap {
            repr_items.push(left_unpacked.clone());
        }
        if left_suffix > 0 {
            let start = left_items.len() - left_suffix;
            repr_items.extend_from_slice(&left_items[start..]);
        }
        let mut left_repr = left.clone();
        if let Type::TupleType { items, .. } = &mut left_repr {
            *items = repr_items;
        }
        match is_subtype(&left_repr, right, ctx, resolver) {
            Some(true) => {}
            Some(false) => return Some(None),
            None => return None,
        }
    }
    Some(Some(true))
}

/// `rust_variadic_tuple_subtype` (subtypes.py:1086-1166), PyO3 seam.
///
/// Returns `Some(true)` when the Rust port decides True; `None` when it
/// cannot (no right unpack, unsupported shape, or a recursive
/// `is_subtype` deferral) so the caller falls through to the
/// fixed-length logic exactly like the Python short-circuit
/// (subtypes.py:1064-1065).
#[pyfunction]
#[allow(dead_code)]
pub(crate) fn rust_variadic_tuple_subtype(
    left_bytes: &[u8],
    right_bytes: &[u8],
    proper_subtype: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let ctx = SubtypeContext::with_callable_flags(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        false, // ignore_promotions
        proper_subtype,
        false, // strict_optional
        false, // ignore_pos_arg_names
        false, // strict_concatenate
    );
    variadic_tuple_subtype(&left, &right, &ctx, resolver.resolver())?
}

/// `is_top_type` (subtypes.py:1168-1171): top types are any AnyType
/// under non-proper subtype, or `builtins.object` instances.
fn is_top_type(typ: &Type, ctx: &SubtypeContext) -> Option<bool> {
    if !ctx.proper_subtype && matches!(typ, Type::AnyType { .. }) {
        return Some(true);
    }
    if let Type::Instance { type_ref, .. } = typ {
        return Some(type_ref == "builtins.object");
    }
    Some(false)
}

/// `find_unpack_in_list` (types.py:5002-5009): index of the first
/// UnpackType item, or None. Variadic middle args arrive as UnpackType.
fn find_unpack_in_list(items: &[Type]) -> Option<usize> {
    for (i, item) in items.iter().enumerate() {
        if matches!(item, Type::UnpackType { .. }) {
            return Some(i);
        }
    }
    None
}

/// `split_with_prefix_and_suffix` (types.py:4904-4925): split a slice
/// into prefix / middle / suffix by counts.
fn split_with_prefix_and_suffix(
    items: &[Type],
    prefix: usize,
    suffix: usize,
) -> Option<(&[Type], &[Type], &[Type])> {
    if items.len() < prefix + suffix {
        return None;
    }
    if suffix == 0 {
        Some((&items[..prefix], &items[prefix..], &items[items.len()..]))
    } else {
        let end = items.len() - suffix;
        Some((&items[..prefix], &items[prefix..end], &items[end..]))
    }
}

/// `_all_subtypes` helper: all left items subtype the corresponding
/// right items (subtypes.py:638-639).
fn all_subtypes(
    lefts: &[Type],
    rights: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    for (li, ri) in lefts.iter().zip(rights.iter()) {
        match is_subtype(li, ri, ctx, resolver) {
            Some(true) => {}
            Some(false) => return Some(false),
            None => return None,
        }
    }
    Some(true)
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
    // TypeVarTupleType right (subtypes.py:617-620): tuple[Any, ...] is
    // like Any for tuples: map left to tuple_fallback and require the
    // mapped first arg to be Any (stays False if proper).
    if let Type::Instance {
        type_ref: left_ref, ..
    } = left
    {
        if matches!(right, Type::TypeVarTupleType { .. }) {
            return visit_instance_variadic_right(left, right, ctx, resolver, left_ref);
        }
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
                    // TypeOfAny.special_form (types.py:309), mirroring
                    // subtypes.py:789-792's TypeType.make_normalized(
                    // AnyType(TypeOfAny.special_form)).
                    type_of_any: ANY_SPECIAL_FORM,
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
    // FunctionLike right needs `find_member("__call__", ...)` (subtypes.py:
    // 678-682). A MRO snapshot without `__call__` (is_operator=True skips the
    // dunder scan) answers False; an existing one still defers to Python.
    if matches!(right, Type::CallableType { .. } | Type::Overloaded { .. }) {
        if let Type::Instance {
            type_ref,
            extra_attrs,
            ..
        } = left
        {
            if !extra_attrs
                .iter()
                .any(|attrs| attrs.attrs.contains_key("__call__"))
            {
                let snap = resolver.get(type_ref);
                if let Some(snap) = snap {
                    if !snap.fallback_to_any
                        && snap.mro.iter().all(|base| {
                            resolver
                                .get(base)
                                .map(|b| !b.member_info.contains_key("__call__"))
                                .unwrap_or(false)
                        })
                    {
                        // `find_member` returns None -> Python's `return False`.
                        return Some(false);
                    }
                }
            }
            // Live fetch extension (issue #1255, subtypes.py:1235-1240): the
            // negative pre-check did not fire, so `__call__` may exist; fetch
            // via find_member(name, left, left, is_operator=True) and recurse.
            if resolver.has_live_info_map() {
                let fetch = pyo3::Python::with_gil(|py| {
                    crate::checker_helpers::get_protocol_member_inner(
                        py, left, left, "__call__", false, false, resolver,
                    )
                });
                if let Some(crate::checker_helpers::GetProtocolMemberResult::Found(call)) = fetch {
                    return is_subtype(&call, right, ctx, resolver);
                }
            }
        }
        return None;
    }
    // Anything else: Python's `else: return False` (subtypes.py:683). Includes
    // NoneType, UninhabitedType, DeletedType, ErasedType (non-proper Erased right
    // is caught by the fast path), TypedDictType, ParamSpecType, LiteralType.
    Some(false)
}

/// `visit_instance` TypeVarTupleType-right branch (subtypes.py:617-620).
///
/// `tuple[Any, ...]` is like `Any` in the world of tuples. Rust ports
/// the Python body exactly:
/// * `left.type.has_base("builtins.tuple")`: a left Instance whose MRO
///   reaches `builtins.tuple` tuples-compatible. The TypeInfo snapshot
///   carries a `has_base` set, mirroring Python's `TypeInfo.has_base`.
/// * Map the left to the typevar's own `tuple_fallback.type` via
///   `map_instance_to_supertype` (Rust-ported subset) and check the
///   mapped first arg for `AnyType` (proper type; the mapped args are
///   already concrete `Instance` args, so skip alias expansion).
/// * Result: `not proper_subtype`.
///
/// Deferral is precomputed: `tuple_fallback` is an inline `Instance`
/// record in the wire `TypeVarTupleType`, and the snapshot lookup /
/// map are pure resolver operations; anything missing (snapshot miss,
/// map failure) returns `None` so the Python fallback stays correct.
fn visit_instance_variadic_right(
    left: &Type,
    right: &Type,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
    left_ref: &str,
) -> Option<bool> {
    let Type::TypeVarTupleType { tuple_fallback, .. } = right else {
        return None;
    };
    let Type::Instance {
        type_ref: fb_ref, ..
    } = tuple_fallback.as_ref()
    else {
        return None;
    };
    // subtypes.py:617-620: has_base("builtins.tuple") guard.
    if !resolver.get(left_ref)?.has_base("builtins.tuple") {
        return Some(false);
    }
    // subtypes.py:619: map_instance_to_supertype(left, tf_fb.type).
    let (left_ref, left_args) = match left {
        Type::Instance { type_ref, args, .. } => (type_ref.as_str(), args.as_slice()),
        _ => {
            return None;
        }
    };
    let mapped = map_instance_to_supertype(left_ref, left_args, fb_ref, resolver)?;
    let first = match mapped.first() {
        Some(t) => t,
        None => return Some(false),
    };
    // subtypes.py:620: isinstance(get_proper_type(mapped.args[0]), AnyType).
    let mapped_first = get_proper_type_or_defer(first, resolver)?;
    if matches!(mapped_first, Type::AnyType { .. }) {
        return Some(!ctx.proper_subtype);
    }
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
        _ => {
            return None;
        }
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
/// does not handle; the caller falls through to Python for those.
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
        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            // visit_callable_type (expandtype.py:870-918). Defer when a
            // declared ParamSpec means Python's param_spec() takes the
            // *args: P.args + **kwargs: P.kwargs branch (Parameters join).
            for v in variables {
                if matches!(v, Type::ParamSpecType { .. }) {
                    return None;
                }
            }
            // Defer a var-arg typed UnpackType: interpolation
            // (expandtype.py:482-491) needs tuple splicing we do not port.
            for (flag, at) in arg_kinds.iter().zip(arg_types.iter()) {
                if *flag == ARG_STAR && matches!(at, Type::UnpackType { .. }) {
                    return None;
                }
            }
            let mut new_arg_types = Vec::with_capacity(arg_types.len());
            for at in arg_types {
                new_arg_types.push(expand_type_by_instance(at, left_ref, left_args)?);
            }
            let new_ret = expand_type_by_instance(ret_type, left_ref, left_args)?;
            let new_guard = match type_guard {
                Some(tg) => Some(Box::new(expand_type_by_instance(tg, left_ref, left_args)?)),
                None => None,
            };
            let new_type_is = match type_is {
                Some(ti) => Some(Box::new(expand_type_by_instance(ti, left_ref, left_args)?)),
                None => None,
            };
            let new_instance_type = match instance_type {
                Some(it) => Some(Box::new(expand_type_by_instance(it, left_ref, left_args)?)),
                None => None,
            };
            // Python expands arg_types, ret_type, type_guard, type_is and
            // instance_type only (expandtype.py:911-917); the fallback
            // and declared variables are definitions, not uses.
            Some(Type::CallableType {
                fallback: fallback.clone(),
                instance_type: new_instance_type,
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                from_type_type: *from_type_type,
                arg_types: new_arg_types,
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: Box::new(new_ret),
                name: name.clone(),
                variables: variables.clone(),
                type_guard: new_guard,
                type_is: new_type_is,
            })
        }
        Type::Overloaded { items } => {
            // visit_overloaded (expandtype.py:811-818): each item is a
            // CallableType, expanded item-wise.
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(expand_type_by_instance(item, left_ref, left_args)?);
            }
            Some(Type::Overloaded { items: new_items })
        }
        Type::TupleType {
            partial_fallback,
            items,
            implicit,
        } => {
            // visit_tuple_type (expandtype.py:720-740). Defer the single-item
            // Tuple[*tuple[X, ...]] normalization and the named-tuple
            // fallback branches; the common case only expands item+fallback.
            if items.len() == 1 {
                if let Type::UnpackType { .. } = &items[0] {
                    return None;
                }
            }
            let mut new_items = Vec::with_capacity(items.len());
            for item in items {
                new_items.push(expand_type_by_instance(item, left_ref, left_args)?);
            }
            let new_fallback = expand_type_by_instance(partial_fallback, left_ref, left_args)?;
            if let Type::Instance { ref type_ref, .. } = new_fallback {
                if type_ref == "builtins.tuple" && new_items.len() == 1 {
                    if let Type::UnpackType { .. } = &new_items[0] {
                        // Single Tuple[*tuple[X, ...]] with a builtins.tuple
                        // fallback normalizes to the inner Instance; defer.
                        return None;
                    }
                }
                Some(Type::TupleType {
                    partial_fallback: Box::new(new_fallback),
                    items: new_items,
                    implicit: *implicit,
                })
            } else {
                None
            }
        }
        Type::TypeType { item, is_type_form } => {
            // visit_type_type (expandtype.py:736-740): expand the item then
            // TypeType.make_normalized. Type[Union[...]] distribution needs
            // the make_normalized port; defer a union item.
            let new_item = expand_type_by_instance(item, left_ref, left_args)?;
            if matches!(new_item, Type::UnionType { .. }) {
                return None;
            }
            Some(Type::TypeType {
                item: Box::new(new_item),
                is_type_form: *is_type_form,
            })
        }
        Type::UnpackType { typ } => {
            // visit_unpack_type (expandtype.py:370-380): expand the inner
            // type and rewrap (the variadic splice happens in the caller).
            let new_typ = expand_type_by_instance(typ, left_ref, left_args)?;
            Some(Type::UnpackType {
                typ: Box::new(new_typ),
            })
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
    // Fast path: a typevars-empty superclass maps to no args regardless
    // of the derivation path (maptype.py:19-21), matching Python before
    // its native seam; a non-generic definer accessor rides this.
    if let Some(right_snap) = resolver.get(right_ref) {
        if right_snap.type_vars.is_empty() {
            return Some(vec![]);
        }
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
    // Variadic left: expand_type_by_instance_core handles the env
    // binding but not all prefix/suffix splicing cases, so defer to
    // Python until split_with_prefix_and_suffix is fully ported.
    if left_snap.has_type_var_tuple_type {
        return None;
    }
    let expand = |typ: &Type| -> Option<Type> { expand_type_by_instance(typ, left_ref, left_args) };
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
                let expanded = expand(&base)?;
                if let Type::Instance { args, .. } = expanded {
                    return Some(args);
                }
                return None;
            }
            // Multi-level: recurse through this base. First map left to
            // this base's frame, then continue from there.
            let mapped = expand(&base)?;
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

// --- Protocol-right port (subtypes.py:1287-1291 -> is_protocol_implementation) ---

thread_local! {
    /// Mirrors Python's `assuming` / `assuming_proper` recursion stacks
    /// (subtypes.py:1972), flattened into one list keyed by the
    /// proper-subtype dimension (Python picks one of the two stacks by
    /// `proper_subtype`; a single list with the flag is equivalent).
    /// Entries live only for the duration of a protocol-right member loop.
    static ASSUMING: RefCell<Vec<(Type, Type, bool)>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn assuming_contains(left: &Type, right: &Type, proper: bool) -> bool {
    ASSUMING.with(|s| {
        s.borrow()
            .iter()
            .any(|(l, r, p)| *p == proper && l == left && r == right)
    })
}

/// Pushes `(left, right, proper)` onto the assuming stack; pops on drop,
/// covering every exit path including deferral (Python's `pop_on_exit`
/// context manager, subtypes.py:1922).
pub(crate) struct AssumingPush;

impl AssumingPush {
    pub(crate) fn new(left: Type, right: Type, proper: bool) -> Self {
        ASSUMING.with(|s| s.borrow_mut().push((left, right, proper)));
        AssumingPush
    }
}

impl Drop for AssumingPush {
    fn drop(&mut self) {
        ASSUMING.with(|s| {
            s.borrow_mut().pop();
        });
    }
}

/// Protocol-right decision site: Python's
/// `if right.type.is_protocol and is_protocol_implementation(...)` tail of
/// `SubtypeVisitor.visit_instance` (subtypes.py:1287-1291), reached when the
/// nominal branch did not apply.
///
/// Returns `Some(bool)` when Rust decided; `None` defers to the pure-Python
/// body (which runs its own assuming stack — safe, just re-work).
fn protocol_right_decision(
    left: &Type,
    right: &Type,
    left_ref: &str,
    right_ref: &str,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    // Without a live TypeInfo map the dependency record and the member-flag
    // loop cannot run; defer. This also keeps pure-Rust cargo tests
    // interpreter-free (their resolvers never install the map).
    if !resolver.has_live_info_map() {
        return None;
    }
    // Recursion guard + stack push live in `is_protocol_implementation_inner`
    // (mirroring Python's `pop_on_exit` inside `is_protocol_implementation`).
    pyo3::Python::with_gil(|py| {
        let left_info = resolver.live_typeinfo(py, left_ref)?;
        let right_info = resolver.live_typeinfo(py, right_ref)?;
        // Fine-grained dependency record (subtypes.py:1962); idempotent
        // set-add on the Python side. Defer on any failure so Python
        // performs the record itself.
        let type_state = py
            .import("mypy.typestate")
            .ok()?
            .getattr("type_state")
            .ok()?;
        type_state
            .call_method1("record_protocol_subtype_check", (left_info, right_info))
            .ok()?;
        crate::protocols::is_protocol_implementation_inner(py, left, right, &[], ctx, resolver)
    })
}

/// What `callable_protocol_call_check` decided for its caller.
enum CallCheck {
    /// Node decided; return this verdict.
    Decide(bool),
    /// Python falls through to the is_type_obj / fallback tail
    /// (subtypes.py:1399-1405); continue with the ported arms.
    FallThrough,
    /// Rust cannot decide this path; defer the whole call.
    Defer,
}

/// subtypes.py:1389-1398 (visit_callable_type, protocol Instance right with
/// "__call__" in protocol_members): the fetched `__call__` member of the
/// protocol must be a supertype of this callable, then either the protocol
/// has only that member or `is_protocol_implementation(left.fallback,
/// right, skip=["__call__"])` runs. When the call check fails, Python falls
/// through to the tail arms, so `FallThrough` maps callers onward.
///
/// The fetch rides the resolver-backed `get_protocol_member_inner` port,
/// which mirrors `find_member(member, right, right, is_operator=True)` for
/// the decidable shapes. `NoneVal` defers (never `false`): it conflates the
/// metaclass-precise wrapper-only special case with a genuine miss, and
/// `typeshed` `builtins.type` defines `__call__`, so a `false` here would
/// flip `call:builtins.type > inst:builtins.type`.
fn callable_protocol_call_check(
    left: &Type,
    left_fallback: &Type,
    right: &Type,
    right_ref: &str,
    right_snap: Option<&crate::typeinfo::TypeInfoSnapshot>,
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> CallCheck {
    // Without a live TypeInfo map the member fetch and the implementation
    // loop cannot run (also keeps cargo tests interpreter-free).
    if !resolver.has_live_info_map() {
        return CallCheck::Defer;
    }
    let call = match pyo3::Python::with_gil(|py| {
        crate::checker_helpers::get_protocol_member_inner(
            py, right, right, "__call__", false, false, resolver,
        )
    }) {
        Some(crate::checker_helpers::GetProtocolMemberResult::Found(t)) => t,
        // NoneVal (metaclass-precise / genuine miss) and Defer defer, and
        // an inner step failure defers the whole call.
        _ => return CallCheck::Defer,
    };
    match is_subtype(left, &call, ctx, resolver) {
        Some(true) => {}
        Some(false) => return CallCheck::FallThrough,
        None => return CallCheck::Defer,
    }
    let only_call = right_snap.is_some_and(|s| s.protocol_members.len() == 1);
    if only_call {
        return CallCheck::Decide(true);
    }
    // is_protocol_implementation(left.fallback, right, skip=["__call__"]):
    // Python's default entry, so a fresh context (all flags False,
    // strict_optional from live state), not the caller's.
    let verdict = pyo3::Python::with_gil(|py| {
        let Type::Instance {
            type_ref: fallback_ref,
            ..
        } = left_fallback
        else {
            return None;
        };
        let left_info = resolver.live_typeinfo(py, fallback_ref)?;
        let right_info = resolver.live_typeinfo(py, right_ref)?;
        let type_state = py
            .import("mypy.typestate")
            .ok()?
            .getattr("type_state")
            .ok()?;
        type_state
            .call_method1("record_protocol_subtype_check", (left_info, right_info))
            .ok()?;
        let fresh = SubtypeContext {
            proper_subtype: false,
            strict_optional: crate::checker_helpers::live_strict_optional(py),
            ..Default::default()
        };
        crate::protocols::is_protocol_implementation_inner(
            py,
            left_fallback,
            right,
            &["__call__".to_string()],
            &fresh,
            resolver,
        )
    });
    match verdict {
        Some(true) => CallCheck::Decide(true),
        Some(false) => CallCheck::FallThrough,
        // The Python body re-runs everything from scratch, so a deferred
        // tail defers the whole call.
        None => CallCheck::Defer,
    }
}

#[allow(clippy::too_many_arguments)]
fn visit_instance_nominal(
    left: &Type,
    left_ref: &str,
    left_args: &[Type],
    right: &Type,
    right_ref: &str,
    right_args: &[Type],
    ctx: &SubtypeContext,
    resolver: &TypeResolver,
) -> Option<bool> {
    // Same-ref fast path, hoisted above the snapshot reads (issue #1096):
    // Python's nominal branch (subtypes.py:554-561) answers True for
    // identical instances before the protocol path.

    // ignore_declared_variance and alias-carrying args still defer: Rust
    // skipping a deferred pair skips the Python visitor whose type_state
    // bookkeeping later protocol checks consult (see the commit message).
    if !ctx.proper_subtype
        && !ctx.ignore_declared_variance
        && left_ref == right_ref
        && left_args == right_args
        && !left_args
            .iter()
            .any(crate::expandtype::result_contains_typealias)
    {
        return Some(true);
    }
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
        // Nominal branch skipped. If right is a protocol, run the
        // protocol-implementation port: Python reaches it exactly when
        // the nominal branch did not apply.

        // When it did apply, Python returns the nominal verdict without
        // a protocol check; otherwise it records a negative cache entry
        // and returns False.
        if right_is_protocol {
            return protocol_right_decision(left, right, left_ref, right_ref, ctx, resolver);
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

    // Map left to right's type. Fast path: left.type == right.type needs no substitution.
    // Slow path: map_instance_to_supertype walks the bases, substituting TypeVars;
    // None on unsupported variants (UnpackType, ParamSpec), Python falls through.
    let mapped_args: Vec<Type> = if ctx.erase_instances {
        // Python (subtypes.py:1151-1155) erases the *mapped* instance via
        // `erase_type`: every arg becomes `AnyType(TypeOfAny.special_form)`
        // over the supertype's own type_vars (erasetype.py:251-253).

        // Per-arg checks compare erased left vs (already erased) right args:
        // relocatable subclass mappings with concrete base args still cover.
        // TVT classes defer earlier: a 1:1 Any per snapshot type var is full.
        (0..right_snap.type_vars_with_variance.len())
            .map(|_| any_type_of(ANY_SPECIAL_FORM))
            .collect()
    } else if left_ref == right_ref {
        left_args.to_vec()
    } else if right_snap.type_vars_with_variance.is_empty() {
        // right has no type vars: map_instance_to_supertype returns
        // Instance(right, []) (no args to substitute).
        Vec::new()
    } else {
        // Generic path: map_instance_to_supertype walks class_derivation_paths over
        // the bases blobs, substituting TypeVars via expand_type_by_instance; None on
        // an unsupported Type variant (UnpackType, ParamSpec), Python falls through.
        map_instance_to_supertype(left_ref, left_args, right_ref, resolver)?
    };

    if ctx.ignore_type_params {
        return Some(true);
    }

    // check_type_parameter over (lefta, righta, tvar) triples
    // (subtypes.py:598-621). VARIANCE_NOT_READY returns None (Python
    // handles infer_class_variances; mutating live defn, deferred).
    let right_tvars = &right_snap.type_vars_with_variance;
    // Python uses zip(t.args, right.args, right.type.defn.type_vars)
    // (subtypes.py:1101), which silently truncates to the shortest
    // iterable. Mirror that instead of deferring on arity mismatch (#820).
    let n = mapped_args
        .len()
        .min(right_args.len())
        .min(right_tvars.len());
    let mut nominal = true;
    for (i, (_tvar_name, variance, kind)) in right_tvars.iter().enumerate() {
        if i >= n {
            break;
        }
        let lefta = &mapped_args[i];
        let righta = &right_args[i];
        // Non-TypeVarType tvars (ParamSpec, kind=1): Python's else branch
        // (subtypes.py:1198-1203) passes COVARIANT. TypeVarTuple (kind=2)
        // is unreachable: those classes defer earlier (has_type_var_tuple_type).
        let effective_variance = if *kind != 0 || (ctx.always_covariant && *variance == INVARIANT) {
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
    // Reflexive fast path: all Python variance checks are reflexive, so
    // wire-equal args are a subtype under every variance. Without this the
    // recursive calls defer on identical ParamSpec / variadic args (C[P] <: C[P]).
    if left == right {
        return Some(true);
    }
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

/// `get_proper_type` for `TypeAliasType` (types.py:4032-4050): expand
/// every alias in a `Type` tree by looking up the `alias_resolver`,
/// substituting type-var args via `expand_type_inner`, and recursing
/// until no `TypeAliasType` remains. Returns `None` (defer to Python)
/// when the alias is missing from the resolver, the alias is variadic
/// (`tvar_tuple_index` set), or any child expansion defers.
pub(crate) fn expand_aliases(
    typ: &Type,
    alias_resolver: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
) -> Option<Type> {
    let mut active: Vec<ActiveAlias> = Vec::new();
    expand_aliases_depth(typ, alias_resolver, strict_optional, 0, &mut active)
}

/// Identity key of an alias occurrence on the current expansion path: the
/// alias fullname plus its type arguments. Python never structurally
/// re-derives an alias under itself (`_expand_once`, types.py:445-478,
/// unrolls exactly once and `get_proper_type` leaves every nested alias
/// node in place), so a repeat occurrence on the descent path is kept
/// unexpanded (issue #1149).
type ActiveAlias = (String, Vec<Type>);

fn expand_aliases_depth(
    typ: &Type,
    alias_resolver: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    depth: u32,
    active: &mut Vec<ActiveAlias>,
) -> Option<Type> {
    if depth > 50 {
        // Over-broad: defer the whole entry rather than guess.
        return None;
    }
    match typ {
        Type::TypeAliasType { args, type_ref } => {
            // Issue #1149: cut an alias already active on this descent (the
            // `_expand_once` fixpoint). Keyed on args identity, so a sibling
            // same-alias-different-args occurrence still expands in place.
            if active.iter().any(|(r, a)| r == type_ref && *a == *args) {
                return Some(typ.clone());
            }
            let snap = match alias_resolver.get(type_ref) {
                Some(s) => s,
                None => {
                    // Issue #1205 best-effort: an unexpansible alias node is
                    // kept in place (the #1149 contract: the engine defers on
                    // every alias node), so only comparisons reaching it defer.
                    return Some(typ.clone());
                }
            };
            if snap.tvar_tuple_index.is_some() {
                // Variadic alias target needs the Unpack splicing machinery
                // (expandtype) that is Python-side; keep the node instead of
                // failing the whole entry.
                return Some(typ.clone());
            }
            active.push((type_ref.clone(), args.clone()));
            let expanded_args: Vec<Type> = match args
                .iter()
                .map(|a| {
                    expand_aliases_depth(a, alias_resolver, strict_optional, depth + 1, active)
                })
                .collect::<Option<_>>()
            {
                Some(a) => a,
                None => {
                    // Recursion depth cap only (nested failures are now kept
                    // locally, so a child can no longer fail from alias
                    // expansion problems).
                    active.pop();
                    return Some(typ.clone());
                }
            };
            let res = if snap.no_args {
                let target = match decode_type(&snap.target) {
                    Some(t) => t,
                    None => {
                        active.pop();
                        return Some(typ.clone());
                    }
                };
                if let Type::Instance { type_ref, .. } = &target {
                    Some(Type::Instance {
                        type_ref: type_ref.clone(),
                        args: expanded_args,
                        last_known_value: None,
                        extra_attrs: None,
                    })
                } else {
                    expand_aliases_depth(
                        &target,
                        alias_resolver,
                        strict_optional,
                        depth + 1,
                        active,
                    )
                }
            } else {
                let mut env: std::collections::HashMap<crate::expandtype::EnvKey, Type> =
                    std::collections::HashMap::new();
                for (tvar, arg) in snap.alias_tvars.iter().zip(expanded_args.iter()) {
                    env.insert(
                        (tvar.raw_id, tvar.meta_level, tvar.namespace.clone()),
                        arg.clone(),
                    );
                }
                let target = match decode_type(&snap.target) {
                    Some(t) => t,
                    None => {
                        active.pop();
                        return Some(typ.clone());
                    }
                };
                let substituted =
                    match crate::expandtype::expand_type_inner(&target, &env, strict_optional) {
                        Some(t) => t,
                        None => {
                            // Substitution walls (ParamSpec / Unpack
                            // variables and var-args) are Python-side work:
                            // keep the alias node unexpanded, don't fail.
                            active.pop();
                            return Some(typ.clone());
                        }
                    };
                expand_aliases_depth(
                    &substituted,
                    alias_resolver,
                    strict_optional,
                    depth + 1,
                    active,
                )
            };
            // Pop on every decided return path: a leaky entry would cut a
            // later sibling occurrence of the same alias+args.
            active.pop();
            res
        }
        Type::Instance {
            type_ref,
            args,
            last_known_value,
            extra_attrs,
        } => {
            if args.is_empty() {
                return Some(typ.clone());
            }
            let new_args = args
                .iter()
                .map(|a| {
                    expand_aliases_depth(a, alias_resolver, strict_optional, depth + 1, active)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::Instance {
                type_ref: type_ref.clone(),
                args: new_args,
                last_known_value: last_known_value.clone(),
                extra_attrs: extra_attrs.clone(),
            })
        }
        Type::UnionType {
            items,
            uses_pep604_syntax,
            can_be_true,
            can_be_false,
        } => {
            let new_items = items
                .iter()
                .map(|i| {
                    expand_aliases_depth(i, alias_resolver, strict_optional, depth + 1, active)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::UnionType {
                items: new_items,
                uses_pep604_syntax: *uses_pep604_syntax,
                can_be_true: *can_be_true,
                can_be_false: *can_be_false,
            })
        }
        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            let fb =
                expand_aliases_depth(fallback, alias_resolver, strict_optional, depth + 1, active)?;
            let it = match instance_type {
                Some(it) => Some(Box::new(expand_aliases_depth(
                    it,
                    alias_resolver,
                    strict_optional,
                    depth + 1,
                    active,
                )?)),
                None => None,
            };
            let ats: Vec<Type> = arg_types
                .iter()
                .map(|a| {
                    expand_aliases_depth(a, alias_resolver, strict_optional, depth + 1, active)
                })
                .collect::<Option<_>>()?;
            let rt = Box::new(expand_aliases_depth(
                ret_type,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            let tg = match type_guard {
                Some(tg) => Some(Box::new(expand_aliases_depth(
                    tg,
                    alias_resolver,
                    strict_optional,
                    depth + 1,
                    active,
                )?)),
                None => None,
            };
            let ti = match type_is {
                Some(ti) => Some(Box::new(expand_aliases_depth(
                    ti,
                    alias_resolver,
                    strict_optional,
                    depth + 1,
                    active,
                )?)),
                None => None,
            };
            Some(Type::CallableType {
                fallback: Box::new(fb),
                instance_type: it,
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                from_type_type: *from_type_type,
                arg_types: ats,
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: rt,
                name: name.clone(),
                variables: variables.clone(),
                type_guard: tg,
                type_is: ti,
            })
        }
        Type::Overloaded { items } => {
            let new_items = items
                .iter()
                .map(|i| {
                    expand_aliases_depth(i, alias_resolver, strict_optional, depth + 1, active)
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::Overloaded { items: new_items })
        }
        Type::TupleType {
            items,
            partial_fallback,
            implicit,
        } => {
            let new_items = items
                .iter()
                .map(|i| {
                    expand_aliases_depth(i, alias_resolver, strict_optional, depth + 1, active)
                })
                .collect::<Option<Vec<_>>>()?;
            let fb = Box::new(expand_aliases_depth(
                partial_fallback,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            Some(Type::TupleType {
                items: new_items,
                partial_fallback: fb,
                implicit: *implicit,
            })
        }
        Type::TypeType { item, is_type_form } => {
            let new_item = Box::new(expand_aliases_depth(
                item,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            Some(Type::TypeType {
                item: new_item,
                is_type_form: *is_type_form,
            })
        }
        Type::TypeVarType {
            name,
            fullname,
            raw_id,
            namespace,
            values,
            upper_bound,
            default,
            variance,
            meta_level,
        } => {
            let new_values = values
                .iter()
                .map(|v| {
                    expand_aliases_depth(v, alias_resolver, strict_optional, depth + 1, active)
                })
                .collect::<Option<Vec<_>>>()?;
            let ub = Box::new(expand_aliases_depth(
                upper_bound,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            let def = Box::new(expand_aliases_depth(
                default,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            Some(Type::TypeVarType {
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                values: new_values,
                upper_bound: ub,
                default: def,
                variance: *variance,
                meta_level: *meta_level,
            })
        }
        Type::ParamSpecType {
            prefix,
            name,
            fullname,
            raw_id,
            namespace,
            flavor,
            upper_bound,
            default,
        } => {
            let new_prefix =
                expand_parameters_depth(prefix, alias_resolver, strict_optional, depth, active)?;
            let ub = Box::new(expand_aliases_depth(
                upper_bound,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            let def = Box::new(expand_aliases_depth(
                default,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            Some(Type::ParamSpecType {
                prefix: Box::new(new_prefix),
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                flavor: *flavor,
                upper_bound: ub,
                default: def,
            })
        }
        Type::TypeVarTupleType {
            tuple_fallback,
            name,
            fullname,
            raw_id,
            namespace,
            upper_bound,
            default,
            min_len,
        } => {
            let fb = Box::new(expand_aliases_depth(
                tuple_fallback,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            let ub = Box::new(expand_aliases_depth(
                upper_bound,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            let def = Box::new(expand_aliases_depth(
                default,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            Some(Type::TypeVarTupleType {
                tuple_fallback: fb,
                name: name.clone(),
                fullname: fullname.clone(),
                raw_id: *raw_id,
                namespace: namespace.clone(),
                upper_bound: ub,
                default: def,
                min_len: *min_len,
            })
        }
        Type::UnpackType { typ } => {
            let inner = Box::new(expand_aliases_depth(
                typ,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            Some(Type::UnpackType { typ: inner })
        }
        Type::TypedDictType {
            fallback,
            items,
            required_keys,
            readonly_keys,
            is_closed,
        } => {
            let fb = Box::new(expand_aliases_depth(
                fallback,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            let new_items = items
                .iter()
                .map(|(k, v)| {
                    Some((
                        k.clone(),
                        expand_aliases_depth(
                            v,
                            alias_resolver,
                            strict_optional,
                            depth + 1,
                            active,
                        )?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Type::TypedDictType {
                fallback: fb,
                items: new_items,
                required_keys: required_keys.clone(),
                readonly_keys: readonly_keys.clone(),
                is_closed: *is_closed,
            })
        }
        Type::LiteralType { fallback, value } => {
            let fb = Box::new(expand_aliases_depth(
                fallback,
                alias_resolver,
                strict_optional,
                depth + 1,
                active,
            )?);
            Some(Type::LiteralType {
                fallback: fb,
                value: value.clone(),
            })
        }
        Type::Parameters(p) => {
            let np = expand_parameters_depth(p, alias_resolver, strict_optional, depth, active)?;
            Some(Type::Parameters(np))
        }
        // Scalars and the deliberately un-walked carriers (UnboundType args,
        // AnyType.source_any): clone as-is. The TypeAliasType early-return
        // in `is_subtype` catches anything we miss here.
        _ => Some(typ.clone()),
    }
}

/// Walk a `Parameters`' type-bearing fields (`arg_types`, `variables`) for
/// `expand_aliases_depth`; the rest are scalars.
fn expand_parameters_depth(
    p: &wire::Parameters,
    alias_resolver: &crate::aliases::TypeAliasResolver,
    strict_optional: bool,
    depth: u32,
    active: &mut Vec<ActiveAlias>,
) -> Option<wire::Parameters> {
    let arg_types = p
        .arg_types
        .iter()
        .map(|a| expand_aliases_depth(a, alias_resolver, strict_optional, depth + 1, active))
        .collect::<Option<Vec<_>>>()?;
    let variables = p
        .variables
        .iter()
        .map(|v| expand_aliases_depth(v, alias_resolver, strict_optional, depth + 1, active))
        .collect::<Option<Vec<_>>>()?;
    Some(wire::Parameters {
        arg_types,
        arg_kinds: p.arg_kinds.clone(),
        arg_names: p.arg_names.clone(),
        variables,
        imprecise_arg_kinds: p.imprecise_arg_kinds,
        is_ellipsis_args: p.is_ellipsis_args,
    })
}

/// Decode a wire-format `Type` blob via `wire::read_type`. Returns
/// `None` on any read failure (truncated input, unknown tag).
fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

#[cfg(test)]
/// Encode a `Type` to its wire format via `wire::write_type`. Returns
/// `None` when the type has no wire representation (e.g. a missing
/// `TypeInfo`), mirroring the `encode_type` helpers in other kernel
/// modules. Used by tests to build `TypeInfoSnapshot.bases` blobs.
fn encode_type(typ: &Type) -> Option<Vec<u8>> {
    let mut wbuf = WriteBuffer::new();
    wire::write_type(&mut wbuf, typ).ok()?;
    Some(wbuf.into_bytes())
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

/// `erase_return_self_types` (subtypes.py:2761-2774): if a type is
/// function-like and returns `self_type`, replace the return type with
/// `Any`.
///
/// Mirrors the Python exactly:
///
/// * `CallableType` whose proper return type is an `Instance == self_type`
///   becomes `Any(implementation_artifact)` (all other fields preserved).
/// * `Overloaded` heads erase each item recursively (each item is a
///   `CallableType`).
/// * Any other head (incl. a bare `Instance == self_type`, which is NOT
///   function-like) is returned unchanged.
///
/// The wire format carries proper types only, so the `get_proper_type`
/// calls in Python are no-ops here: the Python shim serializes after
/// `get_proper_type`, and a `TypeAliasType` head defers (`None`).
///
/// Returns `Some(bytes)` of the round-tripped result, or `None` to defer
/// to Python: a non-wire-readable head (`TypeAliasType`, ParamSpec /
/// TypeVarTuple / meta TypeVar inside `variables`, an `ErasedType`-casting
/// subtree, or any `read_type` failure). The Python shim decodes the
/// result through the shared wirefixup path, so live TypeInfo identity is
/// restored. No object identity is carried across the wire: the caller
/// only relies on equality, and `Overloaded`/`CallableType` equality is
/// structural.
fn erase_return_self_types_wire(typ: &Type, self_type: &Type) -> Option<Type> {
    match typ {
        Type::CallableType {
            fallback,
            instance_type,
            is_ellipsis_args,
            implicit,
            is_bound,
            from_concatenate,
            imprecise_arg_kinds,
            unpack_kwargs,
            from_type_type,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            name,
            variables,
            type_guard,
            type_is,
        } => {
            if !match ret_type.as_ref() {
                Type::Instance { .. } => ret_type.as_ref() == self_type,
                Type::TypeAliasType { .. } => return None,
                _ => false,
            } {
                return Some(typ.clone());
            }
            Some(Type::CallableType {
                fallback: fallback.clone(),
                instance_type: instance_type.clone(),
                is_ellipsis_args: *is_ellipsis_args,
                implicit: *implicit,
                is_bound: *is_bound,
                from_concatenate: *from_concatenate,
                imprecise_arg_kinds: *imprecise_arg_kinds,
                unpack_kwargs: *unpack_kwargs,
                from_type_type: *from_type_type,
                arg_types: arg_types.clone(),
                arg_kinds: arg_kinds.clone(),
                arg_names: arg_names.clone(),
                ret_type: Box::new(Type::AnyType {
                    type_of_any: 8, // TypeOfAny.implementation_artifact
                    source_any: None,
                    missing_import_name: None,
                }),
                name: name.clone(),
                variables: variables.clone(),
                type_guard: type_guard.clone(),
                type_is: type_is.clone(),
            })
        }
        Type::Overloaded { items } => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(erase_return_self_types_wire(item, self_type)?);
            }
            Some(Type::Overloaded { items: out })
        }
        _ => Some(typ.clone()),
    }
}

/// `#[pyfunction]` entry for `erase_return_self_types`.
///
/// Returns `Some(bytes)` when the head is wire-handled (the shim decodes
/// it back to a live `Type`); `None` defers to the pure-Python body.
#[pyfunction]
pub(crate) fn rust_erase_return_self_types(
    typ_bytes: &[u8],
    self_type_bytes: &[u8],
) -> Option<Vec<u8>> {
    let typ = decode_type(typ_bytes)?;
    let self_type = decode_type(self_type_bytes)?;
    let result = erase_return_self_types_wire(&typ, &self_type)?;
    let mut buf = WriteBuffer::new();
    wire::write_type(&mut buf, &result).ok()?;
    Some(buf.into_bytes())
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
pub(crate) fn is_more_precise(
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
///
/// The Python shim passes `right` already expanded
/// (`right = get_proper_type(right)`, subtypes.py:2899) but passes `left`
/// raw. A top-level `left` alias would otherwise hit the recursive
/// `is_subtype` alias guard and defer the whole call; expand aliases here
/// (mirroring `get_proper_type` in the `is_proper_subtype` fallback), so
/// the alias-shaped operand answers natively.
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
    let left = expand_aliases(&left, resolver.alias_resolver(), strict_optional)?;
    let right = expand_aliases(&right, resolver.alias_resolver(), strict_optional)?;
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
pub(crate) fn is_equivalent(
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
///
/// `rust_is_subtype` expands both operands through the alias resolver
/// (mirroring `_is_subtype`'s `get_proper_type`); this seam feeds
/// `is_subtype` directly, so a `TypeAliasType` operand was reaching the
/// recursive alias guard and deferring. Expand both operands here for
/// parity with the `is_subtype(a, b)` fallback.
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
    let a = expand_aliases(&a, resolver.alias_resolver(), strict_optional)?;
    let b = expand_aliases(&b, resolver.alias_resolver(), strict_optional)?;
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
pub(crate) fn is_same_type(
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
    let first = expand_aliases(&first, resolver.alias_resolver(), strict_optional)?;
    for b_bytes in items_bytes.iter().skip(1) {
        let b = decode_type(b_bytes)?;
        let b = expand_aliases(&b, resolver.alias_resolver(), strict_optional)?;
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
///
/// Expand alias operands through the resolver (mirroring the
/// `is_proper_subtype` fallback, which runs `get_proper_type` via
/// `_is_subtype`). Without this, a `TypeAliasType` operand hit the
/// recursive alias guard in `is_subtype` and deferred the whole call.
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
    let a = expand_aliases(&a, resolver.alias_resolver(), strict_optional)?;
    let b = expand_aliases(&b, resolver.alias_resolver(), strict_optional)?;
    let answer = is_same_type(
        &a,
        &b,
        ignore_promotions,
        strict_optional,
        resolver.resolver(),
    );
    answer
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
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let left = expand_aliases(&left, resolver.alias_resolver(), strict_optional)?;
    let right = expand_aliases(&right, resolver.alias_resolver(), strict_optional)?;
    let ctx = SubtypeContext::with_callable_flags(
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        proper_subtype,
        strict_optional,
        ignore_pos_arg_names,
        strict_concatenate,
    );
    let answer = is_subtype(&left, &right, &ctx, resolver.resolver());
    answer
}

/// `#[pyfunction]` entry: batch variant of `rust_is_subtype`.
///
/// Arrives as a flat vec of interleaved `(left, right)` byte blobs plus
/// the single shared `SubtypeContext` flag set (the Python accumulator
/// only batches pairs whose resolved flags are identical). Each entry is
/// decoded and answered with the same `is_subtype` engine as the
/// single-pair entry; the output is one i8 per pair, position-aligned:
/// 1 = true, 0 = false, -1 = defer (decode error or the engine returned
/// `None`). A deferring entry only marks its own slot: the remaining
/// pairs still get their answers, matching the single-pair fallthrough
/// semantics so the Python shim re-runs exactly the deferred pairs.
#[pyfunction]
#[allow(clippy::too_many_arguments, dead_code)]
pub(crate) fn rust_is_subtype_batch(
    pairs_bytes: Vec<&[u8]>,
    ignore_type_params: bool,
    ignore_declared_variance: bool,
    always_covariant: bool,
    ignore_promotions: bool,
    proper_subtype: bool,
    strict_optional: bool,
    ignore_pos_arg_names: bool,
    strict_concatenate: bool,
    resolver: &mut NativeTypeResolver,
) -> Vec<i8> {
    let ctx = SubtypeContext::with_callable_flags(
        ignore_type_params,
        ignore_declared_variance,
        always_covariant,
        ignore_promotions,
        proper_subtype,
        strict_optional,
        ignore_pos_arg_names,
        strict_concatenate,
    );
    let alias_resolver = resolver.alias_resolver();
    let type_resolver = resolver.resolver();
    let mut out = Vec::with_capacity(pairs_bytes.len() / 2);
    let (chunks, remainder) = pairs_bytes.as_chunks::<2>();
    for [a, b] in chunks {
        let answer = match (decode_type(a), decode_type(b)) {
            (Some(left), Some(right)) => {
                let left = match expand_aliases(&left, alias_resolver, strict_optional) {
                    Some(t) => t,
                    None => {
                        out.push(-1);
                        continue;
                    }
                };
                let right = match expand_aliases(&right, alias_resolver, strict_optional) {
                    Some(t) => t,
                    None => {
                        out.push(-1);
                        continue;
                    }
                };
                is_subtype(&left, &right, &ctx, type_resolver)
            }
            _ => None,
        };
        out.push(match answer {
            Some(true) => 1,
            Some(false) => 0,
            None => -1,
        });
    }
    // An odd trailing blob should never arrive (the Python edge always
    // emits full pairs); mark it defer so the shim re-runs and surfaces
    // the mismatch instead of silently dropping.
    out.extend(remainder.iter().map(|_| -1));
    out
}

/// `#[pyfunction]` entry for the TypeVarTupleType-right branch of
/// `visit_instance` (subtypes.py:617-620). The Python parity suite calls
/// this directly with serialized operands to prove the Rust seam engages
/// (returns `Some(bool)`) for the portable path and defers cleanly
/// (`None`) when the left target's snapshot is missing.
///
/// The shim in `mypy/subtypes.py` routes the actual subtype check
/// through this only when `left` is an `Instance` and `right` is a
/// `TypeVarTupleType`; all flags default to the non-proper subtype
/// context, matching the `visit_instance` call the function ports.
#[pyfunction]
#[allow(dead_code)]
pub(crate) fn rust_subtype_tvar_tuple_right(
    left_bytes: &[u8],
    right_bytes: &[u8],
    proper_subtype: bool,
    resolver: &mut NativeTypeResolver,
) -> Option<bool> {
    let left = decode_type(left_bytes)?;
    let right = decode_type(right_bytes)?;
    let Type::Instance { type_ref, .. } = &left else {
        return None;
    };
    let ctx = SubtypeContext::with_callable_flags(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        false, // ignore_promotions
        proper_subtype,
        false, // strict_optional
        false, // ignore_pos_arg_names
        false, // strict_concatenate
    );
    visit_instance_variadic_right(&left, &right, &ctx, resolver.resolver(), type_ref.as_str())
}

/// Decision tags; values must match `NATIVE_ARE_ARGS_*` in mypy/subtypes.py.
const KIND_ARE_ARGS_FALSE: i64 = 0;
const KIND_ARE_ARGS_TRUE: i64 = 1;
const KIND_ARE_ARGS_CALL_IS_COMPAT: i64 = 2;

/// `are_args_compatible.is_different` (subtypes.py:2640-2654): true when the
/// left/right items differ, defaulting to False when the right is unspecified
/// (None) or, under partial overlap, when the left is also unspecified.
fn is_different(left_is_none: bool, right_is_none: bool, equal: bool, allow_overlap: bool) -> bool {
    if right_is_none {
        return false;
    }
    if allow_overlap && left_is_none {
        return false;
    }
    !equal
}

/// Pure decision core of `mypy.subtypes.are_args_compatible`
/// (subtypes.py:2627-2681), over resolved scalar facts. Mirrors the
/// Python branch order: the name gate, the position gate, the
/// required-arity gate, the partial-overlap shortcut, then the tail.
/// Returns FALSE / TRUE / CALL_IS_COMPAT.
#[allow(clippy::too_many_arguments)]
fn classify_are_args_compatible(
    left_name_is_none: bool,
    right_name_is_none: bool,
    names_equal: bool,
    right_pos_is_none: bool,
    pos_equal: bool,
    left_required: bool,
    right_required: bool,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    allow_imprecise_kinds: bool,
) -> i64 {
    // subtypes.py:2636-2638: when both args are required, overlap is off.
    let partial = allow_partial_overlap && !(left_required && right_required);

    // subtypes.py:2658-2661: name gate.
    if is_different(left_name_is_none, right_name_is_none, names_equal, partial)
        && (!ignore_pos_arg_names || right_pos_is_none)
    {
        return KIND_ARE_ARGS_FALSE;
    }

    // subtypes.py:2666-2667: position gate (overlap disabled for pos).
    if is_different(false, right_pos_is_none, pos_equal, false) && !allow_imprecise_kinds {
        return KIND_ARE_ARGS_FALSE;
    }

    // subtypes.py:2672-2673: optional right, required left, no overlap.
    if !partial && !right_required && left_required {
        return KIND_ARE_ARGS_FALSE;
    }

    // subtypes.py:2677-2678: overlap shortcut for two optional args.
    if partial && !left_required && !right_required {
        return KIND_ARE_ARGS_TRUE;
    }

    // subtypes.py:2681: tail `return is_compat(right.typ, left.typ)`.
    KIND_ARE_ARGS_CALL_IS_COMPAT
}

/// Read a bool flag attribute off a live Python object; return `None` to
/// defer on any read/truthiness failure (strangler-fig fallback).
fn read_bool_attr(obj: &PyAny, name: &str) -> PyResult<Option<bool>> {
    match obj.getattr(name) {
        Ok(v) => match v.is_true() {
            Ok(b) => Ok(Some(b)),
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

/// Read an attribute as a borrowed reference; return `None` (defer) when
/// the attribute cannot be read.
fn read_attr_opt<'py>(obj: &'py PyAny, name: &str) -> PyResult<Option<&'py PyAny>> {
    match obj.getattr(name) {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(None),
    }
}

/// Python `a == b` under rich comparison; return `None` (defer) on any
/// comparison or truthiness failure.
fn py_eq(a: &PyAny, b: &PyAny) -> PyResult<Option<bool>> {
    match a.rich_compare(b, CompareOp::Eq) {
        Ok(r) => match r.is_true() {
            Ok(b) => Ok(Some(b)),
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

/// `#[pyfunction]` entry for `mypy.subtypes.are_args_compatible`
/// (subtypes.py:2627-2681).
///
/// Reads the `left`/`right` `FormalArgument` scalar fields (`name`,
/// `pos`, `required`) plus the three bool flag args via PyO3, and
/// returns a tag: FALSE (0), TRUE (1), or CALL_IS_COMPAT (2). The Python
/// shim keeps the trailing `is_compat(right.typ, left.typ)` call and the
/// pure-Python body as the fallback. `None` defers on any unreadable
/// attribute (strangler-fig contract).
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_are_args_compatible(
    left: &PyAny,
    right: &PyAny,
    ignore_pos_arg_names: bool,
    allow_partial_overlap: bool,
    allow_imprecise_kinds: bool,
) -> PyResult<Option<i64>> {
    let left_required = match read_bool_attr(left, "required")? {
        Some(b) => b,
        None => return Ok(None),
    };
    let right_required = match read_bool_attr(right, "required")? {
        Some(b) => b,
        None => return Ok(None),
    };

    let left_name = match read_attr_opt(left, "name")? {
        Some(v) => v,
        None => return Ok(None),
    };
    let right_name = match read_attr_opt(right, "name")? {
        Some(v) => v,
        None => return Ok(None),
    };
    let left_pos = match read_attr_opt(left, "pos")? {
        Some(v) => v,
        None => return Ok(None),
    };
    let right_pos = match read_attr_opt(right, "pos")? {
        Some(v) => v,
        None => return Ok(None),
    };

    let left_name_is_none = left_name.is_none();
    let right_name_is_none = right_name.is_none();
    let right_pos_is_none = right_pos.is_none();

    let names_equal = match py_eq(left_name, right_name)? {
        Some(b) => b,
        None => return Ok(None),
    };
    let pos_equal = match py_eq(left_pos, right_pos)? {
        Some(b) => b,
        None => return Ok(None),
    };

    Ok(Some(classify_are_args_compatible(
        left_name_is_none,
        right_name_is_none,
        names_equal,
        right_pos_is_none,
        pos_equal,
        left_required,
        right_required,
        ignore_pos_arg_names,
        allow_partial_overlap,
        allow_imprecise_kinds,
    )))
}

// =====================================================================
// Issue #998: check_type_parameter variance dispatch (subtypes.py:891)
// =====================================================================

/// Decision tags returned by `rust_classify_type_parameter`; must match
/// the `NATIVE_TYPEPARAM_*` constants in mypy/subtypes.py.
pub(crate) const KIND_TYPEPARAM_SUBTYPE: i64 = 0;
pub(crate) const KIND_TYPEPARAM_SUBTYPE_SWAP: i64 = 1;
pub(crate) const KIND_TYPEPARAM_PROPER_SUBTYPE: i64 = 2;
pub(crate) const KIND_TYPEPARAM_PROPER_SWAP: i64 = 3;
pub(crate) const KIND_TYPEPARAM_SAME: i64 = 4;
pub(crate) const KIND_TYPEPARAM_EQUIVALENT: i64 = 5;

/// Pure variance-dispatch tail of `check_type_parameter`
/// (subtypes.py:903-922): map the (upgraded) variance plus the
/// `proper_subtype` flag onto the leaf-call tag.
fn classify_type_parameter_dispatch(variance: i64, proper_subtype: bool) -> i64 {
    if variance == COVARIANT || variance == VARIANCE_NOT_READY {
        if proper_subtype {
            KIND_TYPEPARAM_PROPER_SUBTYPE
        } else {
            KIND_TYPEPARAM_SUBTYPE
        }
    } else if variance == CONTRAVARIANT {
        if proper_subtype {
            KIND_TYPEPARAM_PROPER_SWAP
        } else {
            KIND_TYPEPARAM_SUBTYPE_SWAP
        }
    } else if proper_subtype {
        KIND_TYPEPARAM_SAME
    } else {
        KIND_TYPEPARAM_EQUIVALENT
    }
}

/// `#[pyfunction]` entry for `mypy.subtypes.check_type_parameter`
/// (subtypes.py:891-922).
///
/// Reads the scalar `variance` / `proper_subtype` facts plus the live
/// `left` type via PyO3 (a `get_proper_type` call, an `isinstance`
/// against `mypy.types.UninhabitedType`, and the `ambiguous` bool) to
/// run the invariant-to-covariant upgrade, then returns the leaf tag:
/// which subtype leaf to call and with which argument order. Python
/// applies the leaf call (`is_subtype`, `is_proper_subtype`,
/// `is_same_type`, `is_equivalent`), keeping the pure-Python body as
/// the fallback. `None` defers on any unreadable PyO3 fact.
#[pyfunction]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn rust_classify_type_parameter(
    py: Python<'_>,
    left: &PyAny,
    variance: i64,
    proper_subtype: bool,
) -> PyResult<Option<i64>> {
    Ok(classify_type_parameter(py, left, variance, proper_subtype))
}

fn classify_type_parameter(
    py: Python<'_>,
    left: &PyAny,
    mut variance: i64,
    proper_subtype: bool,
) -> Option<i64> {
    // subtypes.py:896-899: an ambiguous UninhabitedType on the left is
    // treated as covariant even under an invariant typevar, since such
    // a type can never be stored (checker.is_valid_inferred_type).
    if variance == INVARIANT {
        let mypy_types = py.import("mypy.types").ok()?;
        let get_proper_type = mypy_types.getattr("get_proper_type").ok()?;
        let p_left = get_proper_type.call1((left,)).ok()?;
        let uninhabited = mypy_types.getattr("UninhabitedType").ok()?;
        if p_left.is_instance(uninhabited).ok()? {
            let ambiguous = read_bool_attr(p_left, "ambiguous").ok()??;
            if ambiguous {
                variance = COVARIANT;
            }
        }
    }
    Some(classify_type_parameter_dispatch(variance, proper_subtype))
}

// =====================================================================
// Issue #968: is_descriptor (subtypes.py:2177-2183)
// =====================================================================

/// `is_descriptor` (subtypes.py:2177-2183): recursive bool predicate.
/// An Instance is a descriptor when its class (via MRO) has a `__get__`
/// member. A UnionType is a descriptor when all relevant items are
/// descriptors (`NoneType` items are filtered when `strict_optional` is
/// off, matching `UnionType.relevant_items`). All other types are not
/// descriptors. Defers (None) on `TypeAliasType` (can't expand without
/// the alias target) and when the resolver snapshot is missing for any
/// MRO class consulted.
#[pyfunction]
pub(crate) fn rust_is_descriptor(
    resolver: &NativeTypeResolver,
    type_bytes: &[u8],
    strict_optional: bool,
) -> Option<bool> {
    let typ = decode_type(type_bytes)?;
    is_descriptor_inner(&typ, resolver.resolver(), strict_optional)
}

/// Pure recursion core of `is_descriptor`, operating on decoded wire
/// `Type` values. Defers (None) on `TypeAliasType` and missing resolver
/// snapshots; returns `Some(bool)` for all decidable cases.
fn is_descriptor_inner(typ: &Type, resolver: &TypeResolver, strict_optional: bool) -> Option<bool> {
    // get_proper_type: TypeAliasType can't be expanded from wire alone.
    if matches!(typ, Type::TypeAliasType { .. }) {
        return None;
    }
    match typ {
        Type::Instance { type_ref, .. } => {
            crate::checkmember::has_readable_member_by_ref(resolver, type_ref, "__get__")
        }
        Type::UnionType { items, .. } => {
            // Mirror UnionType.relevant_items(): when strict_optional is
            // off, NoneType items are filtered out. TypeAliasType items
            // can't be expanded to check if they're NoneType, so defer.
            let mut relevant: Vec<&Type> = Vec::with_capacity(items.len());
            for item in items {
                if !strict_optional {
                    if matches!(item, Type::TypeAliasType { .. }) {
                        return None;
                    }
                    if matches!(item, Type::NoneType) {
                        continue;
                    }
                }
                relevant.push(item);
            }
            // all([]) is True in Python.
            if relevant.is_empty() {
                return Some(true);
            }
            for item in &relevant {
                match is_descriptor_inner(item, resolver, strict_optional) {
                    Some(true) => continue,
                    Some(false) => return Some(false),
                    None => return None,
                }
            }
            Some(true)
        }
        _ => Some(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::TypeInfoSnapshot;
    use crate::wire::Parameters;

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
            type_of_any: 8, // TypeOfAny.implementation_artifact
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
            from_type_type: false,
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
    fn same_ref_equal_args_fast_path_needs_no_snapshot() {
        // Issue #1096: the same-ref fast path fires above the
        // snapshot-miss defer. typing.Iterator has no snapshot, and
        // Python's nominal branch answers True for the identical pair.
        let r = make_resolver(vec![]);
        let arg = instance("builtins.int", vec![]);
        let left = instance("typing.Iterator", vec![arg.clone()]);
        let right = instance("typing.Iterator", vec![arg]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn same_ref_alias_arg_defers_without_snapshot() {
        // Alias-carrying args keep the hoisted fast path out of the way:
        // a deferred same-ref pair lets Python run its full visitor whose
        // type_state bookkeeping later protocol checks consult, so defer.
        let r = make_resolver(vec![]);
        let alias = alias_type(vec![], "mod.Iter");
        let left = instance("typing.Iterator", vec![alias.clone()]);
        let right = instance("typing.Iterator", vec![alias]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn erased_right_non_proper_fast_path() {
        // subtypes.py:754-761: non-proper subtype of an ErasedType right is always
        // True. The shim gate (subtypes.py:842-844) filters only top-level Erased
        // operands, so nested leaves ride this path; the Instance tail says False.
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = instance("a.A", vec![]);
        assert_eq!(
            is_subtype(&left, &Type::ErasedType, &ctx_nominal(), &r),
            Some(true)
        );
    }

    #[test]
    fn union_right_with_erased_item_true() {
        // subtypes.py:363-410 union-right recursion per item; the non-proper check
        // matches the Erased item via the fast path, so union-right answers True
        // though the other item is unrelated (Python: same result).
        let r = make_resolver(vec![
            snap("a.A", "A"),
            snap("b.B", "B"),
            snap("builtins.object", "object"),
        ]);
        let left = instance("a.A", vec![]);
        let right = Type::UnionType {
            items: vec![instance("b.B", vec![]), Type::ErasedType],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn erased_right_proper_subtype_false() {
        // proper_subtype=True: the fast path does not fire; Python's
        // SubtypeVisitor.visit_instance falls through to `else: return
        // False` for an ErasedType right (subtypes.py:683).
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let left = instance("a.A", vec![]);
        let ctx = SubtypeContext::new(false, false, false, false, true, true);
        assert_eq!(is_subtype(&left, &Type::ErasedType, &ctx, &r), Some(false));
    }

    #[test]
    fn unpack_left_erased_right_defers() {
        // Python's fast-path guard is `not isinstance(left, UnpackType)`
        // (subtypes.py:757-760); mirror it as a defer so the pure-Python
        // body decides the rare Unpack-vs-Erased pair.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = Type::UnpackType {
            typ: Box::new(instance("a.A", vec![])),
        };
        assert_eq!(
            is_subtype(&left, &Type::ErasedType, &ctx_nominal(), &r),
            None
        );
    }

    #[test]
    fn same_ref_nested_alias_arg_defers_without_snapshot() {
        // The alias check is recursive: an alias three levels deep
        // (arg -> Instance args -> TypeAliasType) also defers.
        let r = make_resolver(vec![]);
        let arg = instance("builtins.list", vec![alias_type(vec![], "mod.Iter")]);
        let left = instance("typing.Iterator", vec![arg.clone()]);
        let right = instance("typing.Iterator", vec![arg]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn same_ref_args_differ_defers_without_snapshot() {
        // Different args keep the fast path out of the way: without a
        // snapshot the variance walk cannot decide, so defer.
        let r = make_resolver(vec![]);
        let left = instance("typing.Iterator", vec![instance("builtins.int", vec![])]);
        let right = instance("typing.Iterator", vec![instance("builtins.str", vec![])]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn same_ref_fast_path_skipped_under_ignore_declared_variance() {
        // Python skips the nominal branch under ignore_declared_variance
        // and can then answer False, so the fast path must not fire.
        let r = make_resolver(vec![]);
        let arg = instance("builtins.int", vec![]);
        let left = instance("typing.Iterator", vec![arg.clone()]);
        let right = instance("typing.Iterator", vec![arg]);
        let ctx = SubtypeContext::new(false, true, false, false, false, true);
        assert_eq!(is_subtype(&left, &right, &ctx, &r), None);
    }

    #[test]
    fn same_ref_fast_path_skipped_in_proper_mode() {
        // proper-subtype checks keep the full walk (protocol/cache
        // semantics), matching the old post-snapshot fast path guard.
        let r = make_resolver(vec![]);
        let arg = instance("builtins.int", vec![]);
        let left = instance("typing.Iterator", vec![arg.clone()]);
        let right = instance("typing.Iterator", vec![arg]);
        let ctx = SubtypeContext::new(false, false, false, false, true, true);
        assert_eq!(is_subtype(&left, &right, &ctx, &r), None);
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
    fn instance_left_typevartuple_right_is_false() {
        // Instance <: TypeVarTupleType: has_base("builtins.tuple") guard
        // fails for a.A, so the variadic branch does not apply and the
        // visit_instance tail answers False (subtypes.py:736-741, 683).
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
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
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
        base.type_vars = vec!["T".to_string()];
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
    fn variadic_left_defers_at_derivation_path() {
        // Variadic-left frames defer: the subset walker doesn't fully
        // handle prefix/suffix splicing for TypeVarTuple substitution.
        // The guard in map_derivation_path returns None (defer to Python).
        let tvt = Type::TypeVarTupleType {
            name: "Ts".to_string(),
            fullname: "a.Sub.Ts".to_string(),
            raw_id: 1,
            namespace: "a.Sub".to_string(),
            upper_bound: Box::new(instance("builtins.object", vec![])),
            tuple_fallback: Box::new(instance("builtins.tuple", vec![])),
            default: Box::new(Type::UninhabitedType { ambiguous: false }),
            min_len: 0,
        };
        let mut base = snap("a.Gen", "Gen");
        base.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let mut derived = snap("a.Sub", "Sub");
        derived.bases.push(
            encode_type(&instance(
                "a.Gen",
                vec![Type::TupleType {
                    partial_fallback: Box::new(instance("builtins.tuple", vec![])),
                    items: vec![Type::UnpackType {
                        typ: Box::new(tvt.clone()),
                    }],
                    implicit: false,
                }],
            ))
            .unwrap(),
        );
        derived.has_base.insert("a.Gen".to_string());
        derived.mro.push("a.Gen".to_string());
        derived.has_type_var_tuple_type = true;
        derived.type_vars_with_variance = vec![("Ts".to_string(), COVARIANT, 2)];
        derived.type_var_raw_ids = vec![1];
        derived.type_var_tuple_prefix = Some(0);
        derived.type_var_tuple_suffix = Some(0);
        derived.type_var_tuple_fallback = Some(encode_type(&tvt).unwrap());
        let r = make_resolver(vec![base, derived]);
        let left = instance("a.Sub", vec![instance("builtins.int", vec![])]);
        let right = instance("a.Gen", vec![any_type()]);
        let result = is_subtype(&left, &right, &ctx_nominal(), &r);
        // Variadic left defers: the guard catches has_type_var_tuple_type
        // and returns None before attempting the env-based expansion.
        assert_eq!(result, None);
    }

    #[test]
    fn variadic_left_without_bases_still_defers() {
        // Left is variadic but has no bases blobs (stale snapshot):
        // map_instance_to_supertype still returns None -> defer.
        let mut derived = snap("a.Sub", "Sub");
        derived.has_type_var_tuple_type = true;
        derived.has_base.insert("a.Gen".to_string());
        let mut base = snap("a.Gen", "Gen");
        base.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let r = make_resolver(vec![base, derived]);
        let left = instance("a.Sub", vec![instance("builtins.int", vec![])]);
        let right = instance("a.Gen", vec![any_type()]);
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
        let expanded = expand_type_by_instance(&base, "a.Sub", std::slice::from_ref(&left_arg));
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
        let expanded = expand_type_by_instance(&outer, "a.Sub", std::slice::from_ref(&left_arg));
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
    fn expand_type_by_instance_expands_tuple_items_and_fallback() {
        // TupleType is handled by the subset walker: items and the
        // builtins.tuple fallback are expanded recursively.
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
        let t = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![tvar.clone()])),
            items: vec![tvar],
            implicit: false,
        };
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&t, "a.Sub", std::slice::from_ref(&left_arg));
        let expected = Type::TupleType {
            partial_fallback: Box::new(instance("builtins.tuple", vec![left_arg.clone()])),
            items: vec![left_arg],
            implicit: false,
        };
        assert_eq!(expanded, Some(expected));
    }

    #[test]
    fn expand_type_by_instance_defers_single_unpack_tuple() {
        // Tuple[*tuple[X, ...]] with a builtins.tuple fallback normalizes
        // to the inner Instance in Python (expandtype.py:973-989); the
        // normalization is not ported, so defer.
        let unpack = Type::UnpackType {
            typ: Box::new(instance("builtins.tuple", vec![any_type()])),
        };
        let t = tuple_type(vec![unpack]);
        assert_eq!(expand_type_by_instance(&t, "a.Sub", &[any_type()]), None);
    }

    #[test]
    fn expand_type_by_instance_expands_callable_args_and_ret() {
        // Callable[[Instance[T]], Instance[T]] with left = a.Sub[A]:
        // arg_types and ret_type get the TypeVar substituted.
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
        let t = callable_type(
            vec![instance("a.Gen", vec![tvar.clone()])],
            instance("a.Gen", vec![tvar]),
            None,
        );
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&t, "a.Sub", std::slice::from_ref(&left_arg));
        let expected = callable_type(
            vec![instance("a.Gen", vec![left_arg.clone()])],
            instance("a.Gen", vec![left_arg]),
            None,
        );
        assert_eq!(expanded, Some(expected));
    }

    #[test]
    fn expand_type_by_instance_defers_paramspec_arg() {
        // A Callable declaring a ParamSpec takes the *args: P.args +
        // **kwargs: P.kwargs branch in Python (expandtype.py:871-899);
        // Parameters join/prefix merging is not ported, so defer.
        let paramspec = Type::ParamSpecType {
            prefix: Box::new(Parameters {
                arg_types: vec![],
                arg_kinds: vec![],
                arg_names: vec![],
                variables: vec![],
                imprecise_arg_kinds: false,
                is_ellipsis_args: false,
            }),
            name: "P".to_string(),
            fullname: "a.Sub.P".to_string(),
            raw_id: 1,
            namespace: "a.Sub".to_string(),
            flavor: 0,
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
        };
        let t = callable_type(vec![], any_type(), None);
        let with_paramspec = match t {
            Type::CallableType {
                variables: _,
                fallback,
                instance_type,
                is_ellipsis_args,
                implicit,
                is_bound,
                from_concatenate,
                imprecise_arg_kinds,
                unpack_kwargs,
                from_type_type,
                arg_types,
                arg_kinds,
                arg_names,
                ret_type,
                name,
                type_guard,
                type_is,
            } => Type::CallableType {
                variables: vec![paramspec],
                fallback,
                instance_type,
                is_ellipsis_args,
                implicit,
                is_bound,
                from_concatenate,
                imprecise_arg_kinds,
                unpack_kwargs,
                from_type_type,
                arg_types,
                arg_kinds,
                arg_names,
                ret_type,
                name,
                type_guard,
                type_is,
            },
            _ => unreachable!(),
        };
        assert_eq!(
            expand_type_by_instance(&with_paramspec, "a.Sub", &[any_type()]),
            None
        );
    }

    #[test]
    fn expand_type_by_instance_defers_star_unpack_arg() {
        // A var-arg typed UnpackType needs interpolate_args_for_unpack
        // (expandtype.py:840-868), which is not ported; defer.
        let unpack = Type::UnpackType {
            typ: Box::new(instance("builtins.tuple", vec![any_type()])),
        };
        let t = callable_type(vec![], any_type(), None);
        let with_star = match t {
            Type::CallableType {
                variables: _,
                fallback,
                instance_type,
                is_ellipsis_args,
                implicit,
                is_bound,
                from_concatenate,
                imprecise_arg_kinds,
                unpack_kwargs,
                from_type_type,
                mut arg_types,
                mut arg_kinds,
                mut arg_names,
                ret_type,
                name,
                type_guard,
                type_is,
            } => {
                arg_types.push(unpack);
                arg_kinds.push(ARG_STAR);
                arg_names.push(None);
                Type::CallableType {
                    variables: vec![],
                    fallback,
                    instance_type,
                    is_ellipsis_args,
                    implicit,
                    is_bound,
                    from_concatenate,
                    imprecise_arg_kinds,
                    unpack_kwargs,
                    from_type_type,
                    arg_types,
                    arg_kinds,
                    arg_names,
                    ret_type,
                    name,
                    type_guard,
                    type_is,
                }
            }
            _ => unreachable!(),
        };
        assert_eq!(
            expand_type_by_instance(&with_star, "a.Sub", &[any_type()]),
            None
        );
    }

    #[test]
    fn expand_type_by_instance_expands_overloaded_items() {
        // Overloaded items are CallableTypes expanded item-wise
        // (expandtype.py:941-948).
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
        let t = Type::Overloaded {
            items: vec![
                callable_type(vec![tvar.clone()], any_type(), None),
                callable_type(vec![], tvar.clone(), None),
            ],
        };
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&t, "a.Sub", std::slice::from_ref(&left_arg));
        let expected = Type::Overloaded {
            items: vec![
                callable_type(vec![left_arg.clone()], any_type(), None),
                callable_type(vec![], left_arg, None),
            ],
        };
        assert_eq!(expanded, Some(expected));
    }

    #[test]
    fn expand_type_by_instance_expands_type_type_item() {
        // TypeType[Instance[T]] with left = a.Sub[A] expands the item
        // (expandtype.py:1036-1041).
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
        let t = Type::TypeType {
            item: Box::new(instance("a.Gen", vec![tvar])),
            is_type_form: false,
        };
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&t, "a.Sub", std::slice::from_ref(&left_arg));
        assert_eq!(
            expanded,
            Some(Type::TypeType {
                item: Box::new(instance("a.Gen", vec![left_arg])),
                is_type_form: false,
            })
        );
    }

    #[test]
    fn expand_type_by_instance_defers_type_type_union_item() {
        // Type[Union[...]] distributes via TypeType.make_normalized in
        // Python (expandtype.py:1036-1041); the union distribution is not
        // ported, so defer.
        let t = Type::TypeType {
            item: Box::new(Type::UnionType {
                items: vec![any_type(), any_type()],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            }),
            is_type_form: false,
        };
        assert_eq!(expand_type_by_instance(&t, "a.Sub", &[any_type()]), None);
    }

    #[test]
    fn expand_type_by_instance_expands_unpack_inner() {
        // UnpackType[Instance[T]] with left = a.Sub[A]: the inner type
        // expands and the unpack re-wraps (expandtype.py:804-815).
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
        let t = Type::UnpackType {
            typ: Box::new(instance("a.Gen", vec![tvar])),
        };
        let left_arg = instance("a.A", vec![]);
        let expanded = expand_type_by_instance(&t, "a.Sub", std::slice::from_ref(&left_arg));
        assert_eq!(
            expanded,
            Some(Type::UnpackType {
                typ: Box::new(instance("a.Gen", vec![left_arg])),
            })
        );
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
        // Different args keep the same-ref fast path out of the way.
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars_with_variance = vec![("T".to_string(), VARIANCE_NOT_READY, 0)];
        let r = make_resolver(vec![gen]);
        let left = instance("a.Gen", vec![any_type()]);
        let right = instance("a.Gen", vec![instance("a.A", vec![])]);
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
        // (subtypes.py:1126-1130). The lane routed Callable-vs-Callable
        // into the native callable_compat engine, so the item check no

        // longer defers: identical signatures -> True.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let item = callable_type(vec![], Type::NoneType, None);
        let left = Type::Overloaded {
            items: vec![item.clone()],
        };
        assert_eq!(is_subtype(&left, &item, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn overloaded_not_subtype_of_callable_when_none_match() {
        // right is CallableType: no item matches -> False. The items differ
        // (ret_type None vs int instance); callable_compat decides it.
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap("builtins.int", "int"),
        ]);
        let item1 = callable_type(vec![], Type::NoneType, None);
        let item2 = callable_type(vec![], instance("builtins.int", vec![]), None);
        let left = Type::Overloaded {
            items: vec![item1.clone()],
        };
        // item1 vs item2: Callable-vs-Callable -> False.
        assert_eq!(is_subtype(&left, &item2, &ctx_nominal(), &r), Some(false));
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
    fn overloaded_instance_right_recurses_on_fallback() {
        // right is a plain Instance: is_subtype(left.fallback, right)
        // (subtypes.py:1118). builtins.function <: builtins.function.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let item = callable_type(vec![], Type::NoneType, None);
        let left = Type::Overloaded { items: vec![item] };
        let right = instance("builtins.function", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn overloaded_protocol_call_right_still_defers() {
        // right is a protocol whose members include __call__: needs
        // find_member + is_protocol_implementation (Python-only). Defer
        // (subtypes.py:1115-1125).
        let mut proto = snap("mod.CallProto", "CallProto");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__call__".to_string()];
        let r = make_resolver(vec![snap("builtins.function", "function"), proto]);
        let item = callable_type(vec![], Type::NoneType, None);
        let left = Type::Overloaded { items: vec![item] };
        let right = instance("mod.CallProto", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn callable_protocol_call_right_defers_without_live_map() {
        // Issue #1255: the call-check port engages only under a live
        // TypeInfo map; a snapshot-only resolver defers, as before.
        let mut proto = snap("mod.CallProto", "CallProto");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__call__".to_string()];
        let r = make_resolver(vec![snap("builtins.function", "function"), proto]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = instance("mod.CallProto", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn callable_protocol_call_right_nocall_defers_protocol_impl() {
        // Protocol right without __call__: the is_type_obj arm decides
        // False, then the fallback recursion re-enters the protocol-right
        // arm, which needs a live map -> defer (None).
        let mut proto = snap("mod.IterableProto", "IterableProto");
        proto.is_protocol = true;
        proto.protocol_members = vec!["__iter__".to_string()];
        let r = make_resolver(vec![snap("builtins.function", "function"), proto]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = instance("mod.IterableProto", vec![]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn overloaded_left_unbound_right_is_true() {
        // subtypes.py:1169: right UnboundType -> True. Non-proper contexts
        // short-circuit True at _is_subtype (subtypes.py:754-761), so a
        // proper context is what reaches the visit_overloaded tail.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = Type::Overloaded {
            items: vec![callable_type(vec![], Type::NoneType, None)],
        };
        let right = Type::UnboundType {
            name: "A".to_string(),
            args: vec![],
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_proper(), &r), Some(true));
    }

    #[test]
    fn overloaded_left_non_type_obj_not_subtype_of_type_type() {
        // subtypes.py:1171-1177: items[0].is_type_obj() is False (plain
        // builtins.function fallback) -> False.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = Type::Overloaded {
            items: vec![callable_type(vec![], instance("a.A", vec![]), None)],
        };
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn overloaded_left_type_obj_item_subtype_of_type_type_recurses() {
        // subtypes.py:1171-1177: items[0].is_type_obj() is True (metaclass
        // fallback); recurse on items[0]. A constructor callable with
        // instance_type compares it against right.item.
        let mut meta = snap("a.M", "M");
        meta.has_base.insert("builtins.type".to_string());
        meta.mro.push("builtins.type".to_string());
        let r = make_resolver(vec![meta, snap("a.A", "A")]);
        let ctor = Type::CallableType {
            fallback: Box::new(instance("a.M", vec![])),
            instance_type: Some(Box::new(instance("a.A", vec![]))),
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
            ret_type: Box::new(instance("a.A", vec![])),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        };
        let left = Type::Overloaded {
            items: vec![ctor.clone()],
        };
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn overloaded_left_unresolvable_fallback_defers_on_type_type() {
        // subtypes.py:1171-1177: items[0].is_type_obj() needs the fallback
        // snapshot; a missing one defers.
        let r = make_resolver(vec![]);
        let left = Type::Overloaded {
            items: vec![callable_type(vec![], instance("a.A", vec![]), None)],
        };
        let right = Type::TypeType {
            item: Box::new(instance("a.A", vec![])),
            is_type_form: false,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn overloaded_left_not_subtype_of_none_right() {
        // subtypes.py:1178: any other right shape (NoneType) -> False.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = Type::Overloaded {
            items: vec![callable_type(vec![], Type::NoneType, None)],
        };
        assert_eq!(
            is_subtype(&left, &Type::NoneType, &ctx_nominal(), &r),
            Some(false)
        );
    }

    // ---- TypeType left, right CallableType/Overloaded (issue #1225,
    // subtypes.py:1240-1278) ----

    fn ctx_proper() -> SubtypeContext {
        // (ignore_type_params, ignore_declared_variance, always_covariant,
        //  ignore_promotions, proper_subtype, strict_optional)
        SubtypeContext::new(false, false, false, false, true, true)
    }

    fn type_obj_callable(ret_type: Type) -> Type {
        // A CallableType whose fallback is a metaclass so
        // callable_compat::is_type_obj answers True.
        Type::CallableType {
            fallback: Box::new(instance("a.M", vec![])),
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
            ret_type: Box::new(ret_type),
            name: None,
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    fn type_type(item: Type) -> Type {
        Type::TypeType {
            item: Box::new(item),
            is_type_form: false,
        }
    }

    #[test]
    fn type_type_proper_subtype_of_non_type_obj_callable_is_false() {
        // subtypes.py:1246-1250: a proper subtype check of Type[X] against
        // a callable that is not a type object is False (transitivity).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = type_type(Type::NoneType);
        let right = callable_type(vec![], instance("builtins.object", vec![]), None);
        assert_eq!(is_subtype(&left, &right, &ctx_proper(), &r), Some(false));
    }

    #[test]
    fn type_type_instance_item_defers_on_callable_right() {
        // subtypes.py:1256-1268: an Instance item needs the live
        // type_object_type; defer. The proper gate passes (right is a
        // metaclass-fallback callable, i.e. a type object).
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap("a.A", "A"),
        ]);
        let left = type_type(instance("a.A", vec![]));
        let right = type_obj_callable(instance("builtins.object", vec![]));
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn type_type_proper_gate_defers_when_right_fallback_unresolvable() {
        // subtypes.py:1246-1250 with is_type_obj undecidable (missing
        // fallback snapshot): defer.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = type_type(instance("a.A", vec![]));
        let right = callable_type(vec![], instance("builtins.object", vec![]), None);
        assert_eq!(is_subtype(&left, &right, &ctx_proper(), &r), None);
    }

    #[test]
    fn type_type_tuple_item_takes_fallback_then_defers() {
        // subtypes.py:1251-1255: a TupleType item is compared via its
        // tuple_fallback (an Instance builtins.tuple[...]), whose
        // constructor needs live machinery -> defer.
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap("a.A", "A"),
        ]);
        let left = type_type(tuple_type(vec![instance("builtins.int", vec![])]));
        let right = type_obj_callable(instance("a.A", vec![]));
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn type_type_item_none_falls_through_to_ret_type_check() {
        // subtypes.py:1271: otherwise the unsound fallthrough compares the
        // item against right.ret_type: NoneType <: object -> True.
        let r = make_resolver(vec![
            snap("builtins.function", "function"),
            snap("builtins.object", "object"),
        ]);
        let left = type_type(Type::NoneType);
        let right = type_obj_callable(instance("builtins.object", vec![]));
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn type_type_right_overloaded_non_type_obj_is_false() {
        // subtypes.py:1240-1243: only a constructor overload qualifies;
        // items[0].is_type_obj() is False -> the overload falls through the
        // CallableType/Instance arms to `return False`.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = type_type(Type::NoneType);
        let right = Type::Overloaded {
            items: vec![callable_type(vec![], instance("a.A", vec![]), None)],
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn type_type_right_type_obj_overloaded_recurses_on_first_item() {
        // subtypes.py:1240-1243: a constructor overload recurses on
        // items[0]: Type[None] <: Callable[..., object] via the fallthrough
        // -> True.
        let mut meta = snap("a.M", "M");
        meta.has_base.insert("builtins.type".to_string());
        meta.mro.push("builtins.type".to_string());
        let r = make_resolver(vec![
            meta,
            snap("builtins.function", "function"),
            snap("builtins.object", "object"),
        ]);
        let left = type_type(Type::NoneType);
        let right = Type::Overloaded {
            items: vec![type_obj_callable(instance("builtins.object", vec![]))],
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn type_type_right_overloaded_unresolvable_item_defers() {
        // subtypes.py:1240-1243: is_type_obj on the first overload item
        // needs its fallback snapshot; a missing one defers.
        let r = make_resolver(vec![]);
        let left = type_type(Type::NoneType);
        let right = Type::Overloaded {
            items: vec![callable_type(vec![], instance("a.A", vec![]), None)],
        };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn type_type_right_unbound_is_false() {
        // subtypes.py:1279: a non-Callable, non-Overloaded, non-Instance
        // right (UnboundType) falls to the tail -> False. Proper context:
        // non-proper Unbound-right short-circuits True at _is_subtype.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = type_type(Type::NoneType);
        let right = Type::UnboundType {
            name: "A".to_string(),
            args: vec![],
            original_str_expr: None,
            original_str_fallback: None,
        };
        assert_eq!(is_subtype(&left, &right, &ctx_proper(), &r), Some(false));
    }

    // ---- Parameters left (issue #1225, visit_parameters
    // subtypes.py:1381-1418) ----

    fn parameters(arg_types: Vec<Type>) -> Type {
        Type::Parameters(crate::wire::Parameters {
            arg_types,
            arg_kinds: vec![],
            arg_names: vec![],
            variables: vec![],
            imprecise_arg_kinds: false,
            is_ellipsis_args: false,
        })
    }

    #[test]
    fn parameters_left_subtype_of_object() {
        // subtypes.py:1414-1416: right Instance builtins.object -> True.
        let r = make_resolver(vec![]);
        let left = parameters(vec![instance("builtins.int", vec![])]);
        assert_eq!(
            is_subtype(
                &left,
                &instance("builtins.object", vec![]),
                &ctx_nominal(),
                &r
            ),
            Some(true)
        );
    }

    #[test]
    fn parameters_left_not_subtype_of_other_instance() {
        // subtypes.py:1414-1416: a non-object Instance right -> False.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = parameters(vec![instance("builtins.int", vec![])]);
        assert_eq!(
            is_subtype(&left, &instance("a.A", vec![]), &ctx_nominal(), &r),
            Some(false)
        );
    }

    #[test]
    fn parameters_left_defers_on_parameters_right() {
        // subtypes.py:1392-1413: right Parameters routes through
        // are_parameters_compatible; the engine cannot decide this shape ->
        // defer to Python.
        let r = make_resolver(vec![]);
        let left = parameters(vec![instance("builtins.int", vec![])]);
        let right = parameters(vec![instance("builtins.object", vec![])]);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn parameters_left_not_subtype_of_callable() {
        // subtypes.py:1417-1418: any other right (CallableType) -> False.
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = parameters(vec![instance("builtins.int", vec![])]);
        let right = callable_type(vec![], Type::NoneType, None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    // ---- visit_callable_type (issue #443, subtypes.py:807-889) ----

    #[test]
    fn callable_subtype_of_non_protocol_instance_via_fallback() {
        // right is non-protocol Instance: is_subtype(left.fallback, right)
        // (subtypes.py:884). builtins.function <: builtins.object.
        let _r = make_resolver(vec![
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
    fn callable_subtype_of_callable_right() {
        // right is CallableType: routed into the native callable_compat
        // engine. Identical signatures -> True (was defer before the lane).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = callable_type(vec![], Type::NoneType, None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
    }

    #[test]
    fn callable_subtype_of_overloaded_right() {
        // right is Overloaded: at least one item must match
        // (subtypes.py:866-867). Identical item -> True (was defer before
        // the lane).
        let r = make_resolver(vec![snap("builtins.function", "function")]);
        let item = callable_type(vec![], Type::NoneType, None);
        let left = callable_type(vec![], Type::NoneType, None);
        let right = Type::Overloaded { items: vec![item] };
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(true));
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

    #[test]
    fn erase_return_self_types_callable_ret_self_becomes_any() {
        // Callable[[], C] -> Callable[[], Any] (subtypes.py:2764-2768).
        let self_t = instance("mod.C", vec![]);
        let c = callable_type(vec![], instance("mod.C", vec![]), None);
        let result = erase_return_self_types_wire(&c, &self_t).unwrap();
        match result {
            Type::CallableType { ret_type, .. } => assert_eq!(*ret_type, any_type()),
            other => panic!("expected erased CallableType, got {other:?}"),
        }
    }

    #[test]
    fn erase_return_self_types_callable_ret_non_self_unchanged() {
        // Callable[[], int] with self C stays Callable[[], int].
        let self_t = instance("mod.C", vec![]);
        let ret = instance("builtins.int", vec![]);
        let c = callable_type(vec![], ret.clone(), None);
        let result = erase_return_self_types_wire(&c, &self_t).unwrap();
        assert_eq!(result, c);
    }

    #[test]
    fn erase_return_self_types_generic_self_instance_matches() {
        // Callable[[], C[T]] with self C[T] erases (same type_ref + args).
        let self_t = instance("mod.C", vec![any_type()]);
        let c = callable_type(vec![], instance("mod.C", vec![any_type()]), None);
        let result = erase_return_self_types_wire(&c, &self_t).unwrap();
        match result {
            Type::CallableType { ret_type, .. } => assert_eq!(*ret_type, any_type()),
            other => panic!("expected erased CallableType, got {other:?}"),
        }
    }

    #[test]
    fn erase_return_self_types_bare_instance_unchanged() {
        // A bare Instance == self_type is NOT function-like; Python
        // returns it unchanged (subtypes.py:2773).
        let self_t = instance("mod.C", vec![]);
        let bare = instance("mod.C", vec![]);
        let result = erase_return_self_types_wire(&bare, &self_t).unwrap();
        assert_eq!(result, bare);
    }

    #[test]
    fn erase_return_self_types_overloaded_recurses() {
        // Overloaded([()->C, (int)->int]) with self C: first item erased
        // to Any, second unchanged.
        let self_t = instance("mod.C", vec![]);
        let c1 = callable_type(vec![], instance("mod.C", vec![]), None);
        let c2 = callable_type(vec![], instance("builtins.int", vec![]), None);
        let ov = Type::Overloaded {
            items: vec![c1, c2],
        };
        let result = erase_return_self_types_wire(&ov, &self_t).unwrap();
        match result {
            Type::Overloaded { items } => {
                assert_eq!(items.len(), 2);
                match &items[0] {
                    Type::CallableType { ret_type, .. } => assert_eq!(**ret_type, any_type()),
                    other => panic!("expected erased item, got {other:?}"),
                }
                assert_eq!(
                    items[1],
                    callable_type(vec![], instance("builtins.int", vec![]), None)
                );
            }
            other => panic!("expected Overloaded, got {other:?}"),
        }
    }

    #[test]
    fn erase_return_self_types_non_function_unchanged() {
        // Union [A, B], NoneType pass through unchanged.
        let self_t = instance("mod.C", vec![]);
        let union = Type::UnionType {
            items: vec![instance("a.A", vec![]), instance("a.B", vec![])],
            uses_pep604_syntax: false,
            can_be_true: false,
            can_be_false: false,
        };
        assert_eq!(
            erase_return_self_types_wire(&union, &self_t).unwrap(),
            union
        );
        assert_eq!(
            erase_return_self_types_wire(&Type::NoneType, &self_t).unwrap(),
            Type::NoneType
        );
    }

    fn encode(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, t).expect("encode type for batch test");
        buf.into_bytes()
    }

    #[test]
    fn batch_identical_pairs_match_single_pair() {
        // Same (left, right) offered multiple times in one batch: every
        // entry must produce the identical answer to the single-pair
        // entry.

        // Equal wire bytes make equal answers; dedup at the Python edge
        // does not change the Rust contract.
        let r = make_resolver(vec![
            snap("a.A", "A"),
            snap("a.B", "B"),
            snap("builtins.object", "object"),
        ]);
        let left = instance("a.A", vec![]);
        let right = instance("a.B", vec![]);
        let pairs: Vec<Vec<u8>> = vec![
            encode(&left),
            encode(&right),
            encode(&left),
            encode(&right),
            encode(&left),
            encode(&right),
            encode(&left),
            encode(&right),
        ];
        let flat: Vec<&[u8]> = pairs.iter().map(|b| b.as_slice()).collect();
        let single = is_subtype(&left, &right, &ctx_nominal(), &r);
        let expect = match single {
            Some(true) => 1_i8,
            Some(false) => 0,
            None => -1,
        };
        let got = rust_is_subtype_batch(
            flat,
            false, // ignore_type_params
            false, // ignore_declared_variance
            false, // always_covariant
            false, // ignore_promotions
            false, // proper_subtype
            true,  // strict_optional
            false, // ignore_pos_arg_names
            false, // strict_concatenate
            &mut NativeTypeResolver::from_resolver(r),
        );
        assert_eq!(got, vec![expect; 4]);
    }

    #[test]
    fn batch_deferring_pair_marks_only_its_slot() {
        // a.Sub[Gen] without bases blobs defers to None (Rust cannot map
        // to the supertype); the other pairs in the same batch still get
        // their answers instead of the whole batch failing.
        let mut gen = snap("a.Gen", "Gen");
        gen.type_vars = vec!["T".to_string()];
        gen.type_vars_with_variance = vec![("T".to_string(), COVARIANT, 0)];
        let mut derived = snap("a.Sub", "Sub");
        derived.has_base.insert("a.Gen".to_string());
        derived.mro.push("a.Gen".to_string());
        let r = make_resolver(vec![gen, derived, snap("builtins.object", "object")]);
        // Deferring pair: a.Sub[Any] <: a.Gen[Any] (no bases blobs).
        let defer_left = instance("a.Sub", vec![any_type()]);
        let defer_right = instance("a.Gen", vec![any_type()]);
        // Decided pair: anything <: object is true.
        let ok_left = instance("a.Sub", vec![any_type()]);
        let ok_right = instance("builtins.object", vec![]);
        let ok_left_b = encode(&ok_left);
        let ok_right_b = encode(&ok_right);
        let defer_left_b = encode(&defer_left);
        let defer_right_b = encode(&defer_right);
        let flat: Vec<&[u8]> = [
            ok_left_b.as_slice(),
            ok_right_b.as_slice(),
            defer_left_b.as_slice(),
            defer_right_b.as_slice(),
        ]
        .to_vec();
        let got = rust_is_subtype_batch(
            flat,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            &mut NativeTypeResolver::from_resolver(r),
        );
        assert_eq!(got, vec![1, -1]);
    }

    #[test]
    fn test_engine_defers_on_kept_alias() {
        // Issue #1205 keep-node contract: an alias missing from the alias
        // resolver is kept as a node and the engine defers on comparisons
        // that must reach it.
        let mut native = NativeTypeResolver::new(
            make_resolver(vec![snap("builtins.int", "int")]),
            make_alias_resolver(vec![]),
        );
        let left = encode(&alias_type(vec![], "mod.Missing"));
        let right = encode(&instance("builtins.int", vec![]));
        let got = rust_is_subtype(
            &left,
            &right,
            false, // ignore_type_params
            false, // ignore_declared_variance
            false, // always_covariant
            false, // ignore_promotions
            false, // proper_subtype
            true,  // strict_optional
            false, // ignore_pos_arg_names
            false, // strict_concatenate
            &mut native,
        );
        assert_eq!(got, None);
    }

    fn instance_with_attrs(type_ref: &str, attrs: Vec<(&str, Type)>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args: vec![],
            last_known_value: None,
            extra_attrs: Some(wire::ExtraAttrs {
                attrs: attrs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                immutable: HashSet::new(),
                mod_name: None,
            }),
        }
    }

    #[test]
    fn member_call_fast_false_without_call_in_mro() {
        // Issue #1205: Instance <: Callable with no resolvable `__call__`
        // anywhere in the MRO is decided False (find_member miss).
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = instance("a.A", vec![]);
        let right = callable_type(vec![], instance("builtins.bool", vec![]), None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), Some(false));
    }

    #[test]
    fn member_call_defers_when_call_present() {
        let mut a = snap("a.A", "A");
        a.member_info.insert("__call__".to_string(), (false, false));
        let r = make_resolver(vec![a]);
        let left = instance("a.A", vec![]);
        let right = callable_type(vec![], instance("builtins.bool", vec![]), None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn member_call_defers_on_fallback_to_any() {
        // Under a proper-subtype comparison the fallback_to_any
        // short-circuit does not fire, so the probe defer applies.
        let a = SubtypeContext::new(false, false, false, false, true, true);
        let mut s = snap("a.A", "A");
        s.fallback_to_any = true;
        let r = make_resolver(vec![s]);
        let left = instance("a.A", vec![]);
        let right = callable_type(vec![], instance("builtins.bool", vec![]), None);
        assert_eq!(is_subtype(&left, &right, &a, &r), None);
    }

    #[test]
    fn member_call_defers_on_missing_base_snapshot() {
        // A base missing from the resolver cannot prove the miss, so the
        // fast-False probe must not fire.
        let mut a = snap("a.A", "A");
        a.mro.push("a.MissingBase".to_string());
        let r = make_resolver(vec![a]);
        let left = instance("a.A", vec![]);
        let right = callable_type(vec![], instance("builtins.bool", vec![]), None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    #[test]
    fn member_call_defers_with_extra_attrs_call() {
        // An extra_attrs carrier with a `__call__` key defers: the member
        // may resolve through module attributes.
        let r = make_resolver(vec![snap("a.A", "A")]);
        let left = instance_with_attrs(
            "a.A",
            vec![("__call__", instance("builtins.object", vec![]))],
        );
        let right = callable_type(vec![], instance("builtins.bool", vec![]), None);
        assert_eq!(is_subtype(&left, &right, &ctx_nominal(), &r), None);
    }

    // --- expand_aliases unit tests ---

    use crate::aliases::{AliasTvar, TypeAliasResolver, TypeAliasSnapshot};

    fn encode_for_alias(t: &Type) -> Vec<u8> {
        let mut buf = WriteBuffer::new();
        wire::write_type(&mut buf, t).expect("encode type");
        buf.into_bytes()
    }

    fn make_alias_resolver(snaps: Vec<TypeAliasSnapshot>) -> TypeAliasResolver {
        let mut r = TypeAliasResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn alias_type(args: Vec<Type>, type_ref: &str) -> Type {
        Type::TypeAliasType {
            args,
            type_ref: type_ref.to_string(),
        }
    }

    fn alias_tvar_fn(name: &str, raw_id: i64) -> AliasTvar {
        AliasTvar {
            name: name.to_string(),
            raw_id,
            ..Default::default()
        }
    }

    #[test]
    fn test_expand_aliases_simple_no_args() {
        // A = str; TypeAliasType(A, []) -> Instance(str, [])
        let target = instance("builtins.str", vec![]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            no_args: true,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = alias_type(vec![], "mod.A");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(result, Some(instance("builtins.str", vec![])));
    }

    #[test]
    fn test_expand_aliases_no_args_with_explicit_args() {
        // A = List (no_args=True); A[int] -> List[int]
        let target = instance("builtins.list", vec![]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            no_args: true,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = alias_type(vec![instance("builtins.int", vec![])], "mod.A");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(instance(
                "builtins.list",
                vec![instance("builtins.int", vec![])]
            ))
        );
    }

    #[test]
    fn test_expand_aliases_generic_substitution() {
        // A = List[T]; A[int] -> List[int]
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: 1,
            meta_level: 0,
        };
        let target = instance("builtins.list", vec![tvar]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            alias_tvars: vec![alias_tvar_fn("T", 1)],
            no_args: false,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = alias_type(vec![instance("builtins.int", vec![])], "mod.A");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(instance(
                "builtins.list",
                vec![instance("builtins.int", vec![])]
            ))
        );
    }

    #[test]
    fn test_expand_aliases_missing_keeps_node() {
        // Issue #1205: an alias missing from the resolver is kept in place
        // (best-effort expansion) instead of failing the whole entry.
        let ar = make_alias_resolver(vec![]);
        let input = alias_type(vec![], "mod.Missing");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(result, Some(input.clone()));
    }

    #[test]
    fn test_expand_aliases_variadic_keeps_node() {
        // tvar_tuple_index set: the variadic target needs the Python-side
        // Unpack splicing machinery; the node is kept instead of failing.
        let target = instance("builtins.tuple", vec![]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            tvar_tuple_index: Some(0),
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = alias_type(vec![], "mod.A");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(result, Some(input.clone()));
    }

    #[test]
    fn test_expand_aliases_partial_keep_union_sibling() {
        // A missing-alias union item is kept while a sibling alias in the
        // same union still expands (partial keep, issue #1205).
        let target = instance("builtins.str", vec![]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            no_args: true,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = Type::UnionType {
            items: vec![
                alias_type(vec![], "mod.Missing"),
                alias_type(vec![], "mod.A"),
                instance("builtins.int", vec![]),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: false,
        };
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(Type::UnionType {
                items: vec![
                    alias_type(vec![], "mod.Missing"),
                    instance("builtins.str", vec![]),
                    instance("builtins.int", vec![]),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: false,
            })
        );
    }

    #[test]
    fn test_expand_aliases_in_union_item() {
        // Union[A, B] where A = str -> Union[str, B]
        let target = instance("builtins.str", vec![]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            no_args: true,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = Type::UnionType {
            items: vec![
                alias_type(vec![], "mod.A"),
                instance("builtins.int", vec![]),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: false,
        };
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(Type::UnionType {
                items: vec![
                    instance("builtins.str", vec![]),
                    instance("builtins.int", vec![]),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: false,
            })
        );
    }

    #[test]
    fn test_expand_aliases_in_instance_args() {
        // List[A] where A = str -> List[str]
        let target = instance("builtins.str", vec![]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            no_args: true,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = instance("builtins.list", vec![alias_type(vec![], "mod.A")]);
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(instance(
                "builtins.list",
                vec![instance("builtins.str", vec![])]
            ))
        );
    }

    #[test]
    fn test_expand_aliases_recursive_chain() {
        // A = B; B = str; A -> str
        let str_target = instance("builtins.str", vec![]);
        let b_snap = TypeAliasSnapshot {
            fullname: "mod.B".to_string(),
            target: encode_for_alias(&str_target),
            no_args: true,
            ..Default::default()
        };
        let a_snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&alias_type(vec![], "mod.B")),
            no_args: true,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![a_snap, b_snap]);
        let input = alias_type(vec![], "mod.A");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(result, Some(instance("builtins.str", vec![])));
    }

    #[test]
    fn test_expand_aliases_leaf_passthrough() {
        // Non-alias types pass through unchanged
        let ar = make_alias_resolver(vec![]);
        assert_eq!(
            expand_aliases(&Type::NoneType, &ar, true),
            Some(Type::NoneType)
        );
        assert_eq!(expand_aliases(&any_type(), &ar, true), Some(any_type()));
        assert_eq!(
            expand_aliases(&instance("builtins.str", vec![]), &ar, true),
            Some(instance("builtins.str", vec![]))
        );
    }

    #[test]
    fn test_expand_aliases_depth_cap() {
        // Chain of 60 aliases exceeds the depth cap (50) -> None
        let mut snaps = Vec::new();
        for i in 0..60 {
            let next = if i == 59 {
                instance("builtins.str", vec![])
            } else {
                alias_type(vec![], &format!("mod.A{}", i + 1))
            };
            snaps.push(TypeAliasSnapshot {
                fullname: format!("mod.A{i}"),
                target: encode_for_alias(&next),
                no_args: true,
                ..Default::default()
            });
        }
        let ar = make_alias_resolver(snaps);
        let input = alias_type(vec![], "mod.A0");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(result, None);
    }

    fn recursive_union_alias_snap(fullname: &str) -> TypeAliasSnapshot {
        // type A = Union[str, A]
        let target = Type::UnionType {
            items: vec![
                instance("builtins.str", vec![]),
                alias_type(vec![], fullname),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        TypeAliasSnapshot {
            fullname: fullname.to_string(),
            target: encode_for_alias(&target),
            no_args: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_expand_aliases_self_recursive_union_cuts() {
        // type A = Union[str, A]: get_proper_type keeps the inner alias node
        // in place, so the fixpoint must terminate via the active-path cut
        // (issue #1149) instead of recursing to the depth cap (a defer).
        let ar = make_alias_resolver(vec![recursive_union_alias_snap("mod.A")]);
        let input = alias_type(vec![], "mod.A");
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(Type::UnionType {
                items: vec![
                    instance("builtins.str", vec![]),
                    alias_type(vec![], "mod.A"),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            })
        );
    }

    #[test]
    fn test_expand_aliases_recursive_reentry_consistent() {
        // A repeat call re-enters with a fresh active stack (no cross-call
        // pollution) and reproduces the same cut shape.
        let ar = make_alias_resolver(vec![recursive_union_alias_snap("mod.A")]);
        let input = alias_type(vec![], "mod.A");
        let first = expand_aliases(&input, &ar, true);
        let second = expand_aliases(&input, &ar, true);
        assert_eq!(first, second);
        // Re-expanding a cut shape terminates too: the union item unrolls
        // one more level per call, but never defers (never returns None up
        // to the depth cap); the engine defers onto cut nodes instead.
        let deeper = expand_aliases(first.as_ref().unwrap(), &ar, true);
        assert!(deeper.is_some());
    }

    #[test]
    fn test_expand_aliases_sibling_same_alias_different_args_expands_both() {
        // Union[A[int], A[str]] with A = List[T]: each occurrence carries
        // different args, so the args-identity key must not cut the second
        // one; both substitute.
        let tvar = Type::TypeVarType {
            name: "T".to_string(),
            fullname: "mod.T".to_string(),
            raw_id: 1,
            namespace: "".to_string(),
            values: vec![],
            upper_bound: Box::new(instance("builtins.object", vec![])),
            default: Box::new(any_type()),
            variance: 1,
            meta_level: 0,
        };
        let target = instance("builtins.list", vec![tvar]);
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&target),
            alias_tvars: vec![alias_tvar_fn("T", 1)],
            no_args: false,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = Type::UnionType {
            items: vec![
                alias_type(vec![instance("builtins.int", vec![])], "mod.A"),
                alias_type(vec![instance("builtins.str", vec![])], "mod.A"),
            ],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(Type::UnionType {
                items: vec![
                    instance("builtins.list", vec![instance("builtins.int", vec![])]),
                    instance("builtins.list", vec![instance("builtins.str", vec![])]),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            })
        );
    }

    #[test]
    fn test_expand_aliases_sibling_same_alias_same_args_still_expands_after_pop() {
        // Union[A, A] with A = str: a leaky active stack would cut the second
        // occurrence; finishing one expansion must pop its entry, so each
        // sibling expands fully.
        let snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&instance("builtins.str", vec![])),
            no_args: true,
            ..Default::default()
        };
        let ar = make_alias_resolver(vec![snap]);
        let input = Type::UnionType {
            items: vec![alias_type(vec![], "mod.A"), alias_type(vec![], "mod.A")],
            uses_pep604_syntax: false,
            can_be_true: true,
            can_be_false: true,
        };
        let result = expand_aliases(&input, &ar, true);
        assert_eq!(
            result,
            Some(Type::UnionType {
                items: vec![
                    instance("builtins.str", vec![]),
                    instance("builtins.str", vec![]),
                ],
                uses_pep604_syntax: false,
                can_be_true: true,
                can_be_false: true,
            })
        );
    }

    // -- equivalence / same-type / more-precise seam alias expansion --

    /// Build a `NativeTypeResolver` carrying a `builtins.str` profile plus
    /// the given alias snapshot table, matching how the pyfunction seams
    /// receive their resolver.
    fn make_native_with_alias(alias_snaps: Vec<TypeAliasSnapshot>) -> NativeTypeResolver {
        let type_snaps = vec![
            snap("builtins.str", "str"),
            snap("builtins.object", "object"),
        ];
        NativeTypeResolver::new(make_resolver(type_snaps), make_alias_resolver(alias_snaps))
    }

    fn seam_alias_snap() -> TypeAliasSnapshot {
        // mod.A = builtins.str
        TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&instance("builtins.str", vec![])),
            no_args: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_equivalent_alias_expands() {
        // is_equivalent(A, str) with A = str answers natively: both
        // operands expand through the alias resolver before is_subtype.
        let mut native = make_native_with_alias(vec![seam_alias_snap()]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(
            rust_is_equivalent(&a, &s, false, true, &mut native),
            Some(true)
        );
        assert_eq!(
            rust_is_equivalent(&s, &a, false, true, &mut native),
            Some(true)
        );
    }

    #[test]
    fn test_equivalent_alias_missing_snapshot_defers() {
        // No alias snapshot: the expansion defers (None), preserving the
        // fall-through to the pure-Python body.
        let mut native = make_native_with_alias(vec![]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(rust_is_equivalent(&a, &s, false, true, &mut native), None);
    }

    #[test]
    fn test_same_type_alias_expands() {
        let mut native = make_native_with_alias(vec![seam_alias_snap()]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(
            rust_is_same_type(&a, &s, false, true, &mut native),
            Some(true)
        );
    }

    #[test]
    fn test_same_type_alias_missing_snapshot_defers() {
        let mut native = make_native_with_alias(vec![]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(rust_is_same_type(&a, &s, false, true, &mut native), None);
    }

    #[test]
    fn test_more_precise_alias_expands() {
        // is_more_precise(A, str) resolves A -> str, then is_proper_subtype
        // (str, str) answers Some(true) natively.
        let mut native = make_native_with_alias(vec![seam_alias_snap()]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(
            rust_is_more_precise(&a, &s, false, true, &mut native),
            Some(true)
        );
    }

    #[test]
    fn test_more_precise_alias_to_any() {
        // A = Any; is_more_precise(Any, A): right expands to Any -> True
        // even for a non-Instance left (previously defer via alias guard).
        let any_snap = TypeAliasSnapshot {
            fullname: "mod.A".to_string(),
            target: encode_for_alias(&any_type()),
            no_args: true,
            ..Default::default()
        };
        let mut native = make_native_with_alias(vec![any_snap]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let l = encode_for_alias(&any_type());
        assert_eq!(
            rust_is_more_precise(&l, &a, false, true, &mut native),
            Some(true)
        );
    }

    #[test]
    fn test_more_precise_alias_missing_snapshot_defers() {
        let mut native = make_native_with_alias(vec![]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(rust_is_more_precise(&a, &s, false, true, &mut native), None);
    }

    #[test]
    fn test_all_same_types_alias_expands() {
        // all_same_types([A, str]) is True after the alias expands, instead
        // of deferring on the alias operand.
        let mut native = make_native_with_alias(vec![seam_alias_snap()]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(
            rust_all_same_types(vec![&a, &s], false, true, &mut native),
            Some(true)
        );
    }

    #[test]
    fn test_all_same_types_alias_missing_snapshot_defers() {
        let mut native = make_native_with_alias(vec![]);
        let a = encode_for_alias(&alias_type(vec![], "mod.A"));
        let s = encode_for_alias(&instance("builtins.str", vec![]));
        assert_eq!(
            rust_all_same_types(vec![&a, &s], false, true, &mut native),
            None
        );
    }

    // `are_args_compatible` pure-decision tests. Tag values must match
    // KIND_ARE_ARGS_* in this file and NATIVE_ARE_ARGS_* in subtypes.py.
    #[allow(clippy::too_many_arguments)]
    fn are_args(
        left_name: Option<&str>,
        right_name: Option<&str>,
        left_pos: Option<i64>,
        right_pos: Option<i64>,
        left_required: bool,
        right_required: bool,
        ignore_pos_arg_names: bool,
        allow_partial_overlap: bool,
        allow_imprecise_kinds: bool,
    ) -> i64 {
        classify_are_args_compatible(
            left_name.is_none(),
            right_name.is_none(),
            left_name == right_name,
            right_pos.is_none(),
            left_pos == right_pos,
            left_required,
            right_required,
            ignore_pos_arg_names,
            allow_partial_overlap,
            allow_imprecise_kinds,
        )
    }

    #[test]
    fn test_are_args_name_mismatch_returns_false() {
        // Names differ and names matter (not ignoring pos-arg names).
        assert_eq!(
            are_args(
                Some("x"),
                Some("y"),
                None,
                None,
                false,
                false,
                false,
                false,
                false
            ),
            KIND_ARE_ARGS_FALSE
        );
    }

    #[test]
    fn test_are_args_name_mismatch_ignored_with_pos_falls_through() {
        // Names differ but ignore_pos_arg_names and right has a position,
        // so the name gate does not return False; falls through to compat.
        assert_eq!(
            are_args(
                Some("x"),
                Some("y"),
                Some(0),
                Some(0),
                false,
                false,
                true,
                false,
                false
            ),
            KIND_ARE_ARGS_CALL_IS_COMPAT
        );
    }

    #[test]
    fn test_are_args_name_mismatch_ignored_no_right_pos_returns_false() {
        // ignore_pos_arg_names but right.pos is None -> still False.
        assert_eq!(
            are_args(
                Some("x"),
                Some("y"),
                None,
                None,
                false,
                false,
                true,
                false,
                false
            ),
            KIND_ARE_ARGS_FALSE
        );
    }

    #[test]
    fn test_are_args_right_name_none_passes_name_gate() {
        // Right does not care about name: is_different returns False.
        assert_eq!(
            are_args(
                Some("x"),
                None,
                Some(0),
                Some(0),
                false,
                false,
                false,
                false,
                false
            ),
            KIND_ARE_ARGS_CALL_IS_COMPAT
        );
    }

    #[test]
    fn test_are_args_pos_mismatch_returns_false() {
        assert_eq!(
            are_args(
                None,
                None,
                Some(0),
                Some(1),
                false,
                false,
                false,
                false,
                false
            ),
            KIND_ARE_ARGS_FALSE
        );
    }

    #[test]
    fn test_are_args_pos_mismatch_imprecise_falls_through() {
        assert_eq!(
            are_args(
                None,
                None,
                Some(0),
                Some(1),
                false,
                false,
                false,
                false,
                true
            ),
            KIND_ARE_ARGS_CALL_IS_COMPAT
        );
    }

    #[test]
    fn test_are_args_right_pos_none_passes_pos_gate() {
        assert_eq!(
            are_args(None, None, Some(0), None, false, false, false, false, false),
            KIND_ARE_ARGS_CALL_IS_COMPAT
        );
    }

    #[test]
    fn test_are_args_required_left_optional_right_returns_false() {
        assert_eq!(
            are_args(
                None,
                None,
                Some(0),
                Some(0),
                true,
                false,
                false,
                false,
                false
            ),
            KIND_ARE_ARGS_FALSE
        );
    }

    #[test]
    fn test_are_args_overlap_both_optional_returns_true() {
        assert_eq!(
            are_args(
                None,
                None,
                Some(0),
                Some(0),
                false,
                false,
                false,
                true,
                false
            ),
            KIND_ARE_ARGS_TRUE
        );
    }

    #[test]
    fn test_are_args_overlap_one_required_falls_through() {
        // partial overlap but left required -> no shortcut.
        assert_eq!(
            are_args(
                None,
                None,
                Some(0),
                Some(0),
                true,
                false,
                false,
                true,
                false
            ),
            KIND_ARE_ARGS_CALL_IS_COMPAT
        );
    }

    #[test]
    fn test_are_args_both_required_disables_overlap() {
        // Both required: partial overlap has no effect, so the shortcut
        // (which needs both optional) does not fire.
        assert_eq!(
            are_args(None, None, Some(0), Some(0), true, true, false, true, false),
            KIND_ARE_ARGS_CALL_IS_COMPAT
        );
    }

    #[test]
    fn test_are_args_name_left_none_under_overlap_passes_name_gate() {
        // Under partial overlap, a left name of None short-circuits the
        // name is_different to False, so the name gate is skipped.
        assert_eq!(
            are_args(
                None,
                Some("y"),
                Some(0),
                Some(0),
                false,
                false,
                false,
                true,
                false
            ),
            KIND_ARE_ARGS_TRUE
        );
    }

    #[test]
    fn test_typeparam_covariant_subtype() {
        assert_eq!(
            classify_type_parameter_dispatch(COVARIANT, false),
            KIND_TYPEPARAM_SUBTYPE
        );
        assert_eq!(
            classify_type_parameter_dispatch(COVARIANT, true),
            KIND_TYPEPARAM_PROPER_SUBTYPE
        );
    }

    #[test]
    fn test_typeparam_variance_not_ready_defaults_covariant() {
        // Lenient default: not-ready variance routes like covariance.
        assert_eq!(
            classify_type_parameter_dispatch(VARIANCE_NOT_READY, false),
            KIND_TYPEPARAM_SUBTYPE
        );
        assert_eq!(
            classify_type_parameter_dispatch(VARIANCE_NOT_READY, true),
            KIND_TYPEPARAM_PROPER_SUBTYPE
        );
    }

    #[test]
    fn test_typeparam_contravariant_swaps_args() {
        assert_eq!(
            classify_type_parameter_dispatch(CONTRAVARIANT, false),
            KIND_TYPEPARAM_SUBTYPE_SWAP
        );
        assert_eq!(
            classify_type_parameter_dispatch(CONTRAVARIANT, true),
            KIND_TYPEPARAM_PROPER_SWAP
        );
    }

    #[test]
    fn test_typeparam_other_variance_equality_leaves() {
        // The else arm (e.g. bivariant) uses equality leaves.
        assert_eq!(
            classify_type_parameter_dispatch(INVARIANT, false),
            KIND_TYPEPARAM_EQUIVALENT
        );
        assert_eq!(
            classify_type_parameter_dispatch(INVARIANT, true),
            KIND_TYPEPARAM_SAME
        );
        assert_eq!(
            classify_type_parameter_dispatch(7, false),
            KIND_TYPEPARAM_EQUIVALENT
        );
    }

    #[test]
    fn assuming_guard_push_pop_and_flag_keying() {
        // The assuming guard (subtypes.py:1972-1976): a pair being checked
        // under one proper flag is invisible to the other stack, and
        // pop-on-drop clears it on every exit path.
        let left = instance("a.A", vec![]);
        let right = instance("a.P", vec![]);
        let other = instance("a.B", vec![]);
        assert!(!assuming_contains(&left, &right, false));
        let guard = AssumingPush::new(left.clone(), right.clone(), false);
        assert!(assuming_contains(&left, &right, false));
        // Proper flag is part of the key.
        assert!(!assuming_contains(&left, &right, true));
        assert!(!assuming_contains(&other, &right, false));
        assert!(!assuming_contains(&right, &left, false));
        // Structural equality on wire Types: a separately built, equal
        // pair matches (mirrors Python's identity check on live objects
        // the recursion re-encounters).
        assert!(assuming_contains(&instance("a.A", vec![]), &right, false));
        drop(guard);
        assert!(!assuming_contains(&left, &right, false));
    }

    #[test]
    fn protocol_right_defers_without_live_map() {
        // The protocol-right port needs the live TypeInfo map for the
        // dependency record and member-flag loop; without it (pure-Rust
        // cargo tests) it defers rather than guessing.
        let mut proto = snap("a.P", "P");
        proto.is_protocol = true;
        proto.protocol_members.push("read".to_string());
        let r = make_resolver(vec![
            snap("a.A", "A"),
            proto,
            snap("builtins.object", "object"),
        ]);
        let left = instance("a.A", vec![]);
        let right = instance("a.P", vec![]);
        assert_eq!(
            protocol_right_decision(&left, &right, "a.A", "a.P", &ctx_nominal(), &r),
            None
        );
    }
}
