from __future__ import annotations

import re

from librt.internal import ReadBuffer

from mypy import errorcodes as codes
from mypy.cache import read_int
from mypy.errors import Errors
from mypy import message_registry
from mypy.nodes import (
    AssertStmt,
    Block,
    ClassDef,
    FileRawData,
    IfStmt,
    ImportBase,
    MypyFile,
    ParseError,
    Statement,
)
from mypy.options import Options
from mypy.reachability import (
    ALWAYS_FALSE,
    ALWAYS_TRUE,
    MYPY_FALSE,
    MYPY_TRUE,
    assert_will_always_fail,
    infer_condition_value,
)
from mypy.traverser import TraverserVisitor


def parse(
    source: str | bytes | None,
    fnam: str,
    module: str | None,
    errors: Errors,
    options: Options,
    eager: bool = False,
) -> MypyFile:
    """Parse a source file, without doing any semantic analysis.

    Return the parse tree, use the errors object to report parse errors.
    The python_version (major, minor) option determines the Python syntax variant.

    New parser returns empty tree with serialized data. To get the full tree and
    the parse errors, use eager=True.

    `source` must not be `None` if the old parser is used. The new parser will read and
    parse contents from path `fnam` if `source` is `None`.
    """
    if options.native_parser:
        import mypy.nativeparse

        ignore_errors = options.ignore_errors or fnam in errors.ignored_files
        # If errors are ignored, we can drop many function bodies to speed up type checking.
        strip_function_bodies = ignore_errors and not options.preserve_asts
        tree, _, _ = mypy.nativeparse.native_parse(
            fnam, options, source, skip_function_bodies=strip_function_bodies
        )
        # Set is_stub based on file extension
        tree.is_stub = fnam.endswith(".pyi")
        # Note: tree.imports is populated directly by load_from_raw() with deserialized
        # import metadata, so we don't need to collect imports via AST traversal
        if eager and tree.raw_data is not None:
            tree = load_from_raw(fnam, module, tree.raw_data, errors, options)
        return tree

    if source is None:
        raise ValueError("Source cannot be `None` when using the old parser")
    if options.transform_source is not None:
        source = options.transform_source(source)
    import mypy.fastparse

    return mypy.fastparse.parse(source, fnam=fnam, module=module, errors=errors, options=options)


def load_from_raw(
    fnam: str,
    module: str | None,
    raw_data: FileRawData,
    errors: Errors,
    options: Options,
    imports_only: bool = False,
) -> MypyFile:
    """Load AST from parsed binary data and report stored errors.

    If imports_only is true, only deserialize imports and return a mostly
    empty AST.
    """
    from mypy.nativeparse import State, deserialize_imports, read_statements

    state = State(options, is_stub=fnam.endswith(".pyi"))
    if imports_only:
        defs: list[Statement] = []
    else:
        data = ReadBuffer(raw_data.defs)
        n = read_int(data)
        defs = read_statements(state, data, n)
    ignored_lines = dict(raw_data.ignored_lines)
    module_ignore_error: tuple[int, list[str]] | None = None
    ignore_whole_module = False
    if not imports_only and defs and ignored_lines:
        first_ignore_line = min(ignored_lines)
        if first_ignore_line < first_statement_line(defs[0]):
            ignore_whole_module = True
            ignored_codes = ignored_lines[first_ignore_line]
            if ignored_codes:
                module_ignore_error = (first_ignore_line, ignored_codes)
            ignored_lines = {}
            block = Block(defs, is_unreachable=True)
            block.line = defs[0].line
            block.column = defs[0].column
            block.end_line = defs[-1].end_line
            block.end_column = defs[-1].end_column
            defs = [block]
    imports = deserialize_imports(raw_data.imports, dependency_discovery=True)
    skipped_lines: set[int] = set()
    if ignore_whole_module:
        imports = []
    elif not imports_only:
        defs, imports, skipped_lines = truncate_after_failing_toplevel_assert(defs, imports, options)
        skipped_lines.update(collect_skipped_lines(defs, options))

    tree = MypyFile(defs, imports)
    tree.path = fnam
    tree.ignored_lines = ignored_lines
    tree.skipped_lines = skipped_lines
    tree.is_partial_stub_package = raw_data.is_partial_stub_package
    tree.uses_template_strings = raw_data.uses_template_strings
    tree.is_stub = fnam.endswith(".pyi")
    if module is not None:
        tree._fullname = module

    # Report parse errors, this replicates the logic in parse().
    all_errors = raw_data.raw_errors + state.errors
    errors.set_file(fnam, module, options=options)
    if module_ignore_error is not None:
        line, ignored_codes = module_ignore_error
        message = message_registry.TYPE_IGNORE_WITH_ERRCODE_ON_MODULE.format(
            ", ".join(ignored_codes)
        )
        errors.report(line, 0, message.value, blocker=False, code=message.code)
    for error in all_errors:
        # Note we never raise in this function, so it should not be called in coordinator.
        report_parse_error(error, errors)
    if imports_only:
        # Preserve raw data when only de-serializing imports, it will be sent to
        # the parallel workers.
        tree.raw_data = raw_data
    return tree


def first_statement_line(stmt: Statement) -> int:
    if isinstance(stmt, ClassDef) and stmt.decorators:
        return min(decorator.line for decorator in stmt.decorators)
    return stmt.line


def truncate_after_failing_toplevel_assert(
    defs: list[Statement], imports: list[ImportBase], options: Options
) -> tuple[list[Statement], list[ImportBase], set[int]]:
    for index, stmt in enumerate(defs):
        if isinstance(stmt, AssertStmt) and assert_will_always_fail(stmt, options):
            if index == len(defs) - 1:
                return defs, imports, set()
            next_def = defs[index + 1]
            last_def = defs[-1]
            skipped_lines: set[int] = set()
            if last_def.end_line is not None:
                skipped_lines = set(range(next_def.line, last_def.end_line + 1))
            imports = [
                import_node
                for import_node in imports
                if (import_node.line, import_node.column) <= (stmt.line, stmt.column)
            ]
            return defs[: index + 1], imports, skipped_lines
    return defs, imports, set()


def collect_skipped_lines(defs: list[Statement], options: Options) -> set[int]:
    visitor = SkippedLinesVisitor(options)
    for stmt in defs:
        stmt.accept(visitor)
    return visitor.skipped_lines


class SkippedLinesVisitor(TraverserVisitor):
    def __init__(self, options: Options) -> None:
        self.options = options
        self.skipped_lines: set[int] = set()

    def visit_if_stmt(self, stmt: IfStmt) -> None:
        remaining_reachable = True
        for index, expr in enumerate(stmt.expr):
            if not remaining_reachable:
                self.add_block_lines(stmt.body[index])
                continue
            result = infer_condition_value(expr, self.options)
            if result in (ALWAYS_FALSE, MYPY_FALSE):
                self.add_block_lines(stmt.body[index])
            elif result in (ALWAYS_TRUE, MYPY_TRUE):
                remaining_reachable = False
                for body in stmt.body[index + 1 :]:
                    self.add_block_lines(body)
                if stmt.else_body is not None:
                    self.add_block_lines(stmt.else_body)
        for expr in stmt.expr:
            expr.accept(self)
        for block in stmt.body:
            block.accept(self)
        if stmt.else_body is not None:
            stmt.else_body.accept(self)

    def visit_block(self, block: Block) -> None:
        if block.is_unreachable:
            self.add_block_lines(block)
            return
        super().visit_block(block)

    def add_block_lines(self, block: Block) -> None:
        if block.end_line is not None:
            self.skipped_lines.update(range(block.line, block.end_line + 1))


def report_parse_error(error: ParseError, errors: Errors) -> None:
    message = error["message"]
    # Standardize error message by capitalizing the first word
    message = re.sub(r"^(\s*\w)", lambda m: m.group(1).upper(), message)
    # Respect blocker status from error, default to True for syntax errors
    is_blocker = error.get("blocker", True)
    error_code = error.get("code")
    if error_code is None:
        error_code = codes.SYNTAX
    else:
        # Fallback to [syntax] for backwards compatibility.
        error_code = codes.error_codes.get(error_code) or codes.SYNTAX
    errors.report(error["line"], error["column"], message, blocker=is_blocker, code=error_code)
