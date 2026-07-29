"""Generic AST node serialization for the type_kernel strangler-fig seam.

Serializes AST nodes (mypy.nodes.Node subclasses) into a binary wire format
that Rust can traverse. This is a generic serializer: it introspects
``__slots__`` and serializes any field that is a ``Node`` (recursively) or a
list/tuple of ``Node`` objects. Scalar fields (line, column, name, kind, etc.)
are not serialized — the format captures tree structure and node kind only,
which is sufficient for the traverser seekers (has_return_statement,
has_yield_expression, etc.) that only need to walk the tree and match on
node kind.

Wire format per node::

    tag:u8                          # node kind tag (from nodes.py constants)
    num_child_fields:varint         # count of serialized child fields
    child_field*                    # each child field:
        LITERAL_NONE                 # if the field is None
        | node                       # if the field is a single Node
        | LIST_GEN size item*        # if the field is a list of Nodes

Leaf nodes (IntExpr, StrExpr, etc.) have no child fields. The tag alone
identifies them.
"""

from __future__ import annotations

from mypy import nodes
from mypy.cache import (
    DICT_STR_GEN,
    END_TAG,
    LIST_GEN,
    LITERAL_NONE,
    ReadBuffer,
    WriteBuffer,
    write_tag,
)
from librt.internal import (
    read_int as read_int_bare,
    write_int as write_int_bare,
)

# Build the tag lookup: class name -> tag value.
# Maps from the Python class to its wire tag constant.
_NODE_TAGS: dict[type, int] = {}


def _register_node_tags() -> None:
    """Populate ``_NODE_TAGS`` from mypy.nodes tag constants."""
    tag_map = {
        nodes.MYPY_FILE: nodes.MypyFile,
        nodes.OVERLOADED_FUNC_DEF: nodes.OverloadedFuncDef,
        nodes.FUNC_DEF: nodes.FuncDef,
        nodes.DECORATOR: nodes.Decorator,
        nodes.VAR: nodes.Var,
        nodes.TYPE_VAR_EXPR: nodes.TypeVarExpr,
        nodes.PARAM_SPEC_EXPR: nodes.ParamSpecExpr,
        nodes.TYPE_VAR_TUPLE_EXPR: nodes.TypeVarTupleExpr,
        nodes.TYPE_INFO: nodes.TypeInfo,
        nodes.TYPE_ALIAS: nodes.TypeAlias,
        nodes.CLASS_DEF: nodes.ClassDef,
        nodes.SYMBOL_TABLE_NODE: nodes.SymbolTableNode,
        nodes.EXPR_STMT: nodes.ExpressionStmt,
        nodes.CALL_EXPR: nodes.CallExpr,
        nodes.NAME_EXPR: nodes.NameExpr,
        nodes.STR_EXPR: nodes.StrExpr,
        nodes.IMPORT: nodes.Import,
        nodes.MEMBER_EXPR: nodes.MemberExpr,
        nodes.OP_EXPR: nodes.OpExpr,
        nodes.INT_EXPR: nodes.IntExpr,
        nodes.IF_STMT: nodes.IfStmt,
        nodes.ASSIGNMENT_STMT: nodes.AssignmentStmt,
        nodes.TUPLE_EXPR: nodes.TupleExpr,
        nodes.BLOCK: nodes.Block,
        nodes.INDEX_EXPR: nodes.IndexExpr,
        nodes.LIST_EXPR: nodes.ListExpr,
        nodes.SET_EXPR: nodes.SetExpr,
        nodes.RETURN_STMT: nodes.ReturnStmt,
        nodes.WHILE_STMT: nodes.WhileStmt,
        nodes.COMPARISON_EXPR: nodes.ComparisonExpr,
        nodes.BOOL_OP_EXPR: nodes.OpExpr,
        nodes.FUNC_DEF_STMT: nodes.FuncDef,
        nodes.PASS_STMT: nodes.PassStmt,
        nodes.FLOAT_EXPR: nodes.FloatExpr,
        nodes.UNARY_EXPR: nodes.UnaryExpr,
        nodes.DICT_EXPR: nodes.DictExpr,
        nodes.COMPLEX_EXPR: nodes.ComplexExpr,
        nodes.SLICE_EXPR: nodes.SliceExpr,
        nodes.TEMP_NODE: nodes.TempNode,
        nodes.RAISE_STMT: nodes.RaiseStmt,
        nodes.BREAK_STMT: nodes.BreakStmt,
        nodes.CONTINUE_STMT: nodes.ContinueStmt,
        nodes.GENERATOR_EXPR: nodes.GeneratorExpr,
        nodes.YIELD_EXPR: nodes.YieldExpr,
        nodes.YIELD_FROM_EXPR: nodes.YieldFromExpr,
        nodes.LIST_COMPREHENSION: nodes.ListComprehension,
        nodes.SET_COMPREHENSION: nodes.SetComprehension,
        nodes.DICT_COMPREHENSION: nodes.DictionaryComprehension,
        nodes.IMPORT_FROM: nodes.ImportFrom,
        nodes.ASSERT_STMT: nodes.AssertStmt,
        nodes.FOR_STMT: nodes.ForStmt,
        nodes.WITH_STMT: nodes.WithStmt,
        nodes.OPERATOR_ASSIGNMENT_STMT: nodes.OperatorAssignmentStmt,
        nodes.TRY_STMT: nodes.TryStmt,
        nodes.ELLIPSIS_EXPR: nodes.EllipsisExpr,
        nodes.CONDITIONAL_EXPR: nodes.ConditionalExpr,
        nodes.DEL_STMT: nodes.DelStmt,
        nodes.LAMBDA_EXPR: nodes.LambdaExpr,
        nodes.ASSIGNMENT_EXPR: nodes.AssignmentExpr,
        nodes.STAR_EXPR: nodes.StarExpr,
        nodes.BYTES_EXPR: nodes.BytesExpr,
        nodes.GLOBAL_DECL: nodes.GlobalDecl,
        nodes.NONLOCAL_DECL: nodes.NonlocalDecl,
        nodes.AWAIT_EXPR: nodes.AwaitExpr,
        nodes.IMPORT_ALL: nodes.ImportAll,
        nodes.MATCH_STMT: nodes.MatchStmt,
        nodes.TYPE_ALIAS_STMT: nodes.TypeAliasStmt,
    }
    # Patterns.
    from mypy import patterns as p
    tag_map.update({
        nodes.AS_PATTERN: p.AsPattern,
        nodes.OR_PATTERN: p.OrPattern,
        nodes.VALUE_PATTERN: p.ValuePattern,
        nodes.SINGLETON_PATTERN: p.SingletonPattern,
        nodes.SEQUENCE_PATTERN: p.SequencePattern,
        nodes.STARRED_PATTERN: p.StarredPattern,
        nodes.MAPPING_PATTERN: p.MappingPattern,
        nodes.CLASS_PATTERN: p.ClassPattern,
    })
    for tag, cls in tag_map.items():
        _NODE_TAGS[cls] = tag


_register_node_tags()


def _get_all_slots(cls: type) -> list[str]:
    """Collect all ``__slots__`` from ``cls`` and its MRO (excluding
    ``Context`` and ``object``)."""
    result: list[str] = []
    skip = {object, nodes.Context, nodes.Node, nodes.Statement,
            nodes.Expression}
    try:
        from mypy.patterns import Pattern
        skip.add(Pattern)
    except ImportError:
        pass
    for base in cls.__mro__:
        if base in skip:
            continue
        slots = getattr(base, "__slots__", ())
        if isinstance(slots, str):
            result.append(slots)
        else:
            result.extend(slots)
    # Deduplicate while preserving order.
    seen: set[str] = set()
    out: list[str] = []
    for s in result:
        if s not in seen:
            seen.add(s)
            out.append(s)
    return out


def serialize_node(node: nodes.Node | None, buf: WriteBuffer) -> None:
    """Serialize a node into ``buf`` as tag + child fields (iterative).

    Uses an explicit task stack instead of recursion so that deeply nested
    ASTs don't hit Python's recursion limit.
    """
    if node is None:
        write_tag(buf, LITERAL_NONE)
        return

    # Task stack: ("end",), ("none",), ("list", items), ("node", node)
    stack: list = [("node", node)]
    while stack:
        task = stack.pop()
        kind = task[0]
        if kind == "end":
            write_tag(buf, END_TAG)
        elif kind == "none":
            write_tag(buf, LITERAL_NONE)
        elif kind == "list":
            items = task[1]
            write_tag(buf, LIST_GEN)
            write_int_bare(buf, len(items))
            for item in reversed(items):
                if isinstance(item, nodes.Node):
                    stack.append(("node", item))
                else:
                    stack.append(("none",))
        elif kind == "node":
            n = task[1]
            cls = type(n)
            tag = _NODE_TAGS.get(cls)
            if tag is None:
                write_tag(buf, LITERAL_NONE)
                continue
            write_tag(buf, tag)
            slots = _get_all_slots(cls)

            # Collect child fields (Node or list-of-Node).
            child_fields: list = []
            for slot in slots:
                value = getattr(n, slot, None)
                if value is None:
                    child_fields.append(None)
                elif isinstance(value, nodes.Node):
                    child_fields.append(value)
                elif isinstance(value, (list, tuple)):
                    items = list(value)
                    if all(
                        isinstance(item, (nodes.Node, type(None))) for item in items
                    ) and len(items) > 0:
                        child_fields.append(items)
                    else:
                        child_fields.append(None)
                else:
                    child_fields.append(None)

            write_int_bare(buf, len(child_fields))
            # Push END_TAG first (LIFO → processed last), then fields
            # in reverse order so they pop in forward order.
            stack.append(("end",))
            for field in reversed(child_fields):
                if field is None:
                    stack.append(("none",))
                elif isinstance(field, nodes.Node):
                    stack.append(("node", field))
                else:
                    stack.append(("list", field))
