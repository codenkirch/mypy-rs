"""Information about Python operators"""

from __future__ import annotations

from typing import Final

# Map from binary operator id to related method name (in Python 3).
op_methods: Final = {
    "+": "__add__",
    "-": "__sub__",
    "*": "__mul__",
    "/": "__truediv__",
    "%": "__mod__",
    "divmod": "__divmod__",
    "//": "__floordiv__",
    "**": "__pow__",
    "@": "__matmul__",
    "&": "__and__",
    "|": "__or__",
    "^": "__xor__",
    "<<": "__lshift__",
    ">>": "__rshift__",
    "==": "__eq__",
    "!=": "__ne__",
    "<": "__lt__",
    ">=": "__ge__",
    ">": "__gt__",
    "<=": "__le__",
    "in": "__contains__",
}

op_methods_to_symbols: Final = {v: k for (k, v) in op_methods.items()}

ops_falling_back_to_cmp: Final = {"__ne__", "__eq__", "__lt__", "__le__", "__gt__", "__ge__"}


ops_with_inplace_method: Final = {
    "+",
    "-",
    "*",
    "/",
    "%",
    "//",
    "**",
    "@",
    "&",
    "|",
    "^",
    "<<",
    ">>",
}

inplace_operator_methods: Final = {"__i" + op_methods[op][2:] for op in ops_with_inplace_method}

reverse_op_methods: Final = {
    "__add__": "__radd__",
    "__sub__": "__rsub__",
    "__mul__": "__rmul__",
    "__truediv__": "__rtruediv__",
    "__mod__": "__rmod__",
    "__divmod__": "__rdivmod__",
    "__floordiv__": "__rfloordiv__",
    "__pow__": "__rpow__",
    "__matmul__": "__rmatmul__",
    "__and__": "__rand__",
    "__or__": "__ror__",
    "__xor__": "__rxor__",
    "__lshift__": "__rlshift__",
    "__rshift__": "__rrshift__",
    "__eq__": "__eq__",
    "__ne__": "__ne__",
    "__lt__": "__gt__",
    "__ge__": "__le__",
    "__gt__": "__lt__",
    "__le__": "__ge__",
}

reverse_op_method_names: Final = set(reverse_op_methods.values())

# Suppose we have some class A. When we do A() + A(), Python will only check
# the output of A().__add__(A()) and skip calling the __radd__ method entirely.
# This shortcut is used only for the following methods:
op_methods_that_shortcut: Final = {
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
}

normal_from_reverse_op: Final = {m: n for n, m in reverse_op_methods.items()}
reverse_op_method_set: Final = set(reverse_op_methods.values())

unary_op_methods: Final = {"-": "__neg__", "+": "__pos__", "~": "__invert__"}

int_op_to_method: Final = {
    "==": int.__eq__,
    "is": int.__eq__,
    "<": int.__lt__,
    "<=": int.__le__,
    "!=": int.__ne__,
    "is not": int.__ne__,
    ">": int.__gt__,
    ">=": int.__ge__,
}

flip_ops: Final = {"<": ">", "<=": ">=", ">": "<", ">=": "<="}
neg_ops: Final = {
    "==": "!=",
    "!=": "==",
    "is": "is not",
    "is not": "is",
    "<": ">=",
    "<=": ">",
    ">": "<=",
    ">=": "<",
}


# Stage 4c type-kernel seam: when the `type_kernel` Rust extension is
# importable, replace the Python data tables with the Rust-native copies.
# The data is identical (pure static mappings), so this is a zero-risk swap
# that exercises the Rust module loading path and gives other kernel
# modules a single FFI call to fetch all operator tables.
try:
    from type_kernel import rust_operator_tables as _rust_operator_tables

    _rust_tables = _rust_operator_tables()
    op_methods = _rust_tables["op_methods"]  # type: ignore[assignment]
    op_methods_to_symbols = _rust_tables["op_methods_to_symbols"]  # type: ignore[assignment]
    reverse_op_methods = _rust_tables["reverse_op_methods"]  # type: ignore[assignment]
    unary_op_methods = _rust_tables["unary_op_methods"]  # type: ignore[assignment]
    flip_ops = _rust_tables["flip_ops"]  # type: ignore[assignment]
    neg_ops = _rust_tables["neg_ops"]  # type: ignore[assignment]
    ops_with_inplace_method = _rust_tables["ops_with_inplace_method"]  # type: ignore[assignment]
    inplace_operator_methods = _rust_tables["inplace_operator_methods"]  # type: ignore[assignment]
    ops_falling_back_to_cmp = _rust_tables["ops_falling_back_to_cmp"]  # type: ignore[assignment]
    reverse_op_method_names = _rust_tables["reverse_op_method_names"]  # type: ignore[assignment]
    normal_from_reverse_op = _rust_tables["normal_from_reverse_op"]  # type: ignore[assignment]
    op_methods_that_shortcut = _rust_tables["op_methods_that_shortcut"]  # type: ignore[assignment]
    del _rust_tables
except ImportError:
    pass
