"""Semantic analysis of types"""

from __future__ import annotations

import itertools
from collections.abc import Callable, Iterable, Iterator, Sequence
from contextlib import contextmanager
from typing import Any, Final, Protocol, TypeVar

from mypy import errorcodes as codes, message_registry, nodes
from mypy.errorcodes import ErrorCode
from mypy.errors import ErrorInfo
from mypy.expandtype import expand_type
from mypy.message_registry import (
    INVALID_PARAM_SPEC_LOCATION,
    INVALID_PARAM_SPEC_LOCATION_NOTE,
    TYPEDDICT_OVERRIDE_MERGE,
)
from mypy.messages import (
    MessageBuilder,
    format_type,
    format_type_bare,
    quote_type_string,
    wrong_type_arg_count,
)
from mypy.nodes import (
    ARG_NAMED,
    ARG_NAMED_OPT,
    ARG_OPT,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    MISSING_FALLBACK,
    SYMBOL_FUNCBASE_TYPES,
    VAR_NO_INFO,
    ArgKind,
    Context,
    Decorator,
    ImportFrom,
    MypyFile,
    Node,
    ParamSpecExpr,
    PlaceholderNode,
    SymbolTableNode,
    TypeAlias,
    TypeInfo,
    TypeVarExpr,
    TypeVarLikeExpr,
    TypeVarTupleExpr,
    Var,
    check_arg_kinds,
    check_param_names,
)
from mypy.options import INLINE_TYPEDDICT, Options
from mypy.plugin import AnalyzeTypeContext, Plugin, TypeAnalyzerPluginInterface
from mypy.semanal_shared import (
    SemanticAnalyzerCoreInterface,
    SemanticAnalyzerInterface,
    paramspec_args,
    paramspec_kwargs,
)
from mypy.state import state
from mypy.tvar_scope import TypeVarLikeScope
from mypy.types import (
    ANNOTATED_TYPE_NAMES,
    ANY_STRATEGY,
    CONCATENATE_TYPE_NAMES,
    FINAL_TYPE_NAMES,
    LITERAL_TYPE_NAMES,
    MYPYC_NATIVE_INT_NAMES,
    NEVER_NAMES,
    TUPLE_NAMES,
    TYPE_ALIAS_NAMES,
    TYPE_NAMES,
    UNPACK_TYPE_NAMES,
    AnyType,
    BoolTypeQuery,
    CallableArgument,
    CallableType,
    CollectAliasesVisitor,
    DeletedType,
    EllipsisType,
    ErasedType,
    Instance,
    LiteralType,
    NoneType,
    Overloaded,
    Parameters,
    ParamSpecFlavor,
    ParamSpecType,
    PartialType,
    PlaceholderType,
    ProperType,
    RawExpressionType,
    ReadOnlyType,
    RequiredType,
    SyntheticTypeVisitor,
    TrivialSyntheticTypeTranslator,
    TupleType,
    Type,
    TypeAliasType,
    TypedDictType,
    TypeList,
    TypeOfAny,
    TypeQuery,
    TypeType,
    TypeVarId,
    TypeVarLikeType,
    TypeVarTupleType,
    TypeVarType,
    UnboundType,
    UninhabitedType,
    UnionType,
    UnpackType,
    _encode_no_arg_instance,
    callable_with_ellipsis,
    find_unpack_in_list,
    flatten_nested_tuples,
    get_proper_type,
    has_type_vars,
)
from mypy.types_utils import get_bad_type_type_item
from mypy.typevars import fill_typevars

T = TypeVar("T")

type_constructors: Final = {
    "typing.Callable",
    "typing.Optional",
    "typing.Tuple",
    "typing.Type",
    "typing.Union",
    *LITERAL_TYPE_NAMES,
    *ANNOTATED_TYPE_NAMES,
}

ARG_KINDS_BY_CONSTRUCTOR: Final = {
    "mypy_extensions.Arg": ARG_POS,
    "mypy_extensions.DefaultArg": ARG_OPT,
    "mypy_extensions.NamedArg": ARG_NAMED,
    "mypy_extensions.DefaultNamedArg": ARG_NAMED_OPT,
    "mypy_extensions.VarArg": ARG_STAR,
    "mypy_extensions.KwArg": ARG_STAR2,
}

SELF_TYPE_NAMES: Final = {"typing.Self", "typing_extensions.Self"}


def analyze_type_alias(
    type: Type,
    api: SemanticAnalyzerCoreInterface,
    tvar_scope: TypeVarLikeScope,
    plugin: Plugin,
    options: Options,
    cur_mod_node: MypyFile,
    is_typeshed_stub: bool,
    allow_placeholder: bool = False,
    in_dynamic_func: bool = False,
    global_scope: bool = True,
    allowed_alias_tvars: list[TypeVarLikeType] | None = None,
    erase_tvar_defs: list[TypeVarType] | None = None,
    alias_type_params_names: list[str] | None = None,
    python_3_12_type_alias: bool = False,
) -> tuple[Type, set[str]]:
    """Analyze r.h.s. of a (potential) type alias definition.

    If `node` is valid as a type alias rvalue, return the resulting type and a set of
    full names of type aliases it depends on (directly or indirectly).
    'node' must have been semantically analyzed.
    """
    analyzer = TypeAnalyser(
        api,
        tvar_scope,
        plugin,
        options,
        cur_mod_node,
        is_typeshed_stub,
        defining_alias=True,
        allow_placeholder=allow_placeholder,
        prohibit_self_type="type alias target",
        allowed_alias_tvars=allowed_alias_tvars,
        erase_tvar_defs=erase_tvar_defs,
        alias_type_params_names=alias_type_params_names,
        python_3_12_type_alias=python_3_12_type_alias,
    )
    analyzer.in_dynamic_func = in_dynamic_func
    analyzer.global_scope = global_scope
    res = analyzer.anal_type(type, nested=False)
    return res, analyzer.aliases_used


class TypeAnalyser(SyntheticTypeVisitor[Type], TypeAnalyzerPluginInterface):
    """Semantic analyzer for types.

    Converts unbound types into bound types. This is a no-op for already
    bound types.

    If an incomplete reference is encountered, this does a defer. The
    caller never needs to defer.
    """

    # Is this called from an untyped function definition?
    in_dynamic_func: bool = False
    # Is this called from global scope?
    global_scope: bool = True

    def __init__(
        self,
        api: SemanticAnalyzerCoreInterface,
        tvar_scope: TypeVarLikeScope,
        plugin: Plugin,
        options: Options,
        cur_mod_node: MypyFile,
        is_typeshed_stub: bool,
        *,
        defining_alias: bool = False,
        python_3_12_type_alias: bool = False,
        allow_tuple_literal: bool = False,
        allow_unbound_tvars: bool = False,
        allow_placeholder: bool = False,
        allow_typed_dict_special_forms: bool = False,
        allow_final: bool = True,
        allow_param_spec_literals: bool = False,
        allow_unpack: bool = False,
        report_invalid_types: bool = True,
        prohibit_self_type: str | None = None,
        prohibit_special_class_field_types: str | None = None,
        allowed_alias_tvars: list[TypeVarLikeType] | None = None,
        erase_tvar_defs: list[TypeVarType] | None = None,
        allow_type_any: bool = False,
        alias_type_params_names: list[str] | None = None,
        analyzing_tvar_def: bool = False,
    ) -> None:
        self.api = api
        self.fail_func = api.fail
        self.note_func = api.note
        self.tvar_scope = tvar_scope
        # Are we analysing a type alias definition rvalue?
        self.defining_alias = defining_alias
        self.python_3_12_type_alias = python_3_12_type_alias
        self.allow_tuple_literal = allow_tuple_literal
        # Positive if we are analyzing arguments of another (outer) type
        self.nesting_level = 0
        # Should we accept unbound type variables? This is currently used for class bases,
        # and alias right hand sides (before they are analyzed as type aliases).
        self.allow_unbound_tvars = allow_unbound_tvars
        if allowed_alias_tvars is None:
            allowed_alias_tvars = []
        self.allowed_alias_tvars = allowed_alias_tvars
        # Should we erase some type variables? This can be used to mass-erase type
        # variables that were found to be invalid at the class/alias definition.
        if erase_tvar_defs is None:
            erase_tvar_defs = []
        self.erase_tvar_defs = erase_tvar_defs
        self.alias_type_params_names = alias_type_params_names
        # If false, record incomplete ref if we generate PlaceholderType.
        self.allow_placeholder = allow_placeholder
        # Are we in a context where Required[] is allowed?
        self.allow_typed_dict_special_forms = allow_typed_dict_special_forms
        # Set True when we analyze ClassVar else False
        self.allow_final = allow_final
        # Are we in a context where ParamSpec literals are allowed?
        self.allow_param_spec_literals = allow_param_spec_literals
        # Are we in context where literal "..." specifically is allowed?
        self.allow_ellipsis = False
        # Should we report an error whenever we encounter a RawExpressionType outside
        # of a Literal context: e.g. whenever we encounter an invalid type? Normally,
        # we want to report an error, but the caller may want to do more specialized

        # error handling.
        self.report_invalid_types = report_invalid_types
        self.plugin = plugin
        self.options = options
        self.cur_mod_node = cur_mod_node
        self.is_typeshed_stub = is_typeshed_stub
        # Names of type aliases encountered while analysing a type will be collected here.
        self.aliases_used: set[str] = set()
        self.prohibit_self_type = prohibit_self_type
        # Set when we analyze TypedDicts or NamedTuples, since they are special:
        self.prohibit_special_class_field_types = prohibit_special_class_field_types
        # Allow variables typed as Type[Any] and type (useful for base classes).
        self.allow_type_any = allow_type_any
        # Level of nesting at which a TypeVarTuple is allowed. Note we specify exact level
        # to prohibit things like Unpack[list[Ts]], which are not supported.
        self.allow_type_var_tuple = -1
        self.allow_unpack = allow_unpack
        # Set when we are analyzing a default of a type variable.
        self.analyzing_tvar_def = analyzing_tvar_def

    def lookup_qualified(
        self, name: str, ctx: Context, suppress_errors: bool = False
    ) -> SymbolTableNode | None:
        return self.api.lookup_qualified(name, ctx, suppress_errors)

    def lookup_fully_qualified(self, fullname: str) -> SymbolTableNode:
        return self.api.lookup_fully_qualified(fullname)

    def visit_unbound_type(self, t: UnboundType, defining_literal: bool = False) -> Type:
        typ = self.visit_unbound_type_nonoptional(t, defining_literal)
        if t.optional:
            # We don't need to worry about double-wrapping Optionals or
            # wrapping Anys: Union simplification will take care of that.
            return make_optional_type(typ)
        return typ

    def not_declared_in_type_params(self, tvar_name: str) -> bool:
        return (
            self.alias_type_params_names is not None
            and tvar_name not in self.alias_type_params_names
        )

    def visit_unbound_type_nonoptional(self, t: UnboundType, defining_literal: bool) -> Type:
        if (
            _TYPEANAL_HAS_KERNEL
            and _native_typeanal_active
            and self._native_unbound_front_eligible(t)
        ):
            result = self._native_visit_unbound_front(t, defining_literal)
            if result is not None:
                return result
        sym = self.lookup_qualified(t.name, t)
        param_spec_name = None
        if t.name.endswith((".args", ".kwargs")):
            param_spec_name = t.name.rsplit(".", 1)[0]
            maybe_param_spec = self.lookup_qualified(param_spec_name, t)
            if maybe_param_spec and isinstance(maybe_param_spec.node, ParamSpecExpr):
                sym = maybe_param_spec
            else:
                param_spec_name = None

        if sym is not None:
            node = sym.node
            if isinstance(node, PlaceholderNode):
                if node.becomes_typeinfo:
                    # Reference to placeholder type.
                    if self.api.final_iteration:
                        self.cannot_resolve_type(t)
                        return AnyType(TypeOfAny.from_error)
                    elif self.allow_placeholder:
                        self.api.defer()
                    else:
                        self.api.record_incomplete_ref()
                    # Always allow ParamSpec for placeholders, if they are actually not valid,
                    # they will be reported later, after we resolve placeholders.
                    return PlaceholderType(
                        node.fullname,
                        self.anal_array(
                            t.args,
                            allow_param_spec=True,
                            allow_param_spec_literals=True,
                            allow_unpack=True,
                        ),
                        t.line,
                    )
                else:
                    if self.api.final_iteration:
                        self.cannot_resolve_type(t)
                        return AnyType(TypeOfAny.from_error)
                    else:
                        # Reference to an unknown placeholder node.
                        self.api.record_incomplete_ref()
                        return AnyType(TypeOfAny.special_form)
            if node is None:
                self.fail(f"Internal error (node is None, kind={sym.kind})", t)
                return AnyType(TypeOfAny.special_form)
            fullname = node.fullname
            hook = self.plugin.get_type_analyze_hook(fullname)
            if hook is not None:
                return hook(AnalyzeTypeContext(t, t, self))
            tvar_def = self.tvar_scope.get_binding(sym)
            if tvar_def is not None:
                # We need to cover special-case explained in get_typevarlike_argument() here,
                # since otherwise the deferral will not be triggered if the type variable is
                # used in a different module. Using isinstance() should be safe for this purpose.
                tvar_params = [tvar_def.upper_bound, tvar_def.default]
                if isinstance(tvar_def, TypeVarType):
                    tvar_params += tvar_def.values
                if any(isinstance(tp, PlaceholderType) for tp in tvar_params):
                    self.api.defer()
            if isinstance(sym.node, ParamSpecExpr):
                if tvar_def is None:
                    if self.allow_unbound_tvars:
                        return t
                    name = param_spec_name or t.name
                    if self.defining_alias and self.not_declared_in_type_params(t.name):
                        msg = f'ParamSpec "{name}" is not included in type_params'
                    else:
                        msg = f'ParamSpec "{name}" is unbound'
                    self.fail(msg, t, code=codes.VALID_TYPE)
                    return AnyType(TypeOfAny.from_error)
                assert isinstance(tvar_def, ParamSpecType)
                if len(t.args) > 0:
                    self.fail(
                        f'ParamSpec "{t.name}" used with arguments', t, code=codes.VALID_TYPE
                    )
                if param_spec_name is not None and not self.allow_param_spec_literals:
                    self.fail(
                        "ParamSpec components are not allowed here", t, code=codes.VALID_TYPE
                    )
                    return AnyType(TypeOfAny.from_error)
                # Change the line number
                return ParamSpecType(
                    tvar_def.name,
                    tvar_def.fullname,
                    tvar_def.id,
                    tvar_def.flavor,
                    tvar_def.upper_bound,
                    tvar_def.default,
                    line=t.line,
                    column=t.column,
                )
            if (
                isinstance(sym.node, TypeVarExpr)
                and self.defining_alias
                and not defining_literal
                and (tvar_def is None or tvar_def not in self.allowed_alias_tvars)
            ):
                if self.not_declared_in_type_params(t.name):
                    if self.python_3_12_type_alias:
                        msg = message_registry.TYPE_PARAMETERS_SHOULD_BE_DECLARED.format(
                            f'"{t.name}"'
                        )
                    else:
                        msg = f'Type variable "{t.name}" is not included in type_params'
                else:
                    msg = f'Can\'t use bound type variable "{t.name}" to define generic alias'
                self.fail(msg, t, code=codes.VALID_TYPE)
                return AnyType(TypeOfAny.from_error)
            if (
                isinstance(sym.node, TypeVarExpr)
                and tvar_def is not None
                and tvar_def in self.erase_tvar_defs
            ):
                # The caller should have already given a relevant error.
                return AnyType(TypeOfAny.from_error)
            if isinstance(sym.node, TypeVarExpr) and tvar_def is not None:
                assert isinstance(tvar_def, TypeVarType)
                if len(t.args) > 0:
                    self.fail(
                        f'Type variable "{t.name}" used with arguments', t, code=codes.VALID_TYPE
                    )
                # Change the line number
                return tvar_def.copy_modified(line=t.line, column=t.column)
            if isinstance(sym.node, TypeVarTupleExpr) and (
                tvar_def is not None
                and self.defining_alias
                and tvar_def not in self.allowed_alias_tvars
            ):
                if self.not_declared_in_type_params(t.name):
                    msg = f'Type variable "{t.name}" is not included in type_params'
                else:
                    msg = f'Can\'t use bound type variable "{t.name}" to define generic alias'
                self.fail(msg, t, code=codes.VALID_TYPE)
                return AnyType(TypeOfAny.from_error)
            if isinstance(sym.node, TypeVarTupleExpr):
                if tvar_def is None:
                    if self.allow_unbound_tvars:
                        return t
                    if self.defining_alias and self.not_declared_in_type_params(t.name):
                        if self.python_3_12_type_alias:
                            msg = message_registry.TYPE_PARAMETERS_SHOULD_BE_DECLARED.format(
                                f'"{t.name}"'
                            )
                        else:
                            msg = f'TypeVarTuple "{t.name}" is not included in type_params'
                    else:
                        msg = f'TypeVarTuple "{t.name}" is unbound'
                    self.fail(msg, t, code=codes.VALID_TYPE)
                    return AnyType(TypeOfAny.from_error)
                assert isinstance(tvar_def, TypeVarTupleType)
                if self.allow_type_var_tuple != self.nesting_level:
                    self.fail(
                        f'TypeVarTuple "{t.name}" is only valid with an unpack',
                        t,
                        code=codes.VALID_TYPE,
                    )
                    return AnyType(TypeOfAny.from_error)
                if len(t.args) > 0:
                    self.fail(
                        f'Type variable "{t.name}" used with arguments', t, code=codes.VALID_TYPE
                    )

                # Change the line number
                return TypeVarTupleType(
                    tvar_def.name,
                    tvar_def.fullname,
                    tvar_def.id,
                    tvar_def.upper_bound,
                    sym.node.tuple_fallback,
                    tvar_def.default,
                    line=t.line,
                    column=t.column,
                )
            special = self.try_analyze_special_unbound_type(t, fullname)
            if special is not None:
                return special
            if isinstance(node, TypeAlias):
                self.aliases_used.add(fullname)
                an_args = self.anal_array(
                    t.args,
                    allow_param_spec=True,
                    allow_param_spec_literals=node.has_param_spec_type,
                    allow_unpack=True,  # Fixed length unpacks can be used for non-variadic aliases.
                )
                if node.has_param_spec_type and len(node.alias_tvars) == 1:
                    an_args = self.pack_paramspec_args(an_args, t.empty_tuple_index)

                disallow_any = self.options.disallow_any_generics and not self.is_typeshed_stub
                res, used_default = instantiate_type_alias(
                    node,
                    an_args,
                    self.fail,
                    self.note,
                    node.no_args,
                    t,
                    self.options,
                    unexpanded_type=t,
                    disallow_any=disallow_any,
                    empty_tuple_index=t.empty_tuple_index,
                    analyzing_tvar_def=self.analyzing_tvar_def,
                )
                if self.analyzing_tvar_def and used_default and isinstance(res, TypeAliasType):
                    assert res.alias is not None
                    self.api.record_fixed_type(res.alias)
                # The only case where instantiate_type_alias() can return an incorrect instance is
                # when it is top-level instance, so no need to recurse.
                if (
                    isinstance(res, ProperType)
                    and isinstance(res, Instance)
                    and not (self.defining_alias and self.nesting_level == 0)
                    and not validate_instance(res, self.fail, t.empty_tuple_index)
                ):
                    used_default = fix_instance(
                        res,
                        self.fail,
                        self.note,
                        disallow_any=disallow_any,
                        options=self.options,
                        use_generic_error=True,
                        unexpanded_type=t,
                        analyzing_tvar_def=self.analyzing_tvar_def,
                    )
                    if self.analyzing_tvar_def and used_default:
                        self.api.record_fixed_type(res.type)
                if node.eager:
                    res = get_proper_type(res)
                return res
            elif isinstance(node, TypeInfo):
                return self.analyze_type_with_type_info(node, t.args, t, t.empty_tuple_index)
            elif node.fullname in TYPE_ALIAS_NAMES:
                return AnyType(TypeOfAny.special_form)
            # Concatenate is an operator, no need for a proper type
            elif node.fullname in CONCATENATE_TYPE_NAMES:
                # We check the return type further up the stack for valid use locations
                return self.apply_concatenate_operator(t)
            else:
                return self.analyze_unbound_type_without_type_info(t, sym, defining_literal)
        else:  # sym is None
            return AnyType(TypeOfAny.special_form)

    def _native_unbound_front_eligible(self, t: UnboundType) -> bool:
        """Pre-check the unbound-front hub (the #789 pattern).

        `rust_classify_unbound_front` defers for every non-placeholder
        special kind (Var, TypeAlias, TypeInfo, ...) yet the shim still
        pays lookup + ~20 scalar extractions + a PyO3 call per type.
        Return False for the always-defer kinds so the caller runs the
        pure-Python body directly (no duplicate lookup: the body below
        does its own). Staying native for the decidable kinds keeps the
        Rust classifier engaged.
        """
        sym = self.lookup_qualified(t.name, t)
        if sym is None:
            # SYM_NONE: the Rust front decides it (some special forms are
            # handled there), so stay on the native path.
            return True
        node = sym.node
        if node is None:
            return True
        if isinstance(node, (ParamSpecExpr, TypeVarExpr, TypeVarTupleExpr)):
            return True
        # Mirror the shim's .args/.kwargs re-resolution: the base symbol's
        # node kind governs, not the `X.args` lookup result.
        if t.name.endswith((".args", ".kwargs")):
            base = self.lookup_qualified(t.name.rsplit(".", 1)[0], t)
            if base is not None and isinstance(base.node, ParamSpecExpr):
                return True
        if isinstance(node, PlaceholderNode):
            return True
        return False

    def _native_visit_unbound_front(
        self, t: UnboundType, defining_literal: bool
    ) -> Type | None:
        """Classify the visit_unbound_type_nonoptional branch front in Rust.

        Ports the dispatch hub of typeanal.py:310-482. Rust receives only
        scalars / strings (facts) and returns a branch tag; this shim applies
        the side effects (defer / record_incomplete_ref / fail) and builds the
        result object. Returns None when Rust defers, so the caller runs the
        pure-Python body unchanged.
        """
        try:
            sym = self.lookup_qualified(t.name, t)
            param_spec_name = None
            if t.name.endswith((".args", ".kwargs")):
                param_spec_name = t.name.rsplit(".", 1)[0]
                maybe_param_spec = self.lookup_qualified(param_spec_name, t)
                if maybe_param_spec and isinstance(maybe_param_spec.node, ParamSpecExpr):
                    sym = maybe_param_spec
                else:
                    param_spec_name = None

            if sym is None:
                node_kind = _UNBOUND_FRONT_KIND_SYM_NONE
                node = None
            else:
                node = sym.node
                if node is None:
                    node_kind = _UNBOUND_FRONT_KIND_NODE_NONE
                elif isinstance(node, PlaceholderNode):
                    node_kind = _UNBOUND_FRONT_KIND_PLACEHOLDER
                elif isinstance(node, ParamSpecExpr):
                    node_kind = _UNBOUND_FRONT_KIND_PARAM_SPEC
                elif isinstance(node, TypeVarExpr):
                    node_kind = _UNBOUND_FRONT_KIND_TYPE_VAR
                elif isinstance(node, TypeVarTupleExpr):
                    node_kind = _UNBOUND_FRONT_KIND_TYPE_VAR_TUPLE
                else:
                    node_kind = _UNBOUND_FRONT_KIND_OTHER
            if isinstance(node, PlaceholderNode):
                placeholder_becomes_typeinfo = node.becomes_typeinfo
            else:
                placeholder_becomes_typeinfo = False
            if node is not None and not isinstance(node, PlaceholderNode):
                hook = self.plugin.get_type_analyze_hook(node.fullname)
            else:
                hook = None
            # The body reaches tvar_scope.get_binding (typeanal.py:360) only
            # after the placeholder / node-None / hook branches return, i.e.

            # for non-placeholder, non-None nodes. Match that so a node-None
            # symbol (fullname is None) cannot trip the get_binding assert.
            if node is not None and not isinstance(node, PlaceholderNode):
                tvar_def = self.tvar_scope.get_binding(sym)
                # Mirrors the pre-check at typeanal.py:361-369. Re-applied by
                # the shim below only when Rust decides a front branch, since
                # the body reaches it only on the param-spec/typevar arms.
                placeholder_in_tvar_params = False
                if tvar_def is not None:
                    tvar_params = [tvar_def.upper_bound, tvar_def.default]
                    if isinstance(tvar_def, TypeVarType):
                        tvar_params += list(tvar_def.values)
                    placeholder_in_tvar_params = any(
                        isinstance(tp, PlaceholderType) for tp in tvar_params
                    )
            else:
                tvar_def = None
                placeholder_in_tvar_params = False
            tag = _rust_classify_unbound_front(
                node_kind,
                placeholder_becomes_typeinfo,
                self.api.final_iteration,
                self.allow_placeholder,
                hook is not None,
                tvar_def is not None,
                tvar_def is not None and tvar_def in self.allowed_alias_tvars,
                tvar_def is not None and tvar_def in self.erase_tvar_defs,
                placeholder_in_tvar_params,
                self.allow_unbound_tvars,
                self.defining_alias,
                defining_literal,
                param_spec_name is not None,
                self.allow_param_spec_literals,
                len(t.args) > 0,
                self.alias_type_params_names,
                t.name,
                self.allow_type_var_tuple,
                self.nesting_level,
            )
            if tag is None:
                return None
            if node_kind in (
                _UNBOUND_FRONT_KIND_PARAM_SPEC,
                _UNBOUND_FRONT_KIND_TYPE_VAR,
                _UNBOUND_FRONT_KIND_TYPE_VAR_TUPLE,
            ) and placeholder_in_tvar_params:
                # The body applies this deferral before the param-spec /
                # typevar arms, which is exactly where these tags live.
                self.api.defer()
            return self._apply_unbound_front_tag(tag, t, sym, node, param_spec_name, tvar_def)
        except (AssertionError, NotImplementedError):
            return None

    def _apply_unbound_front_tag(
        self,
        tag: int,
        t: UnboundType,
        sym: SymbolTableNode | None,
        node: Node | None,
        param_spec_name: str | None,
        tvar_def: TypeVarLikeType | None,
    ) -> Type:
        """Apply the side effects and result building for a front tag.

        Exactly mirrors the branch bodies of typeanal.py:310-482; see
        crates/type_kernel/src/typeanal_unbound2.rs for the tag table.
        """
        if tag == _UNBOUND_FRONT_TAG_PH_BECOMES_FINAL:
            self.cannot_resolve_type(t)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_PH_BECOMES_DEFER:
            self.api.defer()
            assert isinstance(node, PlaceholderNode)
            return PlaceholderType(
                node.fullname,
                self.anal_array(
                    t.args,
                    allow_param_spec=True,
                    allow_param_spec_literals=True,
                    allow_unpack=True,
                ),
                t.line,
            )
        if tag == _UNBOUND_FRONT_TAG_PH_BECOMES_RECORD:
            self.api.record_incomplete_ref()
            assert isinstance(node, PlaceholderNode)
            return PlaceholderType(
                node.fullname,
                self.anal_array(
                    t.args,
                    allow_param_spec=True,
                    allow_param_spec_literals=True,
                    allow_unpack=True,
                ),
                t.line,
            )
        if tag == _UNBOUND_FRONT_TAG_PH_PLAIN_FINAL:
            self.cannot_resolve_type(t)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_PH_PLAIN_RECORD:
            self.api.record_incomplete_ref()
            return AnyType(TypeOfAny.special_form)
        if tag == _UNBOUND_FRONT_TAG_SYM_NONE:
            return AnyType(TypeOfAny.special_form)
        if tag == _UNBOUND_FRONT_TAG_NODE_NONE:
            assert sym is not None
            self.fail(f"Internal error (node is None, kind={sym.kind})", t)
            return AnyType(TypeOfAny.special_form)
        if tag == _UNBOUND_FRONT_TAG_PSPEC_UNBOUND_TVAR:
            return t
        if tag == _UNBOUND_FRONT_TAG_PSPEC_NOT_DECLARED:
            name = param_spec_name or t.name
            self.fail(
                f'ParamSpec "{name}" is not included in type_params', t, code=codes.VALID_TYPE
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_PSPEC_UNBOUND:
            name = param_spec_name or t.name
            self.fail(f'ParamSpec "{name}" is unbound', t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_PSPEC_ARGS_COMPONENT:
            self.fail(
                f'ParamSpec "{t.name}" used with arguments', t, code=codes.VALID_TYPE
            )
            self.fail("ParamSpec components are not allowed here", t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_PSPEC_ARGS:
            self.fail(
                f'ParamSpec "{t.name}" used with arguments', t, code=codes.VALID_TYPE
            )
            assert isinstance(tvar_def, ParamSpecType)
            return ParamSpecType(
                tvar_def.name,
                tvar_def.fullname,
                tvar_def.id,
                tvar_def.flavor,
                tvar_def.upper_bound,
                tvar_def.default,
                line=t.line,
                column=t.column,
            )
        if tag == _UNBOUND_FRONT_TAG_PSPEC_COMPONENT:
            self.fail("ParamSpec components are not allowed here", t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_PSPEC_OK:
            assert isinstance(tvar_def, ParamSpecType)
            return ParamSpecType(
                tvar_def.name,
                tvar_def.fullname,
                tvar_def.id,
                tvar_def.flavor,
                tvar_def.upper_bound,
                tvar_def.default,
                line=t.line,
                column=t.column,
            )
        if tag == _UNBOUND_FRONT_TAG_TVAR_ALIAS_NOT_DECLARED:
            if self.python_3_12_type_alias:
                msg = message_registry.TYPE_PARAMETERS_SHOULD_BE_DECLARED.format(
                    f'"{t.name}"'
                )
            else:
                msg = f'Type variable "{t.name}" is not included in type_params'
            self.fail(msg, t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVAR_ALIAS_BOUND:
            self.fail(
                f'Can\'t use bound type variable "{t.name}" to define generic alias',
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVAR_ERASED:
            # The caller should have already given a relevant error.
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVAR_ARGS:
            assert isinstance(tvar_def, TypeVarType)
            self.fail(
                f'Type variable "{t.name}" used with arguments', t, code=codes.VALID_TYPE
            )
            return tvar_def.copy_modified(line=t.line, column=t.column)
        if tag == _UNBOUND_FRONT_TAG_TVAR_OK:
            assert isinstance(tvar_def, TypeVarType)
            return tvar_def.copy_modified(line=t.line, column=t.column)
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_ALIAS_NOT_DECLARED:
            self.fail(
                f'Type variable "{t.name}" is not included in type_params',
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_ALIAS_BOUND:
            self.fail(
                f'Can\'t use bound type variable "{t.name}" to define generic alias',
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_UNBOUND_TVAR:
            return t
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_NOT_DECLARED:
            if self.python_3_12_type_alias:
                msg = message_registry.TYPE_PARAMETERS_SHOULD_BE_DECLARED.format(
                    f'"{t.name}"'
                )
            else:
                msg = f'TypeVarTuple "{t.name}" is not included in type_params'
            self.fail(msg, t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_UNBOUND:
            self.fail(f'TypeVarTuple "{t.name}" is unbound', t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_NESTING:
            self.fail(
                f'TypeVarTuple "{t.name}" is only valid with an unpack',
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_ARGS:
            assert isinstance(tvar_def, TypeVarTupleType)
            assert isinstance(node, TypeVarTupleExpr)
            self.fail(
                f'Type variable "{t.name}" used with arguments', t, code=codes.VALID_TYPE
            )
            return TypeVarTupleType(
                tvar_def.name,
                tvar_def.fullname,
                tvar_def.id,
                tvar_def.upper_bound,
                node.tuple_fallback,
                tvar_def.default,
                line=t.line,
                column=t.column,
            )
        if tag == _UNBOUND_FRONT_TAG_TVARTUPLE_OK:
            assert isinstance(tvar_def, TypeVarTupleType)
            assert isinstance(node, TypeVarTupleExpr)
            return TypeVarTupleType(
                tvar_def.name,
                tvar_def.fullname,
                tvar_def.id,
                tvar_def.upper_bound,
                node.tuple_fallback,
                tvar_def.default,
                line=t.line,
                column=t.column,
            )
        raise AssertionError(f"unknown unbound front tag {tag}")

    def pack_paramspec_args(self, an_args: Sequence[Type], empty_tuple_index: bool) -> list[Type]:
        # "Aesthetic" ParamSpec literals for single ParamSpec: C[int, str] -> C[[int, str]].
        # These do not support mypy_extensions VarArgs, etc. as they were already analyzed
        # TODO: should these be re-analyzed to get rid of this inconsistency?
        count = len(an_args)
        if count == 0 and empty_tuple_index:
            return [Parameters([], [], [])]
        elif count == 0:
            return []

        if count == 1 and isinstance(get_proper_type(an_args[0]), AnyType):
            # Single Any is interpreted as ..., rather that a single argument with Any type.
            # I didn't find this in the PEP, but it sounds reasonable.
            return list(an_args)
        if any(isinstance(a, (Parameters, ParamSpecType)) for a in an_args):
            if len(an_args) > 1:
                first_wrong = next(
                    arg for arg in an_args if isinstance(arg, (Parameters, ParamSpecType))
                )
                self.fail(
                    "Nested parameter specifications are not allowed",
                    first_wrong,
                    code=codes.VALID_TYPE,
                )
                return [AnyType(TypeOfAny.from_error)]
            return list(an_args)
        first = an_args[0]
        return [
            Parameters(
                an_args, [ARG_POS] * count, [None] * count, line=first.line, column=first.column
            )
        ]

    def cannot_resolve_type(self, t: UnboundType) -> None:
        # TODO: Move error message generation to messages.py. We'd first
        #       need access to MessageBuilder here. Also move the similar
        #       message generation logic in semanal.py.
        self.api.fail(f'Cannot resolve name "{t.name}" (possible cyclic definition)', t)
        if self.api.is_func_scope():
            self.note("Recursive types are not allowed at function scope", t)

    def apply_concatenate_operator(self, t: UnboundType) -> Type:
        if len(t.args) == 0:
            self.api.fail("Concatenate needs type arguments", t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)

        # Last argument has to be ParamSpec or Ellipsis.
        ps = self.anal_type(t.args[-1], allow_param_spec=True, allow_ellipsis=True)
        if not isinstance(ps, (ParamSpecType, Parameters)):
            if isinstance(ps, UnboundType) and self.allow_unbound_tvars:
                sym = self.lookup_qualified(ps.name, t)
                if sym is not None and isinstance(sym.node, ParamSpecExpr):
                    return ps
            self.api.fail(
                "The last parameter to Concatenate needs to be a ParamSpec",
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        elif isinstance(ps, ParamSpecType) and ps.prefix.arg_types:
            self.api.fail("Nested Concatenates are invalid", t, code=codes.VALID_TYPE)

        args = self.anal_array(t.args[:-1])
        pre = ps.prefix if isinstance(ps, ParamSpecType) else ps

        # mypy can't infer this :(
        names: list[str | None] = [None] * len(args)

        pre = Parameters(
            args + pre.arg_types,
            [ARG_POS] * len(args) + pre.arg_kinds,
            names + pre.arg_names,
            line=t.line,
            column=t.column,
        )
        return ps.copy_modified(prefix=pre) if isinstance(ps, ParamSpecType) else pre

    def _native_special_tag(self, t: UnboundType, fullname: str) -> int | None:
        """Classify the `try_analyze_special_unbound_type` elif-chain in Rust.

        The special forms (`builtins.None`, `typing.Any`, `Final`, `Tuple`,
        `Union`, `Optional`, `Callable`, `Type`, `TypeForm`, `ClassVar`,
        `Never`, `Annotated`, `Required`, `NotRequired`, `ReadOnly`) are all
        decided by the fullname plus scalar facts. Every branch keeps its
        full Python body; this seam only replaces the *decision*, returning
        a tag the caller uses to jump straight to the matching branch.
        """
        if not (_TYPEANAL_HAS_KERNEL and _native_typeanal_active):
            return None
        tuple_missing_or_placeholder = False
        tuple_ellipsis_form = False
        if fullname in TUPLE_NAMES:
            # The Tuple branch needs the `builtins.tuple` symbol lookup and
            # the EllipsisType arity check, which stay Python-owned.
            sym = self.api.lookup_fully_qualified_or_none("builtins.tuple")
            tuple_missing_or_placeholder = bool(
                not sym or isinstance(sym.node, PlaceholderNode)
            )
            tuple_ellipsis_form = bool(
                len(t.args) == 2
                and isinstance(t.args[1], EllipsisType)
                and not tuple_missing_or_placeholder
            )
        try:
            # `allow_typed_dict_special_forms` is lazily read in the original
            # branch bodies, so use its constructor default rather than
            # requiring the attr to exist (test-only analyzers built with

            # `TypeAnalyser.__new__` omit it).
            return _rust_classify_special_unbound(
                fullname,
                len(t.args),
                t.empty_tuple_index,
                getattr(
                    self,
                    "allow_typed_dict_special_forms",
                    False,
                ),
                tuple_missing_or_placeholder,
                tuple_ellipsis_form,
                fullname not in FINAL_TYPE_NAMES,
                fullname not in TUPLE_NAMES,
                fullname not in TYPE_NAMES,
                fullname not in ("typing_extensions.TypeForm", "typing.TypeForm"),
                fullname != "typing.ClassVar",
                fullname not in NEVER_NAMES,
                fullname not in ANNOTATED_TYPE_NAMES,
                fullname not in ("typing_extensions.Required", "typing.Required"),
                fullname not in ("typing_extensions.NotRequired", "typing.NotRequired"),
                fullname not in ("typing_extensions.ReadOnly", "typing.ReadOnly"),
                fullname not in LITERAL_TYPE_NAMES,
                fullname not in UNPACK_TYPE_NAMES,
                fullname not in SELF_TYPE_NAMES,
                getattr(self, "allow_unpack", False),
            )
        except (AssertionError, NotImplementedError):
            return None

    def try_analyze_special_unbound_type(self, t: UnboundType, fullname: str) -> Type | None:
        """Bind special type that is recognized through magic name such as 'typing.Any'.

        Return the bound type if successful, and return None if the type is a normal type.
        """
        spec_tag = self._native_special_tag(t, fullname)
        if spec_tag == _UNBOUND_SPECIAL_TAG_NOT_SPECIAL:
            # Rust proved the name is no special form; the elif-chain below
            # would fall through to `return None` anyway (the TypeGuard/TypeIs
            # checks return None for other names). Skip it entirely.
            return None
        if spec_tag is not None:
            return self._apply_special_unbound_tag(spec_tag, t, fullname)
        if fullname == "builtins.None":
            return NoneType()
        elif fullname == "typing.Any":
            return AnyType(TypeOfAny.explicit, line=t.line, column=t.column)
        elif fullname in FINAL_TYPE_NAMES:
            if self.prohibit_special_class_field_types:
                self.fail(
                    f"Final[...] can't be used inside a {self.prohibit_special_class_field_types}",
                    t,
                    code=codes.VALID_TYPE,
                )
            else:
                if not self.allow_final:
                    self.fail(
                        "Final can be only used as an outermost qualifier in a variable annotation",
                        t,
                        code=codes.VALID_TYPE,
                    )
            return AnyType(TypeOfAny.from_error)
        elif fullname in TUPLE_NAMES:
            # Tuple is special because it is involved in builtin import cycle
            # and may be not ready when used.
            sym = self.api.lookup_fully_qualified_or_none("builtins.tuple")
            if not sym or isinstance(sym.node, PlaceholderNode):
                if self.api.is_incomplete_namespace("builtins"):
                    self.api.record_incomplete_ref()
                else:
                    self.fail('Name "tuple" is not defined', t)
                return AnyType(TypeOfAny.special_form)
            if len(t.args) == 0 and not t.empty_tuple_index:
                # Bare 'Tuple' is same as 'tuple'
                any_type = self.get_omitted_any(t)
                return self.named_type("builtins.tuple", [any_type], line=t.line, column=t.column)
            if len(t.args) == 2 and isinstance(t.args[1], EllipsisType):
                # Tuple[T, ...] (uniform, variable-length tuple)
                instance = self.named_type("builtins.tuple", [self.anal_type(t.args[0])])
                instance.line = t.line
                return instance
            return self.tuple_type(
                self.anal_array(t.args, allow_unpack=True), line=t.line, column=t.column
            )
        elif fullname == "typing.Union":
            items = self.anal_array(t.args)
            return UnionType.make_union(items, line=t.line, column=t.column)
        elif fullname == "typing.Optional":
            if len(t.args) != 1:
                self.fail(
                    "Optional[...] must have exactly one type argument", t, code=codes.VALID_TYPE
                )
                return AnyType(TypeOfAny.from_error)
            item = self.anal_type(t.args[0])
            return make_optional_type(item)
        elif fullname == "typing.Callable":
            return self.analyze_callable_type(t)
        elif fullname in TYPE_NAMES:
            if len(t.args) == 0:
                if fullname == "typing.Type":
                    any_type = self.get_omitted_any(t)
                    return TypeType(any_type, line=t.line, column=t.column)
                else:
                    # To prevent assignment of 'builtins.type' inferred as 'builtins.object'
                    # See https://github.com/python/mypy/issues/9476 for more information
                    return None
            type_str = "Type[...]" if fullname == "typing.Type" else "type[...]"
            if len(t.args) != 1:
                self.fail(
                    f"{type_str} must have exactly one type argument", t, code=codes.VALID_TYPE
                )
            item = self.anal_type(t.args[0])
            bad_item_name = get_bad_type_type_item(item)
            if bad_item_name:
                self.fail(f'{type_str} can\'t contain "{bad_item_name}"', t, code=codes.VALID_TYPE)
                item = AnyType(TypeOfAny.from_error)
            return TypeType.make_normalized(item, line=t.line, column=t.column)
        elif fullname in ("typing_extensions.TypeForm", "typing.TypeForm"):
            if len(t.args) == 0:
                any_type = self.get_omitted_any(t)
                return TypeType(any_type, line=t.line, column=t.column, is_type_form=True)
            if len(t.args) != 1:
                type_str = "TypeForm[...]"
                self.fail(
                    type_str + " must have exactly one type argument", t, code=codes.VALID_TYPE
                )
            item = self.anal_type(t.args[0])
            return TypeType.make_normalized(item, line=t.line, column=t.column, is_type_form=True)
        elif fullname == "typing.ClassVar":
            if self.nesting_level > 0:
                self.fail(
                    "Invalid type: ClassVar nested inside other type", t, code=codes.VALID_TYPE
                )
            if self.prohibit_special_class_field_types:
                self.fail(
                    f"ClassVar[...] can't be used inside a {self.prohibit_special_class_field_types}",
                    t,
                    code=codes.VALID_TYPE,
                )
            if self.defining_alias:
                self.fail(
                    "ClassVar[...] can't be used inside a type alias", t, code=codes.VALID_TYPE
                )
            if len(t.args) == 0:
                return AnyType(TypeOfAny.from_omitted_generics, line=t.line, column=t.column)
            if len(t.args) != 1:
                self.fail(
                    "ClassVar[...] must have at most one type argument", t, code=codes.VALID_TYPE
                )
                return AnyType(TypeOfAny.from_error)
            return self.anal_type(t.args[0], allow_final=self.options.python_version >= (3, 13))
        elif fullname in NEVER_NAMES:
            return UninhabitedType()
        elif fullname in LITERAL_TYPE_NAMES:
            return self.analyze_literal_type(t)
        elif fullname in ANNOTATED_TYPE_NAMES:
            if len(t.args) < 2:
                self.fail(
                    "Annotated[...] must have exactly one type argument"
                    " and at least one annotation",
                    t,
                    code=codes.VALID_TYPE,
                )
                return AnyType(TypeOfAny.from_error)
            return self.anal_type(
                t.args[0], allow_typed_dict_special_forms=self.allow_typed_dict_special_forms
            )
        elif fullname in ("typing_extensions.Required", "typing.Required"):
            if not self.allow_typed_dict_special_forms:
                self.fail(
                    "Required[] can be only used in a TypedDict definition",
                    t,
                    code=codes.VALID_TYPE,
                )
                return AnyType(TypeOfAny.from_error)
            if len(t.args) != 1:
                self.fail(
                    "Required[] must have exactly one type argument", t, code=codes.VALID_TYPE
                )
                return AnyType(TypeOfAny.from_error)
            return RequiredType(
                self.anal_type(t.args[0], allow_typed_dict_special_forms=True), required=True
            )
        elif fullname in ("typing_extensions.NotRequired", "typing.NotRequired"):
            if not self.allow_typed_dict_special_forms:
                self.fail(
                    "NotRequired[] can be only used in a TypedDict definition",
                    t,
                    code=codes.VALID_TYPE,
                )
                return AnyType(TypeOfAny.from_error)
            if len(t.args) != 1:
                self.fail(
                    "NotRequired[] must have exactly one type argument", t, code=codes.VALID_TYPE
                )
                return AnyType(TypeOfAny.from_error)
            return RequiredType(
                self.anal_type(t.args[0], allow_typed_dict_special_forms=True), required=False
            )
        elif fullname in ("typing_extensions.ReadOnly", "typing.ReadOnly"):
            if not self.allow_typed_dict_special_forms:
                self.fail(
                    "ReadOnly[] can be only used in a TypedDict definition",
                    t,
                    code=codes.VALID_TYPE,
                )
                return AnyType(TypeOfAny.from_error)
            if len(t.args) != 1:
                self.fail(
                    '"ReadOnly[]" must have exactly one type argument', t, code=codes.VALID_TYPE
                )
                return AnyType(TypeOfAny.from_error)
            return ReadOnlyType(self.anal_type(t.args[0], allow_typed_dict_special_forms=True))
        elif (
            self.anal_type_guard_arg(t, fullname) is not None
            or self.anal_type_is_arg(t, fullname) is not None
        ):
            # In most contexts, TypeGuard[...] acts as an alias for bool (ignoring its args)
            return self.named_type("builtins.bool")
        elif fullname in UNPACK_TYPE_NAMES:
            if len(t.args) != 1:
                self.fail("Unpack[...] requires exactly one type argument", t)
                return AnyType(TypeOfAny.from_error)
            if not self.allow_unpack:
                self.fail(message_registry.INVALID_UNPACK_POSITION, t, code=codes.VALID_TYPE)
                return AnyType(TypeOfAny.from_error)
            self.allow_type_var_tuple = self.nesting_level + 1
            result = UnpackType(self.anal_type(t.args[0]), line=t.line, column=t.column)
            self.allow_type_var_tuple = -1
            return result
        elif fullname in SELF_TYPE_NAMES:
            if t.args:
                self.fail("Self type cannot have type arguments", t)
            if self.prohibit_self_type is not None:
                self.fail(f"Self type cannot be used in {self.prohibit_self_type}", t)
                return AnyType(TypeOfAny.from_error)
            if self.api.type is None:
                self.fail("Self type is only allowed in annotations within class definition", t)
                return AnyType(TypeOfAny.from_error)
            if self.api.type.has_base("builtins.type"):
                self.fail("Self type cannot be used in a metaclass", t)
            if self.api.type.self_type is not None:
                if self.api.type.is_final or self.api.type.is_enum and self.api.type.enum_members:
                    return fill_typevars(self.api.type)
                return self.api.type.self_type.copy_modified(line=t.line, column=t.column)
            # TODO: verify this is unreachable and replace with an assert?
            self.fail("Unexpected Self type", t)
            return AnyType(TypeOfAny.from_error)
        return None

    def _apply_special_unbound_tag(self, tag: int, t: UnboundType, fullname: str) -> Type | None:
        """Apply the side effects and result building for a special tag.

        Exactly mirrors the branch bodies of typeanal.py:936-1141; see
        crates/type_kernel/src/typeanal_special.rs for the tag table. The
        deferred tags (Rust returned None) never reach this method: the
        caller runs the full pure-Python body instead. A `None` return (the
        bare `typing.Type` branch, #9476) means "not a special type";
        the caller treats it the same as the fallback body's `return None`.
        """
        if tag == _UNBOUND_SPECIAL_TAG_NONE_TYPE:
            return NoneType()
        if tag == _UNBOUND_SPECIAL_TAG_ANY:
            return AnyType(TypeOfAny.explicit, line=t.line, column=t.column)
        if tag == _UNBOUND_SPECIAL_TAG_NEVER:
            return UninhabitedType()
        if tag == _UNBOUND_SPECIAL_TAG_FINAL_ERROR:
            if self.prohibit_special_class_field_types:
                self.fail(
                    f"Final[...] can't be used inside a {self.prohibit_special_class_field_types}",
                    t,
                    code=codes.VALID_TYPE,
                )
            else:
                if not self.allow_final:
                    self.fail(
                        "Final can be only used as an outermost qualifier in a variable annotation",
                        t,
                        code=codes.VALID_TYPE,
                    )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_TUPLE_LOOKUP_DEFER:
            sym = self.api.lookup_fully_qualified_or_none("builtins.tuple")
            if not sym or isinstance(sym.node, PlaceholderNode):
                if self.api.is_incomplete_namespace("builtins"):
                    self.api.record_incomplete_ref()
                else:
                    self.fail('Name "tuple" is not defined', t)
                return AnyType(TypeOfAny.special_form)
        if tag == _UNBOUND_SPECIAL_TAG_TUPLE_BARE:
            any_type = self.get_omitted_any(t)
            return self.named_type("builtins.tuple", [any_type], line=t.line, column=t.column)
        if tag == _UNBOUND_SPECIAL_TAG_TUPLE_ELLIPSIS:
            instance = self.named_type("builtins.tuple", [self.anal_type(t.args[0])])
            instance.line = t.line
            return instance
        if tag == _UNBOUND_SPECIAL_TAG_TUPLE_FULL_DEFER:
            return self.tuple_type(
                self.anal_array(t.args, allow_unpack=True), line=t.line, column=t.column
            )
        if tag == _UNBOUND_SPECIAL_TAG_UNION_DEFER:
            items = self.anal_array(t.args)
            return UnionType.make_union(items, line=t.line, column=t.column)
        if tag == _UNBOUND_SPECIAL_TAG_OPTIONAL_ARG_ERR:
            self.fail(
                "Optional[...] must have exactly one type argument", t, code=codes.VALID_TYPE
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_OPTIONAL_DEFER:
            item = self.anal_type(t.args[0])
            return make_optional_type(item)
        if tag == _UNBOUND_SPECIAL_TAG_CALLABLE_DEFER:
            return self.analyze_callable_type(t)
        if tag == _UNBOUND_SPECIAL_TAG_TYPE_BARE_ANY:
            any_type = self.get_omitted_any(t)
            return TypeType(any_type, line=t.line, column=t.column)
        if tag == _UNBOUND_SPECIAL_TAG_TYPE_BARE_NONE:
            # To prevent assignment of 'builtins.type' inferred as
            # 'builtins.object'. See https://github.com/python/mypy/issues/9476
            return None  # type: ignore[return-value]
        if tag in (_UNBOUND_SPECIAL_TAG_TYPE_ONE_ARG, _UNBOUND_SPECIAL_TAG_TYPE_ARG_ERR):
            type_str = "Type[...]" if fullname == "typing.Type" else "type[...]"
            if len(t.args) != 1:
                self.fail(
                    f"{type_str} must have exactly one type argument", t, code=codes.VALID_TYPE
                )
            item = self.anal_type(t.args[0])
            bad_item_name = get_bad_type_type_item(item)
            if bad_item_name:
                self.fail(f'{type_str} can\'t contain "{bad_item_name}"', t, code=codes.VALID_TYPE)
                item = AnyType(TypeOfAny.from_error)
            return TypeType.make_normalized(item, line=t.line, column=t.column)
        if tag == _UNBOUND_SPECIAL_TAG_TYPEFORM_BARE:
            any_type = self.get_omitted_any(t)
            return TypeType(any_type, line=t.line, column=t.column, is_type_form=True)
        if tag == _UNBOUND_SPECIAL_TAG_TYPEFORM_DEFER:
            type_str = "TypeForm[...]"
            if len(t.args) != 1:
                self.fail(
                    type_str + " must have exactly one type argument", t, code=codes.VALID_TYPE
                )
            item = self.anal_type(t.args[0])
            return TypeType.make_normalized(item, line=t.line, column=t.column, is_type_form=True)
        if tag == _UNBOUND_SPECIAL_TAG_CLASSVAR_ZERO:
            # The three context checks run before the arg-count dispatch in
            # the original (typeanal.py:1081-1094), so bare ClassVar still
            # reports nesting / TypedDict / alias errors.
            if self.nesting_level > 0:
                self.fail(
                    "Invalid type: ClassVar nested inside other type", t, code=codes.VALID_TYPE
                )
            if self.prohibit_special_class_field_types:
                self.fail(
                    f"ClassVar[...] can't be used inside a {self.prohibit_special_class_field_types}",
                    t,
                    code=codes.VALID_TYPE,
                )
            if self.defining_alias:
                self.fail(
                    "ClassVar[...] can't be used inside a type alias", t, code=codes.VALID_TYPE
                )
            return AnyType(TypeOfAny.from_omitted_generics, line=t.line, column=t.column)
        if tag == _UNBOUND_SPECIAL_TAG_CLASSVAR_DEFER:
            if self.nesting_level > 0:
                self.fail(
                    "Invalid type: ClassVar nested inside other type", t, code=codes.VALID_TYPE
                )
            if self.prohibit_special_class_field_types:
                self.fail(
                    f"ClassVar[...] can't be used inside a {self.prohibit_special_class_field_types}",
                    t,
                    code=codes.VALID_TYPE,
                )
            if self.defining_alias:
                self.fail(
                    "ClassVar[...] can't be used inside a type alias", t, code=codes.VALID_TYPE
                )
            if len(t.args) != 1:
                self.fail(
                    "ClassVar[...] must have at most one type argument", t, code=codes.VALID_TYPE
                )
                return AnyType(TypeOfAny.from_error)
            return self.anal_type(t.args[0], allow_final=self.options.python_version >= (3, 13))
        if tag == _UNBOUND_SPECIAL_TAG_ANNOTATED_ARG_ERR:
            self.fail(
                "Annotated[...] must have exactly one type argument"
                " and at least one annotation",
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_ANNOTATED_DEFER:
            return self.anal_type(
                t.args[0], allow_typed_dict_special_forms=self.allow_typed_dict_special_forms
            )
        if tag == _UNBOUND_SPECIAL_TAG_REQUIRED_BAD_CTX:
            self.fail(
                "Required[] can be only used in a TypedDict definition",
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_REQUIRED_ARG_ERR:
            self.fail("Required[] must have exactly one type argument", t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_REQUIRED_DEFER:
            return RequiredType(
                self.anal_type(t.args[0], allow_typed_dict_special_forms=True), required=True
            )
        if tag == _UNBOUND_SPECIAL_TAG_NOTREQUIRED_BAD_CTX:
            self.fail(
                "NotRequired[] can be only used in a TypedDict definition",
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_NOTREQUIRED_ARG_ERR:
            self.fail(
                "NotRequired[] must have exactly one type argument", t, code=codes.VALID_TYPE
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_NOTREQUIRED_DEFER:
            return RequiredType(
                self.anal_type(t.args[0], allow_typed_dict_special_forms=True), required=False
            )
        if tag == _UNBOUND_SPECIAL_TAG_READONLY_BAD_CTX:
            self.fail(
                "ReadOnly[] can be only used in a TypedDict definition",
                t,
                code=codes.VALID_TYPE,
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_READONLY_ARG_ERR:
            self.fail(
                '"ReadOnly[]" must have exactly one type argument', t, code=codes.VALID_TYPE
            )
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_READONLY_DEFER:
            return ReadOnlyType(self.anal_type(t.args[0], allow_typed_dict_special_forms=True))
        if tag == _UNBOUND_SPECIAL_TAG_LITERAL_DEFER:
            return self.analyze_literal_type(t)
        if tag in (_UNBOUND_SPECIAL_TAG_NAME_TYPEGUARD, _UNBOUND_SPECIAL_TAG_NAME_TYPEIS):
            if tag == _UNBOUND_SPECIAL_TAG_NAME_TYPEGUARD:
                self.anal_type_guard_arg(t, fullname)
            else:
                self.anal_type_is_arg(t, fullname)
            return self.named_type("builtins.bool")
        if tag == _UNBOUND_SPECIAL_TAG_UNPACK_ARG_ERR:
            self.fail("Unpack[...] requires exactly one type argument", t)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_UNPACK_POS_ERR:
            self.fail(message_registry.INVALID_UNPACK_POSITION, t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        if tag == _UNBOUND_SPECIAL_TAG_UNPACK_DEFER:
            # Exact gold body (typeanal.py:1181-1186): the mutation around
            # anal_type is applied here, live on self, same as the original.
            self.allow_type_var_tuple = self.nesting_level + 1
            result = UnpackType(self.anal_type(t.args[0]), line=t.line, column=t.column)
            self.allow_type_var_tuple = -1
            return result
        raise AssertionError(f"unknown special unbound tag {tag}")

    def get_omitted_any(self, typ: Type, fullname: str | None = None) -> AnyType:
        disallow_any = not self.is_typeshed_stub and self.options.disallow_any_generics
        return get_omitted_any(disallow_any, self.fail, self.note, typ, self.options, fullname)

    def check_and_warn_deprecated(self, info: TypeInfo, ctx: Context) -> None:
        """Similar logic to `TypeChecker.check_deprecated` and `TypeChecker.warn_deprecated."""

        tag = self._native_deprecated_warn_tag(info)
        if tag is not None:
            if tag == _DEPRECATED_TAG_NOTE:
                self.note(info.deprecated, ctx, code=codes.DEPRECATED)
            elif tag == _DEPRECATED_TAG_FAIL:
                self.fail(info.deprecated, ctx, code=codes.DEPRECATED)
            return

        if (
            (deprecated := info.deprecated)
            and not self.is_typeshed_stub
            and not (self.api.type and (self.api.type.fullname == info.fullname))
            and not any(
                info.fullname == p or info.fullname.startswith(f"{p}.")
                for p in self.options.deprecated_calls_exclude
            )
        ):
            for imp in self.cur_mod_node.imports:
                if isinstance(imp, ImportFrom) and any(info.name == n[0] for n in imp.names):
                    break
            else:
                warn = self.note if self.options.report_deprecated_as_note else self.fail
                warn(deprecated, ctx, code=codes.DEPRECATED)

    def _native_deprecated_warn_tag(self, info: TypeInfo) -> int | None:
        """Classify the `check_and_warn_deprecated` arbitration head in Rust.

        Rust decides silent/note/fail from scalar facts; Python applies the
        self.note / self.fail side effects with the live info.deprecated
        string. None defers to the pure-Python body. Tag table in
        crates/type_kernel/src/typeanal_deprec.rs.
        """
        if not (_TYPEANAL_HAS_KERNEL and _native_typeanal_active):
            return None
        if not info.deprecated:
            # Short-circuit on the falsy head fact so the eager fact reads
            # below (api.type, cur_mod_node.imports) stay as lazy as the
            # `deprecated and ...` chain of the pure-Python body.
            return None
        try:
            return _rust_classify_check_warn_deprecated(
                info.deprecated,
                self.is_typeshed_stub,
                self.api.type.fullname if self.api.type else None,
                info.fullname,
                info.name,
                self.options.deprecated_calls_exclude,
                self.options.report_deprecated_as_note,
                [
                    n[0]
                    for imp in self.cur_mod_node.imports
                    if isinstance(imp, ImportFrom)
                    for n in imp.names
                ],
            )
        except (AssertionError, NotImplementedError):
            return None

    def analyze_type_with_type_info(
        self, info: TypeInfo, args: Sequence[Type], ctx: Context, empty_tuple_index: bool
    ) -> Type:
        """Bind unbound type when were able to find target TypeInfo.

        This handles simple cases like 'int', 'modname.UserClass[str]', etc.
        """

        if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
            try:
                tag = _rust_classify_type_with_info(
                    info.fullname,
                    len(args),
                    info.tuple_type is not None,
                    info.special_alias is not None,
                    info.typeddict_type is not None,
                )
            except (AssertionError, NotImplementedError):
                tag = None
            if tag == _TYPE_WITH_INFO_TAG_TUPLE:
                self.check_and_warn_deprecated(info, ctx)
                fallback = Instance(info, [AnyType(TypeOfAny.special_form)], ctx.line)
                return TupleType(self.anal_array(args, allow_unpack=True), fallback, ctx.line)
            if tag == _TYPE_WITH_INFO_TAG_NONE_TYPE:
                self.check_and_warn_deprecated(info, ctx)
                self.fail(
                    "NoneType should not be used as a type, please use None instead",
                    ctx,
                    code=codes.NONETYPE_TYPE,
                )
                return NoneType(ctx.line, ctx.column)

        self.check_and_warn_deprecated(info, ctx)

        if len(args) > 0 and info.fullname == "builtins.tuple":
            fallback = Instance(info, [AnyType(TypeOfAny.special_form)], ctx.line)
            return TupleType(self.anal_array(args, allow_unpack=True), fallback, ctx.line)

        # Analyze arguments and (usually) construct Instance type. The
        # number of type arguments and their values are
        # checked only later, since we do not always know the

        # valid count at this point. Thus we may construct an
        # Instance with an invalid number of type arguments.

        # We allow ParamSpec literals based on a heuristic: it will be
        # checked later anyways but the error message may be worse.
        instance = Instance(
            info,
            self.anal_array(
                args,
                allow_param_spec=True,
                allow_param_spec_literals=info.has_param_spec_type,
                allow_unpack=True,  # Fixed length tuples can be used for non-variadic types.
            ),
            ctx.line,
            ctx.column,
        )
        instance.end_line = ctx.end_line
        instance.end_column = ctx.end_column
        if len(info.type_vars) == 1 and info.has_param_spec_type:
            instance.args = tuple(self.pack_paramspec_args(instance.args, empty_tuple_index))

        if info.fullname == "librt.vecs.vec" and not check_vec_type_args(
            instance.args, ctx, self.api
        ):
            return AnyType(TypeOfAny.from_error)

        # Check type argument count.
        old_args = instance.args
        instance.args = tuple(flatten_nested_tuples(instance.args))
        if old_args and not instance.args:
            empty_tuple_index = True
        if not (self.defining_alias and self.nesting_level == 0) and not validate_instance(
            instance, self.fail, empty_tuple_index
        ):
            used_default = fix_instance(
                instance,
                self.fail,
                self.note,
                disallow_any=self.options.disallow_any_generics and not self.is_typeshed_stub,
                options=self.options,
                analyzing_tvar_def=self.analyzing_tvar_def,
            )
            if self.analyzing_tvar_def and used_default:
                self.api.record_fixed_type(info)

        tup = info.tuple_type
        if tup is not None:
            # The class has a Tuple[...] base class so it will be
            # represented as a tuple type.
            if info.special_alias:
                res, used_default = instantiate_type_alias(
                    info.special_alias,
                    # TODO: should we allow NamedTuples generic in ParamSpec?
                    self.anal_array(args, allow_unpack=True),
                    self.fail,
                    self.note,
                    False,
                    ctx,
                    self.options,
                    use_standard_error=True,
                    empty_tuple_index=empty_tuple_index,
                    analyzing_tvar_def=self.analyzing_tvar_def,
                )
                if self.analyzing_tvar_def and used_default:
                    # For convenience, we make default depend on the original TypeInfo,
                    # *not* on the special alias.
                    self.api.record_fixed_type(info)
                return res
            return tup.copy_modified(
                items=self.anal_array(tup.items, allow_unpack=True), fallback=instance
            )
        td = info.typeddict_type
        if td is not None:
            # The class has a TypedDict[...] base class so it will be
            # represented as a typeddict type.
            if info.special_alias:
                res, used_default = instantiate_type_alias(
                    info.special_alias,
                    # TODO: should we allow TypedDicts generic in ParamSpec?
                    self.anal_array(args, allow_unpack=True),
                    self.fail,
                    self.note,
                    False,
                    ctx,
                    self.options,
                    use_standard_error=True,
                    analyzing_tvar_def=self.analyzing_tvar_def,
                )
                if self.analyzing_tvar_def and used_default:
                    # For convenience, we make default depend on the original TypeInfo,
                    # *not* on the special alias.
                    self.api.record_fixed_type(info)
                return res
            # Create a named TypedDictType
            return td.copy_modified(
                item_types=self.anal_array(list(td.items.values())), fallback=instance
            )

        if info.fullname == "types.NoneType":
            self.fail(
                "NoneType should not be used as a type, please use None instead",
                ctx,
                code=codes.NONETYPE_TYPE,
            )
            return NoneType(ctx.line, ctx.column)

        return instance

    def analyze_unbound_type_without_type_info(
        self, t: UnboundType, sym: SymbolTableNode, defining_literal: bool
    ) -> Type:
        """Figure out what an unbound type that doesn't refer to a TypeInfo node means.

        This is something unusual. We try our best to find out what it is.
        """
        # Native seam: Rust applies the ordered classification table over the
        # raw node facts below; Python rebuilds the result objects and
        # keeps the message tail. None (defer) falls back to Python.
        if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
            try:
                node = sym.node
                var_typ = None
                if isinstance(node, Var):
                    var_typ = get_proper_type(node.type)
                is_var_any = isinstance(var_typ, AnyType)
                result = _rust_analyze_unbound_without_info(
                    is_var_any,
                    self.allow_type_any,
                    not is_var_any
                    and isinstance(var_typ, Instance)
                    and var_typ.type.fullname == "builtins.type",
                    not is_var_any
                    and isinstance(var_typ, TypeType)
                    and isinstance(var_typ.item, AnyType),
                    isinstance(node, (TypeVarExpr, TypeVarTupleExpr))
                    and self.tvar_scope.get_binding(sym) is None,
                    self.allow_unbound_tvars,
                    isinstance(node, Var)
                    and node.info is not VAR_NO_INFO
                    and node.info.is_enum
                    and node.name in node.info.enum_members,
                    defining_literal,
                )
                if result == 1 and var_typ is not None:
                    return AnyType(
                        TypeOfAny.from_unimported_type,
                        missing_import_name=var_typ.missing_import_name,  # type: ignore[union-attr]
                    )
                if result == 2:
                    return AnyType(TypeOfAny.special_form)
                if result == 3:
                    return t
                if result == 4:
                    assert isinstance(node, Var)
                    return LiteralType(
                        value=node.name,
                        fallback=Instance(node.info, [], line=t.line, column=t.column),
                        line=t.line,
                        column=t.column,
                    )
            except (AssertionError, NotImplementedError):
                pass
        name = sym.fullname
        if name is None:
            assert sym.node is not None
            name = sym.node.name
        # Option 1:
        # Something with an Any type -- make it an alias for Any in a type
        # context. This is slightly problematic as it allows using the type 'Any'

        # as a base class -- however, this will fail soon at runtime so the problem
        # is pretty minor.
        if isinstance(sym.node, Var):
            typ = get_proper_type(sym.node.type)
            if isinstance(typ, AnyType):
                return AnyType(
                    TypeOfAny.from_unimported_type, missing_import_name=typ.missing_import_name
                )
            elif self.allow_type_any:
                if isinstance(typ, Instance) and typ.type.fullname == "builtins.type":
                    return AnyType(TypeOfAny.special_form)
                if isinstance(typ, TypeType) and isinstance(typ.item, AnyType):
                    return AnyType(TypeOfAny.from_another_any, source_any=typ.item)
        # Option 2:
        # Unbound type variable. Currently these may be still valid,
        # for example when defining a generic type alias.
        unbound_tvar = (
            isinstance(sym.node, (TypeVarExpr, TypeVarTupleExpr))
            and self.tvar_scope.get_binding(sym) is None
        )
        if self.allow_unbound_tvars and unbound_tvar:
            return t

        # Option 3:
        # Enum value. Note: we only want to return a LiteralType when
        # we're using this enum value specifically within context of

        # a "Literal[...]" type. So, if `defining_literal` is not set,
        # we bail out early with an error.

        # If, in the distant future, we decide to permit things like
        # `def foo(x: Color.RED) -> None: ...`, we can remove that
        # check entirely.
        if (
            isinstance(sym.node, Var)
            and sym.node.info
            and sym.node.info.is_enum
            and sym.node.name in sym.node.info.enum_members
        ):
            value = sym.node.name
            base_enum_short_name = sym.node.info.name
            if not defining_literal:
                msg = message_registry.INVALID_TYPE_RAW_ENUM_VALUE.format(
                    base_enum_short_name, value
                )
                self.fail(msg.value, t, code=msg.code)
                return AnyType(TypeOfAny.from_error)
            return LiteralType(
                value=value,
                fallback=Instance(sym.node.info, [], line=t.line, column=t.column),
                line=t.line,
                column=t.column,
            )

        # None of the above options worked. We parse the args (if there are any)
        # to make sure there are no remaining semanal-only types, then give up.
        t = t.copy_modified(args=self.anal_array(t.args))
        # TODO: Move this message building logic to messages.py.
        notes: list[str] = []
        error_code = codes.VALID_TYPE
        if isinstance(sym.node, Var):
            notes.append(
                "See https://mypy.readthedocs.io/en/"
                "stable/common_issues.html#variables-vs-type-aliases"
            )
            message = 'Variable "{}" is not valid as a type'
        elif isinstance(sym.node, (SYMBOL_FUNCBASE_TYPES, Decorator)):
            message = 'Function "{}" is not valid as a type'
            if name == "builtins.any":
                notes.append('Perhaps you meant "typing.Any" instead of "any"?')
            elif name == "builtins.callable":
                notes.append('Perhaps you meant "typing.Callable" instead of "callable"?')
            else:
                notes.append('Perhaps you need "Callable[...]" or a callback protocol?')
        elif isinstance(sym.node, MypyFile):
            message = 'Module "{}" is not valid as a type'
            notes.append("Perhaps you meant to use a protocol matching the module structure?")
        elif unbound_tvar:
            assert isinstance(sym.node, TypeVarLikeExpr)
            if sym.node.is_new_style:
                # PEP 695 type parameters are never considered unbound -- they are undefined
                # in contexts where they aren't valid, such as in argument default values.
                message = 'Name "{}" is not defined'
                name = name.split(".")[-1]
                error_code = codes.NAME_DEFINED
            else:
                message = 'Type variable "{}" is unbound'
                short = name.split(".")[-1]
                notes.append(
                    f'(Hint: Use "Generic[{short}]" or "Protocol[{short}]" base class'
                    f' to bind "{short}" inside a class)'
                )
                notes.append(
                    f'(Hint: Use "{short}" in function signature '
                    f'to bind "{short}" inside a function)'
                )
        else:
            message = 'Cannot interpret reference "{}" as a type'
        if not defining_literal:
            # Literal check already gives a custom error. Avoid duplicating errors.
            self.fail(message.format(name), t, code=error_code)
            for note in notes:
                self.note(note, t, code=error_code)

        # TODO: Would it be better to always return Any instead of UnboundType
        # in case of an error? On one hand, UnboundType has a name so error messages
        # are more detailed, on the other hand, some of them may be bogus,

        # see https://github.com/python/mypy/issues/4987.
        return t

    def visit_any(self, t: AnyType) -> Type:
        return t

    def visit_none_type(self, t: NoneType) -> Type:
        return t

    def visit_uninhabited_type(self, t: UninhabitedType) -> Type:
        return t

    def visit_erased_type(self, t: ErasedType) -> Type:
        # This type should exist only temporarily during type inference
        assert False, "Internal error: Unexpected erased type"

    def visit_deleted_type(self, t: DeletedType) -> Type:
        return t

    def visit_type_list(self, t: TypeList) -> Type:
        # Parameters literal (Z[[int, str, Whatever]])
        if self.allow_param_spec_literals:
            params = self.analyze_callable_args(t)
            if params:
                ts, kinds, names = params
                # bind these types
                return Parameters(self.anal_array(ts), kinds, names, line=t.line, column=t.column)
            else:
                return AnyType(TypeOfAny.from_error)
        else:
            self.fail(
                'Bracketed expression "[...]" is not valid as a type', t, code=codes.VALID_TYPE
            )
            if len(t.items) == 1:
                self.note('Did you mean "List[...]"?', t)
            return AnyType(TypeOfAny.from_error)

    def visit_callable_argument(self, t: CallableArgument) -> Type:
        self.fail("Invalid type", t, code=codes.VALID_TYPE)
        return AnyType(TypeOfAny.from_error)

    def visit_instance(self, t: Instance) -> Type:
        return t

    def visit_type_alias_type(self, t: TypeAliasType) -> Type:
        # TODO: should we do something here?
        return t

    def visit_type_var(self, t: TypeVarType) -> Type:
        return t

    def visit_param_spec(self, t: ParamSpecType) -> Type:
        return t

    def visit_type_var_tuple(self, t: TypeVarTupleType) -> Type:
        return t

    def visit_unpack_type(self, t: UnpackType) -> Type:
        if not self.allow_unpack:
            self.fail(message_registry.INVALID_UNPACK_POSITION, t.type, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)
        self.allow_type_var_tuple = self.nesting_level + 1
        result = UnpackType(self.anal_type(t.type), from_star_syntax=t.from_star_syntax)
        self.allow_type_var_tuple = -1
        return result

    def visit_parameters(self, t: Parameters) -> Type:
        raise NotImplementedError("ParamSpec literals cannot have unbound TypeVars")

    def visit_callable_type(
        self, t: CallableType, nested: bool = True, namespace: str = ""
    ) -> Type:
        # Every Callable can bind its own type variables, if they're not in the outer scope
        # TODO: attach namespace for nested free type variables (these appear in return
        # type only).
        with self.tvar_scope_frame(namespace=namespace):
            unpacked_kwargs = t.unpack_kwargs
            if self.defining_alias:
                variables = t.variables
            else:
                variables, _ = self.bind_function_type_variables(t, t)
            type_guard = self.anal_type_guard(t.ret_type) if t.type_guard is None else t.type_guard
            type_is = self.anal_type_is(t.ret_type) if t.type_is is None else t.type_is

            arg_kinds = t.arg_kinds
            arg_types = []
            param_spec_with_args = param_spec_with_kwargs = None
            param_spec_invalid = False
            for kind, ut in zip(arg_kinds, t.arg_types):
                if kind == ARG_STAR:
                    param_spec_with_args, at = self.anal_star_arg_type(ut, kind, nested=nested)
                elif kind == ARG_STAR2:
                    param_spec_with_kwargs, at = self.anal_star_arg_type(ut, kind, nested=nested)
                else:
                    if param_spec_with_args:
                        param_spec_invalid = True
                        self.fail(
                            "Arguments not allowed after ParamSpec.args", t, code=codes.VALID_TYPE
                        )
                    at = self.anal_type(ut, nested=nested, allow_unpack=False)
                arg_types.append(at)

            if nested and arg_types:
                # If we've got a Callable[[Unpack[SomeTypedDict]], None], make sure
                # Unpack is interpreted as `**` and not as `*`.
                last = arg_types[-1]
                if isinstance(last, UnpackType):
                    # TODO: it would be better to avoid this get_proper_type() call.
                    p_at = get_proper_type(last.type)
                    if isinstance(p_at, TypedDictType) and not last.from_star_syntax:
                        # Automatically detect Unpack[Foo] in Callable as backwards
                        # compatible syntax for **Foo, if Foo is a TypedDict.
                        arg_kinds[-1] = ARG_STAR2
                        arg_types[-1] = p_at
                        unpacked_kwargs = True
                arg_types = self.check_unpacks_in_list(arg_types)

            if not param_spec_invalid and param_spec_with_args != param_spec_with_kwargs:
                # If already invalid, do not report more errors - definition has
                # to be fixed anyway
                name = param_spec_with_args or param_spec_with_kwargs
                self.fail(
                    f'ParamSpec must have "*args" typed as "{name}.args" and "**kwargs" typed as "{name}.kwargs"',
                    t,
                    code=codes.VALID_TYPE,
                )
                param_spec_invalid = True

            if param_spec_invalid:
                if ARG_STAR in arg_kinds:
                    arg_types[arg_kinds.index(ARG_STAR)] = AnyType(TypeOfAny.from_error)
                if ARG_STAR2 in arg_kinds:
                    arg_types[arg_kinds.index(ARG_STAR2)] = AnyType(TypeOfAny.from_error)

            # If there were multiple (invalid) unpacks, the arg types list will become shorter,
            # we need to trim the kinds/names as well to avoid crashes.
            arg_kinds = t.arg_kinds[: len(arg_types)]
            arg_names = t.arg_names[: len(arg_types)]

            ret = t.copy_modified(
                arg_types=arg_types,
                arg_kinds=arg_kinds,
                arg_names=arg_names,
                ret_type=self.anal_type(t.ret_type, nested=nested),
                # If the fallback isn't filled in yet,
                # its type will be the falsey FakeInfo
                fallback=(t.fallback if t.fallback.type else self.named_type("builtins.function")),
                variables=self.anal_var_defs(variables),
                type_guard=type_guard,
                type_is=type_is,
                unpack_kwargs=unpacked_kwargs,
            )
        return ret

    def anal_type_guard(self, t: Type) -> Type | None:
        if isinstance(t, UnboundType):
            sym = self.lookup_qualified(t.name, t)
            if sym is not None and sym.node is not None:
                return self.anal_type_guard_arg(t, sym.node.fullname)
        # TODO: What if it's an Instance? Then use t.type.fullname?
        return None

    def anal_type_guard_arg(self, t: UnboundType, fullname: str) -> Type | None:
        if fullname in ("typing_extensions.TypeGuard", "typing.TypeGuard"):
            if len(t.args) != 1:
                self.fail(
                    "TypeGuard must have exactly one type argument", t, code=codes.VALID_TYPE
                )
                return AnyType(TypeOfAny.from_error)
            return self.anal_type(t.args[0])
        return None

    def anal_type_is(self, t: Type) -> Type | None:
        if isinstance(t, UnboundType):
            sym = self.lookup_qualified(t.name, t)
            if sym is not None and sym.node is not None:
                return self.anal_type_is_arg(t, sym.node.fullname)
        # TODO: What if it's an Instance? Then use t.type.fullname?
        return None

    def anal_type_is_arg(self, t: UnboundType, fullname: str) -> Type | None:
        if fullname in ("typing_extensions.TypeIs", "typing.TypeIs"):
            if len(t.args) != 1:
                self.fail("TypeIs must have exactly one type argument", t, code=codes.VALID_TYPE)
                return AnyType(TypeOfAny.from_error)
            return self.anal_type(t.args[0])
        return None

    def anal_star_arg_type(self, t: Type, kind: ArgKind, nested: bool) -> tuple[str | None, Type]:
        """Analyze signature argument type for *args and **kwargs argument."""
        if isinstance(t, UnboundType) and t.name and "." in t.name and not t.args:
            components = t.name.split(".")
            tvar_name = ".".join(components[:-1])
            sym = self.lookup_qualified(tvar_name, t)
            if sym is not None and isinstance(sym.node, ParamSpecExpr):
                tvar_def = self.tvar_scope.get_binding(sym)
                if isinstance(tvar_def, ParamSpecType):
                    if kind == ARG_STAR:
                        make_paramspec = paramspec_args
                        if components[-1] != "args":
                            self.fail(
                                f'Use "{tvar_name}.args" for variadic "*" parameter',
                                t,
                                code=codes.VALID_TYPE,
                            )
                    elif kind == ARG_STAR2:
                        make_paramspec = paramspec_kwargs
                        if components[-1] != "kwargs":
                            self.fail(
                                f'Use "{tvar_name}.kwargs" for variadic "**" parameter',
                                t,
                                code=codes.VALID_TYPE,
                            )
                    else:
                        assert False, kind
                    return tvar_name, make_paramspec(
                        tvar_def.name,
                        tvar_def.fullname,
                        tvar_def.id,
                        named_type_func=self.named_type,
                        line=t.line,
                        column=t.column,
                    )
        return None, self.anal_type(t, nested=nested, allow_unpack=True)

    def visit_overloaded(self, t: Overloaded) -> Type:
        # Overloaded types are manually constructed in semanal.py by analyzing the
        # AST and combining together the Callable types this visitor converts.

        # So if we're ever asked to reanalyze an Overloaded type, we know it's
        # fine to just return it as-is.
        return t

    def visit_tuple_type(self, t: TupleType) -> Type:
        # Types such as (t1, t2, ...) only allowed in assignment statements. They'll
        # generate errors elsewhere, and Tuple[t1, t2, ...] must be used instead.
        tag = self._native_tuple_type_implicit_tag(t)
        if tag is None:
            if not (t.implicit and not self.allow_tuple_literal):
                tag = _TUPLE_TAG_OK
            elif len(t.items) == 0:
                tag = _TUPLE_TAG_EMPTY
            elif len(t.items) == 1:
                tag = _TUPLE_TAG_SINGLE
            else:
                tag = _TUPLE_TAG_MULTI
        if tag == _TUPLE_TAG_OK:
            any_type = AnyType(TypeOfAny.special_form)
            # If the fallback isn't filled in yet, its type will be the falsey FakeInfo
            fallback = (
                t.partial_fallback
                if t.partial_fallback.type
                else self.named_type("builtins.tuple", [any_type])
            )
            return TupleType(self.anal_array(t.items, allow_unpack=True), fallback, t.line)
        self.fail("Syntax error in type annotation", t, code=codes.SYNTAX)
        if tag == _TUPLE_TAG_EMPTY:
            self.note(
                "Suggestion: Use Tuple[()] instead of () for an empty tuple, or "
                "None for a function without a return value",
                t,
                code=codes.SYNTAX,
            )
        elif tag == _TUPLE_TAG_SINGLE:
            self.note("Suggestion: Is there a spurious trailing comma?", t, code=codes.SYNTAX)
        else:
            self.note(
                "Suggestion: Use Tuple[T1, ..., Tn] instead of (T1, ..., Tn)",
                t,
                code=codes.SYNTAX,
            )
        return AnyType(TypeOfAny.from_error)

    def _native_tuple_type_implicit_tag(self, t: TupleType) -> int | None:
        """Classify the `visit_tuple_type` implicit-tuple head in Rust.

        Returns the branch tag (OK/EMPTY/SINGLE/MULTI), or `None` to run
        the pure-Python arbitration. Rust owns the three-scalar decision
        (`t.implicit`, `allow_tuple_literal`, `len(t.items)`); the
        `self.fail` / `self.note` side effects and the named_type +
        anal_array reconstruction stay Python-side. See
        crates/type_kernel/src/typeanal_special.rs for the tag table.
        """
        if not (_TYPEANAL_HAS_KERNEL and _native_typeanal_active):
            return None
        try:
            return _rust_classify_tuple_type_implicit(
                t.implicit, self.allow_tuple_literal, len(t.items)
            )
        except (AssertionError, NotImplementedError):
            return None

    def visit_typeddict_type(self, t: TypedDictType) -> Type:
        req_keys = set()
        readonly_keys = set()
        items = {}
        for item_name, item_type in t.items.items():
            # TODO: rework
            analyzed = self.anal_type(item_type, allow_typed_dict_special_forms=True)
            if isinstance(analyzed, RequiredType):
                if analyzed.required:
                    req_keys.add(item_name)
                analyzed = analyzed.item
            else:
                # Keys are required by default.
                req_keys.add(item_name)
            if isinstance(analyzed, ReadOnlyType):
                readonly_keys.add(item_name)
                analyzed = analyzed.item
            items[item_name] = analyzed
        if t.fallback.type is MISSING_FALLBACK:  # anonymous/inline TypedDict
            if INLINE_TYPEDDICT not in self.options.enable_incomplete_feature:
                self.fail(
                    "Inline TypedDict is experimental,"
                    " must be enabled with --enable-incomplete-feature=InlineTypedDict",
                    t,
                )
            is_closed = False
            required_keys = req_keys
            fallback = self.named_type("typing._TypedDict")
            for typ in t.extra_items_from:
                analyzed = self.analyze_type(typ)
                p_analyzed = get_proper_type(analyzed)
                if not isinstance(p_analyzed, TypedDictType):
                    if not isinstance(p_analyzed, (AnyType, PlaceholderType)):
                        self.fail("Can only merge-in other TypedDict", t, code=codes.VALID_TYPE)
                    continue
                for sub_item_name, sub_item_type in p_analyzed.items.items():
                    if sub_item_name in items:
                        self.fail(TYPEDDICT_OVERRIDE_MERGE.format(sub_item_name), t)
                        continue
                    items[sub_item_name] = sub_item_type
                    if sub_item_name in p_analyzed.required_keys:
                        req_keys.add(sub_item_name)
                    if sub_item_name in p_analyzed.readonly_keys:
                        readonly_keys.add(sub_item_name)
        else:
            readonly_keys = t.readonly_keys
            required_keys = t.required_keys
            fallback = t.fallback
            is_closed = t.is_closed
        return TypedDictType(
            items, required_keys, readonly_keys, fallback, t.line, t.column, is_closed=is_closed
        )

    def visit_raw_expression_type(self, t: RawExpressionType) -> Type:
        # We should never see a bare Literal. We synthesize these raw literals
        # in the earlier stages of semantic analysis, but those
        # "fake literals" should always be wrapped in an UnboundType

        # corresponding to 'Literal'.

        # Note: if at some point in the distant future, we decide to
        # make signatures like "foo(x: 20) -> None" legal, we can change
        # this method so it generates and returns an actual LiteralType

        # instead.

        if self.report_invalid_types:
            msg = self._native_raw_expression_message(t)
            if msg is None:
                if t.base_type_name in ("builtins.int", "builtins.bool"):
                    msg = f"Invalid type: try using Literal[{repr(t.literal_value)}] instead?"
                elif t.base_type_name in ("builtins.float", "builtins.complex"):
                    msg = f"Invalid type: {t.simple_name()} literals cannot be used as a type"
                else:
                    msg = "Invalid type comment or annotation"
            self.fail(msg, t, code=codes.VALID_TYPE)
            if t.note is not None:
                self.note(t.note, t, code=codes.VALID_TYPE)

        return AnyType(TypeOfAny.from_error, line=t.line, column=t.column)

    def _native_raw_expression_message(self, t: RawExpressionType) -> str | None:
        """Classify the `visit_raw_expression_type` message head in Rust.

        Returns the formatted fail message, or `None` to run the pure-Python
        `if`/`elif` chain. Rust owns only the 3-way set-membership branch
        (int/bool, float/complex, else); `self.fail` / `self.note` side
        effects and the trailing `AnyType` stay Python-side. See
        crates/type_kernel/src/typeanal_rawexpr.rs for the tag table.
        """
        if not (_TYPEANAL_HAS_KERNEL and _native_typeanal_active):
            return None
        try:
            tag = _rust_classify_raw_expression_type(
                self.report_invalid_types,
                t.base_type_name,
                t.note is None,
            )
        except (AssertionError, NotImplementedError):
            return None
        if tag is None:
            return None
        if tag == _RAW_EXPR_TAG_LITERAL:
            return f"Invalid type: try using Literal[{repr(t.literal_value)}] instead?"
        if tag == _RAW_EXPR_TAG_NUMERIC_LITERALS:
            return f"Invalid type: {t.simple_name()} literals cannot be used as a type"
        return "Invalid type comment or annotation"

    def visit_literal_type(self, t: LiteralType) -> Type:
        return t

    def visit_union_type(self, t: UnionType) -> Type:
        return UnionType(self.anal_array(t.items), t.line, uses_pep604_syntax=t.uses_pep604_syntax)

    def visit_partial_type(self, t: PartialType) -> Type:
        assert False, "Internal error: Unexpected partial type"

    def visit_ellipsis_type(self, t: EllipsisType) -> Type:
        if self.allow_ellipsis or self.allow_param_spec_literals:
            any_type = AnyType(TypeOfAny.explicit)
            return Parameters(
                [any_type, any_type], [ARG_STAR, ARG_STAR2], [None, None], is_ellipsis_args=True
            )
        else:
            self.fail('Unexpected "..."', t)
            return AnyType(TypeOfAny.from_error)

    def visit_type_type(self, t: TypeType) -> Type:
        return TypeType.make_normalized(
            self.anal_type(t.item), line=t.line, is_type_form=t.is_type_form
        )

    def visit_placeholder_type(self, t: PlaceholderType) -> Type:
        n = (
            None
            # No dot in fullname indicates we are at function scope, and recursive
            # types are not supported there anyway, so we just give up.
            if not t.fullname or "." not in t.fullname
            else self.api.lookup_fully_qualified(t.fullname)
        )
        if not n or isinstance(n.node, PlaceholderNode):
            self.api.defer()  # Still incomplete
            return t
        else:
            # TODO: Handle non-TypeInfo
            assert isinstance(n.node, TypeInfo)
            return self.analyze_type_with_type_info(n.node, t.args, t, False)

    def analyze_callable_args_for_paramspec(
        self, callable_args: Type, ret_type: Type, fallback: Instance
    ) -> CallableType | None:
        """Construct a 'Callable[P, RET]', where P is ParamSpec, return None if we cannot."""
        if not isinstance(callable_args, UnboundType):
            return None
        sym = self.lookup_qualified(callable_args.name, callable_args)
        if sym is None:
            return None
        tvar_def = self.tvar_scope.get_binding(sym)
        if not isinstance(tvar_def, ParamSpecType):
            if (
                tvar_def is None
                and self.allow_unbound_tvars
                and isinstance(sym.node, ParamSpecExpr)
            ):
                # We are analyzing this type in runtime context (e.g. as type application).
                # If it is not valid as a type in this position an error will be given later.
                return callable_with_ellipsis(
                    AnyType(TypeOfAny.explicit), ret_type=ret_type, fallback=fallback
                )
            return None
        elif (
            self.defining_alias
            and self.not_declared_in_type_params(tvar_def.name)
            and tvar_def not in self.allowed_alias_tvars
        ):
            if self.python_3_12_type_alias:
                msg = message_registry.TYPE_PARAMETERS_SHOULD_BE_DECLARED.format(
                    f'"{tvar_def.name}"'
                )
            else:
                msg = f'ParamSpec "{tvar_def.name}" is not included in type_params'
            self.fail(msg, callable_args, code=codes.VALID_TYPE)
            return callable_with_ellipsis(
                AnyType(TypeOfAny.special_form), ret_type=ret_type, fallback=fallback
            )

        return CallableType(
            [
                paramspec_args(
                    tvar_def.name, tvar_def.fullname, tvar_def.id, named_type_func=self.named_type
                ),
                paramspec_kwargs(
                    tvar_def.name, tvar_def.fullname, tvar_def.id, named_type_func=self.named_type
                ),
            ],
            [nodes.ARG_STAR, nodes.ARG_STAR2],
            [None, None],
            ret_type=ret_type,
            fallback=fallback,
        )

    def analyze_callable_args_for_concatenate(
        self, callable_args: Type, ret_type: Type, fallback: Instance
    ) -> CallableType | AnyType | None:
        """Construct a 'Callable[C, RET]', where C is Concatenate[..., P], returning None if we
        cannot.
        """
        if not isinstance(callable_args, UnboundType):
            return None
        sym = self.lookup_qualified(callable_args.name, callable_args)
        if sym is None:
            return None
        if sym.node is None:
            return None
        if sym.node.fullname not in CONCATENATE_TYPE_NAMES:
            return None

        tvar_def = self.anal_type(callable_args, allow_param_spec=True)
        if not isinstance(tvar_def, (ParamSpecType, Parameters)):
            if self.allow_unbound_tvars and isinstance(tvar_def, UnboundType):
                sym = self.lookup_qualified(tvar_def.name, callable_args)
                if sym is not None and isinstance(sym.node, ParamSpecExpr):
                    # We are analyzing this type in runtime context (e.g. as type application).
                    # If it is not valid as a type in this position an error will be given later.
                    return callable_with_ellipsis(
                        AnyType(TypeOfAny.explicit), ret_type=ret_type, fallback=fallback
                    )
            # Error was already given, so prevent further errors.
            return AnyType(TypeOfAny.from_error)
        if isinstance(tvar_def, Parameters):
            # This comes from Concatenate[int, ...]
            return CallableType(
                arg_types=tvar_def.arg_types,
                arg_names=tvar_def.arg_names,
                arg_kinds=tvar_def.arg_kinds,
                ret_type=ret_type,
                fallback=fallback,
                from_concatenate=True,
            )

        # ick, CallableType should take ParamSpecType
        prefix = tvar_def.prefix
        # we don't set the prefix here as generic arguments will get updated at some point
        # in the future. CallableType.param_spec() accounts for this.
        return CallableType(
            [
                *prefix.arg_types,
                paramspec_args(
                    tvar_def.name, tvar_def.fullname, tvar_def.id, named_type_func=self.named_type
                ),
                paramspec_kwargs(
                    tvar_def.name, tvar_def.fullname, tvar_def.id, named_type_func=self.named_type
                ),
            ],
            [*prefix.arg_kinds, nodes.ARG_STAR, nodes.ARG_STAR2],
            [*prefix.arg_names, None, None],
            ret_type=ret_type,
            fallback=fallback,
            from_concatenate=True,
        )

    def analyze_callable_type(self, t: UnboundType) -> Type:
        tag = self._native_callable_type_tag(t)
        if tag is not None:
            return self._apply_callable_type_tag(tag, t)
        fallback = self.named_type("builtins.function")
        if len(t.args) == 0:
            # Callable (bare). Treat as Callable[..., Any].
            any_type = self.get_omitted_any(t)
            ret = callable_with_ellipsis(any_type, any_type, fallback)
        elif len(t.args) == 2:
            callable_args = t.args[0]
            ret_type = t.args[1]
            if isinstance(callable_args, TypeList):
                # Callable[[ARG, ...], RET] (ordinary callable type)
                analyzed_args = self.analyze_callable_args(callable_args)
                if analyzed_args is None:
                    return AnyType(TypeOfAny.from_error)
                args, kinds, names = analyzed_args
                ret = CallableType(args, kinds, names, ret_type=ret_type, fallback=fallback)
            elif isinstance(callable_args, EllipsisType):
                # Callable[..., RET] (with literal ellipsis; accept arbitrary arguments)
                ret = callable_with_ellipsis(
                    AnyType(TypeOfAny.explicit), ret_type=ret_type, fallback=fallback
                )
            else:
                # Callable[P, RET] (where P is ParamSpec)
                with self.tvar_scope_frame(namespace=""):
                    # Temporarily bind ParamSpecs to allow code like this:
                    #     my_fun: Callable[Q, Foo[Q]]

                    # We usually do this later in visit_callable_type(), but the analysis
                    # below happens at very early stage.
                    variables = []
                    for name, tvar_expr in self.find_type_var_likes(callable_args):
                        variables.append(
                            self.tvar_scope.bind_new(name, tvar_expr, self.fail_func, t)
                        )
                    maybe_ret = self.analyze_callable_args_for_paramspec(
                        callable_args, ret_type, fallback
                    ) or self.analyze_callable_args_for_concatenate(
                        callable_args, ret_type, fallback
                    )
                    if isinstance(maybe_ret, CallableType):
                        maybe_ret = maybe_ret.copy_modified(variables=variables)
                if maybe_ret is None:
                    # Callable[?, RET] (where ? is something invalid)
                    self.fail(
                        "The first argument to Callable must be a "
                        'list of types, parameter specification, or "..."',
                        t,
                        code=codes.VALID_TYPE,
                    )
                    self.note(
                        "See https://mypy.readthedocs.io/en/stable/kinds_of_types.html#callable-types-and-lambdas",
                        t,
                    )
                    return AnyType(TypeOfAny.from_error)
                elif isinstance(maybe_ret, AnyType):
                    return maybe_ret
                ret = maybe_ret
        else:
            if self.options.disallow_any_generics:
                self.fail('Please use "Callable[[<parameters>], <return type>]"', t)
            else:
                self.fail('Please use "Callable[[<parameters>], <return type>]" or "Callable"', t)
            return AnyType(TypeOfAny.from_error)
        assert isinstance(ret, CallableType)
        return ret.accept(self)

    def _native_callable_type_tag(self, t: UnboundType) -> int | None:
        """Classify the two-level dispatch head of `analyze_callable_type`.

        Returns a branch tag matching typeanal_callable.rs, or `None` to run
        the pure-Python body. Rust owns the whole decision table from four
        scalar facts (`len(t.args)`, `arg0` is `TypeList`, `arg0` is
        `EllipsisType`, `options.disallow_any_generics`); no live objects
        cross the seam.
        """
        if not (_TYPEANAL_HAS_KERNEL and _native_typeanal_active):
            return None
        arg_count = len(t.args)
        arg0_is_type_list = arg_count == 2 and isinstance(t.args[0], TypeList)
        arg0_is_ellipsis = arg_count == 2 and isinstance(t.args[0], EllipsisType)
        try:
            return _rust_classify_analyze_callable_type(
                arg_count,
                arg0_is_type_list,
                arg0_is_ellipsis,
                self.options.disallow_any_generics,
            )
        except (AssertionError, NotImplementedError):
            return None

    def _apply_callable_type_tag(self, tag: int, t: UnboundType) -> Type:
        """Apply the side effects for the branch `tag` Rust returned.

        Mirrors typeanal_callable.rs: each tag maps to one terminal branch of
        `analyze_callable_type`; the object construction, `tvar_scope` entry,
        and error emission stay Python-side.
        """
        fallback = self.named_type("builtins.function")
        if tag == _CALLABLE_TAG_BARE:
            # Callable (bare). Treat as Callable[..., Any].
            any_type = self.get_omitted_any(t)
            ret = callable_with_ellipsis(any_type, any_type, fallback)
        elif tag == _CALLABLE_TAG_TYPE_LIST:
            # Callable[[ARG, ...], RET] (ordinary callable type).
            analyzed_args = self.analyze_callable_args(t.args[0])
            if analyzed_args is None:
                return AnyType(TypeOfAny.from_error)
            args, kinds, names = analyzed_args
            ret = CallableType(args, kinds, names, ret_type=t.args[1], fallback=fallback)
        elif tag == _CALLABLE_TAG_ELLIPSIS:
            # Callable[..., RET] (with literal ellipsis; accept arbitrary arguments).
            ret = callable_with_ellipsis(
                AnyType(TypeOfAny.explicit), ret_type=t.args[1], fallback=fallback
            )
        elif tag == _CALLABLE_TAG_PARAMSPEC:
            callable_args = t.args[0]
            ret_type = t.args[1]
            # Callable[P, RET] (where P is ParamSpec).
            with self.tvar_scope_frame(namespace=""):
                # Temporarily bind ParamSpecs to allow code like this:
                #     my_fun: Callable[Q, Foo[Q]]
                # We usually do this in visit_callable_type(), but this is early.
                variables = []
                for name, tvar_expr in self.find_type_var_likes(callable_args):
                    variables.append(
                        self.tvar_scope.bind_new(name, tvar_expr, self.fail_func, t)
                    )
                maybe_ret = self.analyze_callable_args_for_paramspec(
                    callable_args, ret_type, fallback
                ) or self.analyze_callable_args_for_concatenate(
                    callable_args, ret_type, fallback
                )
                if isinstance(maybe_ret, CallableType):
                    maybe_ret = maybe_ret.copy_modified(variables=variables)
            if maybe_ret is None:
                # Callable[?, RET] (where ? is something invalid).
                self.fail(
                    "The first argument to Callable must be a "
                    'list of types, parameter specification, or "..."',
                    t,
                    code=codes.VALID_TYPE,
                )
                self.note(
                    "See https://mypy.readthedocs.io/en/stable/kinds_of_types.html#callable-types-and-lambdas",
                    t,
                )
                return AnyType(TypeOfAny.from_error)
            elif isinstance(maybe_ret, AnyType):
                return maybe_ret
            ret = maybe_ret
        elif tag == _CALLABLE_TAG_INVALID_DISALLOW:
            self.fail('Please use "Callable[[<parameters>], <return type>]"', t)
            return AnyType(TypeOfAny.from_error)
        else:
            assert tag == _CALLABLE_TAG_INVALID_ALLOW
            self.fail('Please use "Callable[[<parameters>], <return type>]" or "Callable"', t)
            return AnyType(TypeOfAny.from_error)
        assert isinstance(ret, CallableType)
        return ret.accept(self)

    def refers_to_full_names(self, arg: UnboundType, names: Sequence[str]) -> bool:
        sym = self.lookup_qualified(arg.name, arg)
        if sym is not None:
            if sym.fullname in names:
                return True
        return False

    def analyze_callable_args(
        self, arglist: TypeList
    ) -> tuple[list[Type], list[ArgKind], list[str | None]] | None:
        args: list[Type] = []
        kinds: list[ArgKind] = []
        names: list[str | None] = []
        seen_unpack = False
        unpack_types: list[Type] = []
        invalid_unpacks: list[Type] = []
        second_unpack_last = False
        for i, arg in enumerate(arglist.items):
            if isinstance(arg, CallableArgument):
                args.append(arg.typ)
                names.append(arg.name)
                if arg.constructor is None:
                    return None
                found = self.lookup_qualified(arg.constructor, arg)
                if found is None:
                    # Looking it up already put an error message in
                    return None
                elif found.fullname not in ARG_KINDS_BY_CONSTRUCTOR:
                    self.fail(f'Invalid argument constructor "{found.fullname}"', arg)
                    return None
                else:
                    assert found.fullname is not None
                    kind = ARG_KINDS_BY_CONSTRUCTOR[found.fullname]
                    kinds.append(kind)
                    if arg.name is not None and kind.is_star():
                        self.fail(f"{arg.constructor} arguments should not have names", arg)
                        return None
            elif (
                isinstance(arg, UnboundType)
                and self.refers_to_full_names(arg, UNPACK_TYPE_NAMES)
                or isinstance(arg, UnpackType)
            ):
                if seen_unpack:
                    # Multiple unpacks, preserve them, so we can give an error later.
                    if i == len(arglist.items) - 1 and not invalid_unpacks:
                        # Special case: if there are just two unpacks, and the second one appears
                        # as last type argument, it can be still valid, if the second unpacked type
                        # is a TypedDict. This should be checked by the caller.
                        second_unpack_last = True
                    invalid_unpacks.append(arg)
                    continue
                seen_unpack = True
                unpack_types.append(arg)
            else:
                if seen_unpack:
                    unpack_types.append(arg)
                else:
                    args.append(arg)
                    kinds.append(ARG_POS)
                    names.append(None)
        if seen_unpack:
            if len(unpack_types) == 1:
                args.append(unpack_types[0])
            else:
                first = unpack_types[0]
                if isinstance(first, UnpackType):
                    # UnpackType doesn't have its own line/column numbers,
                    # so use the unpacked type for error messages.
                    first = first.type
                args.append(
                    UnpackType(self.tuple_type(unpack_types, line=first.line, column=first.column))
                )
            kinds.append(ARG_STAR)
            names.append(None)
        for arg in invalid_unpacks:
            args.append(arg)
            kinds.append(ARG_STAR2 if second_unpack_last else ARG_STAR)
            names.append(None)
        # Note that arglist below is only used for error context.
        check_param_names(names, [arglist] * len(args), self.fail, "Callable")
        check_arg_kinds(kinds, [arglist] * len(args), self.fail)
        return args, kinds, names

    def analyze_literal_type(self, t: UnboundType) -> Type:
        if len(t.args) == 0:
            self.fail("Literal[...] must have at least one parameter", t, code=codes.VALID_TYPE)
            return AnyType(TypeOfAny.from_error)

        output: list[Type] = []
        for i, arg in enumerate(t.args):
            analyzed_types = self.analyze_literal_param(i + 1, arg, t)
            if analyzed_types is None:
                return AnyType(TypeOfAny.from_error)
            else:
                output.extend(analyzed_types)
        return UnionType.make_union(output, line=t.line)

    def analyze_literal_param(self, idx: int, arg: Type, ctx: Context) -> list[Type] | None:
        if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
            return self._native_analyze_literal_param(idx, arg, ctx)
        return self._python_analyze_literal_param(idx, arg, ctx)

    def _native_analyze_literal_param(
        self, idx: int, arg: Type, ctx: Context
    ) -> list[Type] | None:
        # Phase 1: branch (a) — string-Literal from original_str_expr,
        # checked on the original arg before visit_unbound_type recursion.
        is_proper = isinstance(arg, ProperType)
        is_unbound_pre = isinstance(arg, UnboundType)
        is_union_pre = isinstance(arg, UnionType)
        if is_proper and (is_unbound_pre or is_union_pre):
            orig_str_not_none = arg.original_str_expr is not None
        else:
            orig_str_not_none = False
        if orig_str_not_none:
            tag = _rust_classify_literal_param(
                is_proper, is_unbound_pre, is_union_pre, True,
                False, 0, False, True, "", False, False, False, True, False,
            )
            if tag == _LITERAL_PARAM_TAG_STR:
                assert arg.original_str_fallback is not None
                return [
                    LiteralType(
                        value=arg.original_str_expr,
                        fallback=self.named_type(arg.original_str_fallback),
                        line=arg.line,
                        column=arg.column,
                    )
                ]
        # Phase 2: branch (b) — unbound recursion, then get_proper_type.
        if isinstance(arg, UnboundType):
            self.nesting_level += 1
            try:
                arg = self.visit_unbound_type(arg, defining_literal=True)
            finally:
                self.nesting_level -= 1
        arg = get_proper_type(arg)
        # Extract post-chain facts and classify branches (c)-(i).
        is_any = isinstance(arg, AnyType)
        type_of_any = arg.type_of_any if is_any else 0
        is_raw_expr = isinstance(arg, RawExpressionType)
        if is_raw_expr:
            literal_value_is_none = arg.literal_value is None
            simple_name = arg.simple_name() if literal_value_is_none else ""
        else:
            literal_value_is_none = True
            simple_name = ""
        is_none_type = isinstance(arg, NoneType)
        is_literal = isinstance(arg, LiteralType)
        is_instance = isinstance(arg, Instance)
        last_known_value_is_none = (
            not is_instance or arg.last_known_value is None
        )
        is_union_post = isinstance(arg, UnionType)
        tag = _rust_classify_literal_param(
            False, False, False, False,
            is_any, type_of_any, is_raw_expr, literal_value_is_none,
            simple_name, is_none_type, is_literal, is_instance,
            last_known_value_is_none, is_union_post,
        )
        return self._apply_literal_param_tag(tag, idx, arg, ctx)

    def _apply_literal_param_tag(
        self, tag: int, idx: int, arg: Type, ctx: Context
    ) -> list[Type] | None:
        if tag == _LITERAL_PARAM_TAG_ANY_FAIL:
            self.fail(
                f'Parameter {idx} of Literal[...] cannot be of type "Any"',
                ctx, code=codes.VALID_TYPE,
            )
            return None
        if tag == _LITERAL_PARAM_TAG_ANY_SILENT:
            return None
        if tag == _LITERAL_PARAM_TAG_RAW_FLOAT_COMPLEX:
            name = arg.simple_name()
            self.fail(
                f'Parameter {idx} of Literal[...] cannot be of type "{name}"',
                ctx, code=codes.VALID_TYPE,
            )
            return None
        if tag == _LITERAL_PARAM_TAG_RAW_ARBITRARY:
            self.fail(
                "Invalid type: Literal[...] cannot contain arbitrary expressions",
                ctx, code=codes.VALID_TYPE,
            )
            return None
        if tag == _LITERAL_PARAM_TAG_RAW_VALUE:
            fallback = self.named_type(arg.base_type_name)
            assert isinstance(fallback, Instance)
            return [
                LiteralType(
                    arg.literal_value, fallback,
                    line=arg.line, column=arg.column,
                )
            ]
        if tag == _LITERAL_PARAM_TAG_NONE_OR_LITERAL:
            return [arg]
        if tag == _LITERAL_PARAM_TAG_INSTANCE_LKV:
            return [arg.last_known_value]
        if tag == _LITERAL_PARAM_TAG_UNION_RECURSE:
            out = []
            for union_arg in arg.items:
                union_result = self.analyze_literal_param(idx, union_arg, ctx)
                if union_result is None:
                    return None
                out.extend(union_result)
            return out
        # _LITERAL_PARAM_TAG_INVALID
        self.fail(
            f"Parameter {idx} of Literal[...] is invalid",
            ctx, code=codes.VALID_TYPE,
        )
        return None

    def _python_analyze_literal_param(
        self, idx: int, arg: Type, ctx: Context
    ) -> list[Type] | None:
        # This UnboundType was originally defined as a string.
        if (
            isinstance(arg, ProperType)
            and isinstance(arg, (UnboundType, UnionType))
            and arg.original_str_expr is not None
        ):
            assert arg.original_str_fallback is not None
            return [
                LiteralType(
                    value=arg.original_str_expr,
                    fallback=self.named_type(arg.original_str_fallback),
                    line=arg.line,
                    column=arg.column,
                )
            ]

        # If arg is an UnboundType that was *not* originally defined as
        # a string, try expanding it in case it's a type alias or something.
        if isinstance(arg, UnboundType):
            self.nesting_level += 1
            try:
                arg = self.visit_unbound_type(arg, defining_literal=True)
            finally:
                self.nesting_level -= 1

        # Literal[...] cannot contain Any. Give up and add an error message
        # (if we haven't already).
        arg = get_proper_type(arg)
        if isinstance(arg, AnyType):
            # Note: We can encounter Literals containing 'Any' under three circumstances:

            # 1. If the user attempts use an explicit Any as a parameter
            # 2. If the user is trying to use an enum value imported from a module with
            #    no type hints, giving it an implicit type of 'Any'

            # 3. If there's some other underlying problem with the parameter.

            # We report an error in only the first two cases. In the third case, we assume
            # some other region of the code has already reported a more relevant error.

            # TODO: Once we start adding support for enums, make sure we report a custom
            # error for case 2 as well.
            if arg.type_of_any not in (TypeOfAny.from_error, TypeOfAny.special_form):
                self.fail(
                    f'Parameter {idx} of Literal[...] cannot be of type "Any"',
                    ctx,
                    code=codes.VALID_TYPE,
                )
            return None
        elif isinstance(arg, RawExpressionType):
            # A raw literal. Convert it directly into a literal if we can.
            if arg.literal_value is None:
                name = arg.simple_name()
                if name in ("float", "complex"):
                    msg = f'Parameter {idx} of Literal[...] cannot be of type "{name}"'
                else:
                    msg = "Invalid type: Literal[...] cannot contain arbitrary expressions"
                self.fail(msg, ctx, code=codes.VALID_TYPE)
                # Note: we deliberately ignore arg.note here: the extra info might normally be
                # helpful, but it generally won't make sense in the context of a Literal[...].
                return None

            # Remap bytes and unicode into the appropriate type for the correct Python version
            fallback = self.named_type(arg.base_type_name)
            assert isinstance(fallback, Instance)
            return [LiteralType(arg.literal_value, fallback, line=arg.line, column=arg.column)]
        elif isinstance(arg, (NoneType, LiteralType)):
            # Types that we can just add directly to the literal/potential union of literals.
            return [arg]
        elif isinstance(arg, Instance) and arg.last_known_value is not None:
            # Types generated from declarations like "var: Final = 4".
            return [arg.last_known_value]
        elif isinstance(arg, UnionType):
            out = []
            for union_arg in arg.items:
                union_result = self.analyze_literal_param(idx, union_arg, ctx)
                if union_result is None:
                    return None
                out.extend(union_result)
            return out
        else:
            self.fail(f"Parameter {idx} of Literal[...] is invalid", ctx, code=codes.VALID_TYPE)
            return None

    def analyze_type(self, typ: Type) -> Type:
        return typ.accept(self)

    def fail(self, msg: str, ctx: Context, *, code: ErrorCode | None = None) -> None:
        self.fail_func(msg, ctx, code=code)

    def note(self, msg: str, ctx: Context, *, code: ErrorCode | None = None) -> None:
        self.note_func(msg, ctx, code=code)

    @contextmanager
    def tvar_scope_frame(self, namespace: str) -> Iterator[None]:
        old_scope = self.tvar_scope
        self.tvar_scope = self.tvar_scope.method_frame(namespace)
        yield
        self.tvar_scope = old_scope

    def find_type_var_likes(self, t: Type) -> TypeVarLikeList:
        visitor = FindTypeVarVisitor(self.api, self.tvar_scope)
        t.accept(visitor)
        return visitor.type_var_likes

    def infer_type_variables(
        self, type: CallableType
    ) -> tuple[list[tuple[str, TypeVarLikeExpr]], bool]:
        """Infer type variables from a callable.

        Return tuple with these items:
         - list of unique type variables referred to in a callable
         - whether there is a reference to the Self type
        """
        visitor = FindTypeVarVisitor(self.api, self.tvar_scope)
        for arg in type.arg_types:
            arg.accept(visitor)

        # When finding type variables in the return type of a function, don't
        # look inside Callable types.  Type variables only appearing in
        # functions in the return type belong to those functions, not the

        # function we're currently analyzing.
        visitor.include_callables = False
        type.ret_type.accept(visitor)

        return visitor.type_var_likes, visitor.has_self_type

    def bind_function_type_variables(
        self, fun_type: CallableType, defn: Context
    ) -> tuple[tuple[TypeVarLikeType, ...], bool]:
        """Find the type variables of the function type and bind them in our tvar_scope"""
        has_self_type = False
        if fun_type.variables:
            defs = []
            for var in fun_type.variables:
                if self.api.type and self.api.type.self_type and var == self.api.type.self_type:
                    has_self_type = True
                    continue
                var_node = self.lookup_qualified(var.name, defn)
                assert var_node, "Binding for function type variable not found within function"
                var_expr = var_node.node
                assert isinstance(var_expr, TypeVarLikeExpr)
                binding = self.tvar_scope.bind_new(var.name, var_expr, self.fail_func, fun_type)
                defs.append(binding)
            return tuple(defs), has_self_type
        typevars, has_self_type = self.infer_type_variables(fun_type)
        # Do not define a new type variable if already defined in scope.
        typevars = [
            (name, tvar) for name, tvar in typevars if not self.is_defined_type_var(name, defn)
        ]
        defs = []
        for name, tvar in typevars:
            if not self.tvar_scope.allow_binding(tvar.fullname):
                err_msg = message_registry.TYPE_VAR_REDECLARED_IN_NESTED_CLASS.format(name)
                self.fail(err_msg.value, defn, code=err_msg.code)
            binding = self.tvar_scope.bind_new(name, tvar, self.fail_func, fun_type)
            defs.append(binding)

        return tuple(defs), has_self_type

    def is_defined_type_var(self, tvar: str, context: Context) -> bool:
        tvar_node = self.lookup_qualified(tvar, context)
        if not tvar_node:
            return False
        return self.tvar_scope.get_binding(tvar_node) is not None

    def anal_array(
        self,
        a: Iterable[Type],
        nested: bool = True,
        *,
        allow_param_spec: bool = False,
        allow_param_spec_literals: bool = False,
        allow_unpack: bool = False,
    ) -> list[Type]:
        old_allow_param_spec_literals = self.allow_param_spec_literals
        self.allow_param_spec_literals = allow_param_spec_literals
        res: list[Type] = []
        for t in a:
            res.append(
                self.anal_type(
                    t, nested, allow_param_spec=allow_param_spec, allow_unpack=allow_unpack
                )
            )
        self.allow_param_spec_literals = old_allow_param_spec_literals
        return self.check_unpacks_in_list(res)

    def anal_type(
        self,
        t: Type,
        nested: bool = True,
        *,
        allow_param_spec: bool = False,
        allow_unpack: bool = False,
        allow_ellipsis: bool = False,
        allow_typed_dict_special_forms: bool = False,
        allow_final: bool = False,
    ) -> Type:
        if nested:
            self.nesting_level += 1
        old_allow_typed_dict_special_forms = self.allow_typed_dict_special_forms
        self.allow_typed_dict_special_forms = allow_typed_dict_special_forms
        self.allow_final = allow_final
        old_allow_ellipsis = self.allow_ellipsis
        self.allow_ellipsis = allow_ellipsis
        old_allow_unpack = self.allow_unpack
        self.allow_unpack = allow_unpack
        try:
            # Strangler-fig: try the native (Rust) path first.
            # None means "Python must handle it" (deferred: UnboundType,
            # TypeAliasType, PlaceholderType, or types needing lookup/hook).
            analyzed = native_analyze_type(
                t,
                allow_tuple_literal=self.allow_tuple_literal,
                allow_param_spec_literals=self.allow_param_spec_literals,
                allow_unpack=self.allow_unpack,
            )
            if analyzed is not None:
                return analyzed
            analyzed = t.accept(self)
        finally:
            if nested:
                self.nesting_level -= 1
            self.allow_typed_dict_special_forms = old_allow_typed_dict_special_forms
            self.allow_ellipsis = old_allow_ellipsis
            self.allow_unpack = old_allow_unpack
        if (
            not allow_param_spec
            and isinstance(analyzed, ParamSpecType)
            and analyzed.flavor == ParamSpecFlavor.BARE
        ):
            if analyzed.prefix.arg_types:
                self.fail("Invalid location for Concatenate", t, code=codes.VALID_TYPE)
                self.note("You can use Concatenate as the first argument to Callable", t)
                analyzed = AnyType(TypeOfAny.from_error)
            else:
                self.fail(
                    INVALID_PARAM_SPEC_LOCATION.format(format_type(analyzed, self.options)),
                    t,
                    code=codes.VALID_TYPE,
                )
                self.note(
                    INVALID_PARAM_SPEC_LOCATION_NOTE.format(analyzed.name),
                    t,
                    code=codes.VALID_TYPE,
                )
                analyzed = AnyType(TypeOfAny.from_error)
        return analyzed

    def anal_var_def(self, var_def: TypeVarLikeType) -> TypeVarLikeType:
        if isinstance(var_def, TypeVarType):
            return TypeVarType(
                name=var_def.name,
                fullname=var_def.fullname,
                id=var_def.id,
                values=self.anal_array(var_def.values),
                upper_bound=var_def.upper_bound.accept(self),
                default=var_def.default.accept(self),
                variance=var_def.variance,
                line=var_def.line,
                column=var_def.column,
            )
        else:
            return var_def

    def anal_var_defs(self, var_defs: Sequence[TypeVarLikeType]) -> list[TypeVarLikeType]:
        return [self.anal_var_def(vd) for vd in var_defs]

    def named_type(
        self, fullname: str, args: list[Type] | None = None, line: int = -1, column: int = -1
    ) -> Instance:
        node = self.lookup_fully_qualified(fullname)
        assert isinstance(node.node, TypeInfo)
        any_type = AnyType(TypeOfAny.special_form)
        if args is not None:
            args = self.check_unpacks_in_list(args)
        return Instance(
            node.node, args or [any_type] * len(node.node.defn.type_vars), line=line, column=column
        )

    def check_unpacks_in_list(self, items: list[Type]) -> list[Type]:
        if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
            try:
                result = _rust_check_unpacks_in_list(items)
                if result is not None:
                    keep, final_unpack_idx = result
                    if final_unpack_idx is not None:
                        final_unpack = items[final_unpack_idx]
                        assert isinstance(final_unpack, UnpackType)
                        self.fail(
                            "More than one variadic Unpack in a type is not allowed",
                            final_unpack.type,
                        )
                    return [items[i] for i in keep]
            except (AssertionError, NotImplementedError):
                pass
        new_items: list[Type] = []
        num_unpacks = 0
        final_unpack = None
        for item in items:
            # TODO: handle forward references here, they appear as Unpack[Any].
            if isinstance(item, UnpackType) and not isinstance(
                get_proper_type(item.type), TupleType
            ):
                if not num_unpacks:
                    new_items.append(item)
                num_unpacks += 1
                final_unpack = item
            else:
                new_items.append(item)

        if num_unpacks > 1:
            assert final_unpack is not None
            self.fail("More than one variadic Unpack in a type is not allowed", final_unpack.type)
        return new_items

    def tuple_type(self, items: list[Type], line: int, column: int) -> TupleType:
        any_type = AnyType(TypeOfAny.special_form)
        return TupleType(
            items, fallback=self.named_type("builtins.tuple", [any_type]), line=line, column=column
        )


TypeVarLikeList = list[tuple[str, TypeVarLikeExpr]]


class MsgCallback(Protocol):
    def __call__(
        self, __msg: str, __ctx: Context, *, code: ErrorCode | None = None
    ) -> ErrorInfo | None: ...


def get_omitted_any(
    disallow_any: bool,
    fail: MsgCallback,
    note: MsgCallback,
    orig_type: Type,
    options: Options,
    fullname: str | None = None,
    unexpanded_type: Type | None = None,
    used_default: bool = False,
) -> AnyType:
    if disallow_any:
        typ = unexpanded_type or orig_type
        type_str = typ.name if isinstance(typ, UnboundType) else format_type_bare(typ, options)

        fail(
            message_registry.BARE_GENERIC.format(quote_type_string(type_str)),
            typ,
            code=codes.TYPE_ARG,
        )
        if used_default:
            note(message_registry.NO_CYCLIC_DEFAULT, typ, code=codes.TYPE_ARG)

        any_type = AnyType(TypeOfAny.from_error, line=typ.line, column=typ.column)
    else:
        any_type = AnyType(
            TypeOfAny.from_omitted_generics, line=orig_type.line, column=orig_type.column
        )
    return any_type


def fix_type_var_tuple_argument(t: Instance) -> None:
    if t.type.has_type_var_tuple_type:
        args = list(t.args)
        assert t.type.type_var_tuple_prefix is not None
        tvt = t.type.defn.type_vars[t.type.type_var_tuple_prefix]
        assert isinstance(tvt, TypeVarTupleType)
        args[t.type.type_var_tuple_prefix] = UnpackType(
            Instance(tvt.tuple_fallback.type, [args[t.type.type_var_tuple_prefix]])
        )
        t.args = tuple(args)


def fix_instance(
    t: Instance,
    fail: MsgCallback,
    note: MsgCallback,
    disallow_any: bool,
    options: Options,
    use_generic_error: bool = False,
    unexpanded_type: Type | None = None,
    analyzing_tvar_def: bool = False,
) -> bool:
    """Fix a malformed instance by replacing all type arguments with TypeVar default or Any.

    Also emit a suitable error if this is not due to implicit Any's.
    """
    used_default = False
    arg_count = len(t.args)
    min_tv_count = sum(
        not tv.has_default() and not isinstance(tv, TypeVarTupleType)
        for tv in t.type.defn.type_vars
    )
    max_tv_count = len(t.type.type_vars)
    if arg_count < min_tv_count or arg_count > max_tv_count:
        # Don't use existing args if arg_count doesn't match
        if arg_count > max_tv_count:
            # Already wrong arg count error, don't emit missing type parameters error as well.
            disallow_any = False
        t.args = ()

    args: list[Type] = list(t.args)
    any_type: AnyType | None = None
    env: dict[TypeVarId, Type] = {}
    tvt_no_default = False

    for tv, arg in itertools.zip_longest(t.type.defn.type_vars, t.args, fillvalue=None):
        if tv is None:
            continue
        if arg is None:
            use_any = False
            if tv.has_default():
                arg = tv.default
                if analyzing_tvar_def:
                    # Record the use of default only when analyzing another default.
                    used_default = True
                    if is_typevar_default_recursive(tv.fullname, t.type):
                        # If this results in infinite recursion, use Any instead.
                        use_any = True
            else:
                use_any = True
            if use_any:
                if any_type is None:
                    fullname = None if use_generic_error else t.type.fullname
                    any_type = get_omitted_any(
                        disallow_any,
                        fail,
                        note,
                        t,
                        options,
                        fullname,
                        unexpanded_type,
                        used_default,
                    )
                arg = any_type
            else:
                assert arg is not None
            if use_any and isinstance(tv, TypeVarTupleType):
                tvt_no_default = True
            # Default such as *tuple[int, str] should be unpacked into individual items.
            if isinstance(arg, UnpackType) and isinstance(
                unpack := get_proper_type(arg.type), TupleType
            ):
                unpacked = unpack.items
            else:
                unpacked = [arg]
            for arg in unpacked:
                with state.strict_optional_set(options.strict_optional):
                    # Gradually expand defaults, as they may depend on previous variables.
                    if tv.has_default():
                        arg = expand_type(arg, env)
                    env[tv.id] = arg
                args.append(arg)
        else:
            env[tv.id] = arg
    t.args = tuple(args)
    if tvt_no_default:
        fix_type_var_tuple_argument(t)
    return used_default


def instantiate_type_alias(
    node: TypeAlias,
    args: list[Type],
    fail: MsgCallback,
    note: MsgCallback,
    no_args: bool,
    ctx: Context,
    options: Options,
    *,
    unexpanded_type: Type | None = None,
    disallow_any: bool = False,
    use_standard_error: bool = False,
    empty_tuple_index: bool = False,
    analyzing_tvar_def: bool = False,
) -> tuple[Type, bool]:
    """Create an instance of a (generic) type alias from alias node and type arguments.

    We are following the rules outlined in TypeAlias docstring.
    Here:
        node: type alias node (definition)
        args: type arguments (types to be substituted in place of type variables
              when expanding the alias)
        fail: error reporter callback
        no_args: whether original definition used a bare generic `A = List`
        ctx: context where expansion happens
        unexpanded_type, disallow_any, use_standard_error: used to customize error messages
    """
    # Type aliases are special, since they can be expanded during semantic analysis,
    # so we need to normalize them as soon as possible.
    # TODO: can this cause an infinite recursion?
    old_args = args
    args = flatten_nested_tuples(args)
    if old_args and not args:
        empty_tuple_index = True
    # Native seam: Rust decides the non-error success paths (bare
    # generic eager expansion, non-generic alias, correct generic
    # instantiation) and returns a branch tag; Python rebuilds the live

    # result object exactly as the pure-Python body below would. Every
    # path that would emit an error or rewrite to Any (set_any_tvars)

    # returns None and the pure-Python body takes over, keeping message
    # side effects single-sourced.
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            result = _rust_instantiate_type_alias(
                node, [_serialize_typeanal_type(a) for a in args], no_args, empty_tuple_index
            )
            if result == 0:
                # non-generic alias, no args, no_args: eager expansion.
                assert isinstance(node.target, Instance)  # type: ignore[misc]
                return (
                    Instance(node.target.type, [], line=ctx.line, column=ctx.column),
                    False,
                )
            if result == 1:
                # non-generic alias with args targeting a bare generic.
                tp = Instance(node.target.type, args)
                tp.line = ctx.line
                tp.column = ctx.column
                tp.end_line = ctx.end_line
                tp.end_column = ctx.end_column
                return tp, False
            if result == 2:
                # Plain success: TypeAliasType(node, args, line, column),
                # including the FlexibleAlias[T, typ] -> typ unwrap.
                typ = TypeAliasType(node, args, ctx.line, ctx.column)
                if (
                    isinstance(typ.alias.target, Instance)  # type: ignore[misc]
                    and typ.alias.target.type.fullname == "mypy_extensions.FlexibleAlias"
                ):
                    exp = get_proper_type(typ)
                    assert isinstance(exp, Instance)
                    return exp.args[-1], False
                return typ, False
        except (AssertionError, NotImplementedError):
            pass
    if any(unknown_unpack(a) for a in args):
        # This type is not ready to be validated, because of unknown total count.
        # Note that we keep the kind of Any for consistency.
        return set_any_tvars(node, [], ctx.line, ctx.column, options, special_form=True)

    if (
        no_args
        and isinstance(node.target, ProperType)
        and isinstance(node.target, Instance)
        and node.target.type.fullname == "builtins.tuple"
        and len(args)
    ):
        no_args = False

    max_tv_count = len(node.alias_tvars)
    act_len = len(args)
    if (
        max_tv_count > 0
        and act_len == 0
        and not (empty_tuple_index and node.tvar_tuple_index is not None)
    ):
        # Interpret bare Alias same as normal generic, i.e., Alias[Any, Any, ...]
        return set_any_tvars(
            node,
            args,
            ctx.line,
            ctx.column,
            options,
            disallow_any=disallow_any,
            fail=fail,
            note=note,
            unexpanded_type=unexpanded_type,
            analyzing_tvar_def=analyzing_tvar_def,
        )
    if max_tv_count == 0 and act_len == 0:
        if no_args:
            assert isinstance(node.target, Instance)  # type: ignore[misc]
            # Note: this is the only case where we use an eager expansion. See more info about
            # no_args aliases like L = List in the docstring for TypeAlias class.
            return Instance(node.target.type, [], line=ctx.line, column=ctx.column), False
        return TypeAliasType(node, [], line=ctx.line, column=ctx.column), False
    if (
        max_tv_count == 0
        and act_len > 0
        and isinstance(node.target, Instance)  # type: ignore[misc]
        and no_args
    ):
        tp = Instance(node.target.type, args)
        tp.line = ctx.line
        tp.column = ctx.column
        tp.end_line = ctx.end_line
        tp.end_column = ctx.end_column
        return tp, False
    if node.tvar_tuple_index is None:
        if any(isinstance(a, UnpackType) for a in args):
            # A variadic unpack in fixed size alias (fixed unpacks must be flattened by the caller)
            fail(message_registry.INVALID_UNPACK_POSITION, ctx, code=codes.VALID_TYPE)
            return set_any_tvars(node, [], ctx.line, ctx.column, options, from_error=True)
        min_tv_count = sum(not tv.has_default() for tv in node.alias_tvars)
        fill_typevars = act_len != max_tv_count
        correct = min_tv_count <= act_len <= max_tv_count
    else:
        min_tv_count = sum(
            not tv.has_default() and not isinstance(tv, TypeVarTupleType)
            for tv in node.alias_tvars
        )
        correct = act_len >= min_tv_count
        for a in args:
            if isinstance(a, UnpackType):
                unpacked = get_proper_type(a.type)
                if isinstance(unpacked, Instance) and unpacked.type.fullname == "builtins.tuple":
                    # Variadic tuple is always correct.
                    correct = True
        fill_typevars = not correct
    if fill_typevars:
        if not correct:
            if use_standard_error:
                # This is used if type alias is an internal representation of another type,
                # for example a generic TypedDict or NamedTuple.
                msg = wrong_type_arg_count(max_tv_count, max_tv_count, str(act_len), node.name)
            else:
                if node.tvar_tuple_index is not None:
                    msg = (
                        "Bad number of arguments for type alias,"
                        f" expected at least {min_tv_count}, given {act_len}"
                    )
                elif min_tv_count != max_tv_count:
                    msg = (
                        "Bad number of arguments for type alias,"
                        f" expected between {min_tv_count} and {max_tv_count}, given {act_len}"
                    )
                else:
                    msg = (
                        "Bad number of arguments for type alias,"
                        f" expected {min_tv_count}, given {act_len}"
                    )
            fail(msg, ctx, code=codes.TYPE_ARG)
            args = []
        return set_any_tvars(
            node,
            args,
            ctx.line,
            ctx.column,
            options,
            disallow_any=disallow_any,
            fail=fail,
            note=note,
            from_error=not correct,
            analyzing_tvar_def=analyzing_tvar_def,
        )
    elif node.tvar_tuple_index is not None:
        # We also need to check if we are not performing a type variable tuple split.
        unpack = find_unpack_in_list(args)
        if unpack is not None:
            unpack_arg = args[unpack]
            assert isinstance(unpack_arg, UnpackType)
            if isinstance(unpack_arg.type, TypeVarTupleType):
                exp_prefix = node.tvar_tuple_index
                act_prefix = unpack
                exp_suffix = len(node.alias_tvars) - node.tvar_tuple_index - 1
                act_suffix = len(args) - unpack - 1
                if act_prefix < exp_prefix or act_suffix < exp_suffix:
                    fail("TypeVarTuple cannot be split", ctx, code=codes.TYPE_ARG)
                    return set_any_tvars(node, [], ctx.line, ctx.column, options, from_error=True)
    # TODO: we need to check args validity w.r.t alias.alias_tvars.
    # Otherwise invalid instantiations will be allowed in runtime context.
    # Note: in type context, these will be still caught by semanal_typeargs.
    typ = TypeAliasType(node, args, ctx.line, ctx.column)
    assert typ.alias is not None
    # HACK: Implement FlexibleAlias[T, typ] by expanding it to typ here.
    if (
        isinstance(typ.alias.target, Instance)  # type: ignore[misc]
        and typ.alias.target.type.fullname == "mypy_extensions.FlexibleAlias"
    ):
        exp = get_proper_type(typ)
        assert isinstance(exp, Instance)
        return exp.args[-1], False
    return typ, False


def set_any_tvars(
    node: TypeAlias,
    args: list[Type],
    newline: int,
    newcolumn: int,
    options: Options,
    *,
    from_error: bool = False,
    disallow_any: bool = False,
    special_form: bool = False,
    fail: MsgCallback | None = None,
    note: MsgCallback | None = None,
    unexpanded_type: Type | None = None,
    analyzing_tvar_def: bool = False,
) -> tuple[TypeAliasType, bool]:
    used_default = False
    if from_error or disallow_any:
        type_of_any = TypeOfAny.from_error
    elif special_form:
        type_of_any = TypeOfAny.special_form
    else:
        type_of_any = TypeOfAny.from_omitted_generics
    any_type = AnyType(type_of_any, line=newline, column=newcolumn)

    env: dict[TypeVarId, Type] = {}
    used_any_type = False
    for tv, arg in itertools.zip_longest(node.alias_tvars, args, fillvalue=None):
        if tv is None:
            continue
        if arg is None:
            if tv.has_default():
                arg = tv.default
                # Same as for instances, record and avoid infinite recursion.
                if analyzing_tvar_def:
                    used_default = True
                    if is_typevar_default_recursive(tv.fullname, node):
                        arg = any_type
                        used_any_type = True
            else:
                arg = any_type
                used_any_type = True
            if used_any_type and isinstance(tv, TypeVarTupleType):
                arg = UnpackType(Instance(tv.tuple_fallback.type, [any_type]))
            # Default such as *tuple[int, str] should be unpacked into individual items.
            if isinstance(arg, UnpackType) and isinstance(
                unpack := get_proper_type(arg.type), TupleType
            ):
                unpacked = unpack.items
            else:
                unpacked = [arg]
            for arg in unpacked:
                with state.strict_optional_set(options.strict_optional):
                    # Gradually expand defaults, as they may depend on previous variables.
                    if tv.has_default():
                        arg = expand_type(arg, env)
                    env[tv.id] = arg
                args.append(arg)
        else:
            env[tv.id] = arg
    t = TypeAliasType(node, args, newline, newcolumn)

    if used_any_type and disallow_any and node.alias_tvars and not from_error:
        assert fail is not None
        if unexpanded_type:
            type_str = (
                unexpanded_type.name
                if isinstance(unexpanded_type, UnboundType)
                else format_type_bare(unexpanded_type, options)
            )
        else:
            type_str = node.name

        fail(
            message_registry.BARE_GENERIC.format(quote_type_string(type_str)),
            Context(newline, newcolumn),
            code=codes.TYPE_ARG,
        )
        if used_default:
            assert note is not None
            note(
                message_registry.NO_CYCLIC_DEFAULT,
                Context(newline, newcolumn),
                code=codes.TYPE_ARG,
            )
    return t, used_default


def is_typevar_default_recursive(tv_fname: str, start: TypeInfo | TypeAlias) -> bool:
    """Check if the type variable can lead to infinite recursion via defaults."""
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            result = _rust_is_typevar_default_recursive(tv_fname, start)
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    if tv_fname not in start.default_depends:
        return False
    todo = start.default_depends[tv_fname].copy()
    seen: set[TypeAlias | TypeInfo] = set()
    while todo:
        node = todo.pop()
        if node is start:
            return True
        if node in seen:
            # We don't return True here, since we are interested only in
            # recursion via the original type variable.
            continue
        seen.add(node)
        for dep_nodes in node.default_depends.values():
            todo |= dep_nodes
    return False


class DivergingAliasDetector(TrivialSyntheticTypeTranslator):
    """See docstring of detect_diverging_alias() for details."""

    # TODO: this doesn't really need to be a translator, but we don't have a trivial
    # visitor.
    def __init__(self, seen_nodes: set[TypeAlias]) -> None:
        super().__init__()
        self.seen_nodes = seen_nodes
        self.diverging = False

    def visit_type_alias_type(self, t: TypeAliasType) -> Type:
        assert t.alias is not None, f"Unfixed type alias {t.type_ref}"
        if t.alias in self.seen_nodes:
            for arg in t.args:
                if not (
                    isinstance(arg, TypeVarLikeType)
                    or isinstance(arg, UnpackType)
                    and isinstance(arg.type, TypeVarLikeType)
                ) and has_type_vars(arg):
                    self.diverging = True
                    return t
            # All clear for this expansion chain.
            return t
        new_nodes = self.seen_nodes | {t.alias}
        visitor = DivergingAliasDetector(new_nodes)
        _ = get_proper_type(t).accept(visitor)
        if visitor.diverging:
            self.diverging = True
        return t


def detect_diverging_alias(node: TypeAlias, target: Type) -> bool:
    """This detects type aliases that will diverge during type checking.

    For example F = Something[..., F[List[T]]]. At each expansion step this will produce
    *new* type aliases: e.g. F[List[int]], F[List[List[int]]], etc. So we can't detect
    recursion. It is a known problem in the literature, recursive aliases and generic types
    don't always go well together. It looks like there is no known systematic solution yet.

    # TODO: should we handle such aliases using type_recursion counter and some large limit?
    They may be handy in rare cases, e.g. to express a union of non-mixed nested lists:
    Nested = Union[T, Nested[List[T]]] ~> Union[T, List[T], List[List[T]], ...]
    """
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            result = _rust_detect_diverging_alias(node, target)
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    is_recursive = node._is_recursive
    if is_recursive is None:
        is_recursive = node in node.target.accept(CollectAliasesVisitor())
    if not is_recursive:
        # Fast path: this is not a recursive alias at all.
        return False
    # Note we only cache positive case, caching negative case is risky, as this type alias
    # (or more importantly any other alias it uses) may be not ready yet.
    node._is_recursive = True
    visitor = DivergingAliasDetector({node})
    _ = target.accept(visitor)
    return visitor.diverging


def check_for_explicit_any(
    typ: Type | None,
    options: Options,
    is_typeshed_stub: bool,
    msg: MessageBuilder,
    context: Context,
) -> None:
    if options.disallow_any_explicit and not is_typeshed_stub and typ and has_explicit_any(typ):
        msg.explicit_any(context)


# Stage 17: native typeanal query seam (parity-only, default-off).
# Kernel routes has_explicit_any / has_any_from_unimported_type /
# collect_all_inner_types / make_optional_type; None falls back to Python.
try:
    from librt.internal import (
        ReadBuffer as _TypeanalReadBuffer,
        WriteBuffer as _TypeanalWriteBuffer,
    )
    from type_kernel import (
        rust_analyze_unbound_without_info as _rust_analyze_unbound_without_info,
        rust_check_unpacks_in_list as _rust_check_unpacks_in_list,
        rust_check_vec_type_args as _rust_check_vec_type_args,
        rust_classify_analyze_callable_type as _rust_classify_analyze_callable_type,
        rust_classify_check_warn_deprecated as _rust_classify_check_warn_deprecated,
        rust_classify_literal_param as _rust_classify_literal_param,
        rust_classify_raw_expression_type as _rust_classify_raw_expression_type,
        rust_classify_special_unbound as _rust_classify_special_unbound,
        rust_classify_tuple_type_implicit as _rust_classify_tuple_type_implicit,
        rust_classify_type_with_info as _rust_classify_type_with_info,
        rust_classify_unbound_front as _rust_classify_unbound_front,
        rust_collect_all_inner_types as _rust_collect_all_inner_types,
        rust_collect_all_inner_types_live as _rust_collect_all_inner_types_live,
        rust_detect_diverging_alias as _rust_detect_diverging_alias,
        rust_find_self_type as _rust_find_self_type,
        rust_has_any_from_unimported_type as _rust_has_any_from_unimported_type,
        rust_has_any_from_unimported_type_live as _rust_has_any_from_unimported_type_live,
        rust_has_explicit_any as _rust_has_explicit_any,
        rust_has_explicit_any_live as _rust_has_explicit_any_live,
        rust_instantiate_type_alias as _rust_instantiate_type_alias,
        rust_is_typevar_default_recursive as _rust_is_typevar_default_recursive,
        rust_make_optional_type as _rust_make_optional_type,
        rust_make_optional_type_live as _rust_make_optional_type_live,
        rust_type_analyze as _rust_type_analyze,
        rust_unknown_unpack as _rust_unknown_unpack,
        rust_unknown_unpack_live as _rust_unknown_unpack_live,
        rust_validate_instance as _rust_validate_instance,
    )

    from mypy.types import read_type as _typeanal_read_type
    from mypy.wirefixup import check_no_fake_info, fixup_wire_type

    _TYPEANAL_HAS_KERNEL = True
except ImportError:
    _rust_has_explicit_any = None  # type: ignore[assignment]
    _rust_has_explicit_any_live = None  # type: ignore[assignment]
    _rust_has_any_from_unimported_type = None  # type: ignore[assignment]
    _rust_has_any_from_unimported_type_live = None  # type: ignore[assignment]
    _rust_collect_all_inner_types = None  # type: ignore[assignment]
    _rust_collect_all_inner_types_live = None  # type: ignore[assignment]
    _rust_make_optional_type = None  # type: ignore[assignment]
    _rust_make_optional_type_live = None  # type: ignore[assignment]
    _rust_type_analyze = None  # type: ignore[assignment]
    _rust_unknown_unpack = None  # type: ignore[assignment]
    _rust_unknown_unpack_live = None  # type: ignore[assignment]
    _rust_validate_instance = None  # type: ignore[assignment]
    _rust_detect_diverging_alias = None  # type: ignore[assignment]
    _rust_find_self_type = None  # type: ignore[assignment]
    _rust_check_vec_type_args = None  # type: ignore[assignment]
    _rust_is_typevar_default_recursive = None  # type: ignore[assignment]
    _rust_instantiate_type_alias = None  # type: ignore[assignment]
    _rust_analyze_unbound_without_info = None  # type: ignore[assignment]
    _rust_check_unpacks_in_list = None  # type: ignore[assignment]
    _rust_classify_type_with_info = None  # type: ignore[assignment]
    _rust_classify_unbound_front = None  # type: ignore[assignment]
    _rust_classify_special_unbound = None  # type: ignore[assignment]
    _rust_classify_tuple_type_implicit = None  # type: ignore[assignment]
    _rust_classify_literal_param = None  # type: ignore[assignment]
    _rust_classify_raw_expression_type = None  # type: ignore[assignment]
    _rust_classify_check_warn_deprecated = None  # type: ignore[assignment]
    _rust_classify_analyze_callable_type = None  # type: ignore[assignment]
    _rust_classify_tuple_type_implicit = None  # type: ignore[assignment]
    _TypeanalWriteBuffer = None  # type: ignore[assignment,misc]
    _TypeanalReadBuffer = None  # type: ignore[assignment,misc]
    _typeanal_read_type = None  # type: ignore[assignment]
    _TYPEANAL_HAS_KERNEL = False

_native_typeanal_active: bool = False


def _set_native_typeanal_active(active: bool) -> None:
    global _native_typeanal_active
    _native_typeanal_active = active


# NativeTypeResolver shared with the typeanal query kernel for
# TypeAliasType expansion (issue #852). Installed per build by
# BuildManager; None keeps the byte-only seams (alias defers to Python).
_native_typeanal_resolver: Any = None


def _set_native_typeanal_resolver(resolver: Any) -> None:
    """Install/clear the NativeTypeResolver for the typeanal kernel."""
    global _native_typeanal_resolver
    _native_typeanal_resolver = resolver


# Front branch tags for `visit_unbound_type_nonoptional` (issue #714).
# Mirrored in crates/type_kernel/src/typeanal_unbound2.rs; Python applies
# the side effect + result construction for the tag Rust returns.
_UNBOUND_FRONT_TAG_PH_BECOMES_FINAL = 1
_UNBOUND_FRONT_TAG_PH_BECOMES_DEFER = 2
_UNBOUND_FRONT_TAG_PH_BECOMES_RECORD = 3
_UNBOUND_FRONT_TAG_PH_PLAIN_FINAL = 4
_UNBOUND_FRONT_TAG_PH_PLAIN_RECORD = 5
_UNBOUND_FRONT_TAG_SYM_NONE = 6
_UNBOUND_FRONT_TAG_NODE_NONE = 7
_UNBOUND_FRONT_TAG_PSPEC_UNBOUND_TVAR = 8
_UNBOUND_FRONT_TAG_PSPEC_NOT_DECLARED = 9
_UNBOUND_FRONT_TAG_PSPEC_UNBOUND = 10
_UNBOUND_FRONT_TAG_PSPEC_ARGS_COMPONENT = 11
_UNBOUND_FRONT_TAG_PSPEC_ARGS = 12
_UNBOUND_FRONT_TAG_PSPEC_COMPONENT = 13
_UNBOUND_FRONT_TAG_PSPEC_OK = 14
_UNBOUND_FRONT_TAG_TVAR_ALIAS_NOT_DECLARED = 15
_UNBOUND_FRONT_TAG_TVAR_ALIAS_BOUND = 16
_UNBOUND_FRONT_TAG_TVAR_ERASED = 17
_UNBOUND_FRONT_TAG_TVAR_ARGS = 18
_UNBOUND_FRONT_TAG_TVAR_OK = 19
_UNBOUND_FRONT_TAG_TVARTUPLE_ALIAS_NOT_DECLARED = 20
_UNBOUND_FRONT_TAG_TVARTUPLE_ALIAS_BOUND = 21
_UNBOUND_FRONT_TAG_TVARTUPLE_UNBOUND_TVAR = 22
_UNBOUND_FRONT_TAG_TVARTUPLE_NOT_DECLARED = 23
_UNBOUND_FRONT_TAG_TVARTUPLE_UNBOUND = 24
_UNBOUND_FRONT_TAG_TVARTUPLE_NESTING = 25
_UNBOUND_FRONT_TAG_TVARTUPLE_ARGS = 26
_UNBOUND_FRONT_TAG_TVARTUPLE_OK = 27

# Node-kind tags for the resolved `sym.node`, computed by the shim:
# -1 sym is None, 0 node is None, 1 PlaceholderNode, 2 ParamSpecExpr,
# 3 TypeVarExpr, 4 TypeVarTupleExpr, 5 anything else (defer).
_UNBOUND_FRONT_KIND_SYM_NONE = -1
_UNBOUND_FRONT_KIND_NODE_NONE = 0
_UNBOUND_FRONT_KIND_PLACEHOLDER = 1
_UNBOUND_FRONT_KIND_PARAM_SPEC = 2
_UNBOUND_FRONT_KIND_TYPE_VAR = 3
_UNBOUND_FRONT_KIND_TYPE_VAR_TUPLE = 4
_UNBOUND_FRONT_KIND_OTHER = 5

# Branch tags for `try_analyze_special_unbound_type` (issue #720).
# Mirrored in crates/type_kernel/src/typeanal_special.rs; Python applies
# the side effect + result construction for the tag Rust returns. Deferred

# branches (tag None) keep the full pure-Python body.
_UNBOUND_SPECIAL_TAG_NONE_TYPE = 2
_UNBOUND_SPECIAL_TAG_ANY = 3
_UNBOUND_SPECIAL_TAG_NEVER = 4
_UNBOUND_SPECIAL_TAG_FINAL_ERROR = 5
_UNBOUND_SPECIAL_TAG_TUPLE_LOOKUP_DEFER = 6
_UNBOUND_SPECIAL_TAG_TUPLE_BARE = 7
_UNBOUND_SPECIAL_TAG_TUPLE_ELLIPSIS = 8
_UNBOUND_SPECIAL_TAG_TUPLE_FULL_DEFER = 9
_UNBOUND_SPECIAL_TAG_UNION_DEFER = 11
_UNBOUND_SPECIAL_TAG_OPTIONAL_ARG_ERR = 12
_UNBOUND_SPECIAL_TAG_OPTIONAL_DEFER = 13
_UNBOUND_SPECIAL_TAG_CALLABLE_DEFER = 14
_UNBOUND_SPECIAL_TAG_TYPE_BARE_ANY = 15
_UNBOUND_SPECIAL_TAG_TYPE_BARE_NONE = 16
_UNBOUND_SPECIAL_TAG_TYPE_ONE_ARG = 17
_UNBOUND_SPECIAL_TAG_TYPE_ARG_ERR = 18
_UNBOUND_SPECIAL_TAG_TYPEFORM_BARE = 19
_UNBOUND_SPECIAL_TAG_TYPEFORM_DEFER = 20
_UNBOUND_SPECIAL_TAG_CLASSVAR_ZERO = 21
_UNBOUND_SPECIAL_TAG_CLASSVAR_DEFER = 22
_UNBOUND_SPECIAL_TAG_ANNOTATED_ARG_ERR = 23
_UNBOUND_SPECIAL_TAG_ANNOTATED_DEFER = 24
_UNBOUND_SPECIAL_TAG_REQUIRED_BAD_CTX = 25
_UNBOUND_SPECIAL_TAG_REQUIRED_ARG_ERR = 26
_UNBOUND_SPECIAL_TAG_REQUIRED_DEFER = 27
_UNBOUND_SPECIAL_TAG_NOTREQUIRED_BAD_CTX = 28
_UNBOUND_SPECIAL_TAG_NOTREQUIRED_ARG_ERR = 29
_UNBOUND_SPECIAL_TAG_NOTREQUIRED_DEFER = 30
_UNBOUND_SPECIAL_TAG_READONLY_BAD_CTX = 31
_UNBOUND_SPECIAL_TAG_READONLY_ARG_ERR = 32
_UNBOUND_SPECIAL_TAG_READONLY_DEFER = 33
_UNBOUND_SPECIAL_TAG_LITERAL_DEFER = 34
_UNBOUND_SPECIAL_TAG_NAME_TYPEGUARD = 35
_UNBOUND_SPECIAL_TAG_NAME_TYPEIS = 36
_UNBOUND_SPECIAL_TAG_UNPACK_ARG_ERR = 37
_UNBOUND_SPECIAL_TAG_UNPACK_POS_ERR = 38
_UNBOUND_SPECIAL_TAG_UNPACK_DEFER = 39
_UNBOUND_SPECIAL_TAG_NOT_SPECIAL = 40

# Branch tags for `analyze_literal_param` (issue #919), mirrored in
# typeanal_literal.rs; the Python shim applies the side effects
# (LiteralType build, errors, recursion, union merge) for each tag.
_LITERAL_PARAM_TAG_STR = 1
_LITERAL_PARAM_TAG_ANY_FAIL = 2
_LITERAL_PARAM_TAG_ANY_SILENT = 3
_LITERAL_PARAM_TAG_RAW_FLOAT_COMPLEX = 4
_LITERAL_PARAM_TAG_RAW_ARBITRARY = 5
_LITERAL_PARAM_TAG_RAW_VALUE = 6
_LITERAL_PARAM_TAG_NONE_OR_LITERAL = 7
_LITERAL_PARAM_TAG_INSTANCE_LKV = 8
_LITERAL_PARAM_TAG_UNION_RECURSE = 9
_LITERAL_PARAM_TAG_INVALID = 10

# Message tags for `visit_raw_expression_type` (issue #924).
# Mirrored in crates/type_kernel/src/typeanal_rawexpr.rs; Python formats the
# message and applies self.fail / self.note for the tag Rust returns.
_RAW_EXPR_TAG_LITERAL = 0
_RAW_EXPR_TAG_NUMERIC_LITERALS = 1
_RAW_EXPR_TAG_GENERIC = 2

# Result tags for `check_and_warn_deprecated` (issue #1002). Mirrored in
# crates/type_kernel/src/typeanal_deprec.rs; Python applies the note/fail
# side effect for the tag Rust returns.
_DEPRECATED_TAG_SILENT = 0
_DEPRECATED_TAG_NOTE = 1
_DEPRECATED_TAG_FAIL = 2

# Message tags for `visit_tuple_type` (issue #983). Mirrored in
# crates/type_kernel/src/typeanal_special.rs; Python applies the
# self.fail + one-of-three note and, on OK, the reconstruction.
_TUPLE_TAG_OK = 0
_TUPLE_TAG_EMPTY = 1
_TUPLE_TAG_SINGLE = 2
_TUPLE_TAG_MULTI = 3

# Branch tags for `analyze_callable_type` (issue #958), mirrored in
# crates/type_kernel/src/typeanal_callable.rs; Python builds the live
# CallableType / enters tvar_scope / emits fail/note for each tag.
_CALLABLE_TAG_BARE = 0
_CALLABLE_TAG_TYPE_LIST = 1
_CALLABLE_TAG_ELLIPSIS = 2
_CALLABLE_TAG_PARAMSPEC = 3
_CALLABLE_TAG_INVALID_DISALLOW = 4
_CALLABLE_TAG_INVALID_ALLOW = 5

# Branch tags for `analyze_type_with_type_info` (issue #721).
# Mirrored in crates/type_kernel/src/typeanal_info.rs; Python applies the
# side effect + result construction for the two inline tags, the rest
# re-run the original body.
_TYPE_WITH_INFO_TAG_TUPLE = 1
_TYPE_WITH_INFO_TAG_VEC = 2
_TYPE_WITH_INFO_TAG_TUPLE_TAIL = 3
_TYPE_WITH_INFO_TAG_TUPLE_TAIL_ALIAS = 4
_TYPE_WITH_INFO_TAG_TYPEDDICT_TAIL = 5
_TYPE_WITH_INFO_TAG_TYPEDDICT_TAIL_ALIAS = 6
_TYPE_WITH_INFO_TAG_NONE_TYPE = 7
_TYPE_WITH_INFO_TAG_INSTANCE = 8


def native_analyze_type(
    t: Type,
    *,
    allow_tuple_literal: bool = False,
    allow_param_spec_literals: bool = False,
    allow_unpack: bool = False,
) -> Type | None:
    """Try the native (Rust) type analyser for an already-bound type.

    Returns the analysed Type as a live Python object, or ``None`` when the
    Rust path does not handle the type (e.g. UnboundType, PlaceholderType),
    matching Python's deferral semantics.  When ``None`` is returned the
    caller should fall through to the pure-Python visitor.  TypeAliasType
    is handled: the Rust path passes the node through unchanged (args
    untouched), mirroring ``visit_type_alias_type``.
    """
    if not _TYPEANAL_HAS_KERNEL or not _native_typeanal_active:
        return None
    # The Rust path always defers UnboundType/PlaceholderType (they need
    # symbol lookup or deferral side effects). Skip the wire round-trip so
    # the hot path does not serialize+decode for known-deferred inputs.
    if isinstance(t, (UnboundType, PlaceholderType)):
        return None
    try:
        payload = _serialize_typeanal_type(t)
        result = _rust_type_analyze(payload, allow_tuple_literal, allow_param_spec_literals, allow_unpack)
        if result is not None:
            return _typeanal_decode(result)
    except (AssertionError, NotImplementedError):
        pass
    return None


def _serialize_typeanal_type(t: Type) -> bytes:
    fast = _encode_no_arg_instance(t, _TypeanalWriteBuffer)
    if fast is not None:
        return fast
    buf = _TypeanalWriteBuffer()
    t.write(buf)
    return buf.getvalue()


def _typeanal_decode(result: bytes) -> Type | None:
    buf = _TypeanalReadBuffer(bytes(result))
    decoded = _typeanal_read_type(buf)
    # Clear instance_cache primitives after read_type so NOT_READY
    # singletons cannot leak into later builds (mirrors applytype).
    from mypy.types import instance_cache

    instance_cache.int_type = None
    instance_cache.str_type = None
    instance_cache.bool_type = None
    instance_cache.object_type = None
    instance_cache.function_type = None
    fixed = fixup_wire_type(decoded, resolve_aliases=True)
    if fixed is None:
        return None
    # Any residual fake TypeInfo crashes later serialization, so defer.
    if not check_no_fake_info(fixed):
        return None
    return fixed


def has_explicit_any(t: Type) -> bool:
    """
    Whether this type is or type it contains is an Any coming from explicit type annotation
    """
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            if _native_typeanal_resolver is not None:
                result = _rust_has_explicit_any_live(
                    _native_typeanal_resolver,
                    _serialize_typeanal_type(t),
                )
            else:
                result = _rust_has_explicit_any(_serialize_typeanal_type(t))
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    return t.accept(HasExplicitAny())


class HasExplicitAny(BoolTypeQuery):
    def __init__(self) -> None:
        super().__init__(ANY_STRATEGY)

    def visit_any(self, t: AnyType) -> bool:
        return t.type_of_any == TypeOfAny.explicit

    def visit_typeddict_type(self, t: TypedDictType) -> bool:
        # typeddict is checked during TypedDict declaration, so don't typecheck it here.
        return False


def has_any_from_unimported_type(t: Type) -> bool:
    """Return true if this type is Any because an import was not followed.

    If type t is such Any type or has type arguments that contain such Any type
    this function will return true.
    """
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            if _native_typeanal_resolver is not None:
                result = _rust_has_any_from_unimported_type_live(
                    _native_typeanal_resolver,
                    _serialize_typeanal_type(t),
                )
            else:
                result = _rust_has_any_from_unimported_type(_serialize_typeanal_type(t))
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    return t.accept(HasAnyFromUnimportedType())


class HasAnyFromUnimportedType(BoolTypeQuery):
    def __init__(self) -> None:
        super().__init__(ANY_STRATEGY)

    def visit_any(self, t: AnyType) -> bool:
        return t.type_of_any == TypeOfAny.from_unimported_type

    def visit_typeddict_type(self, t: TypedDictType) -> bool:
        # typeddict is checked during TypedDict declaration, so don't typecheck it here
        return False


def collect_all_inner_types(t: Type) -> list[Type]:
    """
    Return all types that `t` contains
    """
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            if _native_typeanal_resolver is not None:
                result = _rust_collect_all_inner_types_live(
                    _native_typeanal_resolver,
                    _serialize_typeanal_type(t),
                )
            else:
                result = _rust_collect_all_inner_types(_serialize_typeanal_type(t))
            if result is not None:
                decoded = []
                for item in result:
                    bt = _typeanal_decode(item)
                    if bt is None:
                        break
                    decoded.append(bt)
                else:
                    return decoded
        except (AssertionError, NotImplementedError):
            pass
    return t.accept(CollectAllInnerTypesQuery())


class CollectAllInnerTypesQuery(TypeQuery[list[Type]]):
    def query_types(self, types: Iterable[Type]) -> list[Type]:
        return self.strategy([t.accept(self) for t in types]) + list(types)

    def strategy(self, items: Iterable[list[Type]]) -> list[Type]:
        return list(itertools.chain.from_iterable(items))


def make_optional_type(t: Type) -> Type:
    """Return the type corresponding to Optional[t].

    Note that we can't use normal union simplification, since this function
    is called during semantic analysis and simplification only works during
    type checking.
    """
    if isinstance(t, ProperType) and isinstance(t, NoneType):
        return t
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            if _native_typeanal_resolver is not None:
                result = _rust_make_optional_type_live(
                    _native_typeanal_resolver,
                    _serialize_typeanal_type(t),
                )
            else:
                result = _rust_make_optional_type(_serialize_typeanal_type(t))
            if result is not None:
                decoded = _typeanal_decode(result)
                if decoded is not None:
                    return decoded
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(t, ProperType) and isinstance(t, UnionType):
        # Eagerly expanding aliases is not safe during semantic analysis.
        items = [item for item in t.items if not isinstance(get_proper_type(item), NoneType)]
        return UnionType(items + [NoneType()], t.line, t.column)
    else:
        return UnionType([t, NoneType()], t.line, t.column)


def validate_instance(t: Instance, fail: MsgCallback, indexed: bool) -> bool:
    """Check if this is a well-formed instance with respect to argument count/positions."""
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            result = _rust_validate_instance(t, fail, indexed)
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    # TODO: combine logic with instantiate_type_alias().
    if any(unknown_unpack(a) for a in t.args):
        # This type is not ready to be validated, because of unknown total count.
        # TODO: is it OK to fill with TypeOfAny.from_error instead of special form?
        return False
    empty_tuple_index = indexed and not t.args
    if t.type.has_type_var_tuple_type:
        min_tv_count = sum(
            not tv.has_default() and not isinstance(tv, TypeVarTupleType)
            for tv in t.type.defn.type_vars
        )
        correct = len(t.args) >= min_tv_count
        if any(
            isinstance(a, UnpackType) and isinstance(get_proper_type(a.type), Instance)
            for a in t.args
        ):
            correct = True
        if not t.args:
            if not (empty_tuple_index and len(t.type.type_vars) == 1):
                # The Any arguments should be set by the caller.
                if empty_tuple_index and min_tv_count:
                    fail(
                        f"At least {min_tv_count} type argument(s) expected, none given",
                        t,
                        code=codes.TYPE_ARG,
                    )
                return False
        elif not correct:
            fail(
                f"Bad number of arguments, expected: at least {min_tv_count}, given: {len(t.args)}",
                t,
                code=codes.TYPE_ARG,
            )
            return False
        else:
            # We also need to check if we are not performing a type variable tuple split.
            unpack = find_unpack_in_list(t.args)
            if unpack is not None:
                unpack_arg = t.args[unpack]
                assert isinstance(unpack_arg, UnpackType)
                if isinstance(unpack_arg.type, TypeVarTupleType):
                    assert t.type.type_var_tuple_prefix is not None
                    assert t.type.type_var_tuple_suffix is not None
                    exp_prefix = t.type.type_var_tuple_prefix
                    act_prefix = unpack
                    exp_suffix = t.type.type_var_tuple_suffix
                    act_suffix = len(t.args) - unpack - 1
                    if act_prefix < exp_prefix or act_suffix < exp_suffix:
                        fail("TypeVarTuple cannot be split", t, code=codes.TYPE_ARG)
                        return False
    elif any(isinstance(a, UnpackType) for a in t.args):
        # A variadic unpack in fixed size instance (fixed unpacks must be flattened by
        # the caller)
        fail(message_registry.INVALID_UNPACK_POSITION, t, code=codes.VALID_TYPE)
        t.args = ()
        return False
    elif len(t.args) != len(t.type.type_vars):
        # Invalid number of type parameters.
        arg_count = len(t.args)
        min_tv_count = sum(not tv.has_default() for tv in t.type.defn.type_vars)
        max_tv_count = len(t.type.type_vars)
        if (arg_count or empty_tuple_index) and (
            arg_count < min_tv_count or arg_count > max_tv_count
        ):
            fail(
                wrong_type_arg_count(min_tv_count, max_tv_count, str(arg_count), t.type.name),
                t,
                code=codes.TYPE_ARG,
            )
        return False
    return True


def find_self_type(typ: Type, lookup: Callable[[str], SymbolTableNode | None]) -> bool:
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            result = _rust_find_self_type(typ, lookup)
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    return typ.accept(HasSelfType(lookup))


class HasSelfType(BoolTypeQuery):
    def __init__(self, lookup: Callable[[str], SymbolTableNode | None]) -> None:
        self.lookup = lookup
        super().__init__(ANY_STRATEGY)

    def visit_unbound_type(self, t: UnboundType) -> bool:
        sym = self.lookup(t.name)
        if sym and sym.fullname in SELF_TYPE_NAMES:
            return True
        return super().visit_unbound_type(t)


def unknown_unpack(t: Type) -> bool:
    """Check if a given type is an unpack of an unknown type.

    Unfortunately, there is no robust way to distinguish forward references from
    genuine undefined names here. But this worked well so far, although it looks
    quite fragile.
    """
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            if _native_typeanal_resolver is not None:
                result = _rust_unknown_unpack_live(
                    _native_typeanal_resolver,
                    _serialize_typeanal_type(t),
                )
            else:
                result = _rust_unknown_unpack(_serialize_typeanal_type(t))
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    if isinstance(t, UnpackType):
        unpacked = get_proper_type(t.type)
        if isinstance(unpacked, AnyType) and unpacked.type_of_any == TypeOfAny.special_form:
            return True
    return False


class FindTypeVarVisitor(SyntheticTypeVisitor[None]):
    """Type visitor that looks for type variable types and self types."""

    def __init__(self, api: SemanticAnalyzerCoreInterface, scope: TypeVarLikeScope) -> None:
        self.api = api
        self.scope = scope
        self.type_var_likes: list[tuple[str, TypeVarLikeExpr]] = []
        self.has_self_type = False
        self.include_callables = True

    def _seems_like_callable(self, type: UnboundType) -> bool:
        if not type.args:
            return False
        return isinstance(type.args[0], (EllipsisType, TypeList, ParamSpecType))

    def visit_unbound_type(self, t: UnboundType) -> None:
        name = t.name
        node = self.api.lookup_qualified(name, t)
        if node and node.fullname in SELF_TYPE_NAMES:
            self.has_self_type = True
        if (
            node
            and isinstance(node.node, TypeVarLikeExpr)
            and self.scope.get_binding(node) is None
        ):
            if (name, node.node) not in self.type_var_likes:
                self.type_var_likes.append((name, node.node))
        elif not self.include_callables and self._seems_like_callable(t):
            if find_self_type(
                t, lambda name: self.api.lookup_qualified(name, t, suppress_errors=True)
            ):
                self.has_self_type = True
            return
        elif node and node.fullname in LITERAL_TYPE_NAMES:
            return
        elif node and node.fullname in ANNOTATED_TYPE_NAMES and t.args:
            # Don't query the second argument to Annotated for TypeVars
            self.process_types([t.args[0]])
        elif t.args:
            self.process_types(t.args)

    def visit_type_list(self, t: TypeList) -> None:
        self.process_types(t.items)

    def visit_callable_argument(self, t: CallableArgument) -> None:
        t.typ.accept(self)

    def visit_any(self, t: AnyType) -> None:
        pass

    def visit_uninhabited_type(self, t: UninhabitedType) -> None:
        pass

    def visit_none_type(self, t: NoneType) -> None:
        pass

    def visit_erased_type(self, t: ErasedType) -> None:
        pass

    def visit_deleted_type(self, t: DeletedType) -> None:
        pass

    def visit_type_var(self, t: TypeVarType) -> None:
        self.process_types([t.upper_bound, t.default] + t.values)

    def visit_param_spec(self, t: ParamSpecType) -> None:
        self.process_types([t.upper_bound, t.default, t.prefix])

    def visit_type_var_tuple(self, t: TypeVarTupleType) -> None:
        self.process_types([t.upper_bound, t.default])

    def visit_unpack_type(self, t: UnpackType) -> None:
        self.process_types([t.type])

    def visit_parameters(self, t: Parameters) -> None:
        self.process_types(t.arg_types)

    def visit_partial_type(self, t: PartialType) -> None:
        pass

    def visit_instance(self, t: Instance) -> None:
        self.process_types(t.args)

    def visit_callable_type(self, t: CallableType) -> None:
        # FIX generics
        self.process_types(t.arg_types)
        t.ret_type.accept(self)

    def visit_tuple_type(self, t: TupleType) -> None:
        self.process_types(t.items)

    def visit_typeddict_type(self, t: TypedDictType) -> None:
        self.process_types(list(t.items.values()))

    def visit_raw_expression_type(self, t: RawExpressionType) -> None:
        pass

    def visit_literal_type(self, t: LiteralType) -> None:
        pass

    def visit_union_type(self, t: UnionType) -> None:
        self.process_types(t.items)

    def visit_overloaded(self, t: Overloaded) -> None:
        for it in t.items:
            it.accept(self)

    def visit_type_type(self, t: TypeType) -> None:
        t.item.accept(self)

    def visit_ellipsis_type(self, t: EllipsisType) -> None:
        pass

    def visit_placeholder_type(self, t: PlaceholderType) -> None:
        return self.process_types(t.args)

    def visit_type_alias_type(self, t: TypeAliasType) -> None:
        self.process_types(t.args)

    def process_types(self, types: list[Type] | tuple[Type, ...]) -> None:
        # Redundant type check helps mypyc.
        if isinstance(types, list):
            for t in types:
                t.accept(self)
        else:
            for t in types:
                t.accept(self)


class TypeVarDefaultTranslator(TrivialSyntheticTypeTranslator):
    """Type translate visitor that replaces UnboundTypes with in-scope TypeVars."""

    def __init__(
        self, api: SemanticAnalyzerInterface, tvar_expr_name: str, context: Context
    ) -> None:
        super().__init__()
        self.api = api
        self.tvar_expr_name = tvar_expr_name
        self.context = context

    def visit_unbound_type(self, t: UnboundType) -> Type:
        sym = self.api.lookup_qualified(t.name, t, suppress_errors=True)
        if sym is not None:
            if type_var := self.api.tvar_scope.get_binding(sym):
                return type_var
            if isinstance(sym.node, TypeVarLikeExpr):
                self.api.fail(
                    f'Type parameter "{self.tvar_expr_name}" has a default type '
                    "that refers to one or more type variables that are out of scope",
                    self.context,
                )
                return AnyType(TypeOfAny.from_error)
        return super().visit_unbound_type(t)

    def visit_type_alias_type(self, t: TypeAliasType) -> Type:
        # TypeAliasTypes are analyzed separately already, just return it
        return t


def check_vec_type_args(
    args: tuple[Type, ...] | list[Type], ctx: Context, api: SemanticAnalyzerCoreInterface
) -> bool:
    """Report an error if type args for 'vec' are invalid.

    Return False on error.
    """
    if _TYPEANAL_HAS_KERNEL and _native_typeanal_active:
        try:
            result = _rust_check_vec_type_args(args, ctx, api)
            if result is not None:
                return result
        except (AssertionError, NotImplementedError):
            pass
    ok = True
    if len(args) != 1:
        ok = False
    else:
        arg = get_proper_type(args[0])
        if isinstance(arg, Instance):
            if arg.type.fullname == "builtins.int":
                # A fixed-width integer such as 'i64' must be used instead of plain 'int'
                ok = False
        elif isinstance(arg, UnionType):
            non_optional = None
            items = [get_proper_type(item) for item in arg.items]
            if len(items) != 2:
                ok = False
            elif isinstance(items[0], NoneType):
                if not check_vec_type_args([items[1]], ctx, api):
                    # Error has already been reported so it's fine to return
                    return False
                non_optional = items[1]
            elif isinstance(items[1], NoneType):
                if not check_vec_type_args([items[0]], ctx, api):
                    # Error has already been reported so it's fine to return
                    return False
                non_optional = items[0]
            else:
                ok = False
            if isinstance(non_optional, Instance) and (
                non_optional.type.fullname in MYPYC_NATIVE_INT_NAMES
                or non_optional.type.fullname
                in ("builtins.int", "builtins.float", "builtins.bool", "librt.vecs.vec")
            ):
                ok = False
        elif isinstance(arg, TypeVarType):
            # Generic vec types aren't supported in type checked Python code, but
            # they can be provided in libraries implemented in C (e.g. append).
            if not api.is_stub_file:
                ok = False
        else:
            ok = False
    if not ok:
        api.fail('Invalid item type for "vec"', ctx)
    return ok
