"""Plugin for supporting the attrs library (http://www.attrs.org)"""

from __future__ import annotations

from collections import defaultdict
from collections.abc import Callable, Iterable, Mapping
from functools import reduce
from typing import Final, Literal, cast

import mypy.plugin  # To avoid circular imports.
from mypy.applytype import apply_generic_arguments
from mypy.errorcodes import LITERAL_REQ
from mypy.expandtype import expand_type, expand_type_by_instance
from mypy.exprtotype import TypeTranslationError, expr_to_unanalyzed_type
from mypy.meet import meet_types
from mypy.messages import format_type_bare
from mypy.nodes import (
    ARG_NAMED,
    ARG_NAMED_OPT,
    ARG_OPT,
    ARG_POS,
    MDEF,
    Argument,
    AssignmentStmt,
    CallExpr,
    Context,
    Decorator,
    Expression,
    FuncDef,
    IndexExpr,
    JsonDict,
    LambdaExpr,
    ListExpr,
    MemberExpr,
    NameExpr,
    OverloadedFuncDef,
    PlaceholderNode,
    RefExpr,
    SymbolTableNode,
    TempNode,
    TupleExpr,
    TypeApplication,
    TypeInfo,
    TypeVarExpr,
    Var,
    is_class_var,
)
from mypy.plugin import SemanticAnalyzerPluginInterface
from mypy.plugins.common import (
    _get_argument,
    _get_bool_argument,
    _get_decorator_bool_argument,
    add_attribute_to_class,
    add_method_to_class,
    deserialize_and_fixup_type,
)
from mypy.server.trigger import make_wildcard_trigger
from mypy.state import state
from mypy.typeops import (
    get_type_vars,
    make_simplified_union,
    map_type_from_supertype,
    type_object_type,
)
from mypy.types import (
    AnyType,
    CallableType,
    FunctionLike,
    Instance,
    LiteralType,
    NoneType,
    Overloaded,
    ProperType,
    TupleType,
    Type,
    TypeOfAny,
    TypeType,
    TypeVarId,
    TypeVarType,
    UninhabitedType,
    UnionType,
    get_proper_type,
    is_unannotated_any,
)
from mypy.typevars import fill_typevars
from mypy.util import unmangle

# ---------------------------------------------------------------------------
# Native type-kernel gate for attrs plugin (Issue #357).
# ---------------------------------------------------------------------------

# Module-level flag set by build.py during build start, mirroring the pattern
# used by other type-kernel consumers (erasetype, subtypes, join, etc.).
_HAS_NATIVE_ATTRS: bool = False


def _set_native_attrs_active(active: bool) -> None:
    """Set the attrs plugin native gate. Called from mypy/build.py."""
    global _HAS_NATIVE_ATTRS
    _HAS_NATIVE_ATTRS = active


# Lazy import of the native seam function.
_rust_transform_attrs_fn: Callable[..., bytes | None] | None = None


def _get_rust_transform_attrs() -> Callable[..., bytes | None] | None:
    """Lazily import rust_transform_attrs from type_kernel."""
    global _rust_transform_attrs_fn
    if _rust_transform_attrs_fn is not None:
        return _rust_transform_attrs_fn
    if _HAS_NATIVE_ATTRS:
        try:
            from type_kernel import rust_transform_attrs as _fn

            _rust_transform_attrs_fn = _fn
        except ImportError:
            pass
    return _rust_transform_attrs_fn


# ---------------------------------------------------------------------------
# Attrs wire-format helpers for the Rust seam (Issue #357).
# ---------------------------------------------------------------------------

# Wire tags for attrs metadata (non-colliding with wire.rs tags).
_ATTRS_FIELD_TAG = 200
_ATTRS_INIT_SIG = 201
_ATTRS_METHOD_INFO = 202


def _serialize_attrs_fields_for_rust(attributes: list[Attribute]) -> bytes:
    """Serialize field metadata into bytes for rust_transform_attrs.

    Wire: count(short-int) + for each field:
      ATTRS_FIELD_TAG(200) + name_len(short-int) + name(utf8) +
      init_type_len(short-int) + init_type_bytes (if >0, else 0) +
      has_default(bool) + kw_only(bool) + init(bool) +
      converter_init_type_len(short-int) + converter_init_type_bytes +
      converter_ret_type_len(short-int) + converter_ret_type_bytes
    """
    # We use a bytearray and write a compact wire format matching
    # what Rust's read_attr_field expects.

    def _write_short_int(val: int) -> None:
        """Write 1-byte short-int for values in [-10..117]."""
        MIN_ONE = -10
        if MIN_ONE <= val <= 117:
            buf.append(((val - MIN_ONE) << 1) & 0xFF)
        else:
            # For large values we use 2-byte form.
            val2 = val - (-100)
            if 0 <= val2 <= 16383:
                payload = (val2 << 2) | 1
                buf.append(payload & 0xFF)
                buf.append((payload >> 8) & 0xFF)
            else:
                raise ValueError(f"short-int overflow: {val}")

    def _write_type_bytes(typ: Type) -> int:
        """Serialize a Type to wire bytes, return length."""
        from mypy.cache import WriteBuffer

        wb = WriteBuffer()
        typ.write(wb)
        raw = wb.getvalue()
        buf.extend(raw)
        return len(raw)

    def _write_type_opt(typ: Type | None) -> None:
        """Write a length-prefixed optional type."""
        if typ is not None:
            _write_short_int(_write_type_bytes(typ))
        else:
            _write_short_int(0)

    buf = bytearray()
    _write_short_int(len(attributes))

    for attr in attributes:
        # Tag.
        buf.append(_ATTRS_FIELD_TAG)

        # Name.
        name_bytes = attr.name.encode("utf-8")
        _write_short_int(len(name_bytes))
        buf.extend(name_bytes)

        # Alias (not used in wire, Python handles it).
        _write_short_int(0)

        # init_type.
        _write_type_opt(attr.init_type)

        # has_default, kw_only, init.
        buf.append(1 if attr.has_default else 0)
        buf.append(1 if attr.kw_only else 0)
        buf.append(1 if attr.init else 0)

        # Converter init type.
        conv_init = attr.converter.init_type if attr.converter else None
        _write_type_opt(conv_init)

        # Converter return type.
        conv_ret = attr.converter.ret_type if attr.converter else None
        _write_type_opt(conv_ret)

    return bytes(buf)


def _try_native_init_injection(
    ctx: mypy.plugin.ClassDefContext,
    attributes: list[Attribute],
    adder: MethodAdder,
    method_name: str,
) -> bool:
    """Try to inject __init__ via the Rust kernel.

    Returns True if the native path succeeded, False otherwise (fall back
    to the Python implementation in _add_init).
    """
    rust_fn = _get_rust_transform_attrs()
    if rust_fn is None:
        return False

    try:
        fields_bytes = _serialize_attrs_fields_for_rust(attributes)
        class_fullname = ctx.cls.fullname
        add_order = True  # We'll handle ordering methods separately.

        result = rust_fn(fields_bytes, class_fullname, method_name, add_order)
        if result is None:
            return False

        # Deserialize the result and inject the method.
        _apply_native_init(ctx, result, attributes, adder, method_name)
        return True
    except Exception:
        # Any failure falls back to the Python implementation.
        return False


def _apply_native_init(
    ctx: mypy.plugin.ClassDefContext,
    rust_result: bytes,
    attributes: list[Attribute],
    adder: MethodAdder,
    method_name: str,
) -> None:
    """Apply the native init result to the class.

    The Rust result contains serialized arg structure (names, kinds) and
    types. We reconstruct Argument objects from the original Attribute
    data (which has live types) using the structure Rust computed.
    """
    buf = bytearray(rust_result)
    pos = 0

    def _read_u8() -> int:
        nonlocal pos
        b = buf[pos]
        pos += 1
        return b

    def _read_short_int() -> int:
        nonlocal pos
        first = _read_u8()
        TWO_BYTES_BIT = 1
        FOUR_BYTES_BIT = 2
        MIN_ONE = -10
        if (first & TWO_BYTES_BIT) == 0:
            return (first >> 1) + MIN_ONE
        elif (first & FOUR_BYTES_BIT) == 0:
            second = _read_u8()
            return (second << 6) + (first >> 2) - 100
        else:
            second = _read_u8()
            pos += 2
            two_more = buf[pos - 2] | (buf[pos - 1] << 8)
            higher = (two_more << 13) + (second << 5)
            return higher + (first >> 3) - 10000

    # Skip the ATTRS_INIT_SIG tag.
    _read_u8()

    # Read and skip the init name and info blob; only the arg structure is
    # consumed here (Python rebuilds names and types from the Attribute data).
    init_name_len = _read_short_int()
    pos += init_name_len

    init_info_len = _read_short_int()
    pos += init_info_len

    # Read ordering methods info (we don't inject them here; Python handles
    # ordering methods via _add_order which is unchanged).

    # Now build __init__ args from the original Attribute data.
    # The Rust side computed the structure (positional vs keyword-only,
    # arg_kinds, names). We apply the actual types from the attributes.
    pos_args: list[Argument] = []
    kw_only_args: list[Argument] = []
    sym_table = ctx.cls.info.names

    for attr in attributes:
        if not attr.init:
            continue

        # Compute init_type (same logic as Attribute.argument).
        init_type: Type | None = None
        if attr.converter:
            if attr.converter.init_type:
                init_type = attr.converter.init_type
                if init_type and attr.init_type and attr.converter.ret_type:
                    converter_vars = get_type_vars(attr.converter.ret_type)
                    init_vars = get_type_vars(attr.init_type)
                    if converter_vars and len(converter_vars) == len(init_vars):
                        variables = {
                            binder.id: arg for binder, arg in zip(converter_vars, init_vars)
                        }
                        init_type = expand_type(attr.init_type, variables)
            else:
                ctx.api.fail("Cannot determine __init__ type from converter", attr.context)
                init_type = AnyType(TypeOfAny.from_error)
        else:
            init_type = attr.init_type or ctx.cls.info[attr.name].type

        unannotated = False
        if init_type is None:
            unannotated = True
            init_type = AnyType(TypeOfAny.unannotated)
        else:
            proper_type = get_proper_type(init_type)
            if isinstance(proper_type, AnyType):
                if proper_type.type_of_any == TypeOfAny.unannotated:
                    unannotated = True

        if unannotated and ctx.api.options.disallow_untyped_defs:
            node = ctx.cls.info[attr.name].node
            assert node is not None
            ctx.api.msg.need_annotation_for_var(node, attr.context)

        # Attrs removes leading underscores.
        name = attr.alias or attr.name.lstrip("_")

        if attr.kw_only:
            arg_kind = ARG_NAMED_OPT if attr.has_default else ARG_NAMED
        else:
            arg_kind = ARG_OPT if attr.has_default else ARG_POS

        arg = Argument(Var(name, init_type), init_type, None, arg_kind)

        if attr.kw_only:
            kw_only_args.append(arg)
        else:
            pos_args.append(arg)

        # Final flag handling.
        if not attr.has_default and attr.name in sym_table:
            sym_node = sym_table[attr.name].node
            if isinstance(sym_node, Var) and sym_node.is_final:
                sym_node.final_set_in_init = True

    args = pos_args + kw_only_args
    if all(arg.variable.type and is_unannotated_any(arg.variable.type) for arg in args):
        for a in args:
            a.variable.type = AnyType(TypeOfAny.implementation_artifact)
            a.type_annotation = AnyType(TypeOfAny.implementation_artifact)
    adder.add_method(method_name, args, NoneType())


attr_class_makers: Final = {"attr.s", "attr.attrs", "attr.attributes"}
attr_dataclass_makers: Final = {"attr.dataclass"}
attr_frozen_makers: Final = {"attr.frozen", "attrs.frozen"}
attr_define_makers: Final = {"attr.define", "attr.mutable", "attrs.define", "attrs.mutable"}
attr_attrib_makers: Final = {"attr.ib", "attr.attrib", "attr.attr", "attr.field", "attrs.field"}
attr_optional_converters: Final = {"attr.converters.optional", "attrs.converters.optional"}

SELF_TVAR_NAME: Final = "_AT"
MAGIC_ATTR_NAME: Final = "__attrs_attrs__"
MAGIC_ATTR_CLS_NAME_TEMPLATE: Final = "__{}_AttrsAttributes__"  # The tuple subclass pattern.
ATTRS_INIT_NAME: Final = "__attrs_init__"


class Converter:
    """Holds information about a `converter=` argument"""

    def __init__(self, init_type: Type | None = None, ret_type: Type | None = None) -> None:
        self.init_type = init_type
        self.ret_type = ret_type


class Attribute:
    """The value of an attr.ib() call."""

    def __init__(
        self,
        name: str,
        alias: str | None,
        info: TypeInfo,
        has_default: bool,
        init: bool,
        kw_only: bool,
        converter: Converter | None,
        context: Context,
        init_type: Type | None,
    ) -> None:
        self.name = name
        self.alias = alias
        self.info = info
        self.has_default = has_default
        self.init = init
        self.kw_only = kw_only
        self.converter = converter
        self.context = context
        self.init_type = init_type

    def argument(self, ctx: mypy.plugin.ClassDefContext) -> Argument:
        """Return this attribute as an argument to __init__."""
        assert self.init
        init_type: Type | None = None
        if self.converter:
            if self.converter.init_type:
                init_type = self.converter.init_type
                if init_type and self.init_type and self.converter.ret_type:
                    # The converter return type should be the same type as the attribute type.
                    # Copy type vars from attr type to converter.
                    converter_vars = get_type_vars(self.converter.ret_type)
                    init_vars = get_type_vars(self.init_type)
                    if converter_vars and len(converter_vars) == len(init_vars):
                        variables = {
                            binder.id: arg for binder, arg in zip(converter_vars, init_vars)
                        }
                        init_type = expand_type(init_type, variables)
            else:
                ctx.api.fail("Cannot determine __init__ type from converter", self.context)
                init_type = AnyType(TypeOfAny.from_error)
        else:  # There is no converter, the init type is the normal type.
            init_type = self.init_type or self.info[self.name].type

        unannotated = False
        if init_type is None:
            unannotated = True
            # Convert type not set to Any.
            init_type = AnyType(TypeOfAny.unannotated)
        else:
            proper_type = get_proper_type(init_type)
            if isinstance(proper_type, AnyType):
                if proper_type.type_of_any == TypeOfAny.unannotated:
                    unannotated = True

        if unannotated and ctx.api.options.disallow_untyped_defs:
            # This is a compromise.  If you don't have a type here then the
            # __init__ will be untyped. But since the __init__ is added it's
            # pointing at the decorator. So instead we also show the error in the

            # assignment, which is where you would fix the issue.
            node = self.info[self.name].node
            assert node is not None
            ctx.api.msg.need_annotation_for_var(node, self.context)

        if self.kw_only:
            arg_kind = ARG_NAMED_OPT if self.has_default else ARG_NAMED
        else:
            arg_kind = ARG_OPT if self.has_default else ARG_POS

        # Attrs removes leading underscores when creating the __init__ arguments.
        name = self.alias or self.name.lstrip("_")
        return Argument(Var(name, init_type), init_type, None, arg_kind)

    def serialize(self) -> JsonDict:
        """Serialize this object so it can be saved and restored."""
        return {
            "name": self.name,
            "alias": self.alias,
            "has_default": self.has_default,
            "init": self.init,
            "kw_only": self.kw_only,
            "has_converter": self.converter is not None,
            "converter_init_type": (
                self.converter.init_type.serialize()
                if self.converter and self.converter.init_type
                else None
            ),
            "context_line": self.context.line,
            "context_column": self.context.column,
            "init_type": self.init_type.serialize() if self.init_type else None,
        }

    @classmethod
    def deserialize(
        cls, info: TypeInfo, data: JsonDict, api: SemanticAnalyzerPluginInterface
    ) -> Attribute:
        """Return the Attribute that was serialized."""
        raw_init_type = data["init_type"]
        init_type = deserialize_and_fixup_type(raw_init_type, api) if raw_init_type else None
        raw_converter_init_type = data["converter_init_type"]
        converter_init_type = (
            deserialize_and_fixup_type(raw_converter_init_type, api)
            if raw_converter_init_type
            else None
        )

        return Attribute(
            data["name"],
            data["alias"],
            info,
            data["has_default"],
            data["init"],
            data["kw_only"],
            Converter(converter_init_type) if data["has_converter"] else None,
            Context(line=data["context_line"], column=data["context_column"]),
            init_type,
        )

    def expand_typevar_from_subtype(self, sub_type: TypeInfo) -> None:
        """Expands type vars in the context of a subtype when an attribute is inherited
        from a generic super type."""
        if self.init_type:
            self.init_type = map_type_from_supertype(self.init_type, sub_type, self.info)
        else:
            self.init_type = None


def _determine_eq_order(ctx: mypy.plugin.ClassDefContext) -> bool:
    """
    Validate the combination of *cmp*, *eq*, and *order*. Derive the effective
    value of order.
    """
    cmp = _get_decorator_optional_bool_argument(ctx, "cmp")
    eq = _get_decorator_optional_bool_argument(ctx, "eq")
    order = _get_decorator_optional_bool_argument(ctx, "order")

    if cmp is not None and any((eq is not None, order is not None)):
        ctx.api.fail('Don\'t mix "cmp" with "eq" and "order"', ctx.reason)

    # cmp takes precedence due to bw-compatibility.
    if cmp is not None:
        return cmp

    # If left None, equality is on and ordering mirrors equality.
    if eq is None:
        eq = True

    if order is None:
        order = eq

    if eq is False and order is True:
        ctx.api.fail("eq must be True if order is True", ctx.reason)

    return order


def _get_decorator_optional_bool_argument(
    ctx: mypy.plugin.ClassDefContext, name: str, default: bool | None = None
) -> bool | None:
    """Return the Optional[bool] argument for the decorator.

    This handles both @decorator(...) and @decorator.
    """
    if isinstance(ctx.reason, CallExpr):
        attr_value = _get_argument(ctx.reason, name)
        if attr_value:
            if isinstance(attr_value, NameExpr):
                if attr_value.fullname == "builtins.True":
                    return True
                if attr_value.fullname == "builtins.False":
                    return False
                if attr_value.fullname == "builtins.None":
                    return None
            ctx.api.fail(
                f'"{name}" argument must be a True, False, or None literal',
                ctx.reason,
                code=LITERAL_REQ,
            )
            return default
        return default
    else:
        return default


def attr_tag_callback(ctx: mypy.plugin.ClassDefContext) -> None:
    """Record that we have an attrs class in the main semantic analysis pass.

    The later pass implemented by attr_class_maker_callback will use this
    to detect attrs classes in base classes.
    """
    # The value is ignored, only the existence matters.
    ctx.cls.info.metadata["attrs_tag"] = {}


def attr_class_maker_callback(
    ctx: mypy.plugin.ClassDefContext,
    auto_attribs_default: bool | None = False,
    frozen_default: bool = False,
    slots_default: bool = False,
) -> bool:
    """Add necessary dunder methods to classes decorated with attr.s.

    attrs is a package that lets you define classes without writing dull boilerplate code.

    At a quick glance, the decorator searches the class body for assignments of `attr.ib`s (or
    annotated variables if auto_attribs=True), then depending on how the decorator is called,
    it will add an __init__ or all the compare methods.
    For frozen=True it will turn the attrs into properties.

    Hashability will be set according to https://www.attrs.org/en/stable/hashing.html.

    See https://www.attrs.org/en/stable/how-does-it-work.html for information on how attrs works.

    If this returns False, some required metadata was not ready yet, and we need another
    pass.
    """
    with state.strict_optional_set(ctx.api.options.strict_optional):
        # This hook is called during semantic analysis, but it uses a bunch of
        # type-checking ops, so it needs the strict optional set properly.
        return attr_class_maker_callback_impl(
            ctx, auto_attribs_default, frozen_default, slots_default
        )


def attr_class_maker_callback_impl(
    ctx: mypy.plugin.ClassDefContext,
    auto_attribs_default: bool | None,
    frozen_default: bool,
    slots_default: bool,
) -> bool:
    info = ctx.cls.info

    init = _get_decorator_bool_argument(ctx, "init", True)
    frozen = _get_frozen(ctx, frozen_default)
    order = _determine_eq_order(ctx)
    slots = _get_decorator_bool_argument(ctx, "slots", slots_default)

    auto_attribs = _get_decorator_optional_bool_argument(ctx, "auto_attribs", auto_attribs_default)
    kw_only = _get_decorator_bool_argument(ctx, "kw_only", False)
    match_args = _get_decorator_bool_argument(ctx, "match_args", True)

    for super_info in ctx.cls.info.mro[1:-1]:
        if "attrs_tag" in super_info.metadata and "attrs" not in super_info.metadata:
            # Super class is not ready yet. Request another pass.
            return False

    attributes = _analyze_class(ctx, auto_attribs, kw_only)

    # Check if attribute types are ready.
    for attr in attributes:
        node = info.get(attr.name)
        if node is None:
            # This name is likely blocked by some semantic analysis error that
            # should have been reported already.
            _add_empty_metadata(info)
            return True

    _add_attrs_magic_attribute(ctx, [(attr.name, info[attr.name].type) for attr in attributes])
    if slots:
        _add_slots(ctx, attributes)
    if match_args:
        _add_match_args(ctx, attributes)

    # Save the attributes so that subclasses can reuse them.
    ctx.cls.info.metadata["attrs"] = {
        "attributes": [attr.serialize() for attr in attributes],
        "frozen": frozen,
    }

    adder = MethodAdder(ctx)
    # If __init__ is not being generated, attrs still generates it as __attrs_init__
    # instead.
    _add_init(ctx, attributes, adder, "__init__" if init else ATTRS_INIT_NAME)

    if order:
        _add_order(ctx, adder)
    if frozen:
        _make_frozen(ctx, attributes)
        # Frozen classes are hashable by default, even if inheriting from non-frozen ones.
        hashable: bool | None = _get_decorator_bool_argument(
            ctx, "hash", True
        ) and _get_decorator_bool_argument(ctx, "unsafe_hash", True)
    else:
        hashable = _get_decorator_optional_bool_argument(ctx, "unsafe_hash")
        if hashable is None:  # unspecified
            hashable = _get_decorator_optional_bool_argument(ctx, "hash")

    eq = _get_decorator_optional_bool_argument(ctx, "eq")
    has_own_hash = "__hash__" in ctx.cls.info.names

    if has_own_hash or (hashable is None and eq is False):
        pass  # Do nothing.
    elif hashable:
        # We copy the `__hash__` signature from `object` to make them hashable.
        ctx.cls.info.names["__hash__"] = ctx.cls.info.mro[-1].names["__hash__"]
    else:
        _remove_hashability(ctx)

    return True


def _get_frozen(ctx: mypy.plugin.ClassDefContext, frozen_default: bool) -> bool:
    """Return whether this class is frozen."""
    if _get_decorator_bool_argument(ctx, "frozen", frozen_default):
        return True
    # Subclasses of frozen classes are frozen so check that.
    for super_info in ctx.cls.info.mro[1:-1]:
        if "attrs" in super_info.metadata and super_info.metadata["attrs"]["frozen"]:
            return True
    return False


def _analyze_class(
    ctx: mypy.plugin.ClassDefContext, auto_attribs: bool | None, class_kw_only: bool
) -> list[Attribute]:
    """Analyze the class body of an attr maker, its parents, and return the Attributes found.

    auto_attribs=True means we'll generate attributes from type annotations also.
    auto_attribs=None means we'll detect which mode to use.
    class_kw_only=True means that all attributes created here will be keyword only args by
        default in __init__.
    """
    own_attrs: dict[str, Attribute] = {}
    if auto_attribs is None:
        auto_attribs = _detect_auto_attribs(ctx)

    # Walk the body looking for assignments and decorators.
    for stmt in ctx.cls.defs.body:
        if isinstance(stmt, AssignmentStmt):
            for attr in _attributes_from_assignment(ctx, stmt, auto_attribs, class_kw_only):
                # When attrs are defined twice in the same body we want to use the 2nd definition
                # in the 2nd location. So remove it from the OrderedDict.
                # Unless it's auto_attribs in which case we want the 2nd definition in the

                # 1st location.
                if not auto_attribs and attr.name in own_attrs:
                    del own_attrs[attr.name]
                own_attrs[attr.name] = attr
        elif isinstance(stmt, Decorator):
            _cleanup_decorator(stmt, own_attrs)

    for attribute in own_attrs.values():
        # Even though these look like class level assignments we want them to look like
        # instance level assignments.
        if attribute.name in ctx.cls.info.names:
            node = ctx.cls.info.names[attribute.name].node
            if isinstance(node, PlaceholderNode):
                # This node is not ready yet.
                continue
            assert isinstance(node, Var), node
            node.is_initialized_in_class = False

    # Traverse the MRO and collect attributes from the parents.
    taken_attr_names = set(own_attrs)
    super_attrs = []
    for super_info in ctx.cls.info.mro[1:-1]:
        if "attrs" in super_info.metadata:
            # Each class depends on the set of attributes in its attrs ancestors.
            ctx.api.add_plugin_dependency(make_wildcard_trigger(super_info.fullname))

            for data in super_info.metadata["attrs"]["attributes"]:
                # Only add an attribute if it hasn't been defined before.  This
                # allows for overwriting attribute definitions by subclassing.
                if data["name"] not in taken_attr_names:
                    a = Attribute.deserialize(super_info, data, ctx.api)
                    a.expand_typevar_from_subtype(ctx.cls.info)
                    super_attrs.append(a)
                    taken_attr_names.add(a.name)
    attributes = super_attrs + list(own_attrs.values())

    # Check the init args for correct default-ness.  Note: This has to be done after all the
    # attributes for all classes have been read, because subclasses can override parents.
    last_default = False

    for i, attribute in enumerate(attributes):
        if not attribute.init:
            continue

        if attribute.kw_only:
            # Keyword-only attributes don't care whether they are default or not.
            continue

        # If the issue comes from merging different classes, report it
        # at the class definition point.
        context = attribute.context if i >= len(super_attrs) else ctx.cls

        if not attribute.has_default and last_default:
            ctx.api.fail("Non-default attributes not allowed after default attributes.", context)
        last_default |= attribute.has_default

    return attributes


def _add_empty_metadata(info: TypeInfo) -> None:
    """Add empty metadata to mark that we've finished processing this class."""
    info.metadata["attrs"] = {"attributes": [], "frozen": False}


def _detect_auto_attribs(ctx: mypy.plugin.ClassDefContext) -> bool:
    """Return whether auto_attribs should be enabled or disabled.

    It's disabled if there are any unannotated attribs()
    """
    for stmt in ctx.cls.defs.body:
        if isinstance(stmt, AssignmentStmt):
            for lvalue in stmt.lvalues:
                lvalues, rvalues = _parse_assignments(lvalue, stmt)

                if len(lvalues) != len(rvalues):
                    # This means we have some assignment that isn't 1 to 1.
                    # It can't be an attrib.
                    continue

                for lhs, rvalue in zip(lvalues, rvalues):
                    # Check if the right hand side is a call to an attribute maker.
                    if (
                        isinstance(rvalue, CallExpr)
                        and isinstance(rvalue.callee, RefExpr)
                        and rvalue.callee.fullname in attr_attrib_makers
                        and not stmt.new_syntax
                    ):
                        # This means we have an attrib without an annotation and so
                        # we can't do auto_attribs=True
                        return False
    return True


def _attributes_from_assignment(
    ctx: mypy.plugin.ClassDefContext, stmt: AssignmentStmt, auto_attribs: bool, class_kw_only: bool
) -> Iterable[Attribute]:
    """Return Attribute objects that are created by this assignment.

    The assignments can look like this:
        x = attr.ib()
        x = y = attr.ib()
        x, y = attr.ib(), attr.ib()
    or if auto_attribs is enabled also like this:
        x: type
        x: type = default_value
        x: type = attr.ib(...)
    """
    for lvalue in stmt.lvalues:
        lvalues, rvalues = _parse_assignments(lvalue, stmt)

        if len(lvalues) != len(rvalues):
            # This means we have some assignment that isn't 1 to 1.
            # It can't be an attrib.
            continue

        for lhs, rvalue in zip(lvalues, rvalues):
            # Check if the right hand side is a call to an attribute maker.
            if (
                isinstance(rvalue, CallExpr)
                and isinstance(rvalue.callee, RefExpr)
                and rvalue.callee.fullname in attr_attrib_makers
            ):
                attr = _attribute_from_attrib_maker(
                    ctx, auto_attribs, class_kw_only, lhs, rvalue, stmt
                )
                if attr:
                    yield attr
            elif auto_attribs and stmt.type and stmt.new_syntax and not is_class_var(lhs):
                yield _attribute_from_auto_attrib(ctx, class_kw_only, lhs, rvalue, stmt)


def _cleanup_decorator(stmt: Decorator, attr_map: dict[str, Attribute]) -> None:
    """Handle decorators in class bodies.

    `x.default` will set a default value on x
    `x.validator` and `x.default` will get removed to avoid throwing a type error.
    """
    remove_me = []
    for func_decorator in stmt.decorators:
        if (
            isinstance(func_decorator, MemberExpr)
            and isinstance(func_decorator.expr, NameExpr)
            and func_decorator.expr.name in attr_map
        ):
            if func_decorator.name == "default":
                attr_map[func_decorator.expr.name].has_default = True

            if func_decorator.name in ("default", "validator"):
                # These are decorators on the attrib object that only exist during
                # class creation time.  In order to not trigger a type error later we
                # just remove them.  This might leave us with a Decorator with no

                # decorators (Emperor's new clothes?)
                # TODO: It would be nice to type-check these rather than remove them.
                #       default should be Callable[[], T]

                #       validator should be Callable[[Any, 'Attribute', T], Any]
                #       where T is the type of the attribute.
                remove_me.append(func_decorator)
    for dec in remove_me:
        stmt.decorators.remove(dec)


def _attribute_from_auto_attrib(
    ctx: mypy.plugin.ClassDefContext,
    class_kw_only: bool,
    lhs: NameExpr,
    rvalue: Expression,
    stmt: AssignmentStmt,
) -> Attribute:
    """Return an Attribute for a new type assignment."""
    name = unmangle(lhs.name)
    # `x: int` (without equal sign) assigns rvalue to TempNode(AnyType())
    has_rhs = not isinstance(rvalue, TempNode)
    sym = ctx.cls.info.names.get(name)
    init_type = sym.type if sym else None
    return Attribute(name, None, ctx.cls.info, has_rhs, True, class_kw_only, None, stmt, init_type)


def _attribute_from_attrib_maker(
    ctx: mypy.plugin.ClassDefContext,
    auto_attribs: bool,
    class_kw_only: bool,
    lhs: NameExpr,
    rvalue: CallExpr,
    stmt: AssignmentStmt,
) -> Attribute | None:
    """Return an Attribute from the assignment or None if you can't make one."""
    if auto_attribs and not stmt.new_syntax:
        # auto_attribs requires an annotation on *every* attr.ib.
        assert lhs.node is not None
        ctx.api.msg.need_annotation_for_var(lhs.node, stmt)
        return None

    if len(stmt.lvalues) > 1:
        ctx.api.fail("Too many names for one attribute", stmt)
        return None

    # This is the type that belongs in the __init__ method for this attrib.
    init_type = stmt.type

    # Read all the arguments from the call.
    init = _get_bool_argument(ctx, rvalue, "init", True)
    # The class decorator kw_only value can be overridden by the attribute value
    # See https://github.com/python-attrs/attrs/pull/1457
    kw_only = _get_bool_argument(ctx, rvalue, "kw_only", class_kw_only)

    # TODO: Check for attr.NOTHING
    attr_has_default = bool(_get_argument(rvalue, "default"))
    attr_has_factory = bool(_get_argument(rvalue, "factory"))

    if attr_has_default and attr_has_factory:
        ctx.api.fail('Can\'t pass both "default" and "factory".', rvalue)
    elif attr_has_factory:
        attr_has_default = True

    # If the type isn't set through annotation but is passed through `type=` use that.
    type_arg = _get_argument(rvalue, "type")
    if type_arg and not init_type:
        try:
            un_type = expr_to_unanalyzed_type(type_arg, ctx.api.options, ctx.api.is_stub_file)
        except TypeTranslationError:
            ctx.api.fail("Invalid argument to type", type_arg)
        else:
            init_type = ctx.api.anal_type(un_type)
            if init_type and isinstance(lhs.node, Var) and not lhs.node.type:
                # If there is no annotation, add one.
                lhs.node.type = init_type
                lhs.is_inferred_def = False

    # Note: convert is deprecated but works the same as converter.
    converter = _get_argument(rvalue, "converter")
    convert = _get_argument(rvalue, "convert")
    if convert and converter:
        ctx.api.fail('Can\'t pass both "convert" and "converter".', rvalue)
    elif convert:
        ctx.api.fail("convert is deprecated, use converter", rvalue)
        converter = convert
    converter_info = _parse_converter(ctx, converter)

    # Custom alias might be defined:
    alias = None
    alias_expr = _get_argument(rvalue, "alias")
    if alias_expr:
        alias = ctx.api.parse_str_literal(alias_expr)
        if alias is None:
            ctx.api.fail(
                '"alias" argument to attrs field must be a string literal',
                rvalue,
                code=LITERAL_REQ,
            )
    name = unmangle(lhs.name)
    return Attribute(
        name, alias, ctx.cls.info, attr_has_default, init, kw_only, converter_info, stmt, init_type
    )


def _parse_converter(
    ctx: mypy.plugin.ClassDefContext, converter_expr: Expression | None
) -> Converter | None:
    """Return the Converter object from an Expression."""
    # TODO: Support complex converters, e.g. lambdas, calls, etc.
    if not converter_expr:
        return None
    converter_info = Converter()
    if (
        isinstance(converter_expr, CallExpr)
        and isinstance(converter_expr.callee, RefExpr)
        and converter_expr.callee.fullname in attr_optional_converters
        and converter_expr.args
        and converter_expr.args[0]
    ):
        # Special handling for attr.converters.optional(type)
        # We extract the type and add make the init_args Optional in Attribute.argument
        converter_expr = converter_expr.args[0]
        is_attr_converters_optional = True
    else:
        is_attr_converters_optional = False

    converter_type: Type | None = None
    if isinstance(converter_expr, RefExpr) and converter_expr.node:
        if isinstance(converter_expr.node, FuncDef):
            if converter_expr.node.type and isinstance(converter_expr.node.type, FunctionLike):
                converter_type = converter_expr.node.type
            else:  # The converter is an unannotated function.
                converter_info.init_type = AnyType(TypeOfAny.unannotated)
                return converter_info
        elif isinstance(converter_expr.node, OverloadedFuncDef) and is_valid_overloaded_converter(
            converter_expr.node
        ):
            converter_type = converter_expr.node.type
        elif isinstance(converter_expr.node, TypeInfo):
            converter_type = type_object_type(converter_expr.node)
    elif (
        isinstance(converter_expr, IndexExpr)
        and isinstance(converter_expr.analyzed, TypeApplication)
        and isinstance(converter_expr.base, RefExpr)
        and isinstance(converter_expr.base.node, TypeInfo)
    ):
        # The converter is a generic type.
        converter_type = type_object_type(converter_expr.base.node)
        if isinstance(converter_type, CallableType):
            converter_type = apply_generic_arguments(
                converter_type,
                converter_expr.analyzed.types,
                ctx.api.msg.incompatible_typevar_value,
                converter_type,
            )
        else:
            converter_type = None

    if isinstance(converter_expr, LambdaExpr):
        # TODO: should we send a fail if converter_expr.min_args > 1?
        converter_info.init_type = AnyType(TypeOfAny.unannotated)
        return converter_info

    if not converter_type:
        # Signal that we have an unsupported converter.
        ctx.api.fail(
            "Unsupported converter, only named functions, types and lambdas are currently "
            "supported",
            converter_expr,
        )
        converter_info.init_type = AnyType(TypeOfAny.from_error)
        return converter_info

    converter_type = get_proper_type(converter_type)
    if isinstance(converter_type, CallableType) and converter_type.arg_types:
        converter_info.init_type = converter_type.arg_types[0]
        if not is_attr_converters_optional:
            converter_info.ret_type = converter_type.ret_type
    elif isinstance(converter_type, Overloaded):
        types: list[Type] = []
        for item in converter_type.items:
            # Walk the overloads looking for methods that can accept one argument.
            num_arg_types = len(item.arg_types)
            if not num_arg_types:
                continue
            if num_arg_types > 1 and any(kind == ARG_POS for kind in item.arg_kinds[1:]):
                continue
            types.append(item.arg_types[0])
        # Make a union of all the valid types.
        if types:
            converter_info.init_type = make_simplified_union(types)

    if is_attr_converters_optional and converter_info.init_type:
        # If the converter was attr.converter.optional(type) then add None to
        # the allowed init_type.
        converter_info.init_type = UnionType.make_union([converter_info.init_type, NoneType()])

    return converter_info


def is_valid_overloaded_converter(defn: OverloadedFuncDef) -> bool:
    return all(
        (not isinstance(item, Decorator) or isinstance(item.func.type, FunctionLike))
        for item in defn.items
    )


def _parse_assignments(
    lvalue: Expression, stmt: AssignmentStmt
) -> tuple[list[NameExpr], list[Expression]]:
    """Convert a possibly complex assignment expression into lists of lvalues and rvalues."""
    lvalues: list[NameExpr] = []
    rvalues: list[Expression] = []
    if isinstance(lvalue, (TupleExpr, ListExpr)):
        if all(isinstance(item, NameExpr) for item in lvalue.items):
            lvalues = cast(list[NameExpr], lvalue.items)
        if isinstance(stmt.rvalue, (TupleExpr, ListExpr)):
            rvalues = stmt.rvalue.items
    elif isinstance(lvalue, NameExpr):
        lvalues = [lvalue]
        rvalues = [stmt.rvalue]
    return lvalues, rvalues


def _add_order(ctx: mypy.plugin.ClassDefContext, adder: MethodAdder) -> None:
    """Generate all the ordering methods for this class.

    Native path: the rust side computes __lt__/__le__/__gt__/__ge__ signatures
    (same signature def(self: AT, other: AT) -> bool with a generic AT). Python
    constructs the live TypeVarType for the self-type; Rust returns the wire
    encoding for the 'other' arg (same as self) and the bool return type.
    """
    bool_type = ctx.api.named_type("builtins.bool")
    object_type = ctx.api.named_type("builtins.object")
    # Make the types be:
    #    AT = TypeVar('AT')
    #    def __lt__(self: AT, other: AT) -> bool

    # This way comparisons with subclasses will work correctly.
    fullname = f"{ctx.cls.info.fullname}.{SELF_TVAR_NAME}"
    tvd = TypeVarType(
        SELF_TVAR_NAME,
        fullname,
        # Namespace is patched per-method below.
        id=TypeVarId(-1, namespace=""),
        values=[],
        upper_bound=object_type,
        default=AnyType(TypeOfAny.from_omitted_generics),
    )
    self_tvar_expr = TypeVarExpr(
        SELF_TVAR_NAME, fullname, [], object_type, AnyType(TypeOfAny.from_omitted_generics)
    )
    ctx.cls.info.names[SELF_TVAR_NAME] = SymbolTableNode(MDEF, self_tvar_expr)

    for method in ["__lt__", "__le__", "__gt__", "__ge__"]:
        namespace = f"{ctx.cls.info.fullname}.{method}"
        tvd = tvd.copy_modified(id=TypeVarId(tvd.id.raw_id, namespace=namespace))
        args = [Argument(Var("other", tvd), tvd, None, ARG_POS)]
        adder.add_method(method, args, bool_type, self_type=tvd, tvd=tvd)


def _make_frozen(ctx: mypy.plugin.ClassDefContext, attributes: list[Attribute]) -> None:
    """Turn all the attributes into properties to simulate frozen classes."""
    for attribute in attributes:
        if attribute.name in ctx.cls.info.names:
            # This variable belongs to this class so we can modify it.
            node = ctx.cls.info.names[attribute.name].node
            if not isinstance(node, Var):
                # The superclass attribute was overridden with a non-variable.
                # No need to do anything here, override will be verified during
                # type checking.
                continue
            node.is_property = True
        else:
            # This variable belongs to a super class so create new Var so we
            # can modify it.
            var = Var(attribute.name, attribute.init_type)
            var.info = ctx.cls.info
            var._fullname = f"{ctx.cls.info.fullname}.{var.name}"
            ctx.cls.info.names[var.name] = SymbolTableNode(MDEF, var)
            var.is_property = True


def _add_init(
    ctx: mypy.plugin.ClassDefContext,
    attributes: list[Attribute],
    adder: MethodAdder,
    method_name: Literal["__init__", "__attrs_init__"],
) -> None:
    """Generate an __init__ method for the attributes and add it to the class."""
    # Convert attributes to arguments with kw_only arguments at the end of
    # the argument list
    pos_args = []
    kw_only_args = []
    sym_table = ctx.cls.info.names
    for attribute in attributes:
        if not attribute.init:
            continue
        if attribute.kw_only:
            kw_only_args.append(attribute.argument(ctx))
        else:
            pos_args.append(attribute.argument(ctx))

        # If the attribute is Final, present in `__init__` and has
        # no default, make sure it doesn't error later.
        if not attribute.has_default and attribute.name in sym_table:
            sym_node = sym_table[attribute.name].node
            if isinstance(sym_node, Var) and sym_node.is_final:
                sym_node.final_set_in_init = True
    args = pos_args + kw_only_args
    if all(arg.variable.type and is_unannotated_any(arg.variable.type) for arg in args):
        # This workaround makes --disallow-incomplete-defs usable with attrs,
        # but is definitely suboptimal as a long-term solution.
        # See https://github.com/python/mypy/issues/5954 for discussion.
        for a in args:
            a.variable.type = AnyType(TypeOfAny.implementation_artifact)
            a.type_annotation = AnyType(TypeOfAny.implementation_artifact)
    adder.add_method(method_name, args, NoneType())


def _add_attrs_magic_attribute(
    ctx: mypy.plugin.ClassDefContext, attrs: list[tuple[str, Type | None]]
) -> None:
    any_type = AnyType(TypeOfAny.explicit)
    attributes_types: list[Type] = [
        ctx.api.named_type_or_none("attr.Attribute", [attr_type or any_type]) or any_type
        for _, attr_type in attrs
    ]
    fallback_type = ctx.api.named_type(
        "builtins.tuple", [ctx.api.named_type_or_none("attr.Attribute", [any_type]) or any_type]
    )

    attr_name = MAGIC_ATTR_CLS_NAME_TEMPLATE.format(ctx.cls.fullname.replace(".", "_"))
    ti = ctx.api.basic_new_typeinfo(attr_name, fallback_type, 0)
    for (name, _), attr_type in zip(attrs, attributes_types):
        var = Var(name, attr_type)
        var._fullname = name
        var.is_property = True
        proper_type = get_proper_type(attr_type)
        if isinstance(proper_type, Instance):
            var.info = proper_type.type
        ti.names[name] = SymbolTableNode(MDEF, var, plugin_generated=True)
    attributes_type = Instance(ti, [])

    # We need to stash the type of the magic attribute so it can be
    # loaded on cached runs.
    ctx.cls.info.names[attr_name] = SymbolTableNode(MDEF, ti, plugin_generated=True)

    add_attribute_to_class(
        ctx.api,
        ctx.cls,
        MAGIC_ATTR_NAME,
        TupleType(attributes_types, fallback=attributes_type),
        fullname=f"{ctx.cls.fullname}.{MAGIC_ATTR_NAME}",
        override_allow_incompatible=True,
        is_classvar=True,
    )


def _add_slots(ctx: mypy.plugin.ClassDefContext, attributes: list[Attribute]) -> None:
    if any(p.slots is None for p in ctx.cls.info.mro[1:-1]):
        # At least one type in mro (excluding `self` and `object`)
        # does not have concrete `__slots__` defined. Ignoring.
        return

    # Unlike `@dataclasses.dataclass`, `__slots__` is rewritten here.
    ctx.cls.info.slots = {attr.name for attr in attributes}

    # Also, inject `__slots__` attribute to class namespace:
    slots_type = TupleType(
        [ctx.api.named_type("builtins.str") for _ in attributes],
        fallback=ctx.api.named_type("builtins.tuple"),
    )
    add_attribute_to_class(api=ctx.api, cls=ctx.cls, name="__slots__", typ=slots_type)


def _add_match_args(ctx: mypy.plugin.ClassDefContext, attributes: list[Attribute]) -> None:
    if (
        "__match_args__" not in ctx.cls.info.names
        or ctx.cls.info.names["__match_args__"].plugin_generated
    ):
        str_type = ctx.api.named_type("builtins.str")
        match_args = TupleType(
            [
                str_type.copy_modified(last_known_value=LiteralType(attr.name, fallback=str_type))
                for attr in attributes
                if not attr.kw_only and attr.init
            ],
            fallback=ctx.api.named_type("builtins.tuple"),
        )
        add_attribute_to_class(api=ctx.api, cls=ctx.cls, name="__match_args__", typ=match_args)


def _remove_hashability(ctx: mypy.plugin.ClassDefContext) -> None:
    """Remove hashability from a class."""
    add_attribute_to_class(
        ctx.api, ctx.cls, "__hash__", NoneType(), is_classvar=True, overwrite_existing=True
    )


class MethodAdder:
    """Helper to add methods to a TypeInfo.

    ctx: The ClassDefCtx we are using on which we will add methods.
    """

    # TODO: Combine this with the code build_namedtuple_typeinfo to support both.

    def __init__(self, ctx: mypy.plugin.ClassDefContext) -> None:
        self.ctx = ctx
        self.self_type = fill_typevars(ctx.cls.info)

    def add_method(
        self,
        method_name: str,
        args: list[Argument],
        ret_type: Type,
        self_type: Type | None = None,
        tvd: TypeVarType | None = None,
    ) -> None:
        """Add a method: def <method_name>(self, <args>) -> <ret_type>): ... to info.

        self_type: The type to use for the self argument or None to use the inferred self type.
        tvd: If the method is generic these should be the type variables.
        """
        self_type = self_type if self_type is not None else self.self_type
        add_method_to_class(
            self.ctx.api, self.ctx.cls, method_name, args, ret_type, self_type, tvd
        )


def _get_attrs_init_type(typ: Instance) -> CallableType | None:
    """
    If `typ` refers to an attrs class, get the type of its initializer method.
    """
    magic_attr = typ.type.get(MAGIC_ATTR_NAME)
    if magic_attr is None or not magic_attr.plugin_generated:
        return None
    init_method = typ.type.get_method("__init__") or typ.type.get_method(ATTRS_INIT_NAME)
    if not isinstance(init_method, FuncDef) or not isinstance(init_method.type, CallableType):
        return None
    return init_method.type


def _fail_not_attrs_class(ctx: mypy.plugin.FunctionSigContext, t: Type, parent_t: Type) -> None:
    t_name = format_type_bare(t, ctx.api.options)
    if parent_t is t:
        msg = (
            f'Argument 1 to "evolve" has a variable type "{t_name}" not bound to an attrs class'
            if isinstance(t, TypeVarType)
            else f'Argument 1 to "evolve" has incompatible type "{t_name}"; expected an attrs class'
        )
    else:
        pt_name = format_type_bare(parent_t, ctx.api.options)
        msg = (
            f'Argument 1 to "evolve" has type "{pt_name}" whose item "{t_name}" is not bound to an attrs class'
            if isinstance(t, TypeVarType)
            else f'Argument 1 to "evolve" has incompatible type "{pt_name}" whose item "{t_name}" is not an attrs class'
        )

    ctx.api.fail(msg, ctx.context)


def _get_expanded_attr_types(
    ctx: mypy.plugin.FunctionSigContext,
    typ: ProperType,
    display_typ: ProperType,
    parent_typ: ProperType,
) -> list[Mapping[str, Type]] | None:
    """
    For a given type, determine what attrs classes it can be: for each class, return the field types.
    For generic classes, the field types are expanded.
    If the type contains Any or a non-attrs type, returns None; in the latter case, also reports an error.
    """
    if isinstance(typ, AnyType):
        return None
    elif isinstance(typ, UnionType):
        ret: list[Mapping[str, Type]] | None = []
        for item in typ.relevant_items():
            item = get_proper_type(item)
            item_types = _get_expanded_attr_types(ctx, item, item, parent_typ)
            if ret is not None and item_types is not None:
                ret += item_types
            else:
                ret = None  # but keep iterating to emit all errors
        return ret
    elif isinstance(typ, TypeVarType):
        return _get_expanded_attr_types(
            ctx, get_proper_type(typ.upper_bound), display_typ, parent_typ
        )
    elif isinstance(typ, Instance):
        init_func = _get_attrs_init_type(typ)
        if init_func is None:
            _fail_not_attrs_class(ctx, display_typ, parent_typ)
            return None
        init_func = expand_type_by_instance(init_func, typ)
        # [1:] to skip the self argument of AttrClass.__init__
        field_names = cast(list[str], init_func.arg_names[1:])
        field_types = init_func.arg_types[1:]
        return [dict(zip(field_names, field_types))]
    else:
        _fail_not_attrs_class(ctx, display_typ, parent_typ)
        return None


def _meet_fields(types: list[Mapping[str, Type]]) -> Mapping[str, Type]:
    """
    "Meet" the fields of a list of attrs classes, i.e. for each field, its new type will be the lower bound.
    """
    field_to_types = defaultdict(list)
    for fields in types:
        for name, typ in fields.items():
            field_to_types[name].append(typ)

    return {
        name: (
            get_proper_type(reduce(meet_types, f_types))
            if len(f_types) == len(types)
            else UninhabitedType()
        )
        for name, f_types in field_to_types.items()
    }


def evolve_function_sig_callback(ctx: mypy.plugin.FunctionSigContext) -> CallableType:
    """
    Generate a signature for the 'attr.evolve' function that's specific to the call site
    and dependent on the type of the first argument.
    """
    if len(ctx.args) != 2:
        # Ideally the name and context should be callee's, but we don't have it in
        # FunctionSigContext.
        ctx.api.fail(f'"{ctx.default_signature.name}" has unexpected type annotation', ctx.context)
        return ctx.default_signature

    if len(ctx.args[0]) != 1:
        return ctx.default_signature  # leave it to the type checker to complain

    inst_arg = ctx.args[0][0]
    inst_type = get_proper_type(ctx.api.get_expression_type(inst_arg))
    inst_type_str = format_type_bare(inst_type, ctx.api.options)

    attr_types = _get_expanded_attr_types(ctx, inst_type, inst_type, inst_type)
    if attr_types is None:
        return ctx.default_signature
    fields = _meet_fields(attr_types)

    return CallableType(
        arg_names=["inst", *fields.keys()],
        arg_kinds=[ARG_POS] + [ARG_NAMED_OPT] * len(fields),
        arg_types=[inst_type, *fields.values()],
        ret_type=inst_type,
        fallback=ctx.default_signature.fallback,
        name=f"{ctx.default_signature.name} of {inst_type_str}",
    )


def fields_function_sig_callback(ctx: mypy.plugin.FunctionSigContext) -> CallableType:
    """Provide the signature for `attrs.fields`."""
    if len(ctx.args) != 1 or len(ctx.args[0]) != 1:
        return ctx.default_signature

    proper_type = get_proper_type(ctx.api.get_expression_type(ctx.args[0][0]))

    # fields(Any) -> Any, fields(type[Any]) -> Any
    if (
        isinstance(proper_type, AnyType)
        or isinstance(proper_type, TypeType)
        and isinstance(proper_type.item, AnyType)
    ):
        return ctx.default_signature

    cls = None
    arg_types = ctx.default_signature.arg_types

    if isinstance(proper_type, TypeVarType):
        inner = get_proper_type(proper_type.upper_bound)
        if isinstance(inner, Instance):
            # We need to work arg_types to compensate for the attrs stubs.
            arg_types = [proper_type]
            cls = inner.type
    elif isinstance(proper_type, CallableType):
        cls = proper_type.type_object()

    if cls is not None and MAGIC_ATTR_NAME in cls.names:
        # This is a proper attrs class.
        ret_type = cls.names[MAGIC_ATTR_NAME].type
        assert ret_type is not None
        return ctx.default_signature.copy_modified(arg_types=arg_types, ret_type=ret_type)

    return ctx.default_signature
