"""Plugin that provides support for dataclasses."""

from __future__ import annotations

from collections.abc import Iterator
from typing import TYPE_CHECKING, Final, Literal

from mypy import errorcodes, message_registry
from mypy.expandtype import expand_type, expand_type_by_instance
from mypy.meet import meet_types
from mypy.messages import format_type_bare
from mypy.nodes import (
    ARG_NAMED,
    ARG_NAMED_OPT,
    ARG_OPT,
    ARG_POS,
    ARG_STAR,
    ARG_STAR2,
    MDEF,
    Argument,
    AssignmentStmt,
    Block,
    CallExpr,
    ClassDef,
    Context,
    DataclassTransformSpec,
    Decorator,
    EllipsisExpr,
    Expression,
    FuncDef,
    FuncItem,
    IfStmt,
    JsonDict,
    NameExpr,
    Node,
    PlaceholderNode,
    RefExpr,
    Statement,
    SymbolTableNode,
    TempNode,
    TypeAlias,
    TypeInfo,
    TypeVarExpr,
    Var,
)
from mypy.plugin import ClassDefContext, FunctionSigContext, SemanticAnalyzerPluginInterface
from mypy.plugins.common import (
    _get_callee_type,
    _get_decorator_bool_argument,
    add_attribute_to_class,
    add_method_to_class,
    deserialize_and_fixup_type,
)
from mypy.semanal_shared import find_dataclass_transform_spec, require_bool_literal_argument
from mypy.server.trigger import make_wildcard_trigger
from mypy.state import state
from mypy.typeops import map_type_from_supertype, try_getting_literals_from_type
from mypy.types import (
    AnyType,
    CallableType,
    FunctionLike,
    Instance,
    LiteralType,
    NoneType,
    ProperType,
    TupleType,
    Type,
    TypeOfAny,
    TypeVarId,
    TypeVarType,
    UninhabitedType,
    UnionType,
    get_proper_type,
)
from mypy.typevars import fill_typevars

if TYPE_CHECKING:
    from mypy.checker import TypeChecker

# Native type-kernel seam: when `Options.native_type_kernel` is set, the
# dataclass class-maker callback is routed through Rust `type_kernel` first.
# On `None` from Rust, Python falls back to `DataclassTransformer`.
try:
    from type_kernel import (
        rust_dataclass_post_init_transform as _rust_dataclass_post_init_transform,
        rust_dataclass_transform as _rust_dataclass_transform,
    )

    _HAS_NATIVE_DATACLASS = True
except ImportError:
    _rust_dataclass_transform = None  # type: ignore[assignment]
    _rust_dataclass_post_init_transform = None  # type: ignore[assignment]
    _HAS_NATIVE_DATACLASS = False

_native_dataclasses_active: bool = False


def _set_native_dataclasses_active(active: bool) -> None:
    """Called by the build manager to enable/disable the Rust dataclass path."""
    global _native_dataclasses_active
    _native_dataclasses_active = active


# Native dataclass __init__ wire format (mirrors the Rust side in
# crates/type_kernel/src/dataclasses.rs).
_DATACLASS_FIELD_TAG: Final = 210
_DATACLASS_INIT_SIG_TAG: Final = 211
_NONE_TYPE_TAG: Final = 108
_END_TAG: Final = 255


def _write_short_int(buf: list[int], value: int) -> None:
    """Encode an int in the mypy wire short-int varint.

    Mirrors ''_write_short_int'' in librt_internal.c and
    ''write_int_bare'' in crates/type_kernel/src/wire.rs.
    """
    if -10 <= value <= 117:
        buf.append((value + 10) << 1)
    elif -100 <= value <= 16283:
        payload = ((value + 100) << 2) | 1
        buf.append(payload & 0xFF)
        buf.append((payload >> 8) & 0xFF)
    elif -10000 <= value <= 536860911:
        payload = ((value + 10000) << 3) | 0b011
        buf.append(payload & 0xFF)
        buf.append((payload >> 8) & 0xFF)
        buf.append((payload >> 16) & 0xFF)
        buf.append((payload >> 24) & 0xFF)
    else:
        raise ValueError(f"short-int out of range: {value}")


def _read_short_int(buf: bytes, pos: int) -> tuple[int, int]:
    """Decode a mypy wire short-int varint, returning (value, new_pos)."""
    first = buf[pos]
    if first & 1 == 0:
        return (first >> 1) - 10, pos + 1
    if first & 2 == 0:
        second = buf[pos + 1]
        return (second << 6) + (first >> 2) - 100, pos + 2
    second = buf[pos + 1]
    two_more = int.from_bytes(buf[pos + 2 : pos + 4], "little")
    return (two_more << 13) + (second << 5) + (first >> 3) - 10000, pos + 4


def _serialize_dataclass_fields_for_rust(attributes: list[DataclassAttribute]) -> bytes:
    """Serialize the in-''__init__'' attributes for the Rust seam."""
    buf: list[int] = []
    _write_short_int(buf, len(attributes))
    for attr in attributes:
        buf.append(_DATACLASS_FIELD_TAG)
        name = attr.name.encode()
        _write_short_int(buf, len(name))
        buf.extend(name)
        alias = (attr.alias or "").encode()
        _write_short_int(buf, len(alias))
        buf.extend(alias)
        buf.append(1 if attr.has_default else 0)
        buf.append(1 if attr.kw_only else 0)
        buf.append(1 if attr.is_in_init else 0)
        buf.append(1 if attr.is_init_var else 0)
    return bytes(buf)


def _parse_native_init_signature(rust_result: bytes) -> tuple[list[str], list[int]]:
    """Parse the Rust seam's wire-encoded __init__ signature.

    Returns (names, kinds) in field order. Raises ValueError on malformed
    bytes.
    """
    if not rust_result or rust_result[0] != _DATACLASS_INIT_SIG_TAG:
        raise ValueError("bad dataclass init signature header")
    pos = 1
    init_len, pos = _read_short_int(rust_result, pos)
    if init_len < 0 or pos + init_len != len(rust_result):
        raise ValueError("bad dataclass init signature length")
    body = rust_result[pos : pos + init_len]
    if not body or body[0] != _DATACLASS_INIT_SIG_TAG:
        raise ValueError("bad dataclass init signature body tag")
    bpos = 1
    n_args, bpos = _read_short_int(body, bpos)
    if not (0 <= n_args <= 1000):
        raise ValueError("bad dataclass init signature arg count")
    names: list[str] = []
    kinds: list[int] = []
    for _ in range(n_args):
        name_len, bpos = _read_short_int(body, bpos)
        if name_len < 0 or bpos + name_len > len(body):
            raise ValueError("bad dataclass init arg name")
        names.append(body[bpos : bpos + name_len].decode())
        bpos += name_len
        kind, bpos = _read_short_int(body, bpos)
        if kind not in (0, 1, 3, 5):
            raise ValueError("bad dataclass init arg kind")
        kinds.append(kind)
    if bpos >= len(body):
        raise ValueError("truncated dataclass init signature")
    bpos += 1  # returns_flag
    if bpos + 2 > len(body) or body[bpos] != _NONE_TYPE_TAG or body[bpos + 1] != _END_TAG:
        raise ValueError("bad dataclass init signature trailer")
    return names, kinds


def _apply_native_dataclass_transform(
    transformer: DataclassTransformer, rust_result: bytes, attributes: list[DataclassAttribute]
) -> None:
    """Validate the Rust result against Python's own computation, then apply.

    Raises ValueError on mismatch, so a Rust bug cannot silently change
    semantics; the caller falls back to the Python path.
    """
    info = transformer._cls.info
    rust_names, rust_kinds = _parse_native_init_signature(rust_result)
    expected_names = [attr.alias or attr.name for attr in attributes]
    expected_kinds = [
        (
            5
            if attr.kw_only and attr.has_default
            else 3 if attr.kw_only else 1 if attr.has_default else 0
        )
        for attr in attributes
    ]
    if rust_names != expected_names or rust_kinds != expected_kinds:
        raise ValueError("native dataclass init signature mismatch")
    args = [attr.to_argument(info, of="__init__") for attr in attributes]
    if info.fallback_to_any:
        for arg in args:
            if arg.kind == ARG_POS:
                arg.kind = ARG_OPT
        existing_args_names = {arg.variable.name for arg in args}
        gen_args_name = "generated_args"
        while gen_args_name in existing_args_names:
            gen_args_name += "_"
        gen_kwargs_name = "generated_kwargs"
        while gen_kwargs_name in existing_args_names:
            gen_kwargs_name += "_"
        args = [
            Argument(Var(gen_args_name), AnyType(TypeOfAny.explicit), None, ARG_STAR),
            *args,
            Argument(Var(gen_kwargs_name), AnyType(TypeOfAny.explicit), None, ARG_STAR2),
        ]
    add_method_to_class(
        transformer._api, transformer._cls, "__init__", args=args, return_type=NoneType()
    )


def _try_native_dataclass_transform(
    transformer: DataclassTransformer, attributes: list[DataclassAttribute]
) -> bool:
    """Run the __init__ computation through the Rust seam when enabled.

    Returns True if Rust computed and Python applied the signature. Any
    failure (unsupported input, disabled gate, mismatch) falls back to the
    Python path and returns False.
    """
    if not _HAS_NATIVE_DATACLASS or not _native_dataclasses_active:
        return False
    try:
        filtered = [
            a for a in attributes if a.is_in_init and not transformer._is_kw_only_type(a.type)
        ]
        rust_result = _rust_dataclass_transform(
            _serialize_dataclass_fields_for_rust(filtered),
            transformer._cls.fullname,
            True,  # decorator_init
            True,  # decorator_eq
            False,  # decorator_order
            False,  # decorator_frozen
        )
        if rust_result is None:
            return False
        _apply_native_dataclass_transform(transformer, rust_result, filtered)
        return True
    except Exception:
        return False


def _try_native_dataclass_post_init(
    transformer: DataclassTransformer, attributes: list[DataclassAttribute]
) -> bool:
    """Run the __post_init__ argument computation through the Rust seam.

    The post-init signature is the InitVar fields only, in attribute order,
    all positional with no defaults (mirrors `_add_internal_post_init_method`
    and `to_argument` of="__post_init__"). Any failure falls back to the
    Python path and returns False.
    """
    if not _HAS_NATIVE_DATACLASS or not _native_dataclasses_active:
        return False
    try:
        rust_result = _rust_dataclass_post_init_transform(
            _serialize_dataclass_fields_for_rust(attributes), transformer._cls.fullname
        )
        if rust_result is None:
            return False
        rust_names, rust_kinds = _parse_native_init_signature(rust_result)
        expected_names = [attr.alias or attr.name for attr in attributes if attr.is_init_var]
        expected_kinds = [0] * len(expected_names)
        if rust_names != expected_names or rust_kinds != expected_kinds:
            raise ValueError("native dataclass post-init signature mismatch")
        args = [
            attr.to_argument(transformer._cls.info, of="__post_init__")
            for attr in attributes
            if attr.is_init_var
        ]
        add_method_to_class(
            transformer._api,
            transformer._cls,
            _INTERNAL_POST_INIT_SYM_NAME,
            args=args,
            return_type=NoneType(),
        )
        return True
    except Exception:
        return False


# The set of decorators that generate dataclasses.
dataclass_makers: Final = {"dataclass", "dataclasses.dataclass"}
# Default field specifiers for dataclasses
DATACLASS_FIELD_SPECIFIERS: Final = ("dataclasses.Field", "dataclasses.field")


SELF_TVAR_NAME: Final = "_DT"
_TRANSFORM_SPEC_FOR_DATACLASSES: Final = DataclassTransformSpec(
    eq_default=True,
    order_default=False,
    kw_only_default=False,
    frozen_default=False,
    field_specifiers=DATACLASS_FIELD_SPECIFIERS,
)
_INTERNAL_REPLACE_SYM_NAME: Final = "__mypy-replace"
_INTERNAL_POST_INIT_SYM_NAME: Final = "__mypy-post_init"


class DataclassAttribute:
    def __init__(
        self,
        name: str,
        alias: str | None,
        is_in_init: bool,
        is_init_var: bool,
        has_default: bool,
        line: int,
        column: int,
        type: Type | None,
        info: TypeInfo,
        kw_only: bool,
        is_neither_frozen_nor_nonfrozen: bool,
        api: SemanticAnalyzerPluginInterface,
    ) -> None:
        self.name = name
        self.alias = alias
        self.is_in_init = is_in_init
        self.is_init_var = is_init_var
        self.has_default = has_default
        self.line = line
        self.column = column
        self.type = type  # Type as __init__ argument
        self.info = info
        self.kw_only = kw_only
        self.is_neither_frozen_nor_nonfrozen = is_neither_frozen_nor_nonfrozen
        self._api = api

    def to_argument(
        self, current_info: TypeInfo, *, of: Literal["__init__", "replace", "__post_init__"]
    ) -> Argument:
        if of == "__init__":
            arg_kind = ARG_POS
            if self.kw_only and self.has_default:
                arg_kind = ARG_NAMED_OPT
            elif self.kw_only and not self.has_default:
                arg_kind = ARG_NAMED
            elif not self.kw_only and self.has_default:
                arg_kind = ARG_OPT
        elif of == "replace":
            arg_kind = ARG_NAMED if self.is_init_var and not self.has_default else ARG_NAMED_OPT
        elif of == "__post_init__":
            # We always use `ARG_POS` without a default value. Without a default,
            # callers are not required to provide the argument explicitly, the
            # `__post_init__` protocol resolves it from the field default at runtime.

            # Example: `y: dataclasses.InitVar[str] = 'a'` means `__post_init__`
            # receives `y` from the field default, so a parameter default would
            # over-constrain direct calls.

            # `__post_init__` is not designed to be called directly, and
            # duplicating default values for the sake of type-checking is unpleasant.
            arg_kind = ARG_POS
        return Argument(
            variable=self.to_var(current_info),
            type_annotation=self.expand_type(current_info),
            initializer=EllipsisExpr() if self.has_default else None,  # Only used by stubgen
            kind=arg_kind,
        )

    def expand_type(self, current_info: TypeInfo) -> Type | None:
        if self.type is not None and self.info.self_type is not None:
            # In general, it is not safe to call `expand_type()` during semantic
            # analysis, however this plugin is called very late, so all types
            # should be fully ready. Avoid eager expansion of Self types here.
            with state.strict_optional_set(self._api.options.strict_optional):
                return expand_type(
                    self.type, {self.info.self_type.id: fill_typevars(current_info)}
                )
        return self.type

    def to_var(self, current_info: TypeInfo) -> Var:
        return Var(self.alias or self.name, self.expand_type(current_info))

    def serialize(self) -> JsonDict:
        assert self.type
        return {
            "name": self.name,
            "alias": self.alias,
            "is_in_init": self.is_in_init,
            "is_init_var": self.is_init_var,
            "has_default": self.has_default,
            "line": self.line,
            "column": self.column,
            "type": self.type.serialize(),
            "kw_only": self.kw_only,
            "is_neither_frozen_nor_nonfrozen": self.is_neither_frozen_nor_nonfrozen,
        }

    @classmethod
    def deserialize(
        cls, info: TypeInfo, data: JsonDict, api: SemanticAnalyzerPluginInterface
    ) -> DataclassAttribute:
        data = data.copy()
        typ = deserialize_and_fixup_type(data.pop("type"), api)
        return cls(type=typ, info=info, **data, api=api)

    def expand_typevar_from_subtype(self, sub_type: TypeInfo) -> None:
        """Expands type vars in the context of a subtype when an attribute is inherited
        from a generic super type."""
        if self.type is not None:
            with state.strict_optional_set(self._api.options.strict_optional):
                self.type = map_type_from_supertype(self.type, sub_type, self.info)


class DataclassTransformer:
    """Implement the behavior of @dataclass.

    Note that this may be executed multiple times on the same class, so
    everything here must be idempotent.

    This runs after the main semantic analysis pass, so you can assume that
    there are no placeholders.
    """

    def __init__(
        self,
        cls: ClassDef,
        # Statement must also be accepted since class definition itself may
        # be passed as the reason for subclass/metaclass uses of dataclass_transform
        reason: Expression | Statement,
        spec: DataclassTransformSpec,
        api: SemanticAnalyzerPluginInterface,
    ) -> None:
        self._cls = cls
        self._reason = reason
        self._spec = spec
        self._api = api

    def _add_init(
        self, attributes: list[DataclassAttribute], decorator_arguments: dict[str, bool]
    ) -> None:
        """Generate ``__init__`` for the dataclass.

        Runs through the native type-kernel seam first when enabled; the
        Rust side computes the argument names and kinds, Python applies the
        AST mutation. Falls back to the pure-Python path (the original
        behavior) on any mismatch or unsupported input.
        """
        info = self._cls.info
        if not (
            decorator_arguments["init"]
            and ("__init__" not in info.names or info.names["__init__"].plugin_generated)
            and attributes
        ):
            return
        if _try_native_dataclass_transform(self, attributes):
            return
        args = [
            attr.to_argument(info, of="__init__")
            for attr in attributes
            if attr.is_in_init and not self._is_kw_only_type(attr.type)
        ]
        if info.fallback_to_any:
            # Make positional args optional since we don't know their order.
            for arg in args:
                if arg.kind == ARG_POS:
                    arg.kind = ARG_OPT
            existing_args_names = {arg.variable.name for arg in args}
            gen_args_name = "generated_args"
            while gen_args_name in existing_args_names:
                gen_args_name += "_"
            gen_kwargs_name = "generated_kwargs"
            while gen_kwargs_name in existing_args_names:
                gen_kwargs_name += "_"
            args = [
                Argument(Var(gen_args_name), AnyType(TypeOfAny.explicit), None, ARG_STAR),
                *args,
                Argument(Var(gen_kwargs_name), AnyType(TypeOfAny.explicit), None, ARG_STAR2),
            ]
        add_method_to_class(self._api, self._cls, "__init__", args=args, return_type=NoneType())

    def transform(self) -> bool:
        """Apply all the necessary transformations to the underlying
        dataclass so as to ensure it is fully type checked according
        to the rules in PEP 557.
        """
        info = self._cls.info
        attributes = self.collect_attributes()
        if attributes is None:
            # Some definitions are not ready. We need another pass.
            return False
        for attr in attributes:
            if attr.type is None:
                return False
        decorator_arguments = {
            "init": self._get_bool_arg("init", True),
            "eq": self._get_bool_arg("eq", self._spec.eq_default),
            "order": self._get_bool_arg("order", self._spec.order_default),
            "frozen": self._get_bool_arg("frozen", self._spec.frozen_default),
            "slots": self._get_bool_arg("slots", False),
            "match_args": self._get_bool_arg("match_args", True),
        }

        # If there are no attributes, the semantic analyzer may not have processed
        # them yet. Skip generating __init__ when there are no attributes: the
        # object default __init__ with an empty signature will be present anyway.
        self._add_init(attributes, decorator_arguments)

        if (
            decorator_arguments["eq"]
            and info.get("__eq__") is None
            or decorator_arguments["order"]
        ):
            # Type variable for self types in generated methods.
            obj_type = self._api.named_type("builtins.object")
            self_tvar_expr = TypeVarExpr(
                SELF_TVAR_NAME,
                info.fullname + "." + SELF_TVAR_NAME,
                [],
                obj_type,
                AnyType(TypeOfAny.from_omitted_generics),
            )
            info.names[SELF_TVAR_NAME] = SymbolTableNode(MDEF, self_tvar_expr)

        # Add <, >, <=, >=, but only if the class has an eq method.
        if decorator_arguments["order"]:
            if not decorator_arguments["eq"]:
                self._api.fail('"eq" must be True if "order" is True', self._reason)

            for method_name in ["__lt__", "__gt__", "__le__", "__ge__"]:
                # Like for __eq__ and __ne__, we want "other" to match
                # the self type.
                obj_type = self._api.named_type("builtins.object")
                order_tvar_def = TypeVarType(
                    SELF_TVAR_NAME,
                    f"{info.fullname}.{SELF_TVAR_NAME}",
                    id=TypeVarId(-1, namespace=f"{info.fullname}.{method_name}"),
                    values=[],
                    upper_bound=obj_type,
                    default=AnyType(TypeOfAny.from_omitted_generics),
                )
                order_return_type = self._api.named_type("builtins.bool")
                order_args = [
                    Argument(Var("other", order_tvar_def), order_tvar_def, None, ARG_POS)
                ]

                existing_method = info.get(method_name)
                if existing_method is not None and not existing_method.plugin_generated:
                    assert existing_method.node
                    self._api.fail(
                        f'You may not have a custom "{method_name}" method when "order" is True',
                        existing_method.node,
                    )

                add_method_to_class(
                    self._api,
                    self._cls,
                    method_name,
                    args=order_args,
                    return_type=order_return_type,
                    self_type=order_tvar_def,
                    tvar_def=order_tvar_def,
                )

        parent_decorator_arguments = []
        for parent in info.mro[1:-1]:
            parent_args = parent.metadata.get("dataclass")

            # Ignore parent classes that directly specify a dataclass transform-decorated metaclass
            # when searching for usage of the frozen parameter. PEP 681 states that a class that
            # directly specifies such a metaclass must be treated as neither frozen nor non-frozen.
            if parent_args and not _has_direct_dataclass_transform_metaclass(parent):
                parent_decorator_arguments.append(parent_args)

        if decorator_arguments["frozen"]:
            if any(not parent["frozen"] for parent in parent_decorator_arguments):
                self._api.fail("Frozen dataclass cannot inherit from a non-frozen dataclass", info)
            self._propertize_callables(attributes, settable=False)
            self._freeze(attributes)
        else:
            if any(parent["frozen"] for parent in parent_decorator_arguments):
                self._api.fail("Non-frozen dataclass cannot inherit from a frozen dataclass", info)
            self._propertize_callables(attributes)

        if decorator_arguments["slots"]:
            self.add_slots(info, attributes)

        self.reset_init_only_vars(info, attributes)

        if decorator_arguments["match_args"] and (
            "__match_args__" not in info.names or info.names["__match_args__"].plugin_generated
        ):
            str_type = self._api.named_type("builtins.str")
            literals: list[Type] = [
                LiteralType(attr.name, str_type)
                for attr in attributes
                if attr.is_in_init and not attr.kw_only
            ]
            match_args_type = TupleType(literals, self._api.named_type("builtins.tuple"))
            add_attribute_to_class(self._api, self._cls, "__match_args__", match_args_type)

        self._add_dataclass_fields_magic_attribute()
        self._add_internal_replace_method(attributes)
        if self._api.options.python_version >= (3, 13):
            self._add_dunder_replace(attributes)

        if "__post_init__" in info.names:
            self._add_internal_post_init_method(attributes)

        info.metadata["dataclass"] = {
            "attributes": [attr.serialize() for attr in attributes],
            "frozen": decorator_arguments["frozen"],
        }

        return True

    def _add_dunder_replace(self, attributes: list[DataclassAttribute]) -> None:
        """Add a `__replace__` method to the class, which is used to replace attributes in the `copy` module."""
        args = [
            attr.to_argument(self._cls.info, of="replace")
            for attr in attributes
            if attr.is_in_init
        ]
        add_method_to_class(
            self._api,
            self._cls,
            "__replace__",
            args=args,
            return_type=fill_typevars(self._cls.info),
        )

    def _add_internal_replace_method(self, attributes: list[DataclassAttribute]) -> None:
        """
        Stashes the signature of 'dataclasses.replace(...)' for this specific dataclass
        to be used later whenever 'dataclasses.replace' is called for this dataclass.
        """
        add_method_to_class(
            self._api,
            self._cls,
            _INTERNAL_REPLACE_SYM_NAME,
            args=[attr.to_argument(self._cls.info, of="replace") for attr in attributes],
            return_type=NoneType(),
            is_staticmethod=True,
        )

    def _add_internal_post_init_method(self, attributes: list[DataclassAttribute]) -> None:
        if _try_native_dataclass_post_init(self, attributes):
            return
        add_method_to_class(
            self._api,
            self._cls,
            _INTERNAL_POST_INIT_SYM_NAME,
            args=[
                attr.to_argument(self._cls.info, of="__post_init__")
                for attr in attributes
                if attr.is_init_var
            ],
            return_type=NoneType(),
        )

    def add_slots(self, info: TypeInfo, attributes: list[DataclassAttribute]) -> None:
        existing_slots = info.names.get("__slots__")
        slots_defined_by_plugin = existing_slots is not None and existing_slots.plugin_generated
        if existing_slots is not None and not slots_defined_by_plugin:
            # This means we have a slots conflict: the class explicitly specifies a
            # different `__slots__` field and `@dataclass(slots=True)` is used. In
            # runtime this raises a type error.
            self._api.fail(
                '"{}" both defines "__slots__" and is used with "slots=True"'.format(
                    self._cls.name
                ),
                self._cls,
            )
            return

        if any(p.slots is None for p in info.mro[1:-1]):
            # At least one type in mro (excluding `self` and `object`)
            # does not have concrete `__slots__` defined. Ignoring.
            return

        generated_slots = {attr.name for attr in attributes}
        info.slots = generated_slots

        # Now, insert `.__slots__` attribute to class namespace:
        slots_type = TupleType(
            [self._api.named_type("builtins.str") for _ in generated_slots],
            self._api.named_type("builtins.tuple"),
        )
        add_attribute_to_class(
            self._api,
            self._cls,
            "__slots__",
            slots_type,
            overwrite_existing=slots_defined_by_plugin,
        )

    def reset_init_only_vars(self, info: TypeInfo, attributes: list[DataclassAttribute]) -> None:
        """Remove init-only vars from the class and reset init var declarations."""
        for attr in attributes:
            if attr.is_init_var:
                if attr.name in info.names:
                    del info.names[attr.name]
                else:
                    # Nodes of superclass InitVars not used in __init__ cannot be reached.
                    assert attr.is_init_var
                for stmt in info.defn.defs.body:
                    if isinstance(stmt, AssignmentStmt) and stmt.unanalyzed_type:
                        lvalue = stmt.lvalues[0]
                        if isinstance(lvalue, NameExpr) and lvalue.name == attr.name:
                            # Reset node so that another semantic analysis pass will
                            # recreate a symbol node for this attribute.
                            lvalue.node = None

    def _get_assignment_statements_from_if_statement(
        self, stmt: IfStmt
    ) -> Iterator[AssignmentStmt]:
        for body in stmt.body:
            if not body.is_unreachable:
                yield from self._get_assignment_statements_from_block(body)
        if stmt.else_body is not None and not stmt.else_body.is_unreachable:
            yield from self._get_assignment_statements_from_block(stmt.else_body)

    def _get_assignment_statements_from_block(self, block: Block) -> Iterator[AssignmentStmt]:
        for stmt in block.body:
            if isinstance(stmt, AssignmentStmt):
                yield stmt
            elif isinstance(stmt, IfStmt):
                yield from self._get_assignment_statements_from_if_statement(stmt)

    def collect_attributes(self) -> list[DataclassAttribute] | None:
        """Collect all attributes declared in the dataclass and its parents.

        All assignments of the form

          a: SomeType
          b: SomeOtherType = ...

        are collected.

        Return None if some dataclass base class hasn't been processed
        yet and thus we'll need to ask for another pass.
        """
        cls = self._cls

        # First, collect attributes belonging to any class in the MRO, ignoring
        # duplicates. Iterate the MRO in reverse so parent attrs appear before
        # child attrs. See https://docs.python.org/3/library/dataclasses.html#inheritance

        # However, we also want attributes defined in the subtype to override ones
        # defined in the parent. We can implement this via a dict without disrupting
        # the attr order because dicts preserve insertion order in Python 3.7+.
        found_attrs: dict[str, DataclassAttribute] = {}
        for info in reversed(cls.info.mro[1:-1]):
            if "dataclass_tag" in info.metadata and "dataclass" not in info.metadata:
                # We haven't processed the base class yet. Need another pass.
                return None
            if "dataclass" not in info.metadata:
                continue

            # Each class depends on the set of attributes in its dataclass ancestors.
            self._api.add_plugin_dependency(make_wildcard_trigger(info.fullname))

            for data in info.metadata["dataclass"]["attributes"]:
                name: str = data["name"]

                attr = DataclassAttribute.deserialize(info, data, self._api)
                # TODO: We shouldn't be performing type operations during the main
                #       semantic analysis pass, since some TypeInfo attributes might
                #       still be in flux. This should be performed in a later phase.
                attr.expand_typevar_from_subtype(cls.info)
                found_attrs[name] = attr

                sym_node = cls.info.names.get(name)
                if sym_node and sym_node.node and not isinstance(sym_node.node, Var):
                    self._api.fail(
                        "Dataclass attribute may only be overridden by another attribute",
                        sym_node.node,
                    )

        # Second, collect attributes belonging to the current class.
        current_attr_names: set[str] = set()
        kw_only = self._get_bool_arg("kw_only", self._spec.kw_only_default)
        for stmt in self._get_assignment_statements_from_block(cls.defs):
            # Any assignment that doesn't use the new type declaration
            # syntax can be ignored out of hand.
            if not stmt.new_syntax:
                continue

            # a: int, b: str = 1, 'foo' is not supported syntax so we
            # don't have to worry about it.
            lhs = stmt.lvalues[0]
            if not isinstance(lhs, NameExpr):
                continue

            sym = cls.info.names.get(lhs.name)
            if sym is None:
                # There was probably a semantic analysis error.
                continue

            node = sym.node
            assert not isinstance(node, PlaceholderNode)

            if isinstance(node, TypeAlias):
                self._api.fail(
                    ("Type aliases inside dataclass definitions are not supported at runtime"),
                    node,
                )
                # Skip processing this node. This doesn't match the runtime behaviour,
                # but the only alternative would be to modify the SymbolTable,
                # and it's a little hairy to do that in a plugin.
                continue
            if isinstance(node, Decorator):
                # This might be a property / field name clash.
                # We will issue an error later.
                continue

            assert isinstance(node, Var), node

            # x: ClassVar[int] is ignored by dataclasses.
            if node.is_classvar:
                continue

            # x: InitVar[int] is turned into x: int and is removed from the class.
            is_init_var = False
            node_type = get_proper_type(node.type)
            if (
                isinstance(node_type, Instance)
                and node_type.type.fullname == "dataclasses.InitVar"
            ):
                is_init_var = True
                node.type = node_type.args[0]

            if self._is_kw_only_type(node_type):
                kw_only = True

            has_field_call, field_args = self._collect_field_args(stmt.rvalue)

            is_in_init_param = field_args.get("init")
            if is_in_init_param is None:
                is_in_init = self._get_default_init_value_for_field_specifier(stmt.rvalue)
            else:
                is_in_init = bool(self._api.parse_bool(is_in_init_param))

            has_default = False
            # Ensure that something like x: int = field() is rejected
            # after an attribute with a default.
            if has_field_call:
                has_default = (
                    "default" in field_args
                    or "default_factory" in field_args
                    # alias for default_factory defined in PEP 681
                    or "factory" in field_args
                )

            # All other assignments are already type checked.
            elif not isinstance(stmt.rvalue, TempNode):
                has_default = True

            if not has_default and self._spec is _TRANSFORM_SPEC_FOR_DATACLASSES:
                # Non-default dataclass attributes are set on self in generated __init__()
                # rather than the class body, so mark them implicit. Custom dataclass
                # transforms initialize differently, so theirs stay non-implicit (mypy#14868).
                sym.implicit = True

            is_kw_only = kw_only
            # Use the kw_only field arg if it is provided. Otherwise use the
            # kw_only value from the decorator parameter.
            field_kw_only_param = field_args.get("kw_only")
            if field_kw_only_param is not None:
                value = self._api.parse_bool(field_kw_only_param)
                if value is not None:
                    is_kw_only = value
                else:
                    self._api.fail('"kw_only" argument must be a boolean literal', stmt.rvalue)

            # Allow bare x: Final[int] in class body, since it will be set in the generated
            # __init__() method (unless it is an InitVar), to match regular class semantics.
            if node.is_final and node.final_unset_in_class:
                if is_init_var:
                    self._api.fail("InitVar cannot be final", stmt.rvalue)
                else:
                    node.final_set_in_init = True

            if sym.type is None and node.is_final and node.is_inferred:
                # Special case: assignment like x: Final = 42 is classified as annotated
                # above, but mypy strips `Final` turning it into x = 42. We only infer
                # simple literals in dataclasses, otherwise require an explicit type.
                typ = self._api.analyze_simple_literal_type(stmt.rvalue, is_final=True)
                if typ:
                    node.type = typ
                else:
                    self._api.fail(
                        "Need type argument for Final[...] with non-literal default in dataclass",
                        stmt,
                    )
                    node.type = AnyType(TypeOfAny.from_error)

            alias = None
            if "alias" in field_args:
                alias = self._api.parse_str_literal(field_args["alias"])
                if alias is None:
                    self._api.fail(
                        message_registry.DATACLASS_FIELD_ALIAS_MUST_BE_LITERAL,
                        stmt.rvalue,
                        code=errorcodes.LITERAL_REQ,
                    )

            current_attr_names.add(lhs.name)
            with state.strict_optional_set(self._api.options.strict_optional):
                init_type = self._infer_dataclass_attr_init_type(sym, lhs.name, stmt)
            found_attrs[lhs.name] = DataclassAttribute(
                name=lhs.name,
                alias=alias,
                is_in_init=is_in_init,
                is_init_var=is_init_var,
                has_default=has_default,
                line=stmt.line,
                column=stmt.column,
                type=init_type,
                info=cls.info,
                kw_only=is_kw_only,
                is_neither_frozen_nor_nonfrozen=_has_direct_dataclass_transform_metaclass(
                    cls.info
                ),
                api=self._api,
            )

        all_attrs = list(found_attrs.values())
        all_attrs.sort(key=lambda a: a.kw_only)

        # Third, ensure that arguments without a default don't follow
        # arguments that have a default and that the KW_ONLY sentinel
        # is only provided once.
        found_default = False
        found_kw_sentinel = False
        for attr in all_attrs:
            # If we find any attribute that is_in_init, not kw_only, and that
            # doesn't have a default after one that does have one,
            # then that's an error.
            if found_default and attr.is_in_init and not attr.has_default and not attr.kw_only:
                # If the issue comes from merging different classes, report it
                # at the class definition point.
                context: Context = cls
                if attr.name in current_attr_names:
                    context = Context(line=attr.line, column=attr.column)
                self._api.fail(
                    "Attributes without a default cannot follow attributes with one", context
                )

            found_default = found_default or (attr.has_default and attr.is_in_init)
            if found_kw_sentinel and self._is_kw_only_type(attr.type):
                context = cls
                if attr.name in current_attr_names:
                    context = Context(line=attr.line, column=attr.column)
                self._api.fail(
                    "There may not be more than one field with the KW_ONLY type", context
                )
            found_kw_sentinel = found_kw_sentinel or self._is_kw_only_type(attr.type)
        return all_attrs

    def _freeze(self, attributes: list[DataclassAttribute]) -> None:
        """Converts all attributes to @property methods in order to
        emulate frozen classes.
        """
        info = self._cls.info
        for attr in attributes:
            # Classes that directly specify a dataclass_transform metaclass must be
            # neither frozen nor non-frozen per PEP681, so their attributes must be
            # writable even when the rest of the hierarchy is frozen (matches Pyright).
            if attr.is_neither_frozen_nor_nonfrozen:
                continue

            sym_node = info.names.get(attr.name)
            if sym_node is not None:
                var = sym_node.node
                if isinstance(var, Var):
                    if var.is_final:
                        continue  # do not turn `Final` attrs to `@property`
                    var.is_property = True
            else:
                var = attr.to_var(info)
                var.info = info
                var.is_property = True
                var._fullname = info.fullname + "." + var.name
                info.names[var.name] = SymbolTableNode(MDEF, var)

    def _propertize_callables(
        self, attributes: list[DataclassAttribute], settable: bool = True
    ) -> None:
        """Converts all attributes with callable types to @property methods.

        This avoids the typechecker getting confused and thinking that
        `my_dataclass_instance.callable_attr(foo)` is going to receive a
        `self` argument (it is not).

        """
        info = self._cls.info
        for attr in attributes:
            if isinstance(get_proper_type(attr.type), CallableType):
                var = attr.to_var(info)
                var.info = info
                var.is_property = True
                var.is_settable_property = settable
                var._fullname = info.fullname + "." + var.name
                info.names[var.name] = SymbolTableNode(MDEF, var)

    def _is_kw_only_type(self, node: Type | None) -> bool:
        """Checks if the type of the node is the KW_ONLY sentinel value."""
        if node is None:
            return False
        node_type = get_proper_type(node)
        if not isinstance(node_type, Instance):
            return False
        return node_type.type.fullname == "dataclasses.KW_ONLY"

    def _add_dataclass_fields_magic_attribute(self) -> None:
        attr_name = "__dataclass_fields__"
        any_type = AnyType(TypeOfAny.explicit)
        # For `dataclasses`, use the type `dict[str, Field[Any]]` for accuracy. For
        # dataclass transforms, it's inaccurate to use `Field` since a given transform
        # may use a completely different type (or none); fall back to `Any` there.

        # In either case, we're aiming to match the Typeshed stub for `is_dataclass`,
        # which expects the instance to have a `__dataclass_fields__` attribute of
        # type `dict[str, Field[Any]]`.
        if self._spec is _TRANSFORM_SPEC_FOR_DATACLASSES:
            field_type = self._api.named_type_or_none("dataclasses.Field", [any_type]) or any_type
        else:
            field_type = any_type
        attr_type = self._api.named_type(
            "builtins.dict", [self._api.named_type("builtins.str"), field_type]
        )
        var = Var(name=attr_name, type=attr_type)
        var.info = self._cls.info
        var._fullname = self._cls.info.fullname + "." + attr_name
        var.is_classvar = True
        self._cls.info.names[attr_name] = SymbolTableNode(
            kind=MDEF, node=var, plugin_generated=True
        )

    def _collect_field_args(self, expr: Expression) -> tuple[bool, dict[str, Expression]]:
        """Returns a tuple where the first value represents whether or not
        the expression is a call to dataclass.field and the second is a
        dictionary of the keyword arguments that field() was called with.
        """
        if (
            isinstance(expr, CallExpr)
            and isinstance(expr.callee, RefExpr)
            and expr.callee.fullname in self._spec.field_specifiers
        ):
            # field() only takes keyword arguments.
            args = {}
            for name, arg, kind in zip(expr.arg_names, expr.args, expr.arg_kinds):
                if not kind.is_named():
                    if kind.is_named(star=True):
                        # This means that `field` is used with `**` unpacking,
                        # the best we can do for now is not to fail.
                        # TODO: we can infer what's inside `**` and try to collect it.
                        message = 'Unpacking **kwargs in "field()" is not supported'
                    elif self._spec is not _TRANSFORM_SPEC_FOR_DATACLASSES:
                        # dataclasses.field requires keyword args, but only for the
                        # *standardized* arguments to dataclass_transform field specifiers.
                        # If not a dataclasses.dataclass class, safely skip positional args.
                        continue
                    else:
                        message = '"field()" does not accept positional arguments'
                    self._api.fail(message, expr)
                    return True, {}
                assert name is not None
                args[name] = arg
            return True, args
        return False, {}

    def _get_bool_arg(self, name: str, default: bool) -> bool:
        # Expressions are always CallExprs (either directly or via a wrapper like Decorator), so
        # we can use the helpers from common
        if isinstance(self._reason, Expression):
            return _get_decorator_bool_argument(
                ClassDefContext(self._cls, self._reason, self._api), name, default
            )

        # Subclass/metaclass use of `typing.dataclass_transform` reads the parameters from the
        # class's keyword arguments (ie `class Subclass(Parent, kwarg1=..., kwarg2=...)`)
        expression = self._cls.keywords.get(name)
        if expression is not None:
            return require_bool_literal_argument(self._api, expression, name, default)
        return default

    def _get_default_init_value_for_field_specifier(self, call: Expression) -> bool:
        """
        Find a default value for the `init` parameter of the specifier being called. If the
        specifier's type signature includes an `init` parameter with a type of `Literal[True]` or
        `Literal[False]`, return the appropriate boolean value from the literal. Otherwise,
        fall back to the standard default of `True`.
        """
        if not isinstance(call, CallExpr):
            return True

        specifier_type = _get_callee_type(call)
        if specifier_type is None:
            return True

        parameter = specifier_type.argument_by_name("init")
        if parameter is None:
            return True

        literals = try_getting_literals_from_type(parameter.typ, bool, "builtins.bool")
        if literals is None or len(literals) != 1:
            return True

        return literals[0]

    def _infer_dataclass_attr_init_type(
        self, sym: SymbolTableNode, name: str, context: Context
    ) -> Type | None:
        """Infer __init__ argument type for an attribute.

        In particular, possibly use the signature of __set__.
        """
        default = sym.type
        if sym.implicit:
            return default
        t = get_proper_type(sym.type)

        # Perform simple-minded inference from the signature of __set__. We can't use
        # mypy.checkmember here since this plugin runs before type checking. We support
        # some basic scenarios here, which is sufficient for most use cases.
        if not isinstance(t, Instance):
            return default
        setter = t.type.get("__set__")
        if setter:
            if isinstance(setter.node, FuncDef):
                super_info = t.type.get_containing_type_info("__set__")
                assert super_info
                if setter.type:
                    setter_type = get_proper_type(
                        map_type_from_supertype(setter.type, t.type, super_info)
                    )
                else:
                    return AnyType(TypeOfAny.unannotated)
                if isinstance(setter_type, CallableType) and setter_type.arg_kinds == [
                    ARG_POS,
                    ARG_POS,
                    ARG_POS,
                ]:
                    return expand_type_by_instance(setter_type.arg_types[2], t)
                else:
                    self._api.fail(
                        f'Unsupported signature for "__set__" in "{t.type.name}"', context
                    )
            else:
                self._api.fail(f'Unsupported "__set__" in "{t.type.name}"', context)

        return default


def add_dataclass_tag(info: TypeInfo) -> None:
    # The value is ignored, only the existence matters.
    info.metadata["dataclass_tag"] = {}


def dataclass_tag_callback(ctx: ClassDefContext) -> None:
    """Record that we have a dataclass in the main semantic analysis pass.

    The later pass implemented by DataclassTransformer will use this
    to detect dataclasses in base classes.
    """
    add_dataclass_tag(ctx.cls.info)


def dataclass_class_maker_callback(ctx: ClassDefContext) -> bool:
    """Hooks into the class typechecking process to add support for dataclasses."""
    if any(i.is_named_tuple for i in ctx.cls.info.mro):
        ctx.api.fail("A NamedTuple cannot be a dataclass", ctx=ctx.cls.info)
        return True
    transformer = DataclassTransformer(
        ctx.cls, ctx.reason, _get_transform_spec(ctx.reason), ctx.api
    )
    return transformer.transform()


def _get_transform_spec(reason: Expression) -> DataclassTransformSpec:
    """Find the relevant transform parameters from the decorator/parent class/metaclass that
    triggered the dataclasses plugin.

    Although the resulting DataclassTransformSpec is based on the typing.dataclass_transform
    function, we also use it for traditional dataclasses.dataclass classes as well for simplicity.
    In those cases, we return a default spec rather than one based on a call to
    `typing.dataclass_transform`.
    """
    if _is_dataclasses_decorator(reason):
        return _TRANSFORM_SPEC_FOR_DATACLASSES

    spec = find_dataclass_transform_spec(reason)
    assert spec is not None, (
        "trying to find dataclass transform spec, but reason is neither dataclasses.dataclass nor "
        "decorated with typing.dataclass_transform"
    )
    return spec


def _is_dataclasses_decorator(node: Node) -> bool:
    if isinstance(node, CallExpr):
        node = node.callee
    if isinstance(node, RefExpr):
        return node.fullname in dataclass_makers
    return False


def _has_direct_dataclass_transform_metaclass(info: TypeInfo) -> bool:
    return (
        info.declared_metaclass is not None
        and info.declared_metaclass.type.dataclass_transform_spec is not None
    )


def _get_expanded_dataclasses_fields(
    ctx: FunctionSigContext, typ: ProperType, display_typ: ProperType, parent_typ: ProperType
) -> list[CallableType] | None:
    """
    For a given type, determine what dataclasses it can be: for each class, return the field types.
    For generic classes, the field types are expanded.
    If the type contains Any or a non-dataclass, returns None; in the latter case, also reports an error.
    """
    if isinstance(typ, UnionType):
        ret: list[CallableType] | None = []
        for item in typ.relevant_items():
            item = get_proper_type(item)
            item_types = _get_expanded_dataclasses_fields(ctx, item, item, parent_typ)
            if ret is not None and item_types is not None:
                ret += item_types
            else:
                ret = None  # but keep iterating to emit all errors
        return ret
    elif isinstance(typ, TypeVarType):
        return _get_expanded_dataclasses_fields(
            ctx, get_proper_type(typ.upper_bound), display_typ, parent_typ
        )
    elif isinstance(typ, Instance):
        replace_sym = typ.type.get_method(_INTERNAL_REPLACE_SYM_NAME)
        if replace_sym is None:
            return None
        replace_sig = replace_sym.type
        assert isinstance(replace_sig, ProperType)
        assert isinstance(replace_sig, CallableType)
        return [expand_type_by_instance(replace_sig, typ)]
    else:
        return None


# TODO: we can potentially get the function signature hook to allow returning a union
#  and leave this to the regular machinery of resolving a union of callables
#  (https://github.com/python/mypy/issues/15457)
def _meet_replace_sigs(sigs: list[CallableType]) -> CallableType:
    """
    Produces the lowest bound of the 'replace' signatures of multiple dataclasses.
    """
    args = {
        name: (typ, kind)
        for name, typ, kind in zip(sigs[0].arg_names, sigs[0].arg_types, sigs[0].arg_kinds)
    }

    for sig in sigs[1:]:
        sig_args = {
            name: (typ, kind)
            for name, typ, kind in zip(sig.arg_names, sig.arg_types, sig.arg_kinds)
        }
        for name in (*args.keys(), *sig_args.keys()):
            sig_typ, sig_kind = args.get(name, (UninhabitedType(), ARG_NAMED_OPT))
            sig2_typ, sig2_kind = sig_args.get(name, (UninhabitedType(), ARG_NAMED_OPT))
            args[name] = (
                meet_types(sig_typ, sig2_typ),
                ARG_NAMED_OPT if sig_kind == sig2_kind == ARG_NAMED_OPT else ARG_NAMED,
            )

    return sigs[0].copy_modified(
        arg_names=list(args.keys()),
        arg_types=[typ for typ, _ in args.values()],
        arg_kinds=[kind for _, kind in args.values()],
    )


def replace_function_sig_callback(ctx: FunctionSigContext) -> CallableType:
    """
    Returns a signature for the 'dataclasses.replace' function that's dependent on the type
    of the first positional argument.
    """
    if len(ctx.args) != 2:
        # Ideally the name and context should be the callee's, but we don't have
        # it in FunctionSigContext.
        ctx.api.fail(f'"{ctx.default_signature.name}" has unexpected type annotation', ctx.context)
        return ctx.default_signature

    if len(ctx.args[0]) != 1:
        return ctx.default_signature  # leave it to the type checker to complain

    obj_arg = ctx.args[0][0]
    obj_type = get_proper_type(ctx.api.get_expression_type(obj_arg))
    inst_type_str = format_type_bare(obj_type, ctx.api.options)

    replace_sigs = _get_expanded_dataclasses_fields(ctx, obj_type, obj_type, obj_type)
    if replace_sigs is None:
        return ctx.default_signature
    replace_sig = _meet_replace_sigs(replace_sigs)

    return replace_sig.copy_modified(
        arg_names=[None, *replace_sig.arg_names],
        arg_kinds=[ARG_POS, *replace_sig.arg_kinds],
        arg_types=[obj_type, *replace_sig.arg_types],
        ret_type=obj_type,
        fallback=ctx.default_signature.fallback,
        name=f"{ctx.default_signature.name} of {inst_type_str}",
    )


def is_processed_dataclass(info: TypeInfo) -> bool:
    return bool(info) and "dataclass" in info.metadata


def check_post_init(api: TypeChecker, defn: FuncItem, info: TypeInfo) -> None:
    if defn.type is None:
        return
    assert isinstance(defn.type, FunctionLike)

    ideal_sig_method = info.get_method(_INTERNAL_POST_INIT_SYM_NAME)
    assert ideal_sig_method is not None and ideal_sig_method.type is not None
    ideal_sig = ideal_sig_method.type
    assert isinstance(ideal_sig, ProperType)  # we set it ourselves
    assert isinstance(ideal_sig, CallableType)
    ideal_sig = ideal_sig.copy_modified(name="__post_init__")

    api.check_override(
        override=defn.type,
        original=ideal_sig,
        name="__post_init__",
        name_in_super="__post_init__",
        supertype="dataclass",
        original_class_or_static=False,
        override_class_or_static=False,
        node=defn,
    )
