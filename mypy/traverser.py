"""Generic node traverser visitor"""

from __future__ import annotations

from mypy_extensions import mypyc_attr, trait

from mypy.nodes import (
    REVEAL_TYPE,
    AssertStmt,
    AssertTypeExpr,
    AssignmentExpr,
    AssignmentStmt,
    AwaitExpr,
    Block,
    BreakStmt,
    BytesExpr,
    CallExpr,
    CastExpr,
    ClassDef,
    ComparisonExpr,
    ComplexExpr,
    ConditionalExpr,
    ContinueStmt,
    Decorator,
    DelStmt,
    DictExpr,
    DictionaryComprehension,
    EllipsisExpr,
    EnumCallExpr,
    Expression,
    ExpressionStmt,
    FloatExpr,
    ForStmt,
    FuncBase,
    FuncDef,
    FuncItem,
    GeneratorExpr,
    GlobalDecl,
    IfStmt,
    Import,
    ImportAll,
    ImportFrom,
    IndexExpr,
    IntExpr,
    LambdaExpr,
    ListComprehension,
    ListExpr,
    MatchStmt,
    MemberExpr,
    MypyFile,
    NamedTupleExpr,
    NameExpr,
    NewTypeExpr,
    Node,
    NonlocalDecl,
    OperatorAssignmentStmt,
    OpExpr,
    OverloadedFuncDef,
    ParamSpecExpr,
    PassStmt,
    PromoteExpr,
    RaiseStmt,
    ReturnStmt,
    RevealExpr,
    SetComprehension,
    SetExpr,
    SliceExpr,
    StarExpr,
    StrExpr,
    SuperExpr,
    TemplateStrExpr,
    TempNode,
    TryStmt,
    TupleExpr,
    TypeAlias,
    TypeAliasExpr,
    TypeAliasStmt,
    TypeApplication,
    TypedDictExpr,
    TypeFormExpr,
    TypeVarExpr,
    TypeVarTupleExpr,
    UnaryExpr,
    Var,
    WhileStmt,
    WithStmt,
    YieldExpr,
    YieldFromExpr,
)
from mypy.patterns import (
    AsPattern,
    ClassPattern,
    MappingPattern,
    OrPattern,
    SequencePattern,
    SingletonPattern,
    StarredPattern,
    ValuePattern,
)
from mypy.visitor import NodeVisitor

# ---------------------------------------------------------------------------
# Stage 14: native traverser shims (parity-only).

# Routes has_return_statement, has_str_expression, has_yield_expression,
# has_yield_from_expression, has_await_expression through the AST
# wire-format (mypy/astwire.py) into the Rust type_kernel extension.

# Also imports count functions for all_* collectors (parity validation).

# Rust traverses the serialized tree; falls back to pure-Python on error.
try:
    from type_kernel import rust_has_return_statement as _rust_has_return_statement
    from type_kernel import rust_has_str_expression as _rust_has_str_expression
    from type_kernel import rust_has_yield_expression as _rust_has_yield_expression
    from type_kernel import (
        rust_has_yield_from_expression as _rust_has_yield_from_expression,
    )
    from type_kernel import rust_has_await_expression as _rust_has_await_expression
    from type_kernel import (
        rust_count_return_statements as _rust_count_return_statements,
    )
    from type_kernel import (
        rust_count_yield_expressions as _rust_count_yield_expressions,
    )
    from type_kernel import (
        rust_count_yield_from_expressions as _rust_count_yield_from_expressions,
    )
    from type_kernel import (
        rust_count_name_and_member_expressions as _rust_count_name_and_member_exprs,
    )
    from type_kernel import (
        rust_count_return_statements_and_flags as _rust_count_returns_and_flags,
    )
    from type_kernel import rust_count_all_returns as _rust_count_all_returns
    from type_kernel import rust_has_yield_return as _rust_has_yield_return
    from type_kernel import rust_has_complex_slice as _rust_has_complex_slice
    from type_kernel import (
        rust_count_non_extension_handlers as _rust_count_non_extension_handlers,
    )
    from type_kernel import rust_is_global_expr as _rust_is_global_expr
    from type_kernel import (
        rust_count_non_literal_handlers as _rust_count_non_literal_handlers,
    )
    from mypy.astwire import serialize_node as _ast_serialize_node
    from mypy.cache import WriteBuffer as _AstWriteBuffer

    _TRAVERSER_HAS_KERNEL = True
except ImportError:
    _rust_has_return_statement = None  # type: ignore[assignment]
    _rust_has_str_expression = None  # type: ignore[assignment]
    _rust_has_yield_expression = None  # type: ignore[assignment]
    _rust_has_yield_from_expression = None  # type: ignore[assignment]
    _rust_has_await_expression = None  # type: ignore[assignment]
    _rust_count_return_statements = None  # type: ignore[assignment]
    _rust_count_yield_expressions = None  # type: ignore[assignment]
    _rust_count_yield_from_expressions = None  # type: ignore[assignment]
    _rust_count_name_and_member_exprs = None  # type: ignore[assignment]
    _rust_count_returns_and_flags = None  # type: ignore[assignment]
    _rust_count_all_returns = None  # type: ignore[assignment]
    _rust_has_yield_return = None  # type: ignore[assignment]
    _rust_has_complex_slice = None  # type: ignore[assignment]
    _rust_count_non_extension_handlers = None  # type: ignore[assignment]
    _rust_is_global_expr = None  # type: ignore[assignment]
    _rust_count_non_literal_handlers = None  # type: ignore[assignment]
    _ast_serialize_node = None  # type: ignore[assignment]
    _AstWriteBuffer = None  # type: ignore[assignment,misc]
    _TRAVERSER_HAS_KERNEL = False


_serialize_ast_cache: dict[int, tuple[Node, bytes]] = {}


def _serialize_ast_node(node: Node) -> bytes:
    """Serialize a node for the Rust traverser (returns bytes).

    Memoized per node identity: the live AST is immutable during a build
    and each call serializes the same root with a fresh visited-set, so
    the bytes are deterministic per node. The cache holds a strong ref
    to the node so a recycled ``id()`` can never serve stale bytes (the
    identity check below defeats id-reuse). Callers must clear the cache
    (``_clear_serialize_ast_cache``) when the AST may have changed.
    """
    key = id(node)
    entry = _serialize_ast_cache.get(key)
    if entry is not None and entry[0] is node:
        return entry[1]
    buf = _AstWriteBuffer()
    _ast_serialize_node(node, buf)
    bytes_out = buf.getvalue()
    _serialize_ast_cache[key] = (node, bytes_out)
    return bytes_out


def _clear_serialize_ast_cache() -> None:
    _serialize_ast_cache.clear()


@trait
@mypyc_attr(allow_interpreted_subclasses=True)
class TraverserVisitor(NodeVisitor[None]):
    """A parse tree visitor that traverses the parse tree during visiting.

    It does not perform any actions outside the traversal. Subclasses
    should override visit methods to perform actions during
    traversal. Calling the superclass method allows reusing the
    traversal implementation.

    TODO: split this into more limited visitor (e.g. statements-only etc).
    This will improve performance since in many cases we don't need to recurse
    all the way down in various visitors that subclass this.
    """

    def __init__(self) -> None:
        pass

    # Visit methods

    def visit_mypy_file(self, o: MypyFile, /) -> None:
        for d in o.defs:
            d.accept(self)

    def visit_block(self, block: Block, /) -> None:
        for s in block.body:
            s.accept(self)

    def visit_func(self, o: FuncItem, /) -> None:
        if o.arguments is not None:
            for arg in o.arguments:
                init = arg.initializer
                if init is not None:
                    init.accept(self)

            for arg in o.arguments:
                self.visit_var(arg.variable)

        o.body.accept(self)

    def visit_func_def(self, o: FuncDef, /) -> None:
        self.visit_func(o)

    def visit_overloaded_func_def(self, o: OverloadedFuncDef, /) -> None:
        for item in o.items:
            item.accept(self)
        if o.impl:
            o.impl.accept(self)

    def visit_class_def(self, o: ClassDef, /) -> None:
        for d in o.decorators:
            d.accept(self)
        for base in o.base_type_exprs:
            base.accept(self)
        if o.metaclass:
            o.metaclass.accept(self)
        for v in o.keywords.values():
            v.accept(self)
        o.defs.accept(self)
        if o.analyzed:
            o.analyzed.accept(self)

    def visit_decorator(self, o: Decorator, /) -> None:
        o.func.accept(self)
        o.var.accept(self)
        for decorator in o.decorators:
            decorator.accept(self)

    def visit_expression_stmt(self, o: ExpressionStmt, /) -> None:
        o.expr.accept(self)

    def visit_assignment_stmt(self, o: AssignmentStmt, /) -> None:
        o.rvalue.accept(self)
        for l in o.lvalues:
            l.accept(self)

    def visit_operator_assignment_stmt(self, o: OperatorAssignmentStmt, /) -> None:
        o.rvalue.accept(self)
        o.lvalue.accept(self)

    def visit_while_stmt(self, o: WhileStmt, /) -> None:
        o.expr.accept(self)
        o.body.accept(self)
        if o.else_body:
            o.else_body.accept(self)

    def visit_for_stmt(self, o: ForStmt, /) -> None:
        o.index.accept(self)
        o.expr.accept(self)
        o.body.accept(self)
        if o.else_body:
            o.else_body.accept(self)

    def visit_return_stmt(self, o: ReturnStmt, /) -> None:
        if o.expr is not None:
            o.expr.accept(self)

    def visit_assert_stmt(self, o: AssertStmt, /) -> None:
        if o.expr is not None:
            o.expr.accept(self)
        if o.msg is not None:
            o.msg.accept(self)

    def visit_del_stmt(self, o: DelStmt, /) -> None:
        if o.expr is not None:
            o.expr.accept(self)

    def visit_if_stmt(self, o: IfStmt, /) -> None:
        for e in o.expr:
            e.accept(self)
        for b in o.body:
            b.accept(self)
        if o.else_body:
            o.else_body.accept(self)

    def visit_raise_stmt(self, o: RaiseStmt, /) -> None:
        if o.expr is not None:
            o.expr.accept(self)
        if o.from_expr is not None:
            o.from_expr.accept(self)

    def visit_try_stmt(self, o: TryStmt, /) -> None:
        o.body.accept(self)
        for i in range(len(o.types)):
            tp = o.types[i]
            if tp is not None:
                tp.accept(self)
            o.handlers[i].accept(self)
        for v in o.vars:
            if v is not None:
                v.accept(self)
        if o.else_body is not None:
            o.else_body.accept(self)
        if o.finally_body is not None:
            o.finally_body.accept(self)

    def visit_with_stmt(self, o: WithStmt, /) -> None:
        for i in range(len(o.expr)):
            o.expr[i].accept(self)
            targ = o.target[i]
            if targ is not None:
                targ.accept(self)
        o.body.accept(self)

    def visit_match_stmt(self, o: MatchStmt, /) -> None:
        o.subject.accept(self)
        for i in range(len(o.patterns)):
            o.patterns[i].accept(self)
            guard = o.guards[i]
            if guard is not None:
                guard.accept(self)
            o.bodies[i].accept(self)

    def visit_type_alias_stmt(self, o: TypeAliasStmt, /) -> None:
        o.name.accept(self)
        o.value.accept(self)

    def visit_member_expr(self, o: MemberExpr, /) -> None:
        o.expr.accept(self)

    def visit_yield_from_expr(self, o: YieldFromExpr, /) -> None:
        o.expr.accept(self)

    def visit_yield_expr(self, o: YieldExpr, /) -> None:
        if o.expr:
            o.expr.accept(self)

    def visit_call_expr(self, o: CallExpr, /) -> None:
        o.callee.accept(self)
        for a in o.args:
            a.accept(self)
        if o.analyzed:
            o.analyzed.accept(self)

    def visit_op_expr(self, o: OpExpr, /) -> None:
        o.left.accept(self)
        o.right.accept(self)
        if o.analyzed is not None:
            o.analyzed.accept(self)

    def visit_comparison_expr(self, o: ComparisonExpr, /) -> None:
        for operand in o.operands:
            operand.accept(self)

    def visit_slice_expr(self, o: SliceExpr, /) -> None:
        if o.begin_index is not None:
            o.begin_index.accept(self)
        if o.end_index is not None:
            o.end_index.accept(self)
        if o.stride is not None:
            o.stride.accept(self)

    def visit_cast_expr(self, o: CastExpr, /) -> None:
        o.expr.accept(self)

    def visit_type_form_expr(self, o: TypeFormExpr, /) -> None:
        pass

    def visit_assert_type_expr(self, o: AssertTypeExpr, /) -> None:
        o.expr.accept(self)

    def visit_reveal_expr(self, o: RevealExpr, /) -> None:
        if o.kind == REVEAL_TYPE:
            assert o.expr is not None
            o.expr.accept(self)
        else:
            # RevealLocalsExpr doesn't have an inner expression
            pass

    def visit_assignment_expr(self, o: AssignmentExpr, /) -> None:
        o.target.accept(self)
        o.value.accept(self)

    def visit_unary_expr(self, o: UnaryExpr, /) -> None:
        o.expr.accept(self)

    def visit_list_expr(self, o: ListExpr, /) -> None:
        for item in o.items:
            item.accept(self)

    def visit_tuple_expr(self, o: TupleExpr, /) -> None:
        for item in o.items:
            item.accept(self)

    def visit_dict_expr(self, o: DictExpr, /) -> None:
        for k, v in o.items:
            if k is not None:
                k.accept(self)
            v.accept(self)

    def visit_template_str_expr(self, o: TemplateStrExpr, /) -> None:
        for item in o.items:
            if isinstance(item, tuple):
                item[0].accept(self)
                if item[3] is not None:
                    item[3].accept(self)
            else:
                item.accept(self)

    def visit_set_expr(self, o: SetExpr, /) -> None:
        for item in o.items:
            item.accept(self)

    def visit_index_expr(self, o: IndexExpr, /) -> None:
        o.base.accept(self)
        o.index.accept(self)
        if o.analyzed:
            o.analyzed.accept(self)

    def visit_generator_expr(self, o: GeneratorExpr, /) -> None:
        for index, sequence, conditions in zip(o.indices, o.sequences, o.condlists):
            sequence.accept(self)
            index.accept(self)
            for cond in conditions:
                cond.accept(self)
        o.left_expr.accept(self)

    def visit_dictionary_comprehension(self, o: DictionaryComprehension, /) -> None:
        for index, sequence, conditions in zip(o.indices, o.sequences, o.condlists):
            sequence.accept(self)
            index.accept(self)
            for cond in conditions:
                cond.accept(self)
        o.key.accept(self)
        o.value.accept(self)

    def visit_list_comprehension(self, o: ListComprehension, /) -> None:
        o.generator.accept(self)

    def visit_set_comprehension(self, o: SetComprehension, /) -> None:
        o.generator.accept(self)

    def visit_conditional_expr(self, o: ConditionalExpr, /) -> None:
        o.cond.accept(self)
        o.if_expr.accept(self)
        o.else_expr.accept(self)

    def visit_type_application(self, o: TypeApplication, /) -> None:
        o.expr.accept(self)

    def visit_lambda_expr(self, o: LambdaExpr, /) -> None:
        self.visit_func(o)

    def visit_star_expr(self, o: StarExpr, /) -> None:
        o.expr.accept(self)

    def visit_await_expr(self, o: AwaitExpr, /) -> None:
        o.expr.accept(self)

    def visit_super_expr(self, o: SuperExpr, /) -> None:
        o.call.accept(self)

    def visit_as_pattern(self, o: AsPattern, /) -> None:
        if o.pattern is not None:
            o.pattern.accept(self)
        if o.name is not None:
            o.name.accept(self)

    def visit_or_pattern(self, o: OrPattern, /) -> None:
        for p in o.patterns:
            p.accept(self)

    def visit_value_pattern(self, o: ValuePattern, /) -> None:
        o.expr.accept(self)

    def visit_sequence_pattern(self, o: SequencePattern, /) -> None:
        for p in o.patterns:
            p.accept(self)

    def visit_starred_pattern(self, o: StarredPattern, /) -> None:
        if o.capture is not None:
            o.capture.accept(self)

    def visit_mapping_pattern(self, o: MappingPattern, /) -> None:
        for key in o.keys:
            key.accept(self)
        for value in o.values:
            value.accept(self)
        if o.rest is not None:
            o.rest.accept(self)

    def visit_class_pattern(self, o: ClassPattern, /) -> None:
        o.class_ref.accept(self)
        for p in o.positionals:
            p.accept(self)
        for v in o.keyword_values:
            v.accept(self)

    def visit_import(self, o: Import, /) -> None:
        for a in o.assignments:
            a.accept(self)

    def visit_import_from(self, o: ImportFrom, /) -> None:
        for a in o.assignments:
            a.accept(self)

    # leaf nodes
    def visit_name_expr(self, o: NameExpr, /) -> None:
        return None

    def visit_str_expr(self, o: StrExpr, /) -> None:
        return None

    def visit_int_expr(self, o: IntExpr, /) -> None:
        return None

    def visit_float_expr(self, o: FloatExpr, /) -> None:
        return None

    def visit_bytes_expr(self, o: BytesExpr, /) -> None:
        return None

    def visit_ellipsis(self, o: EllipsisExpr, /) -> None:
        return None

    def visit_var(self, o: Var, /) -> None:
        return None

    def visit_continue_stmt(self, o: ContinueStmt, /) -> None:
        return None

    def visit_pass_stmt(self, o: PassStmt, /) -> None:
        return None

    def visit_break_stmt(self, o: BreakStmt, /) -> None:
        return None

    def visit_temp_node(self, o: TempNode, /) -> None:
        return None

    def visit_nonlocal_decl(self, o: NonlocalDecl, /) -> None:
        return None

    def visit_global_decl(self, o: GlobalDecl, /) -> None:
        return None

    def visit_import_all(self, o: ImportAll, /) -> None:
        return None

    def visit_type_var_expr(self, o: TypeVarExpr, /) -> None:
        return None

    def visit_paramspec_expr(self, o: ParamSpecExpr, /) -> None:
        return None

    def visit_type_var_tuple_expr(self, o: TypeVarTupleExpr, /) -> None:
        return None

    def visit_type_alias_expr(self, o: TypeAliasExpr, /) -> None:
        return None

    def visit_type_alias(self, o: TypeAlias, /) -> None:
        return None

    def visit_namedtuple_expr(self, o: NamedTupleExpr, /) -> None:
        return None

    def visit_typeddict_expr(self, o: TypedDictExpr, /) -> None:
        return None

    def visit_newtype_expr(self, o: NewTypeExpr, /) -> None:
        return None

    def visit__promote_expr(self, o: PromoteExpr, /) -> None:
        return None

    def visit_complex_expr(self, o: ComplexExpr, /) -> None:
        return None

    def visit_enum_call_expr(self, o: EnumCallExpr, /) -> None:
        return None

    def visit_singleton_pattern(self, o: SingletonPattern, /) -> None:
        return None


class ExtendedTraverserVisitor(TraverserVisitor):
    """This is a more flexible traverser.

    In addition to the base traverser it:
        * has visit_ methods for leaf nodes
        * has common method that is called for all nodes
        * allows skipping recursing into a node

    Note that this traverser still doesn't visit some internal
    mypy constructs like _promote expression and Var.
    """

    def visit(self, o: Node) -> bool:
        # If returns True, will continue to nested nodes.
        return True

    def visit_mypy_file(self, o: MypyFile, /) -> None:
        if not self.visit(o):
            return
        super().visit_mypy_file(o)

    # Module structure

    def visit_import(self, o: Import, /) -> None:
        if not self.visit(o):
            return
        super().visit_import(o)

    def visit_import_from(self, o: ImportFrom, /) -> None:
        if not self.visit(o):
            return
        super().visit_import_from(o)

    def visit_import_all(self, o: ImportAll, /) -> None:
        if not self.visit(o):
            return
        super().visit_import_all(o)

    # Definitions

    def visit_func_def(self, o: FuncDef, /) -> None:
        if not self.visit(o):
            return
        super().visit_func_def(o)

    def visit_overloaded_func_def(self, o: OverloadedFuncDef, /) -> None:
        if not self.visit(o):
            return
        super().visit_overloaded_func_def(o)

    def visit_class_def(self, o: ClassDef, /) -> None:
        if not self.visit(o):
            return
        super().visit_class_def(o)

    def visit_global_decl(self, o: GlobalDecl, /) -> None:
        if not self.visit(o):
            return
        super().visit_global_decl(o)

    def visit_nonlocal_decl(self, o: NonlocalDecl, /) -> None:
        if not self.visit(o):
            return
        super().visit_nonlocal_decl(o)

    def visit_decorator(self, o: Decorator, /) -> None:
        if not self.visit(o):
            return
        super().visit_decorator(o)

    def visit_type_alias(self, o: TypeAlias, /) -> None:
        if not self.visit(o):
            return
        super().visit_type_alias(o)

    # Statements

    def visit_block(self, block: Block, /) -> None:
        if not self.visit(block):
            return
        super().visit_block(block)

    def visit_expression_stmt(self, o: ExpressionStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_expression_stmt(o)

    def visit_assignment_stmt(self, o: AssignmentStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_assignment_stmt(o)

    def visit_operator_assignment_stmt(self, o: OperatorAssignmentStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_operator_assignment_stmt(o)

    def visit_while_stmt(self, o: WhileStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_while_stmt(o)

    def visit_for_stmt(self, o: ForStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_for_stmt(o)

    def visit_return_stmt(self, o: ReturnStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_return_stmt(o)

    def visit_assert_stmt(self, o: AssertStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_assert_stmt(o)

    def visit_del_stmt(self, o: DelStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_del_stmt(o)

    def visit_if_stmt(self, o: IfStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_if_stmt(o)

    def visit_break_stmt(self, o: BreakStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_break_stmt(o)

    def visit_continue_stmt(self, o: ContinueStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_continue_stmt(o)

    def visit_pass_stmt(self, o: PassStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_pass_stmt(o)

    def visit_raise_stmt(self, o: RaiseStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_raise_stmt(o)

    def visit_try_stmt(self, o: TryStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_try_stmt(o)

    def visit_with_stmt(self, o: WithStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_with_stmt(o)

    def visit_match_stmt(self, o: MatchStmt, /) -> None:
        if not self.visit(o):
            return
        super().visit_match_stmt(o)

    # Expressions (default no-op implementation)

    def visit_int_expr(self, o: IntExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_int_expr(o)

    def visit_str_expr(self, o: StrExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_str_expr(o)

    def visit_bytes_expr(self, o: BytesExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_bytes_expr(o)

    def visit_float_expr(self, o: FloatExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_float_expr(o)

    def visit_complex_expr(self, o: ComplexExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_complex_expr(o)

    def visit_ellipsis(self, o: EllipsisExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_ellipsis(o)

    def visit_star_expr(self, o: StarExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_star_expr(o)

    def visit_name_expr(self, o: NameExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_name_expr(o)

    def visit_member_expr(self, o: MemberExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_member_expr(o)

    def visit_yield_from_expr(self, o: YieldFromExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_yield_from_expr(o)

    def visit_yield_expr(self, o: YieldExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_yield_expr(o)

    def visit_call_expr(self, o: CallExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_call_expr(o)

    def visit_op_expr(self, o: OpExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_op_expr(o)

    def visit_comparison_expr(self, o: ComparisonExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_comparison_expr(o)

    def visit_cast_expr(self, o: CastExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_cast_expr(o)

    def visit_type_form_expr(self, o: TypeFormExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_type_form_expr(o)

    def visit_assert_type_expr(self, o: AssertTypeExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_assert_type_expr(o)

    def visit_reveal_expr(self, o: RevealExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_reveal_expr(o)

    def visit_super_expr(self, o: SuperExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_super_expr(o)

    def visit_assignment_expr(self, o: AssignmentExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_assignment_expr(o)

    def visit_unary_expr(self, o: UnaryExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_unary_expr(o)

    def visit_list_expr(self, o: ListExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_list_expr(o)

    def visit_dict_expr(self, o: DictExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_dict_expr(o)

    def visit_template_str_expr(self, o: TemplateStrExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_template_str_expr(o)

    def visit_tuple_expr(self, o: TupleExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_tuple_expr(o)

    def visit_set_expr(self, o: SetExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_set_expr(o)

    def visit_index_expr(self, o: IndexExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_index_expr(o)

    def visit_type_application(self, o: TypeApplication, /) -> None:
        if not self.visit(o):
            return
        super().visit_type_application(o)

    def visit_lambda_expr(self, o: LambdaExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_lambda_expr(o)

    def visit_list_comprehension(self, o: ListComprehension, /) -> None:
        if not self.visit(o):
            return
        super().visit_list_comprehension(o)

    def visit_set_comprehension(self, o: SetComprehension, /) -> None:
        if not self.visit(o):
            return
        super().visit_set_comprehension(o)

    def visit_dictionary_comprehension(self, o: DictionaryComprehension, /) -> None:
        if not self.visit(o):
            return
        super().visit_dictionary_comprehension(o)

    def visit_generator_expr(self, o: GeneratorExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_generator_expr(o)

    def visit_slice_expr(self, o: SliceExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_slice_expr(o)

    def visit_conditional_expr(self, o: ConditionalExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_conditional_expr(o)

    def visit_type_var_expr(self, o: TypeVarExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_type_var_expr(o)

    def visit_paramspec_expr(self, o: ParamSpecExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_paramspec_expr(o)

    def visit_type_var_tuple_expr(self, o: TypeVarTupleExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_type_var_tuple_expr(o)

    def visit_type_alias_expr(self, o: TypeAliasExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_type_alias_expr(o)

    def visit_namedtuple_expr(self, o: NamedTupleExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_namedtuple_expr(o)

    def visit_enum_call_expr(self, o: EnumCallExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_enum_call_expr(o)

    def visit_typeddict_expr(self, o: TypedDictExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_typeddict_expr(o)

    def visit_newtype_expr(self, o: NewTypeExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_newtype_expr(o)

    def visit_await_expr(self, o: AwaitExpr, /) -> None:
        if not self.visit(o):
            return
        super().visit_await_expr(o)

    # Patterns

    def visit_as_pattern(self, o: AsPattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_as_pattern(o)

    def visit_or_pattern(self, o: OrPattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_or_pattern(o)

    def visit_value_pattern(self, o: ValuePattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_value_pattern(o)

    def visit_singleton_pattern(self, o: SingletonPattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_singleton_pattern(o)

    def visit_sequence_pattern(self, o: SequencePattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_sequence_pattern(o)

    def visit_starred_pattern(self, o: StarredPattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_starred_pattern(o)

    def visit_mapping_pattern(self, o: MappingPattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_mapping_pattern(o)

    def visit_class_pattern(self, o: ClassPattern, /) -> None:
        if not self.visit(o):
            return
        super().visit_class_pattern(o)


class ReturnSeeker(TraverserVisitor):
    def __init__(self) -> None:
        self.found = False

    def visit_return_stmt(self, o: ReturnStmt) -> None:
        if o.expr is None or isinstance(o.expr, NameExpr) and o.expr.name == "None":
            return
        self.found = True


def has_return_statement(fdef: FuncBase) -> bool:
    """Find if a function has a non-trivial return statement.

    Plain 'return' and 'return None' don't count.
    """
    if _TRAVERSER_HAS_KERNEL:
        try:
            # None = defer: the node kind has no wire tag (e.g. a bare
            # FuncItem serializes to LITERAL_NONE), so Rust cannot answer
            # and we fall back to the pure-Python seeker (#1030).
            native = _rust_has_return_statement(_serialize_ast_node(fdef))
            if native is not None:
                return native
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    seeker = ReturnSeeker()
    fdef.accept(seeker)
    return seeker.found


class NameAndMemberCollector(TraverserVisitor):
    def __init__(self) -> None:
        super().__init__()
        self.name_exprs: list[NameExpr] = []
        self.member_exprs: list[MemberExpr] = []

    def visit_name_expr(self, o: NameExpr, /) -> None:
        self.name_exprs.append(o)
        super().visit_name_expr(o)

    def visit_member_expr(self, o: MemberExpr, /) -> None:
        self.member_exprs.append(o)
        super().visit_member_expr(o)


def all_name_and_member_expressions(
    node: MypyFile | Expression,
) -> tuple[list[NameExpr], list[MemberExpr]]:
    if _TRAVERSER_HAS_KERNEL:
        try:
            rust_names, rust_members = _rust_count_name_and_member_exprs(
                _serialize_ast_node(node)
            )
            if rust_names == 0 and rust_members == 0:
                return ([], [])
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = NameAndMemberCollector()
    node.accept(v)
    return (v.name_exprs, v.member_exprs)


class StringSeeker(TraverserVisitor):
    def __init__(self) -> None:
        self.found = False

    def visit_str_expr(self, o: StrExpr, /) -> None:
        self.found = True


def has_str_expression(node: MypyFile | Expression) -> bool:
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_has_str_expression(_serialize_ast_node(node))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = StringSeeker()
    node.accept(v)
    return v.found


class FuncCollectorBase(TraverserVisitor):
    def __init__(self) -> None:
        self.inside_func = False

    def visit_func_def(self, defn: FuncDef) -> None:
        if not self.inside_func:
            self.inside_func = True
            super().visit_func_def(defn)
            self.inside_func = False


class YieldSeeker(FuncCollectorBase):
    def __init__(self) -> None:
        super().__init__()
        self.found = False

    def visit_yield_expr(self, o: YieldExpr) -> None:
        self.found = True


def has_yield_expression(fdef: FuncBase) -> bool:
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_has_yield_expression(_serialize_ast_node(fdef))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    seeker = YieldSeeker()
    fdef.accept(seeker)
    return seeker.found


class YieldFromSeeker(FuncCollectorBase):
    def __init__(self) -> None:
        super().__init__()
        self.found = False

    def visit_yield_from_expr(self, o: YieldFromExpr) -> None:
        self.found = True


def has_yield_from_expression(fdef: FuncBase) -> bool:
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_has_yield_from_expression(_serialize_ast_node(fdef))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    seeker = YieldFromSeeker()
    fdef.accept(seeker)
    return seeker.found


class AwaitSeeker(TraverserVisitor):
    def __init__(self) -> None:
        super().__init__()
        self.found = False

    def visit_await_expr(self, o: AwaitExpr) -> None:
        self.found = True


def has_await_expression(expr: MypyFile | FuncDef | Expression) -> bool:
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_has_await_expression(_serialize_ast_node(expr))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    seeker = AwaitSeeker()
    expr.accept(seeker)
    return seeker.found


def has_await_in_generator(gen: GeneratorExpr) -> bool:
    """Combined has-await for the checkexpr GeneratorExpr check.

    Walks left_expr, sequences[1:] and condlists with a Python
    AwaitSeeker. This is the original fallback semantics; the Rust
    kernel mirror (rust_has_await_in_generator) was retired from the
    hot path: serializing the whole GeneratorExpr subtree cost more
    than the Rust walk saved at a 0/1137 true rate in the self-check
    (one serialize per cache miss). The kernel function stays exported
    for parity tests. The first sequence and the indices are NOT
    checked, matching checkexpr.py:7417-7422.
    """
    # Read `found` through a fresh local each time: mypy narrows the
    # `seeker.found` member after the first check and accept() does not
    # invalidate that narrowing, so the direct reads look unreachable.
    seeker = AwaitSeeker()
    gen.left_expr.accept(seeker)
    found = seeker.found
    if found:
        return True
    for sequence in gen.sequences[1:]:
        sequence.accept(seeker)
        found = seeker.found
        if found:
            return True
    for condlist in gen.condlists:
        for cond in condlist:
            cond.accept(seeker)
            found = seeker.found
            if found:
                return True
    return False


class ReturnCollector(FuncCollectorBase):
    def __init__(self) -> None:
        super().__init__()
        self.return_statements: list[ReturnStmt] = []

    def visit_return_stmt(self, stmt: ReturnStmt) -> None:
        self.return_statements.append(stmt)


def all_return_statements(node: Node) -> list[ReturnStmt]:
    if _TRAVERSER_HAS_KERNEL:
        try:
            if _rust_count_return_statements(_serialize_ast_node(node)) == 0:
                return []
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = ReturnCollector()
    node.accept(v)
    return v.return_statements


class YieldCollector(FuncCollectorBase):
    def __init__(self) -> None:
        super().__init__()
        self.in_assignment = False
        self.yield_expressions: list[tuple[YieldExpr, bool]] = []

    def visit_assignment_stmt(self, stmt: AssignmentStmt) -> None:
        self.in_assignment = True
        super().visit_assignment_stmt(stmt)
        self.in_assignment = False

    def visit_yield_expr(self, expr: YieldExpr) -> None:
        self.yield_expressions.append((expr, self.in_assignment))


def all_yield_expressions(node: Node) -> list[tuple[YieldExpr, bool]]:
    if _TRAVERSER_HAS_KERNEL:
        try:
            if _rust_count_yield_expressions(_serialize_ast_node(node)) == 0:
                return []
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = YieldCollector()
    node.accept(v)
    return v.yield_expressions


class YieldFromCollector(FuncCollectorBase):
    def __init__(self) -> None:
        super().__init__()
        self.in_assignment = False
        self.yield_from_expressions: list[tuple[YieldFromExpr, bool]] = []

    def visit_assignment_stmt(self, stmt: AssignmentStmt) -> None:
        self.in_assignment = True
        super().visit_assignment_stmt(stmt)
        self.in_assignment = False

    def visit_yield_from_expr(self, expr: YieldFromExpr) -> None:
        self.yield_from_expressions.append((expr, self.in_assignment))


def all_yield_from_expressions(node: Node) -> list[tuple[YieldFromExpr, bool]]:
    if _TRAVERSER_HAS_KERNEL:
        try:
            if _rust_count_yield_from_expressions(_serialize_ast_node(node)) == 0:
                return []
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = YieldFromCollector()
    node.accept(v)
    return v.yield_from_expressions


# ---------------------------------------------------------------------------
# Issue #541: remaining seekers (structural, wire-format backed).
# These functions do not have pre-existing pure-Python collectors in

# traverser.py. Each uses the Rust extension for the structural walk
# and falls back to a pure-Python implementation on ImportError or
# runtime error.


class ReturnAndFlagsCollector(FuncCollectorBase):
    def __init__(self) -> None:
        super().__init__()
        self.in_finally = False
        self.return_statements: list[tuple[ReturnStmt, bool]] = []

    def visit_try_stmt(self, o: TryStmt) -> None:
        # TraverserVisitor.visit_try_stmt visits body, handlers, else,
        # then finally. We set in_finally only while visiting finally_body.
        o.body.accept(self)
        for i in range(len(o.types)):
            tp = o.types[i]
            if tp is not None:
                tp.accept(self)
            o.handlers[i].accept(self)
        for v in o.vars:
            if v is not None:
                v.accept(self)
        if o.else_body is not None:
            o.else_body.accept(self)
        if o.finally_body is not None:
            old = self.in_finally
            self.in_finally = True
            o.finally_body.accept(self)
            self.in_finally = old

    def visit_return_stmt(self, stmt: ReturnStmt) -> None:
        self.return_statements.append((stmt, self.in_finally))


def all_return_statements_and_flags(
    node: Node,
) -> list[tuple[ReturnStmt, bool]]:
    """Collect (ReturnStmt, in_finally) pairs."""
    if _TRAVERSER_HAS_KERNEL:
        try:
            total, in_finally = _rust_count_returns_and_flags(
                _serialize_ast_node(node)
            )
            if total == 0:
                return []
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = ReturnAndFlagsCollector()
    node.accept(v)
    return v.return_statements


def count_returns(node: Node) -> int:
    """Count ALL return statements, including those in nested functions."""
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_count_all_returns(_serialize_ast_node(node))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    count = 0

    class Counter(TraverserVisitor):
        def visit_return_stmt(self, o: ReturnStmt) -> None:
            nonlocal count
            count += 1

    v = Counter()
    node.accept(v)
    return count


class YieldReturnSeeker(TraverserVisitor):
    def __init__(self) -> None:
        super().__init__()
        self.found = False

    def visit_return_stmt(self, o: ReturnStmt) -> None:
        if o.expr is not None and isinstance(o.expr, YieldExpr):
            self.found = True


def has_yield_return(node: Node) -> bool:
    """Check if the subtree contains `return yield <expr>`."""
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_has_yield_return(_serialize_ast_node(node))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = YieldReturnSeeker()
    node.accept(v)
    return v.found


class ComplexSliceSeeker(TraverserVisitor):
    def __init__(self) -> None:
        super().__init__()
        self.found = False

    def visit_slice_expr(self, o: SliceExpr) -> None:
        if o.stride is not None:
            self.found = True


def has_complex_slice(node: Node) -> bool:
    """Check if the subtree contains a slice with a stride (step)."""
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_has_complex_slice(_serialize_ast_node(node))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = ComplexSliceSeeker()
    node.accept(v)
    return v.found


class NonExtensionHandlerCollector(TraverserVisitor):
    def __init__(self) -> None:
        super().__init__()
        self.funcs: list[FuncDef] = []

    def visit_class_def(self, o: ClassDef) -> None:
        for d in o.defs.body:
            if isinstance(d, FuncDef) and not d.is_decorated:
                self.funcs.append(d)


def find_non_extension_handlers(node: Node) -> list[FuncDef]:
    """Find methods (FuncDef) not wrapped in a Decorator."""
    if _TRAVERSER_HAS_KERNEL:
        try:
            if _rust_count_non_extension_handlers(
                _serialize_ast_node(node)
            ) == 0:
                return []
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = NonExtensionHandlerCollector()
    node.accept(v)
    return v.funcs


class GlobalDeclSeeker(TraverserVisitor):
    def __init__(self) -> None:
        super().__init__()
        self.found = False

    def visit_global_decl(self, o: GlobalDecl) -> None:
        self.found = True


def is_global_expr(node: Node) -> bool:
    """Check if the subtree contains a `global` declaration."""
    if _TRAVERSER_HAS_KERNEL:
        try:
            return _rust_is_global_expr(_serialize_ast_node(node))
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = GlobalDeclSeeker()
    node.accept(v)
    return v.found


class NonLiteralHandlerCollector(TraverserVisitor):
    def __init__(self) -> None:
        super().__init__()
        self.funcs: list[FuncDef] = []

    def visit_class_def(self, o: ClassDef) -> None:
        for d in o.defs.body:
            if isinstance(d, FuncDef) and not _is_literal_handler(d):
                self.funcs.append(d)


def _is_literal_handler(func: FuncDef) -> bool:
    """A literal handler has only literal expr stmts or pass in its body."""
    for stmt in func.body.body:
        if isinstance(stmt, PassStmt):
            continue
        if isinstance(stmt, ExpressionStmt):
            expr = stmt.expr
            if isinstance(
                expr, (IntExpr, StrExpr, FloatExpr, BytesExpr, ComplexExpr, EllipsisExpr)
            ):
                continue
        return False
    return True


def find_non_literal_handlers(node: Node) -> list[FuncDef]:
    """Find methods whose body is not all-literal expressions."""
    if _TRAVERSER_HAS_KERNEL:
        try:
            if _rust_count_non_literal_handlers(
                _serialize_ast_node(node)
            ) == 0:
                return []
        except (AssertionError, NotImplementedError, RecursionError):
            pass
    v = NonLiteralHandlerCollector()
    node.accept(v)
    return v.funcs
