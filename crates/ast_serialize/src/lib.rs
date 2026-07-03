use pyo3::exceptions::{PyNotImplementedError, PyUnicodeDecodeError};
use pyo3::prelude::*;
use ruff_python_ast::{self as ast, token::TokenKind, AnyParameterRef, ArgOrKeyword, PySourceType};
#[cfg(test)]
use ruff_python_parser::parse_module;
use ruff_python_parser::{parse_expression, parse_unchecked_source};
use ruff_text_size::Ranged;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;

const LITERAL_NONE: u8 = 2;
const LITERAL_INT: u8 = 3;
const LITERAL_STR: u8 = 4;
const LITERAL_FLOAT: u8 = 6;
const LIST_GEN: u8 = 20;
const LIST_INT: u8 = 21;
const DICT_STR_GEN: u8 = 30;
const DECORATOR: u8 = 53;
const CLASS_DEF: u8 = 60;
const LOCATION: u8 = 152;
const END_TAG: u8 = 255;

const EXPR_STMT: u8 = 160;
const CALL_EXPR: u8 = 161;
const NAME_EXPR: u8 = 162;
const STR_EXPR: u8 = 163;
const IMPORT: u8 = 164;
const MEMBER_EXPR: u8 = 165;
const OP_EXPR: u8 = 166;
const INT_EXPR: u8 = 167;
const IF_STMT: u8 = 168;
const BLOCK: u8 = 171;
const ASSIGNMENT_STMT: u8 = 169;
const TUPLE_EXPR: u8 = 170;
const INDEX_EXPR: u8 = 172;
const LIST_EXPR: u8 = 173;
const SET_EXPR: u8 = 174;
const RETURN_STMT: u8 = 175;
const WHILE_STMT: u8 = 176;
const COMPARISON_EXPR: u8 = 177;
const BOOL_OP_EXPR: u8 = 178;
const FUNC_DEF_STMT: u8 = 179;
const PASS_STMT: u8 = 180;
const FLOAT_EXPR: u8 = 181;
const DICT_EXPR: u8 = 183;
const UNARY_EXPR: u8 = 182;
const COMPLEX_EXPR: u8 = 184;
const SLICE_EXPR: u8 = 185;
const TEMP_NODE: u8 = 186;
const RAISE_STMT: u8 = 187;
const BREAK_STMT: u8 = 188;
const CONTINUE_STMT: u8 = 189;
const GENERATOR_EXPR: u8 = 190;
const YIELD_EXPR: u8 = 191;
const YIELD_FROM_EXPR: u8 = 192;
const LIST_COMPREHENSION: u8 = 193;
const SET_COMPREHENSION: u8 = 194;
const DICT_COMPREHENSION: u8 = 195;
const IMPORT_FROM: u8 = 196;
const ASSERT_STMT: u8 = 197;
const FOR_STMT: u8 = 198;
const WITH_STMT: u8 = 199;
const TRY_STMT: u8 = 201;
const ELLIPSIS_EXPR: u8 = 202;
const CONDITIONAL_EXPR: u8 = 203;
const DEL_STMT: u8 = 204;
const FSTRING_EXPR: u8 = 205;
const FSTRING_INTERPOLATION: u8 = 206;
const OPERATOR_ASSIGNMENT_STMT: u8 = 200;
const LAMBDA_EXPR: u8 = 207;
const ASSIGNMENT_EXPR: u8 = 208;
const STAR_EXPR: u8 = 209;
const BYTES_EXPR: u8 = 210;
const GLOBAL_DECL: u8 = 211;
const NONLOCAL_DECL: u8 = 212;
const AWAIT_EXPR: u8 = 213;
const BIG_INT_EXPR: u8 = 214;
const IMPORT_ALL: u8 = 215;
const MATCH_STMT: u8 = 216;
const AS_PATTERN: u8 = 217;
const OR_PATTERN: u8 = 218;
const VALUE_PATTERN: u8 = 219;
const SINGLETON_PATTERN: u8 = 220;
const SEQUENCE_PATTERN: u8 = 221;
const STARRED_PATTERN: u8 = 222;
const MAPPING_PATTERN: u8 = 223;
const CLASS_PATTERN: u8 = 224;
const TYPE_ALIAS_STMT: u8 = 225;
const IMPORT_METADATA: u8 = 226;
const IMPORTFROM_METADATA: u8 = 227;
const IMPORTALL_METADATA: u8 = 228;

const UNBOUND_TYPE: u8 = 104;
const UNPACK_TYPE: u8 = 105;
const TUPLE_TYPE: u8 = 112;
const TYPED_DICT_TYPE: u8 = 113;
const UNION_TYPE: u8 = 115;
const LIST_TYPE: u8 = 118;
const ELLIPSIS_TYPE: u8 = 119;
const RAW_EXPRESSION_TYPE: u8 = 120;
const CALL_TYPE: u8 = 121;

const ARG_POS: i64 = 0;
const ARG_OPT: i64 = 1;
const ARG_STAR: i64 = 2;
const ARG_NAMED: i64 = 3;
const ARG_STAR2: i64 = 4;
const ARG_NAMED_OPT: i64 = 5;

const IMPORT_FLAG_TOP_LEVEL: i64 = 0x01;
const IMPORT_FLAG_MYPY_ONLY: i64 = 0x04;

const TYPE_VAR_KIND: i64 = 0;
const PARAM_SPEC_KIND: i64 = 1;
const TYPE_VAR_TUPLE_KIND: i64 = 2;

#[derive(Debug, Clone)]
struct SourceLocation {
    line: i64,
    column: i64,
    end_line: i64,
    end_column: i64,
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn tag(&mut self, tag: u8) {
        self.bytes.push(tag);
    }

    fn bare_int(&mut self, value: i64) {
        const MIN_ONE_BYTE_INT: i64 = -10;
        const MAX_ONE_BYTE_INT: i64 = 117;
        const MIN_TWO_BYTES_INT: i64 = -100;
        const MAX_TWO_BYTES_INT: i64 = 16_283;
        const MIN_FOUR_BYTES_INT: i64 = -10_000;
        const MAX_FOUR_BYTES_INT: i64 = 536_860_911;
        const TWO_BYTES_INT_BIT: u16 = 1;
        const FOUR_BYTES_INT_TRAILER: u32 = 3;
        const LONG_INT_TRAILER: u8 = 15;

        if (MIN_ONE_BYTE_INT..=MAX_ONE_BYTE_INT).contains(&value) {
            self.bytes.push(((value - MIN_ONE_BYTE_INT) << 1) as u8);
        } else if (MIN_TWO_BYTES_INT..=MAX_TWO_BYTES_INT).contains(&value) {
            let encoded = (((value - MIN_TWO_BYTES_INT) as u16) << 2) | TWO_BYTES_INT_BIT;
            self.bytes.extend_from_slice(&encoded.to_le_bytes());
        } else if (MIN_FOUR_BYTES_INT..=MAX_FOUR_BYTES_INT).contains(&value) {
            let encoded = (((value - MIN_FOUR_BYTES_INT) as u32) << 3) | FOUR_BYTES_INT_TRAILER;
            self.bytes.extend_from_slice(&encoded.to_le_bytes());
        } else {
            self.bytes.push(LONG_INT_TRAILER);
            let negative = value < 0;
            let mut abs_bytes = value.unsigned_abs().to_le_bytes().to_vec();
            while abs_bytes.last() == Some(&0) {
                abs_bytes.pop();
            }
            let encoded_size = ((abs_bytes.len() as i64) << 1) | i64::from(negative);
            self.bare_int(encoded_size);
            self.bytes.extend_from_slice(&abs_bytes);
        }
    }

    fn int(&mut self, value: i64) {
        self.tag(LITERAL_INT);
        self.bare_int(value);
    }

    fn float(&mut self, value: f64) {
        self.tag(LITERAL_FLOAT);
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bool(&mut self, value: bool) {
        self.bytes.push(if value { 1 } else { 0 });
    }

    fn string(&mut self, value: &str) {
        self.tag(LITERAL_STR);
        self.bare_int(value.len() as i64);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn none(&mut self) {
        self.tag(LITERAL_NONE);
    }

    fn loc(&mut self, loc: &SourceLocation) {
        self.tag(LOCATION);
        self.bare_int(loc.line);
        self.bare_int(loc.column);
        self.bare_int(loc.end_line - loc.line);
        self.bare_int(loc.end_column - loc.column);
    }

    fn int_list(&mut self, values: &[i64]) {
        self.tag(LIST_INT);
        self.bare_int(values.len() as i64);
        for value in values {
            self.bare_int(*value);
        }
    }

    fn opt_str_list(&mut self, values: &[Option<String>]) {
        self.tag(LIST_GEN);
        self.bare_int(values.len() as i64);
        for value in values {
            match value {
                Some(value) => self.string(value),
                None => self.none(),
            }
        }
    }

    fn expr_list(&mut self, len: usize) {
        self.tag(LIST_GEN);
        self.bare_int(len as i64);
    }
}

struct Serializer<'a> {
    writer: Writer,
    imports: ImportCollector,
    type_comments: HashMap<i64, String>,
    line_starts: Vec<usize>,
    source: &'a str,
    python_version: (i64, i64),
    skip_function_bodies: bool,
    class_depth: usize,
}

impl<'a> Serializer<'a> {
    fn new(
        source: &'a str,
        python_version: (i64, i64),
        skip_function_bodies: bool,
        type_comments: HashMap<i64, String>,
    ) -> Self {
        Self {
            writer: Writer::default(),
            imports: ImportCollector::default(),
            type_comments,
            line_starts: line_starts(source),
            source,
            python_version,
            skip_function_bodies,
            class_depth: 0,
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.writer.into_bytes()
    }

    fn loc<T: Ranged>(&self, node: &T) -> SourceLocation {
        let range = node.range();
        let (line, column) = self.position(range.start().to_usize());
        let (end_line, end_column) = self.position(range.end().to_usize());
        SourceLocation {
            line,
            column,
            end_line,
            end_column,
        }
    }

    fn position(&self, offset: usize) -> (i64, i64) {
        let line_index = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let line_start = self.line_starts[line_index];
        let column = self.source[line_start..offset].chars().count();
        ((line_index + 1) as i64, column as i64)
    }
}

#[derive(Clone)]
struct ImportMetadata {
    tag: u8,
    module: String,
    relative: i64,
    asname: Option<String>,
    names: Vec<(String, Option<String>)>,
    loc: SourceLocation,
    flags: i64,
}

#[derive(Default)]
struct ImportCollector {
    imports: Vec<ImportMetadata>,
    function_depth: usize,
    unreachable_depth: usize,
    mypy_only_depth: usize,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum BranchMode {
    Normal,
    MypyOnly,
    Unreachable,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum ConditionValue {
    AlwaysTrue,
    AlwaysFalse,
    MypyTrue,
    MypyFalse,
    Unknown,
}

impl ImportCollector {
    fn push(&mut self, import: ImportMetadata) {
        if self.unreachable_depth == 0 {
            self.imports.push(import);
        }
    }

    fn flags(&self) -> i64 {
        let mut flags = if self.function_depth == 0 {
            IMPORT_FLAG_TOP_LEVEL
        } else {
            0
        };
        if self.mypy_only_depth > 0 {
            flags |= IMPORT_FLAG_MYPY_ONLY;
        }
        flags
    }

    fn enter_function(&mut self) {
        self.function_depth += 1;
    }

    fn leave_function(&mut self) {
        self.function_depth -= 1;
    }

    fn enter_unreachable(&mut self) {
        self.unreachable_depth += 1;
    }

    fn leave_unreachable(&mut self) {
        self.unreachable_depth -= 1;
    }

    fn enter_mypy_only(&mut self) {
        self.mypy_only_depth += 1;
    }

    fn leave_mypy_only(&mut self) {
        self.mypy_only_depth -= 1;
    }
}

#[pyfunction]
#[pyo3(signature = (
    fnam,
    source = None,
    skip_function_bodies = false,
    python_version = None,
    platform = None,
    always_true = None,
    always_false = None,
    cache_version = 0
))]
fn parse(
    py: Python<'_>,
    fnam: &str,
    source: Option<&PyAny>,
    skip_function_bodies: bool,
    python_version: Option<(u8, u8)>,
    platform: Option<String>,
    always_true: Option<Vec<String>>,
    always_false: Option<Vec<String>>,
    cache_version: i64,
) -> PyResult<PyObject> {
    let _ = (platform, always_true, always_false, cache_version);
    let source = read_source(py, source, fnam)?;
    let parsed = parse_unchecked_source(&source, PySourceType::Python);
    let errors = parse_errors_to_py(py, &source, parsed.errors())?;
    let (type_ignores, type_comments) = collect_comment_directives(&source, parsed.tokens());
    let module = parsed.into_syntax();
    let python_version = python_version.unwrap_or((3, 10));
    let python_version = (i64::from(python_version.0), i64::from(python_version.1));
    let (ast_bytes, imports) = serialize_suite(
        &module.body,
        &source,
        python_version,
        skip_function_bodies,
        type_comments,
    )?;
    let import_bytes = serialize_import_metadata(&imports);
    let data = pyo3::types::PyDict::new(py);
    data.set_item("is_partial_package", false)?;
    data.set_item("uses_template_strings", false)?;
    data.set_item("mypy_ignores", type_ignores.clone())?;
    data.set_item("source_hash", source_hash(&source))?;
    data.set_item("mypy_comments", Vec::<(i64, String)>::new())?;
    Ok((
        pyo3::types::PyBytes::new(py, &ast_bytes),
        errors,
        type_ignores,
        pyo3::types::PyBytes::new(py, &import_bytes),
        data,
    )
        .into_py(py))
}

fn read_source(py: Python<'_>, source: Option<&PyAny>, fnam: &str) -> PyResult<String> {
    if let Some(source) = source {
        if let Ok(text) = source.extract::<String>() {
            return Ok(text);
        }
        if let Ok(bytes) = source.extract::<&[u8]>() {
            return match std::str::from_utf8(bytes) {
                Ok(text) => Ok(text.to_owned()),
                Err(err) => Err(PyErr::from_value(PyUnicodeDecodeError::new_utf8(
                    py, bytes, err,
                )?)),
            };
        }
    }
    fs::read_to_string(fnam).map_err(|err| {
        pyo3::exceptions::PyOSError::new_err(format!("failed to read {fnam}: {err}"))
    })
}

fn to_parse_error(error: ruff_python_parser::ParseError) -> PyErr {
    pyo3::exceptions::PySyntaxError::new_err(error.to_string())
}

fn parse_errors_to_py(
    py: Python<'_>,
    source: &str,
    errors: &[ruff_python_parser::ParseError],
) -> PyResult<Vec<PyObject>> {
    let line_starts = line_starts(source);
    let mut result = Vec::with_capacity(errors.len());
    for error in errors {
        let offset = error.location.start().to_usize();
        let line_index = line_starts.partition_point(|start| *start <= offset) - 1;
        let line_start = line_starts[line_index];
        let column = source[line_start..offset].chars().count() + 1;

        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("line", line_index + 1)?;
        dict.set_item("column", column)?;
        dict.set_item("message", error.error.to_string())?;
        dict.set_item("blocker", true)?;
        dict.set_item("code", "syntax")?;
        result.push(dict.into_py(py));
    }
    Ok(result)
}

fn serialize_suite(
    suite: &ast::Suite,
    source: &str,
    python_version: (i64, i64),
    skip_function_bodies: bool,
    type_comments: HashMap<i64, String>,
) -> PyResult<(Vec<u8>, Vec<ImportMetadata>)> {
    let mut serializer =
        Serializer::new(source, python_version, skip_function_bodies, type_comments);
    serializer.writer.int(suite.len() as i64);
    for statement in suite {
        serialize_stmt(&mut serializer, statement)?;
    }
    let imports = serializer.imports.imports.clone();
    Ok((serializer.into_bytes(), imports))
}

fn serialize_import_metadata(imports: &[ImportMetadata]) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.tag(LIST_GEN);
    writer.bare_int(imports.len() as i64);
    for import in imports {
        writer.tag(import.tag);
        match import.tag {
            IMPORT_METADATA => {
                writer.string(&import.module);
                writer.int(import.relative);
                write_optional_string(&mut writer, import.asname.as_deref());
            }
            IMPORTFROM_METADATA => {
                writer.string(&import.module);
                writer.int(import.relative);
                writer.tag(LIST_GEN);
                writer.bare_int(import.names.len() as i64);
                for (name, asname) in &import.names {
                    writer.string(name);
                    write_optional_string(&mut writer, asname.as_deref());
                }
            }
            IMPORTALL_METADATA => {
                writer.string(&import.module);
                writer.int(import.relative);
            }
            _ => unreachable!("unexpected import metadata tag"),
        }
        write_import_metadata_tail(&mut writer, &import.loc, import.flags);
    }
    writer.into_bytes()
}

fn serialize_stmt(serializer: &mut Serializer<'_>, statement: &ast::Stmt) -> PyResult<()> {
    match statement {
        ast::Stmt::Expr(expr) => {
            serializer.writer.tag(EXPR_STMT);
            serialize_expr(serializer, &expr.value)?;
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Stmt::Assign(assign) => {
            let loc = serializer.loc(statement);
            serializer.writer.tag(ASSIGNMENT_STMT);
            serializer.writer.expr_list(assign.targets.len());
            for target in &assign.targets {
                serialize_lvalue(serializer, target)?;
            }
            serialize_expr(serializer, &assign.value)?;
            if let Some(type_comment) = serializer.type_comments.get(&loc.end_line).cloned() {
                let parsed_type = parse_expression(&type_comment)
                    .map_err(to_parse_error)?
                    .into_expr();
                serializer.writer.bool(true);
                serialize_type(serializer, &parsed_type)?;
            } else {
                serializer.writer.bool(false);
            }
            serializer.writer.bool(false);
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Stmt::AnnAssign(assign) => serialize_ann_assign(serializer, assign),
        ast::Stmt::AugAssign(assign) => {
            let loc = serializer.loc(statement);
            serializer.writer.tag(OPERATOR_ASSIGNMENT_STMT);
            serializer.writer.string(operator_string(assign.op));
            serialize_lvalue(serializer, &assign.target)?;
            serialize_expr(serializer, &assign.value)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Stmt::Return(return_stmt) => {
            let loc = serializer.loc(statement);
            serializer.writer.tag(RETURN_STMT);
            serializer.writer.bool(return_stmt.value.is_some());
            if let Some(value) = &return_stmt.value {
                serialize_expr(serializer, value)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Stmt::Raise(raise) => {
            let loc = serializer.loc(statement);
            serializer.writer.tag(RAISE_STMT);
            serializer.writer.bool(raise.exc.is_some());
            if let Some(exc) = &raise.exc {
                serialize_expr(serializer, exc)?;
            }
            serializer.writer.bool(raise.cause.is_some());
            if let Some(cause) = &raise.cause {
                serialize_expr(serializer, cause)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Stmt::Assert(assert_stmt) => {
            let loc = serializer.loc(statement);
            serializer.writer.tag(ASSERT_STMT);
            serialize_expr(serializer, &assert_stmt.test)?;
            serializer.writer.bool(assert_stmt.msg.is_some());
            if let Some(message) = &assert_stmt.msg {
                serialize_expr(serializer, message)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Stmt::Delete(delete) => {
            let loc = serializer.loc(statement);
            serializer.writer.tag(DEL_STMT);
            if delete.targets.len() == 1 {
                serialize_expr(serializer, &delete.targets[0])?;
            } else {
                serializer.writer.tag(TUPLE_EXPR);
                serializer.writer.expr_list(delete.targets.len());
                for target in &delete.targets {
                    serialize_expr(serializer, target)?;
                }
                serializer.writer.loc(&loc);
                serializer.writer.tag(END_TAG);
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Stmt::FunctionDef(function) => {
            if function.decorator_list.is_empty() {
                serialize_function_def(serializer, function)
            } else {
                serialize_decorated_function_def(serializer, function)
            }
        }
        ast::Stmt::ClassDef(class_def) => serialize_class_def(serializer, class_def),
        ast::Stmt::If(if_stmt) => serialize_if_stmt(serializer, if_stmt),
        ast::Stmt::For(for_stmt) => serialize_for_stmt(serializer, for_stmt),
        ast::Stmt::While(while_stmt) => serialize_while_stmt(serializer, while_stmt),
        ast::Stmt::With(with_stmt) => serialize_with_stmt(serializer, with_stmt),
        ast::Stmt::Try(try_stmt) => serialize_try_stmt(serializer, try_stmt),
        ast::Stmt::Match(match_stmt) => serialize_match_stmt(serializer, match_stmt),
        ast::Stmt::TypeAlias(type_alias) => serialize_type_alias_stmt(serializer, type_alias),
        ast::Stmt::Import(import) => serialize_import(serializer, import),
        ast::Stmt::ImportFrom(import) => serialize_import_from(serializer, import),
        ast::Stmt::Global(global) => {
            serialize_name_decl(serializer, statement, GLOBAL_DECL, &global.names)
        }
        ast::Stmt::Nonlocal(nonlocal) => {
            serialize_name_decl(serializer, statement, NONLOCAL_DECL, &nonlocal.names)
        }
        ast::Stmt::Pass(_) => serialize_simple_stmt(serializer, statement, PASS_STMT),
        ast::Stmt::Break(_) => serialize_simple_stmt(serializer, statement, BREAK_STMT),
        ast::Stmt::Continue(_) => serialize_simple_stmt(serializer, statement, CONTINUE_STMT),
        _ => Err(PyNotImplementedError::new_err(format!(
            "mypy in-tree Rust parser does not serialize this statement yet: {statement:?}"
        ))),
    }
}

fn serialize_ann_assign(
    serializer: &mut Serializer<'_>,
    assign: &ast::StmtAnnAssign,
) -> PyResult<()> {
    let loc = serializer.loc(assign);
    serializer.writer.tag(ASSIGNMENT_STMT);
    serializer.writer.expr_list(1);
    serialize_lvalue(serializer, &assign.target)?;
    if let Some(value) = &assign.value {
        serialize_expr(serializer, value)?;
    } else {
        serializer.writer.tag(TEMP_NODE);
        serializer.writer.tag(END_TAG);
    }
    serializer.writer.bool(true);
    serialize_type(serializer, &assign.annotation)?;
    serializer.writer.bool(true);
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_type_alias_stmt(
    serializer: &mut Serializer<'_>,
    type_alias: &ast::StmtTypeAlias,
) -> PyResult<()> {
    let loc = serializer.loc(type_alias);
    serializer.writer.tag(TYPE_ALIAS_STMT);
    serialize_expr(serializer, &type_alias.name)?;
    if let Some(type_params) = &type_alias.type_params {
        serialize_type_params(serializer, type_params)?;
    } else {
        serializer.writer.bare_int(0);
    }
    serialize_expr(serializer, &type_alias.value)?;
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_import(serializer: &mut Serializer<'_>, import: &ast::StmtImport) -> PyResult<()> {
    let loc = serializer.loc(import);
    let flags = serializer.imports.flags();
    serializer.writer.tag(IMPORT);
    serializer.writer.int(import.names.len() as i64);
    for alias in &import.names {
        serializer.writer.string(alias.name.as_str());
        write_optional_string(
            &mut serializer.writer,
            alias.asname.as_ref().map(|name| name.as_str()),
        );
    }
    write_import_metadata_tail(&mut serializer.writer, &loc, flags);
    serializer.writer.tag(END_TAG);

    for alias in &import.names {
        serializer.imports.push(ImportMetadata {
            tag: IMPORT_METADATA,
            module: alias.name.as_str().to_owned(),
            relative: 0,
            asname: alias.asname.as_ref().map(|name| name.as_str().to_owned()),
            names: Vec::new(),
            loc: loc.clone(),
            flags,
        });
    }
    Ok(())
}

fn serialize_import_from(
    serializer: &mut Serializer<'_>,
    import: &ast::StmtImportFrom,
) -> PyResult<()> {
    let loc = serializer.loc(import);
    let flags = serializer.imports.flags();
    let module = import
        .module
        .as_ref()
        .map_or_else(String::new, |module| module.as_str().to_owned());
    let relative = i64::from(import.level);

    if import.names.len() == 1 && import.names[0].name.as_str() == "*" {
        serializer.writer.tag(IMPORT_ALL);
        serializer.writer.string(&module);
        serializer.writer.int(relative);
        write_import_metadata_tail(&mut serializer.writer, &loc, flags);
        serializer.writer.tag(END_TAG);
        serializer.imports.push(ImportMetadata {
            tag: IMPORTALL_METADATA,
            module,
            relative,
            asname: None,
            names: Vec::new(),
            loc,
            flags,
        });
        return Ok(());
    }

    let names = import_alias_names(&import.names);
    serializer.writer.tag(IMPORT_FROM);
    serializer.writer.int(relative);
    serializer.writer.string(&module);
    serializer.writer.int(names.len() as i64);
    for (name, asname) in &names {
        serializer.writer.string(name);
        write_optional_string(&mut serializer.writer, asname.as_deref());
    }
    write_import_metadata_tail(&mut serializer.writer, &loc, flags);
    serializer.writer.tag(END_TAG);

    serializer.imports.push(ImportMetadata {
        tag: IMPORTFROM_METADATA,
        module,
        relative,
        asname: None,
        names,
        loc,
        flags,
    });
    Ok(())
}

fn serialize_expr(serializer: &mut Serializer<'_>, expression: &ast::Expr) -> PyResult<()> {
    match expression {
        ast::Expr::Call(call) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(CALL_EXPR);
            serialize_expr(serializer, &call.func)?;

            let args: Vec<ArgOrKeyword<'_>> = call.arguments.iter_source_order().collect();
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(args.len() as i64);
            for arg in &args {
                serialize_call_arg_value(serializer, arg)?;
            }

            let mut arg_kinds = Vec::with_capacity(args.len());
            let mut arg_names = Vec::with_capacity(args.len());
            for arg in args {
                match arg {
                    ArgOrKeyword::Arg(expr) => {
                        arg_kinds.push(if matches!(expr, ast::Expr::Starred(_)) {
                            ARG_STAR
                        } else {
                            ARG_POS
                        });
                        arg_names.push(None);
                    }
                    ArgOrKeyword::Keyword(keyword) => {
                        if let Some(name) = &keyword.arg {
                            arg_kinds.push(ARG_NAMED);
                            arg_names.push(Some(name.as_str().to_owned()));
                        } else {
                            arg_kinds.push(ARG_STAR2);
                            arg_names.push(None);
                        }
                    }
                }
            }
            serializer.writer.int_list(&arg_kinds);
            serializer.writer.opt_str_list(&arg_names);
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Name(name) => {
            let loc = serializer.loc(expression);
            serialize_name_expr(&mut serializer.writer, name.id.as_str(), &loc);
            Ok(())
        }
        ast::Expr::NoneLiteral(_) => {
            let loc = serializer.loc(expression);
            serialize_name_expr(&mut serializer.writer, "None", &loc);
            Ok(())
        }
        ast::Expr::BooleanLiteral(boolean) => {
            let loc = serializer.loc(expression);
            serialize_name_expr(
                &mut serializer.writer,
                if boolean.value { "True" } else { "False" },
                &loc,
            );
            Ok(())
        }
        ast::Expr::EllipsisLiteral(_) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(ELLIPSIS_EXPR);
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Compare(compare) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(COMPARISON_EXPR);
            serialize_expr(serializer, &compare.left)?;
            serializer.writer.int_list(
                &compare
                    .ops
                    .iter()
                    .map(|op| comparison_index(*op))
                    .collect::<Vec<_>>(),
            );
            serializer.writer.expr_list(compare.comparators.len());
            for comparator in &compare.comparators {
                serialize_expr(serializer, comparator)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::BoolOp(bool_op) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(BOOL_OP_EXPR);
            serializer.writer.int(bool_op_index(bool_op.op));
            serializer.writer.expr_list(bool_op.values.len());
            for value in &bool_op.values {
                serialize_expr(serializer, value)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::UnaryOp(unary_op) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(UNARY_EXPR);
            serializer.writer.int(unary_op_index(unary_op.op));
            serialize_expr(serializer, &unary_op.operand)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Await(await_expr) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(AWAIT_EXPR);
            serialize_expr(serializer, &await_expr.value)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Yield(yield_expr) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(YIELD_EXPR);
            serializer.writer.bool(yield_expr.value.is_some());
            if let Some(value) = &yield_expr.value {
                serialize_expr(serializer, value)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::YieldFrom(yield_from) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(YIELD_FROM_EXPR);
            serialize_expr(serializer, &yield_from.value)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::FString(f_string) => serialize_f_string_expr(serializer, f_string),
        ast::Expr::If(if_expr) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(CONDITIONAL_EXPR);
            serialize_expr(serializer, &if_expr.body)?;
            serialize_expr(serializer, &if_expr.test)?;
            serialize_expr(serializer, &if_expr.orelse)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Named(named) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(ASSIGNMENT_EXPR);
            serialize_expr(serializer, &named.target)?;
            serialize_expr(serializer, &named.value)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Lambda(lambda) => serialize_lambda_expr(serializer, expression, lambda),
        ast::Expr::Starred(starred) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(STAR_EXPR);
            serialize_expr(serializer, &starred.value)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Attribute(attribute) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(MEMBER_EXPR);
            serialize_expr(serializer, &attribute.value)?;
            serializer.writer.string(attribute.attr.as_str());
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::BinOp(bin_op) => {
            serializer.writer.tag(OP_EXPR);
            serializer.writer.int(operator_index(bin_op.op)?);
            serialize_expr(serializer, &bin_op.left)?;
            serialize_expr(serializer, &bin_op.right)?;
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::NumberLiteral(number) => match &number.value {
            ast::Number::Int(value) => {
                let loc = serializer.loc(expression);
                if let Some(value) = value.as_i64() {
                    serializer.writer.tag(INT_EXPR);
                    serializer.writer.int(value);
                } else {
                    serializer.writer.tag(BIG_INT_EXPR);
                    serializer.writer.string(&value.to_string());
                }
                serializer.writer.loc(&loc);
                serializer.writer.tag(END_TAG);
                Ok(())
            }
            ast::Number::Float(value) => {
                let loc = serializer.loc(expression);
                serializer.writer.tag(FLOAT_EXPR);
                serializer.writer.float(*value);
                serializer.writer.loc(&loc);
                serializer.writer.tag(END_TAG);
                Ok(())
            }
            ast::Number::Complex { real, imag } => {
                let loc = serializer.loc(expression);
                serializer.writer.tag(COMPLEX_EXPR);
                serializer.writer.float(*real);
                serializer.writer.float(*imag);
                serializer.writer.loc(&loc);
                serializer.writer.tag(END_TAG);
                Ok(())
            }
        },
        ast::Expr::Tuple(tuple) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(TUPLE_EXPR);
            serializer.writer.expr_list(tuple.elts.len());
            for item in &tuple.elts {
                serialize_expr(serializer, item)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::List(list) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(LIST_EXPR);
            serializer.writer.expr_list(list.elts.len());
            for item in &list.elts {
                serialize_expr(serializer, item)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::ListComp(list_comp) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(LIST_COMPREHENSION);
            serialize_generator_payload(serializer, &list_comp.elt, &list_comp.generators)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Set(set) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(SET_EXPR);
            serializer.writer.expr_list(set.elts.len());
            for item in &set.elts {
                serialize_expr(serializer, item)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::SetComp(set_comp) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(SET_COMPREHENSION);
            serialize_generator_payload(serializer, &set_comp.elt, &set_comp.generators)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Dict(dict) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(DICT_EXPR);
            serializer.writer.expr_list(dict.items.len());
            for item in &dict.items {
                serializer.writer.bool(item.key.is_some());
                if let Some(key) = &item.key {
                    serialize_expr(serializer, key)?;
                }
            }
            serializer.writer.expr_list(dict.items.len());
            for item in &dict.items {
                serialize_expr(serializer, &item.value)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::DictComp(dict_comp) => {
            let loc = serializer.loc(expression);
            let Some(key) = &dict_comp.key else {
                return Err(PyNotImplementedError::new_err(
                    "mypy in-tree Rust parser does not serialize dict unpack comprehensions yet",
                ));
            };
            serializer.writer.tag(DICT_COMPREHENSION);
            serialize_expr(serializer, key)?;
            serialize_expr(serializer, &dict_comp.value)?;
            serialize_comprehension_generators(serializer, &dict_comp.generators)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Generator(generator) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(GENERATOR_EXPR);
            serialize_generator_payload(serializer, &generator.elt, &generator.generators)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::StringLiteral(string) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(STR_EXPR);
            serializer.writer.string(string.value.to_str());
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::BytesLiteral(bytes) => {
            let loc = serializer.loc(expression);
            let value = escaped_bytes(bytes.value.bytes());
            serializer.writer.tag(BYTES_EXPR);
            serializer.writer.string(&value);
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Subscript(subscript) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(INDEX_EXPR);
            serialize_expr(serializer, &subscript.value)?;
            serialize_expr(serializer, &subscript.slice)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Slice(slice) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(SLICE_EXPR);
            serializer.writer.bool(slice.lower.is_some());
            if let Some(lower) = &slice.lower {
                serialize_expr(serializer, lower)?;
            }
            serializer.writer.bool(slice.upper.is_some());
            if let Some(upper) = &slice.upper {
                serialize_expr(serializer, upper)?;
            }
            serializer.writer.bool(slice.step.is_some());
            if let Some(step) = &slice.step {
                serialize_expr(serializer, step)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        _ => Err(PyNotImplementedError::new_err(format!(
            "mypy in-tree Rust parser does not serialize this expression yet: {expression:?}"
        ))),
    }
}

fn serialize_lvalue(serializer: &mut Serializer<'_>, expression: &ast::Expr) -> PyResult<()> {
    match expression {
        ast::Expr::Tuple(tuple) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(TUPLE_EXPR);
            serializer.writer.expr_list(tuple.elts.len());
            for item in &tuple.elts {
                serialize_lvalue(serializer, item)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::List(list) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(TUPLE_EXPR);
            serializer.writer.expr_list(list.elts.len());
            for item in &list.elts {
                serialize_lvalue(serializer, item)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Starred(starred) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(STAR_EXPR);
            serialize_lvalue(serializer, &starred.value)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        _ => serialize_expr(serializer, expression),
    }
}

fn serialize_f_string_expr(
    serializer: &mut Serializer<'_>,
    f_string: &ast::ExprFString,
) -> PyResult<()> {
    let loc = serializer.loc(f_string);
    serializer.writer.tag(FSTRING_EXPR);
    serializer
        .writer
        .int(f_string.value.as_slice().len() as i64);
    for part in f_string.value.as_slice() {
        match part {
            ast::FStringPart::Literal(literal) => {
                let loc = serializer.loc(literal);
                serializer.writer.bool(false);
                serializer.writer.string(literal.as_str());
                serializer.writer.loc(&loc);
            }
            ast::FStringPart::FString(part) => {
                serializer.writer.bool(true);
                serialize_f_string_items(serializer, &part.elements)?;
            }
        }
    }
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_f_string_items(
    serializer: &mut Serializer<'_>,
    elements: &ast::InterpolatedStringElements,
) -> PyResult<()> {
    let extra_debug_literals = elements
        .iter()
        .filter(|element| {
            matches!(
                element,
                ast::InterpolatedStringElement::Interpolation(interpolation)
                    if interpolation.debug_text.is_some()
            )
        })
        .count();
    serializer
        .writer
        .int((elements.len() + extra_debug_literals) as i64);
    for element in elements {
        match element {
            ast::InterpolatedStringElement::Literal(literal) => {
                let loc = serializer.loc(literal);
                serializer.writer.string(&literal.value);
                serializer.writer.loc(&loc);
            }
            ast::InterpolatedStringElement::Interpolation(interpolation) => {
                if let Some(debug_text) = &interpolation.debug_text {
                    let loc = serializer.loc(interpolation);
                    serializer.writer.string(debug_text.as_str());
                    serializer.writer.loc(&loc);
                }
                serialize_f_string_interpolation(serializer, interpolation)?;
            }
        }
    }
    Ok(())
}

fn serialize_f_string_interpolation(
    serializer: &mut Serializer<'_>,
    interpolation: &ast::InterpolatedElement,
) -> PyResult<()> {
    serializer.writer.tag(FSTRING_INTERPOLATION);
    serialize_expr(serializer, &interpolation.expression)?;

    let conversion = if interpolation.debug_text.is_some()
        && interpolation.conversion == ast::ConversionFlag::None
    {
        Some('r')
    } else {
        interpolation.conversion.to_char()
    };
    serializer.writer.bool(conversion.is_some());
    if let Some(conversion) = conversion {
        serializer.writer.string(&format!("!{conversion}"));
    }

    serializer.writer.bool(interpolation.format_spec.is_some());
    if let Some(format_spec) = &interpolation.format_spec {
        serialize_f_string_items(serializer, &format_spec.elements)?;
        let loc = serializer.loc(&**format_spec);
        serializer.writer.loc(&loc);
    }

    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_class_def(
    serializer: &mut Serializer<'_>,
    class_def: &ast::StmtClassDef,
) -> PyResult<()> {
    let full_loc = serializer.loc(class_def);
    let name_loc = serializer.loc(&class_def.name);
    let loc = SourceLocation {
        line: name_loc.line,
        column: (name_loc.column - 6).max(0),
        end_line: full_loc.end_line,
        end_column: full_loc.end_column,
    };
    serializer.writer.tag(CLASS_DEF);
    serializer.writer.string(class_def.name.as_str());
    serializer.class_depth += 1;
    let body_result = serialize_block(serializer, &class_def.body, &loc);
    serializer.class_depth -= 1;
    body_result?;

    let base_expr_count = class_def
        .arguments
        .as_ref()
        .map_or(0, |arguments| arguments.args.len());
    serializer.writer.expr_list(base_expr_count);
    if let Some(arguments) = &class_def.arguments {
        for base_expr in &arguments.args {
            serialize_expr(serializer, base_expr)?;
        }
    }

    serializer.writer.tag(LIST_GEN);
    serializer
        .writer
        .bare_int(class_def.decorator_list.len() as i64);
    for decorator in &class_def.decorator_list {
        serialize_expr(serializer, &decorator.expression)?;
    }

    serializer.writer.bool(class_def.type_params.is_some());
    if let Some(type_params) = &class_def.type_params {
        serialize_type_params(serializer, type_params)?;
    }

    let keyword_count = class_def.arguments.as_ref().map_or(0, |arguments| {
        arguments
            .keywords
            .iter()
            .filter(|keyword| keyword.arg.is_some())
            .count()
    });
    serializer.writer.tag(DICT_STR_GEN);
    serializer.writer.bare_int(keyword_count as i64);
    if let Some(arguments) = &class_def.arguments {
        for keyword in &arguments.keywords {
            let Some(name) = &keyword.arg else {
                continue;
            };
            serializer.writer.string(name.as_str());
            serialize_expr(serializer, &keyword.value)?;
        }
    }

    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_if_stmt(serializer: &mut Serializer<'_>, if_stmt: &ast::StmtIf) -> PyResult<()> {
    let loc = serializer.loc(if_stmt);
    let mut remaining_mode = BranchMode::Normal;
    let condition = evaluate_condition(serializer, &if_stmt.test);
    serializer.writer.tag(IF_STMT);
    serialize_expr(serializer, &if_stmt.test)?;
    let (body_mode, is_closed, next_remaining_mode) =
        branch_modes_for_condition(condition, remaining_mode);
    serialize_block_with_import_mode(serializer, &if_stmt.body, &loc, body_mode)?;
    remaining_mode = next_remaining_mode;
    let mut previous_branch_is_exhaustive = is_closed;

    let elif_count = if_stmt
        .elif_else_clauses
        .iter()
        .filter(|clause| clause.test.is_some())
        .count();
    serializer.writer.int(elif_count as i64);
    let mut else_clause = None;
    for clause in &if_stmt.elif_else_clauses {
        if let Some(test) = &clause.test {
            let clause_loc = serializer.loc(clause);
            let condition = evaluate_condition(serializer, test);
            let (body_mode, is_closed, next_remaining_mode) = if previous_branch_is_exhaustive {
                (BranchMode::Unreachable, true, BranchMode::Unreachable)
            } else {
                branch_modes_for_condition(condition, remaining_mode)
            };
            serialize_expr(serializer, test)?;
            serialize_block_with_import_mode(serializer, &clause.body, &clause_loc, body_mode)?;
            remaining_mode = next_remaining_mode;
            previous_branch_is_exhaustive = previous_branch_is_exhaustive || is_closed;
        } else {
            else_clause = Some(clause);
        }
    }

    serializer.writer.bool(else_clause.is_some());
    if let Some(clause) = else_clause {
        let clause_loc = serializer.loc(clause);
        let mode = if previous_branch_is_exhaustive {
            BranchMode::Unreachable
        } else {
            remaining_mode
        };
        serialize_block_with_import_mode(serializer, &clause.body, &clause_loc, mode)?;
    }

    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn branch_modes_for_condition(
    condition: ConditionValue,
    remaining_mode: BranchMode,
) -> (BranchMode, bool, BranchMode) {
    match condition {
        ConditionValue::AlwaysTrue => (remaining_mode, true, BranchMode::Unreachable),
        ConditionValue::AlwaysFalse => (BranchMode::Unreachable, false, remaining_mode),
        ConditionValue::MypyTrue => (BranchMode::MypyOnly, true, BranchMode::Unreachable),
        ConditionValue::MypyFalse => (BranchMode::Unreachable, false, BranchMode::MypyOnly),
        ConditionValue::Unknown => (remaining_mode, false, remaining_mode),
    }
}

fn serialize_while_stmt(
    serializer: &mut Serializer<'_>,
    while_stmt: &ast::StmtWhile,
) -> PyResult<()> {
    let loc = serializer.loc(while_stmt);
    serializer.writer.tag(WHILE_STMT);
    serialize_expr(serializer, &while_stmt.test)?;
    serialize_block(serializer, &while_stmt.body, &loc)?;
    serialize_optional_block(serializer, &while_stmt.orelse)?;
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_for_stmt(serializer: &mut Serializer<'_>, for_stmt: &ast::StmtFor) -> PyResult<()> {
    let loc = serializer.loc(for_stmt);
    serializer.writer.tag(FOR_STMT);
    serialize_lvalue(serializer, &for_stmt.target)?;
    serialize_expr(serializer, &for_stmt.iter)?;
    serialize_block(serializer, &for_stmt.body, &loc)?;
    serialize_optional_block(serializer, &for_stmt.orelse)?;
    serializer.writer.bool(for_stmt.is_async);
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_with_stmt(serializer: &mut Serializer<'_>, with_stmt: &ast::StmtWith) -> PyResult<()> {
    let loc = serializer.loc(with_stmt);
    serializer.writer.tag(WITH_STMT);
    serializer.writer.int(with_stmt.items.len() as i64);
    for item in &with_stmt.items {
        serialize_expr(serializer, &item.context_expr)?;
        serializer.writer.bool(item.optional_vars.is_some());
        if let Some(target) = &item.optional_vars {
            serialize_lvalue(serializer, target)?;
        }
    }
    serialize_block(serializer, &with_stmt.body, &loc)?;
    serializer.writer.bool(with_stmt.is_async);
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_try_stmt(serializer: &mut Serializer<'_>, try_stmt: &ast::StmtTry) -> PyResult<()> {
    let loc = serializer.loc(try_stmt);
    serializer.writer.tag(TRY_STMT);
    serialize_block(serializer, &try_stmt.body, &loc)?;
    serializer.writer.int(try_stmt.handlers.len() as i64);

    for handler in &try_stmt.handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        serializer.writer.bool(handler.type_.is_some());
        if let Some(exc_type) = &handler.type_ {
            serialize_expr(serializer, exc_type)?;
        }
    }

    for handler in &try_stmt.handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        serializer.writer.bool(handler.name.is_some());
        if let Some(name) = &handler.name {
            serializer.writer.string(name.as_str());
            let name_loc = serializer.loc(name);
            serializer.writer.loc(&name_loc);
        }
    }

    for handler in &try_stmt.handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        let handler_loc = serializer.loc(handler);
        serialize_block(serializer, &handler.body, &handler_loc)?;
    }

    serializer.writer.bool(!try_stmt.orelse.is_empty());
    if !try_stmt.orelse.is_empty() {
        serialize_optional_block(serializer, &try_stmt.orelse)?;
    }

    serializer.writer.bool(!try_stmt.finalbody.is_empty());
    if !try_stmt.finalbody.is_empty() {
        serialize_optional_block(serializer, &try_stmt.finalbody)?;
    }

    serializer.writer.bool(try_stmt.is_star);
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_match_stmt(
    serializer: &mut Serializer<'_>,
    match_stmt: &ast::StmtMatch,
) -> PyResult<()> {
    let loc = serializer.loc(match_stmt);
    serializer.writer.tag(MATCH_STMT);
    serialize_expr(serializer, &match_stmt.subject)?;
    serializer.writer.int(match_stmt.cases.len() as i64);
    for case in &match_stmt.cases {
        serialize_pattern(serializer, &case.pattern)?;
        serializer.writer.bool(case.guard.is_some());
        if let Some(guard) = &case.guard {
            serialize_expr(serializer, guard)?;
        }
        serialize_block(serializer, &case.body, &serializer.loc(case))?;
    }
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_pattern(serializer: &mut Serializer<'_>, pattern: &ast::Pattern) -> PyResult<()> {
    match pattern {
        ast::Pattern::MatchAs(as_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(AS_PATTERN);
            serializer.writer.bool(as_pattern.pattern.is_some());
            if let Some(inner) = &as_pattern.pattern {
                serialize_pattern(serializer, inner)?;
            }
            serializer.writer.bool(as_pattern.name.is_some());
            if let Some(name) = &as_pattern.name {
                let name_loc = serializer.loc(name);
                serializer.writer.string(name.as_str());
                serializer.writer.loc(&name_loc);
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Pattern::MatchOr(or_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(OR_PATTERN);
            serializer.writer.int(or_pattern.patterns.len() as i64);
            for item in &or_pattern.patterns {
                serialize_pattern(serializer, item)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Pattern::MatchValue(value_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(VALUE_PATTERN);
            serialize_expr(serializer, &value_pattern.value)?;
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Pattern::MatchSingleton(singleton_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(SINGLETON_PATTERN);
            match singleton_pattern.value {
                ast::Singleton::None => serializer.writer.none(),
                ast::Singleton::True => serializer.writer.bool(true),
                ast::Singleton::False => serializer.writer.bool(false),
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Pattern::MatchSequence(sequence_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(SEQUENCE_PATTERN);
            serializer
                .writer
                .int(sequence_pattern.patterns.len() as i64);
            for item in &sequence_pattern.patterns {
                serialize_pattern(serializer, item)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Pattern::MatchStar(star_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(STARRED_PATTERN);
            serializer.writer.bool(star_pattern.name.is_some());
            if let Some(name) = &star_pattern.name {
                let name_loc = serializer.loc(name);
                serializer.writer.string(name.as_str());
                serializer.writer.loc(&name_loc);
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Pattern::MatchMapping(mapping_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(MAPPING_PATTERN);
            serializer.writer.int(mapping_pattern.keys.len() as i64);
            for (key, value) in mapping_pattern
                .keys
                .iter()
                .zip(mapping_pattern.patterns.iter())
            {
                serialize_expr(serializer, key)?;
                serialize_pattern(serializer, value)?;
            }
            serializer.writer.bool(mapping_pattern.rest.is_some());
            if let Some(rest) = &mapping_pattern.rest {
                let rest_loc = serializer.loc(rest);
                serializer.writer.string(rest.as_str());
                serializer.writer.loc(&rest_loc);
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Pattern::MatchClass(class_pattern) => {
            let loc = serializer.loc(pattern);
            serializer.writer.tag(CLASS_PATTERN);
            serialize_expr(serializer, &class_pattern.cls)?;
            serializer
                .writer
                .int(class_pattern.arguments.patterns.len() as i64);
            for positional in &class_pattern.arguments.patterns {
                serialize_pattern(serializer, positional)?;
            }
            serializer
                .writer
                .int(class_pattern.arguments.keywords.len() as i64);
            for keyword in &class_pattern.arguments.keywords {
                serializer.writer.string(keyword.attr.as_str());
                serialize_pattern(serializer, &keyword.pattern)?;
            }
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
    }
}

fn serialize_function_def(
    serializer: &mut Serializer<'_>,
    function: &ast::StmtFunctionDef,
) -> PyResult<()> {
    let full_loc = serializer.loc(function);
    let name_loc = serializer.loc(&function.name);
    let loc = SourceLocation {
        line: name_loc.line,
        column: if function.is_async {
            (name_loc.column - 10).max(0)
        } else {
            (name_loc.column - 4).max(0)
        },
        end_line: full_loc.end_line,
        end_column: full_loc.end_column,
    };
    serializer.writer.tag(FUNC_DEF_STMT);
    serializer.writer.string(function.name.as_str());
    let type_comment = function_type_comment(serializer, function, full_loc.line);
    let comment_arg_types = type_comment
        .as_ref()
        .and_then(|comment| comment.arg_types.as_deref());
    serialize_parameters(serializer, &function.parameters, comment_arg_types)?;
    serializer.imports.enter_function();
    let body_result = if serializer.skip_function_bodies
        && !function_body_must_be_preserved(&function.body, serializer.class_depth > 0)
    {
        serialize_stripped_block(serializer, &function.body, &loc)
    } else {
        serialize_block(serializer, &function.body, &loc)
    };
    serializer.imports.leave_function();
    body_result?;
    serializer.writer.bool(function.is_async);
    serializer.writer.bool(function.type_params.is_some());
    if let Some(type_params) = &function.type_params {
        serialize_type_params(serializer, type_params)?;
    }
    let comment_return_type = type_comment
        .as_ref()
        .map(|comment| comment.return_type.as_str());
    serializer
        .writer
        .bool(function.returns.is_some() || comment_return_type.is_some());
    if let Some(return_type) = &function.returns {
        serialize_type(serializer, return_type)?;
    } else if let Some(return_type) = comment_return_type {
        let parsed_type = parse_expression(return_type).map_err(|err| {
            PyNotImplementedError::new_err(format!(
                "mypy in-tree Rust parser does not parse this function return type comment yet: {err}"
            ))
        })?;
        serialize_type_with_loc(serializer, &parsed_type.into_expr(), &loc)?;
    }
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_decorated_function_def(
    serializer: &mut Serializer<'_>,
    function: &ast::StmtFunctionDef,
) -> PyResult<()> {
    let loc = serializer.loc(function);
    serializer.writer.tag(DECORATOR);
    serializer.writer.tag(LIST_GEN);
    serializer
        .writer
        .bare_int(function.decorator_list.len() as i64);
    for decorator in &function.decorator_list {
        serialize_expr(serializer, &decorator.expression)?;
    }
    serializer.writer.int(loc.line);
    serializer.writer.int(loc.column);
    serialize_function_def(serializer, function)?;
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_lambda_expr(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
    lambda: &ast::ExprLambda,
) -> PyResult<()> {
    let loc = serializer.loc(expression);
    serializer.writer.tag(LAMBDA_EXPR);
    if let Some(parameters) = &lambda.parameters {
        serialize_parameters(serializer, parameters, None)?;
    } else {
        serialize_empty_parameters(serializer);
    }
    serialize_lambda_body(serializer, &lambda.body)?;
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

#[derive(Debug)]
struct ParsedFunctionTypeComment {
    arg_types: Option<Vec<Option<String>>>,
    return_type: String,
}

fn function_type_comment(
    serializer: &Serializer<'_>,
    function: &ast::StmtFunctionDef,
    def_line: i64,
) -> Option<ParsedFunctionTypeComment> {
    let first_body_line = function
        .body
        .first()
        .map(|statement| serializer.loc(statement).line)
        .unwrap_or(def_line + 1);
    for line in def_line..first_body_line {
        let Some(comment) = serializer.type_comments.get(&line) else {
            continue;
        };
        let Some(mut parsed) = parse_function_type_comment(comment) else {
            continue;
        };
        if let Some(arg_types) = &mut parsed.arg_types {
            if serializer.class_depth > 0 && arg_types.len() + 1 == function.parameters.len() {
                arg_types.insert(0, None);
            }
        }
        return Some(parsed);
    }
    None
}

fn parse_function_type_comment(comment: &str) -> Option<ParsedFunctionTypeComment> {
    let arrow = find_top_level_arrow(comment)?;
    let args = comment[..arrow].trim();
    let return_type = comment[arrow + 2..].trim();
    if return_type.is_empty() || !args.starts_with('(') || !args.ends_with(')') {
        return None;
    }

    let args = args[1..args.len() - 1].trim();
    let arg_types = if args == "..." {
        None
    } else if args.is_empty() {
        Some(Vec::new())
    } else {
        let mut parsed_args = Vec::new();
        for arg in split_top_level_commas(args) {
            let arg = strip_function_type_comment_arg_prefix(arg.trim());
            if arg.is_empty() {
                return None;
            }
            parsed_args.push(Some(arg.to_owned()));
        }
        Some(parsed_args)
    };

    Some(ParsedFunctionTypeComment {
        arg_types,
        return_type: return_type.to_owned(),
    })
}

fn find_top_level_arrow(value: &str) -> Option<usize> {
    let mut depth = 0_i32;
    let mut quote = None;
    let mut escaped = false;
    let mut previous = None;

    for (index, ch) in value.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            previous = Some(ch);
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '>' if previous == Some('-') && depth == 0 => return Some(index - 1),
            _ => {}
        }
        previous = Some(ch);
    }

    None
}

fn split_top_level_commas(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut quote = None;
    let mut escaped = false;
    let mut start = 0;

    for (index, ch) in value.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim());
    parts
}

fn strip_function_type_comment_arg_prefix(arg: &str) -> &str {
    arg.strip_prefix("**")
        .or_else(|| arg.strip_prefix('*'))
        .unwrap_or(arg)
        .trim()
}

fn comment_annotation(comment_arg_types: Option<&[Option<String>]>, index: usize) -> Option<&str> {
    comment_arg_types
        .and_then(|arg_types| arg_types.get(index))
        .and_then(|annotation| annotation.as_deref())
}

fn serialize_parameters(
    serializer: &mut Serializer<'_>,
    parameters: &ast::Parameters,
    comment_arg_types: Option<&[Option<String>]>,
) -> PyResult<()> {
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(parameters.len() as i64);

    let mut index = 0;
    for parameter in &parameters.posonlyargs {
        let comment_annotation = comment_annotation(comment_arg_types, index);
        serialize_parameter_with_default(
            serializer,
            parameter,
            ARG_POS,
            ARG_OPT,
            true,
            comment_annotation,
        )?;
        index += 1;
    }
    for parameter in &parameters.args {
        let comment_annotation = comment_annotation(comment_arg_types, index);
        serialize_parameter_with_default(
            serializer,
            parameter,
            ARG_POS,
            ARG_OPT,
            false,
            comment_annotation,
        )?;
        index += 1;
    }
    if let Some(parameter) = &parameters.vararg {
        let comment_annotation = comment_annotation(comment_arg_types, index);
        serialize_parameter(
            serializer,
            AnyParameterRef::Variadic(parameter),
            ARG_STAR,
            false,
            comment_annotation,
        )?;
        index += 1;
    }
    for parameter in &parameters.kwonlyargs {
        let comment_annotation = comment_annotation(comment_arg_types, index);
        serialize_parameter_with_default(
            serializer,
            parameter,
            ARG_NAMED,
            ARG_NAMED_OPT,
            false,
            comment_annotation,
        )?;
        index += 1;
    }
    if let Some(parameter) = &parameters.kwarg {
        let comment_annotation = comment_annotation(comment_arg_types, index);
        serialize_parameter(
            serializer,
            AnyParameterRef::Variadic(parameter),
            ARG_STAR2,
            false,
            comment_annotation,
        )?;
    }

    Ok(())
}

fn serialize_empty_parameters(serializer: &mut Serializer<'_>) {
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(0);
}

fn serialize_lambda_body(serializer: &mut Serializer<'_>, body: &ast::Expr) -> PyResult<()> {
    let loc = serializer.loc(body);
    serializer.writer.tag(BLOCK);
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(1);
    serializer.writer.bool(false);
    serializer.writer.tag(RETURN_STMT);
    serializer.writer.bool(true);
    serialize_expr(serializer, body)?;
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_generator_payload(
    serializer: &mut Serializer<'_>,
    elt: &ast::Expr,
    generators: &[ast::Comprehension],
) -> PyResult<()> {
    serialize_expr(serializer, elt)?;
    serialize_comprehension_generators(serializer, generators)
}

fn serialize_comprehension_generators(
    serializer: &mut Serializer<'_>,
    generators: &[ast::Comprehension],
) -> PyResult<()> {
    serializer.writer.int(generators.len() as i64);
    for generator in generators {
        serialize_lvalue(serializer, &generator.target)?;
    }
    for generator in generators {
        serialize_expr(serializer, &generator.iter)?;
    }
    for generator in generators {
        serializer.writer.expr_list(generator.ifs.len());
        for condition in &generator.ifs {
            serialize_expr(serializer, condition)?;
        }
    }
    for generator in generators {
        serializer.writer.bool(generator.is_async);
    }
    Ok(())
}

fn serialize_parameter_with_default(
    serializer: &mut Serializer<'_>,
    parameter: &ast::ParameterWithDefault,
    required_kind: i64,
    optional_kind: i64,
    pos_only: bool,
    comment_annotation: Option<&str>,
) -> PyResult<()> {
    let kind = if parameter.default.is_some() {
        optional_kind
    } else {
        required_kind
    };
    serialize_parameter(
        serializer,
        AnyParameterRef::NonVariadic(parameter),
        kind,
        pos_only,
        comment_annotation,
    )
}

fn serialize_parameter(
    serializer: &mut Serializer<'_>,
    parameter: AnyParameterRef<'_>,
    kind: i64,
    pos_only: bool,
    comment_annotation: Option<&str>,
) -> PyResult<()> {
    let loc = serializer.loc(&parameter);
    let name = parameter.name().as_str();
    let pos_only = pos_only || argument_elide_name(name);
    let inline_type_comment = if comment_annotation.is_none() && parameter.annotation().is_none() {
        serializer
            .type_comments
            .get(&loc.end_line)
            .filter(|comment| parse_function_type_comment(comment).is_none())
            .cloned()
    } else {
        None
    };
    let comment_annotation = comment_annotation.or(inline_type_comment.as_deref());
    serializer.writer.string(name);
    serializer.writer.int(kind);
    serializer
        .writer
        .bool(parameter.annotation().is_some() || comment_annotation.is_some());
    if let Some(annotation) = comment_annotation {
        let parsed_type = parse_expression(annotation).map_err(|err| {
            PyNotImplementedError::new_err(format!(
                "mypy in-tree Rust parser does not parse this parameter type comment yet: {err}"
            ))
        })?;
        serialize_type_with_loc(serializer, &parsed_type.into_expr(), &loc)?;
    } else if let Some(annotation) = parameter.annotation() {
        serialize_type(serializer, annotation)?;
    }
    serializer.writer.bool(parameter.default().is_some());
    if let Some(default) = parameter.default() {
        serialize_expr(serializer, default)?;
    }
    serializer.writer.bool(pos_only);
    serializer.writer.loc(&loc);
    Ok(())
}

fn serialize_block(
    serializer: &mut Serializer<'_>,
    body: &ast::Suite,
    fallback_loc: &SourceLocation,
) -> PyResult<()> {
    serializer.writer.tag(BLOCK);
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(body.len() as i64);
    serializer.writer.bool(false);
    if body.is_empty() {
        serializer.writer.loc(fallback_loc);
    } else {
        for statement in body {
            serialize_stmt(serializer, statement)?;
        }
    }
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_stripped_block(
    serializer: &mut Serializer<'_>,
    body: &ast::Suite,
    fallback_loc: &SourceLocation,
) -> PyResult<()> {
    let loc = body
        .first()
        .map(|statement| serializer.loc(statement))
        .unwrap_or_else(|| fallback_loc.clone());
    serializer.writer.tag(BLOCK);
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(0);
    serializer.writer.bool(false);
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_block_with_import_mode(
    serializer: &mut Serializer<'_>,
    body: &ast::Suite,
    fallback_loc: &SourceLocation,
    mode: BranchMode,
) -> PyResult<()> {
    match mode {
        BranchMode::Normal => serialize_block(serializer, body, fallback_loc),
        BranchMode::MypyOnly => {
            serializer.imports.enter_mypy_only();
            let result = serialize_block(serializer, body, fallback_loc);
            serializer.imports.leave_mypy_only();
            result
        }
        BranchMode::Unreachable => {
            serializer.imports.enter_unreachable();
            let result = serialize_block(serializer, body, fallback_loc);
            serializer.imports.leave_unreachable();
            result
        }
    }
}

fn serialize_optional_block(serializer: &mut Serializer<'_>, body: &ast::Suite) -> PyResult<()> {
    serializer.writer.tag(BLOCK);
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(body.len() as i64);
    serializer.writer.bool(false);
    for statement in body {
        serialize_stmt(serializer, statement)?;
    }
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn function_body_must_be_preserved(body: &ast::Suite, preserve_attribute_defs: bool) -> bool {
    body.iter().any(|statement| {
        statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
    })
}

fn statement_must_be_preserved_in_skipped_body(
    statement: &ast::Stmt,
    preserve_attribute_defs: bool,
) -> bool {
    match statement {
        ast::Stmt::Assign(assign) => {
            preserve_attribute_defs && assign.targets.iter().any(expr_defines_attribute)
        }
        ast::Stmt::AnnAssign(assign) => {
            preserve_attribute_defs && expr_defines_attribute(&assign.target)
        }
        ast::Stmt::AugAssign(assign) => {
            preserve_attribute_defs && expr_defines_attribute(&assign.target)
        }
        ast::Stmt::For(for_stmt) => {
            preserve_attribute_defs && expr_defines_attribute(&for_stmt.target)
                || for_stmt.body.iter().any(|statement| {
                    statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
                })
                || for_stmt.orelse.iter().any(|statement| {
                    statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
                })
        }
        ast::Stmt::With(with_stmt) => {
            preserve_attribute_defs
                && with_stmt.items.iter().any(|item| {
                    item.optional_vars
                        .as_deref()
                        .is_some_and(expr_defines_attribute)
                })
                || with_stmt.body.iter().any(|statement| {
                    statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
                })
        }
        ast::Stmt::If(if_stmt) => {
            if_stmt.body.iter().any(|statement| {
                statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
            }) || if_stmt.elif_else_clauses.iter().any(|clause| {
                clause.body.iter().any(|statement| {
                    statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
                })
            })
        }
        ast::Stmt::While(while_stmt) => {
            while_stmt.body.iter().any(|statement| {
                statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
            }) || while_stmt.orelse.iter().any(|statement| {
                statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
            })
        }
        ast::Stmt::Try(try_stmt) => {
            try_stmt.body.iter().any(|statement| {
                statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
            }) || try_stmt.handlers.iter().any(|handler| {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                handler.body.iter().any(|statement| {
                    statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
                })
            }) || try_stmt.orelse.iter().any(|statement| {
                statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
            }) || try_stmt.finalbody.iter().any(|statement| {
                statement_must_be_preserved_in_skipped_body(statement, preserve_attribute_defs)
            })
        }
        ast::Stmt::Expr(expr) => matches!(&*expr.value, ast::Expr::EllipsisLiteral(_)),
        _ => false,
    }
}

fn expr_defines_attribute(expression: &ast::Expr) -> bool {
    match expression {
        ast::Expr::Attribute(_) => true,
        ast::Expr::Tuple(tuple) => tuple.elts.iter().any(expr_defines_attribute),
        ast::Expr::List(list) => list.elts.iter().any(expr_defines_attribute),
        ast::Expr::Starred(starred) => expr_defines_attribute(&starred.value),
        _ => false,
    }
}

fn serialize_simple_stmt(
    serializer: &mut Serializer<'_>,
    statement: &ast::Stmt,
    tag: u8,
) -> PyResult<()> {
    let loc = serializer.loc(statement);
    serializer.writer.tag(tag);
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_name_decl(
    serializer: &mut Serializer<'_>,
    statement: &ast::Stmt,
    tag: u8,
    names: &[ast::Identifier],
) -> PyResult<()> {
    let loc = serializer.loc(statement);
    serializer.writer.tag(tag);
    serializer.writer.int(names.len() as i64);
    for name in names {
        serializer.writer.string(name.as_str());
    }
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_type_params(
    serializer: &mut Serializer<'_>,
    type_params: &ast::TypeParams,
) -> PyResult<()> {
    serializer
        .writer
        .bare_int(type_params.type_params.len() as i64);
    for type_param in &type_params.type_params {
        serialize_type_param(serializer, type_param)?;
    }
    Ok(())
}

fn serialize_type_param(
    serializer: &mut Serializer<'_>,
    type_param: &ast::TypeParam,
) -> PyResult<()> {
    match type_param {
        ast::TypeParam::TypeVar(type_var) => {
            serializer.writer.int(TYPE_VAR_KIND);
            serializer.writer.string(type_var.name.as_str());
            let constraint_values = type_var
                .bound
                .as_deref()
                .and_then(type_param_constraint_values);
            serializer
                .writer
                .bool(type_var.bound.is_some() && constraint_values.is_none());
            if constraint_values.is_none() {
                if let Some(bound) = &type_var.bound {
                    serialize_type(serializer, bound)?;
                }
            }
            serializer.writer.tag(LIST_GEN);
            if let Some(values) = constraint_values {
                serializer.writer.bare_int(values.len() as i64);
                for value in values {
                    serialize_type(serializer, value)?;
                }
            } else {
                serializer.writer.bare_int(0);
            }
            serializer.writer.bool(type_var.default.is_some());
            if let Some(default) = &type_var.default {
                serialize_type(serializer, default)?;
            }
        }
        ast::TypeParam::ParamSpec(param_spec) => {
            serializer.writer.int(PARAM_SPEC_KIND);
            serializer.writer.string(param_spec.name.as_str());
            serializer.writer.bool(false);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(0);
            serializer.writer.bool(param_spec.default.is_some());
            if let Some(default) = &param_spec.default {
                serialize_type(serializer, default)?;
            }
        }
        ast::TypeParam::TypeVarTuple(type_var_tuple) => {
            serializer.writer.int(TYPE_VAR_TUPLE_KIND);
            serializer.writer.string(type_var_tuple.name.as_str());
            serializer.writer.bool(false);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(0);
            serializer.writer.bool(type_var_tuple.default.is_some());
            if let Some(default) = &type_var_tuple.default {
                serialize_type(serializer, default)?;
            }
        }
    }
    Ok(())
}

fn type_param_constraint_values(bound: &ast::Expr) -> Option<&[ast::Expr]> {
    match bound {
        ast::Expr::Tuple(tuple) => Some(&tuple.elts),
        _ => None,
    }
}

fn serialize_name_expr(writer: &mut Writer, name: &str, loc: &SourceLocation) {
    writer.tag(NAME_EXPR);
    writer.string(name);
    writer.loc(loc);
    writer.tag(END_TAG);
}

fn write_optional_string(writer: &mut Writer, value: Option<&str>) {
    writer.bool(value.is_some());
    if let Some(value) = value {
        writer.string(value);
    }
}

fn write_import_metadata_tail(writer: &mut Writer, loc: &SourceLocation, flags: i64) {
    writer.loc(loc);
    writer.int(flags);
}

fn import_alias_names(aliases: &[ast::Alias]) -> Vec<(String, Option<String>)> {
    aliases
        .iter()
        .map(|alias| {
            (
                alias.name.as_str().to_owned(),
                alias.asname.as_ref().map(|name| name.as_str().to_owned()),
            )
        })
        .collect()
}

fn serialize_type(serializer: &mut Serializer<'_>, expression: &ast::Expr) -> PyResult<()> {
    let loc = serializer.loc(expression);
    serialize_type_with_loc(serializer, expression, &loc)
}

fn serialize_type_with_loc(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
    loc: &SourceLocation,
) -> PyResult<()> {
    match expression {
        ast::Expr::Name(_) | ast::Expr::Attribute(_) => {
            let Some(name) = dotted_name(expression) else {
                return serialize_invalid_raw_expression_type(serializer, loc, None);
            };
            serialize_unbound_type(serializer, &name, &[], loc)
        }
        ast::Expr::NoneLiteral(_) => serialize_unbound_type(serializer, "None", &[], loc),
        ast::Expr::Subscript(subscript) => {
            let Some(name) = dotted_name(&subscript.value) else {
                return serialize_invalid_raw_expression_type(serializer, loc, None);
            };
            let args = type_arg_expressions(&subscript.slice);
            serializer.writer.tag(UNBOUND_TYPE);
            serializer.writer.string(&name);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(args.len() as i64);
            for arg in args {
                serialize_type(serializer, arg)?;
            }
            serializer.writer.bool(false);
            serializer.writer.none();
            serializer.writer.none();
            serializer.writer.loc(loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Call(call) => {
            if is_arg_constructor_call(&call.func) {
                serialize_call_type(serializer, call, loc)
            } else {
                serialize_invalid_raw_expression_type(serializer, loc, None)
            }
        }
        ast::Expr::List(list) => {
            serializer.writer.tag(LIST_TYPE);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(list.elts.len() as i64);
            for item in &list.elts {
                serialize_type(serializer, item)?;
            }
            serializer.writer.loc(loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::EllipsisLiteral(_) => {
            serializer.writer.tag(ELLIPSIS_TYPE);
            serializer.writer.loc(loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Starred(starred) => {
            serializer.writer.tag(UNPACK_TYPE);
            serialize_type(serializer, &starred.value)?;
            serializer.writer.bool(true);
            serializer.writer.loc(loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::BinOp(bin_op) if bin_op.op == ast::Operator::BitOr => {
            let mut items = Vec::new();
            flatten_union_items(&mut items, expression);
            serializer.writer.tag(UNION_TYPE);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(items.len() as i64);
            for item in items {
                serialize_type(serializer, item)?;
            }
            serializer.writer.bool(true);
            serializer.writer.none();
            serializer.writer.none();
            serializer.writer.bool(false);
            serializer.writer.loc(loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::StringLiteral(string) => {
            let text = string.value.to_str();
            serialize_type_string(serializer, text, "builtins.str", loc)
        }
        ast::Expr::BytesLiteral(bytes) => serialize_raw_expression_type(
            serializer,
            "builtins.bytes",
            RawTypeValue::Str(escaped_bytes(bytes.value.bytes())),
            loc,
        ),
        ast::Expr::NumberLiteral(number) => match &number.value {
            ast::Number::Int(value) => {
                if let Some(value) = value.as_i64() {
                    serialize_raw_expression_type(
                        serializer,
                        "builtins.int",
                        RawTypeValue::Int(value),
                        loc,
                    )
                } else {
                    serialize_raw_expression_type(serializer, "typing.Any", RawTypeValue::None, loc)
                }
            }
            _ => serialize_raw_expression_type(serializer, "typing.Any", RawTypeValue::None, loc),
        },
        ast::Expr::UnaryOp(unary)
            if unary.op == ast::UnaryOp::USub
                && matches!(&*unary.operand, ast::Expr::NumberLiteral(_)) =>
        {
            if let ast::Expr::NumberLiteral(number) = &*unary.operand {
                if let ast::Number::Int(value) = &number.value {
                    if let Some(value) = value.as_i64() {
                        return serialize_raw_expression_type(
                            serializer,
                            "builtins.int",
                            RawTypeValue::Int(-value),
                            loc,
                        );
                    }
                }
            }
            serialize_raw_expression_type(serializer, "typing.Any", RawTypeValue::None, loc)
        }
        ast::Expr::UnaryOp(unary)
            if unary.op == ast::UnaryOp::UAdd
                && matches!(&*unary.operand, ast::Expr::NumberLiteral(_)) =>
        {
            if let ast::Expr::NumberLiteral(number) = &*unary.operand {
                if let ast::Number::Int(value) = &number.value {
                    if let Some(value) = value.as_i64() {
                        return serialize_raw_expression_type(
                            serializer,
                            "builtins.int",
                            RawTypeValue::Int(value),
                            loc,
                        );
                    }
                }
            }
            serialize_raw_expression_type(serializer, "typing.Any", RawTypeValue::None, loc)
        }
        ast::Expr::BooleanLiteral(boolean) => serialize_raw_expression_type(
            serializer,
            "builtins.bool",
            RawTypeValue::Bool(boolean.value),
            loc,
        ),
        ast::Expr::Tuple(tuple) => {
            serializer.writer.tag(TUPLE_TYPE);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(tuple.elts.len() as i64);
            for item in &tuple.elts {
                serialize_type(serializer, item)?;
            }
            serializer.writer.bool(true);
            serializer.writer.loc(loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Dict(dict) => serialize_typed_dict_type(serializer, dict, loc),
        ast::Expr::Slice(_) => serialize_invalid_raw_expression_type(
            serializer,
            loc,
            Some("did you mean to use ',' instead of ':' ?"),
        ),
        ast::Expr::BinOp(_)
        | ast::Expr::Compare(_)
        | ast::Expr::If(_)
        | ast::Expr::Lambda(_)
        | ast::Expr::ListComp(_)
        | ast::Expr::Set(_)
        | ast::Expr::UnaryOp(_) => {
            serialize_raw_expression_type(serializer, "typing.Any", RawTypeValue::None, loc)
        }
        _ => Err(PyNotImplementedError::new_err(format!(
            "mypy in-tree Rust parser does not serialize this type annotation yet: {expression:?}"
        ))),
    }
}

fn serialize_typed_dict_type(
    serializer: &mut Serializer<'_>,
    dict: &ast::ExprDict,
    loc: &SourceLocation,
) -> PyResult<()> {
    if dict.items.is_empty() {
        return serialize_invalid_raw_expression_type(serializer, loc, None);
    }

    let mut keys = Vec::with_capacity(dict.items.len());
    for item in &dict.items {
        match &item.key {
            Some(ast::Expr::StringLiteral(key)) => keys.push(Some(key.value.to_str().to_owned())),
            None => keys.push(None),
            _ => return serialize_invalid_raw_expression_type(serializer, loc, None),
        }
    }

    serializer.writer.tag(TYPED_DICT_TYPE);
    serializer.writer.opt_str_list(&keys);
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(dict.items.len() as i64);
    for item in &dict.items {
        serialize_type(serializer, &item.value)?;
    }
    serializer.writer.loc(loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn argument_elide_name(name: &str) -> bool {
    name.starts_with("__") && !name.ends_with("__")
}

fn serialize_type_string(
    serializer: &mut Serializer<'_>,
    text: &str,
    fallback_name: &str,
    loc: &SourceLocation,
) -> PyResult<()> {
    match parse_expression(&format!("({text})")) {
        Ok(parsed) => {
            let expression = parsed.into_expr();
            if serialize_type_with_original_string(
                serializer,
                &expression,
                text,
                fallback_name,
                loc,
            )? {
                Ok(())
            } else {
                serialize_raw_expression_type(
                    serializer,
                    fallback_name,
                    RawTypeValue::Str(text.to_owned()),
                    loc,
                )
            }
        }
        Err(_) => serialize_raw_expression_type(
            serializer,
            fallback_name,
            RawTypeValue::Str(text.to_owned()),
            loc,
        ),
    }
}

fn serialize_type_with_original_string(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
    original_str_expr: &str,
    original_str_fallback: &str,
    loc: &SourceLocation,
) -> PyResult<bool> {
    match expression {
        ast::Expr::Name(_) | ast::Expr::Attribute(_) => {
            let Some(name) = dotted_name(expression) else {
                return Ok(false);
            };
            serialize_unbound_type_with_original_string(
                serializer,
                &name,
                &[],
                false,
                original_str_expr,
                original_str_fallback,
                loc,
            )?;
            Ok(true)
        }
        ast::Expr::Subscript(subscript) => {
            let Some(name) = dotted_name(&subscript.value) else {
                return Ok(false);
            };
            let args = type_arg_expressions(&subscript.slice);
            serialize_unbound_type_with_original_string(
                serializer,
                &name,
                args.as_slice(),
                matches!(&*subscript.slice, ast::Expr::Tuple(tuple) if tuple.elts.is_empty()),
                original_str_expr,
                original_str_fallback,
                loc,
            )?;
            Ok(true)
        }
        ast::Expr::BinOp(bin_op) if bin_op.op == ast::Operator::BitOr => {
            let mut items = Vec::new();
            flatten_union_items(&mut items, expression);
            serializer.writer.tag(UNION_TYPE);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(items.len() as i64);
            for item in items {
                serialize_type(serializer, item)?;
            }
            serializer.writer.bool(true);
            serializer.writer.string(original_str_expr);
            serializer.writer.string(original_str_fallback);
            serializer.writer.bool(false);
            serializer.writer.loc(loc);
            serializer.writer.tag(END_TAG);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn serialize_unbound_type(
    serializer: &mut Serializer<'_>,
    name: &str,
    args: &[ast::Expr],
    loc: &SourceLocation,
) -> PyResult<()> {
    let args: Vec<_> = args.iter().collect();
    serialize_unbound_type_with_originals(serializer, name, &args, false, None, None, loc)
}

fn serialize_unbound_type_with_original_string(
    serializer: &mut Serializer<'_>,
    name: &str,
    args: &[&ast::Expr],
    empty_tuple_index: bool,
    original_str_expr: &str,
    original_str_fallback: &str,
    loc: &SourceLocation,
) -> PyResult<()> {
    serialize_unbound_type_with_originals(
        serializer,
        name,
        args,
        empty_tuple_index,
        Some(original_str_expr),
        Some(original_str_fallback),
        loc,
    )
}

fn serialize_unbound_type_with_originals(
    serializer: &mut Serializer<'_>,
    name: &str,
    args: &[&ast::Expr],
    empty_tuple_index: bool,
    original_str_expr: Option<&str>,
    original_str_fallback: Option<&str>,
    loc: &SourceLocation,
) -> PyResult<()> {
    serializer.writer.tag(UNBOUND_TYPE);
    serializer.writer.string(name);
    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(args.len() as i64);
    for arg in args {
        serialize_type(serializer, arg)?;
    }
    serializer.writer.bool(empty_tuple_index);
    match original_str_expr {
        Some(value) => serializer.writer.string(value),
        None => serializer.writer.none(),
    }
    match original_str_fallback {
        Some(value) => serializer.writer.string(value),
        None => serializer.writer.none(),
    }
    serializer.writer.loc(loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_call_type(
    serializer: &mut Serializer<'_>,
    call: &ast::ExprCall,
    loc: &SourceLocation,
) -> PyResult<()> {
    serializer.writer.tag(CALL_TYPE);
    serialize_type(serializer, &call.func)?;

    serializer.writer.tag(LIST_GEN);
    serializer.writer.bare_int(call.arguments.args.len() as i64);
    for arg in &call.arguments.args {
        serialize_type(serializer, arg)?;
    }

    serializer.writer.tag(LIST_GEN);
    serializer
        .writer
        .bare_int(call.arguments.keywords.len() as i64);
    for keyword in &call.arguments.keywords {
        match &keyword.arg {
            Some(name) => serializer.writer.string(name.as_str()),
            None => serializer.writer.none(),
        }
        serialize_type(serializer, &keyword.value)?;
    }

    serializer.writer.loc(loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn is_arg_constructor_call(func: &ast::Expr) -> bool {
    let Some(name) = dotted_name(func) else {
        return false;
    };
    matches!(
        name.as_str(),
        "Arg"
            | "DefaultArg"
            | "NamedArg"
            | "DefaultNamedArg"
            | "VarArg"
            | "KwArg"
            | "mypy_extensions.Arg"
            | "mypy_extensions.DefaultArg"
            | "mypy_extensions.NamedArg"
            | "mypy_extensions.DefaultNamedArg"
            | "mypy_extensions.VarArg"
            | "mypy_extensions.KwArg"
    )
}

enum RawTypeValue {
    Bool(bool),
    Int(i64),
    Str(String),
    None,
}

fn serialize_raw_expression_type(
    serializer: &mut Serializer<'_>,
    base_type_name: &str,
    value: RawTypeValue,
    loc: &SourceLocation,
) -> PyResult<()> {
    serializer.writer.tag(RAW_EXPRESSION_TYPE);
    serializer.writer.string(base_type_name);
    match value {
        RawTypeValue::Bool(value) => serializer.writer.bool(value),
        RawTypeValue::Int(value) => serializer.writer.int(value),
        RawTypeValue::Str(value) => serializer.writer.string(&value),
        RawTypeValue::None => {
            serializer.writer.none();
            serializer.writer.none();
        }
    }
    serializer.writer.loc(loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_invalid_raw_expression_type(
    serializer: &mut Serializer<'_>,
    loc: &SourceLocation,
    note: Option<&str>,
) -> PyResult<()> {
    serializer.writer.tag(RAW_EXPRESSION_TYPE);
    serializer.writer.string("typing.Any");
    serializer.writer.none();
    match note {
        Some(note) => serializer.writer.string(note),
        None => serializer.writer.none(),
    }
    serializer.writer.loc(loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn type_arg_expressions(expression: &ast::Expr) -> Vec<&ast::Expr> {
    match expression {
        ast::Expr::Tuple(tuple) => tuple.elts.iter().collect(),
        _ => vec![expression],
    }
}

fn flatten_union_items<'a>(items: &mut Vec<&'a ast::Expr>, expression: &'a ast::Expr) {
    match expression {
        ast::Expr::BinOp(bin_op) if bin_op.op == ast::Operator::BitOr => {
            flatten_union_items(items, &bin_op.left);
            flatten_union_items(items, &bin_op.right);
        }
        _ => items.push(expression),
    }
}

fn serialize_call_arg_value(
    serializer: &mut Serializer<'_>,
    arg: &ArgOrKeyword<'_>,
) -> PyResult<()> {
    match arg {
        ArgOrKeyword::Arg(ast::Expr::Starred(starred)) => {
            serialize_expr(serializer, &starred.value)
        }
        ArgOrKeyword::Arg(expr) => serialize_expr(serializer, expr),
        ArgOrKeyword::Keyword(keyword) => serialize_expr(serializer, &keyword.value),
    }
}

fn operator_string(operator: ast::Operator) -> &'static str {
    match operator {
        ast::Operator::Add => "+",
        ast::Operator::Sub => "-",
        ast::Operator::Mult => "*",
        ast::Operator::MatMult => "@",
        ast::Operator::Div => "/",
        ast::Operator::Mod => "%",
        ast::Operator::Pow => "**",
        ast::Operator::LShift => "<<",
        ast::Operator::RShift => ">>",
        ast::Operator::BitOr => "|",
        ast::Operator::BitXor => "^",
        ast::Operator::BitAnd => "&",
        ast::Operator::FloorDiv => "//",
    }
}

fn operator_index(operator: ast::Operator) -> PyResult<i64> {
    match operator {
        ast::Operator::Add => Ok(0),
        ast::Operator::Sub => Ok(1),
        ast::Operator::Mult => Ok(2),
        ast::Operator::MatMult => Ok(3),
        ast::Operator::Div => Ok(4),
        ast::Operator::Mod => Ok(5),
        ast::Operator::Pow => Ok(6),
        ast::Operator::LShift => Ok(7),
        ast::Operator::RShift => Ok(8),
        ast::Operator::BitOr => Ok(9),
        ast::Operator::BitXor => Ok(10),
        ast::Operator::BitAnd => Ok(11),
        ast::Operator::FloorDiv => Ok(12),
    }
}

fn bool_op_index(operator: ast::BoolOp) -> i64 {
    match operator {
        ast::BoolOp::And => 0,
        ast::BoolOp::Or => 1,
    }
}

fn unary_op_index(operator: ast::UnaryOp) -> i64 {
    match operator {
        ast::UnaryOp::Invert => 0,
        ast::UnaryOp::Not => 1,
        ast::UnaryOp::UAdd => 2,
        ast::UnaryOp::USub => 3,
    }
}

fn comparison_index(operator: ast::CmpOp) -> i64 {
    match operator {
        ast::CmpOp::Eq => 0,
        ast::CmpOp::NotEq => 1,
        ast::CmpOp::Lt => 2,
        ast::CmpOp::LtE => 3,
        ast::CmpOp::Gt => 4,
        ast::CmpOp::GtE => 5,
        ast::CmpOp::Is => 6,
        ast::CmpOp::IsNot => 7,
        ast::CmpOp::In => 8,
        ast::CmpOp::NotIn => 9,
    }
}

fn evaluate_condition(serializer: &Serializer<'_>, expression: &ast::Expr) -> ConditionValue {
    match expression {
        ast::Expr::Name(name) => evaluate_special_name(name.id.as_str()),
        ast::Expr::Attribute(attribute) => evaluate_special_name(attribute.attr.as_str()),
        ast::Expr::BooleanLiteral(boolean) => {
            if boolean.value {
                ConditionValue::AlwaysTrue
            } else {
                ConditionValue::AlwaysFalse
            }
        }
        ast::Expr::UnaryOp(unary) if unary.op == ast::UnaryOp::Not => {
            negate_condition(evaluate_condition(serializer, &unary.operand))
        }
        ast::Expr::BoolOp(bool_op) => evaluate_bool_op(serializer, bool_op),
        ast::Expr::Compare(compare) => evaluate_version_compare(serializer, compare)
            .map(bool_condition)
            .unwrap_or(ConditionValue::Unknown),
        _ => ConditionValue::Unknown,
    }
}

fn evaluate_special_name(name: &str) -> ConditionValue {
    match name {
        "PY3" => ConditionValue::AlwaysTrue,
        "PY2" => ConditionValue::AlwaysFalse,
        "MYPY" | "TYPE_CHECKING" => ConditionValue::MypyTrue,
        _ => ConditionValue::Unknown,
    }
}

fn negate_condition(condition: ConditionValue) -> ConditionValue {
    match condition {
        ConditionValue::AlwaysTrue => ConditionValue::AlwaysFalse,
        ConditionValue::AlwaysFalse => ConditionValue::AlwaysTrue,
        ConditionValue::MypyTrue => ConditionValue::MypyFalse,
        ConditionValue::MypyFalse => ConditionValue::MypyTrue,
        ConditionValue::Unknown => ConditionValue::Unknown,
    }
}

fn evaluate_bool_op(serializer: &Serializer<'_>, bool_op: &ast::ExprBoolOp) -> ConditionValue {
    let values: Vec<_> = bool_op
        .values
        .iter()
        .map(|value| evaluate_condition(serializer, value))
        .collect();
    match bool_op.op {
        ast::BoolOp::And => evaluate_and_conditions(&values),
        ast::BoolOp::Or => evaluate_or_conditions(&values),
    }
}

fn evaluate_and_conditions(values: &[ConditionValue]) -> ConditionValue {
    if values.contains(&ConditionValue::AlwaysFalse) {
        ConditionValue::AlwaysFalse
    } else if values.contains(&ConditionValue::MypyFalse) {
        ConditionValue::MypyFalse
    } else if values.contains(&ConditionValue::Unknown) {
        ConditionValue::Unknown
    } else if values.contains(&ConditionValue::MypyTrue) {
        ConditionValue::MypyTrue
    } else {
        ConditionValue::AlwaysTrue
    }
}

fn evaluate_or_conditions(values: &[ConditionValue]) -> ConditionValue {
    if values.contains(&ConditionValue::AlwaysTrue) {
        ConditionValue::AlwaysTrue
    } else if values.contains(&ConditionValue::MypyTrue) {
        ConditionValue::MypyTrue
    } else if values.contains(&ConditionValue::Unknown) {
        ConditionValue::Unknown
    } else if values
        .iter()
        .all(|value| *value == ConditionValue::MypyFalse)
    {
        ConditionValue::MypyFalse
    } else {
        ConditionValue::AlwaysFalse
    }
}

fn bool_condition(value: bool) -> ConditionValue {
    if value {
        ConditionValue::AlwaysTrue
    } else {
        ConditionValue::AlwaysFalse
    }
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
enum VersionValue {
    Int(i64),
    Tuple(Vec<i64>),
}

fn evaluate_version_compare(
    serializer: &Serializer<'_>,
    compare: &ast::ExprCompare,
) -> Option<bool> {
    if compare.ops.len() != 1 || compare.comparators.len() != 1 {
        return None;
    }
    let left = version_value(serializer, &compare.left)?;
    let right = version_value(serializer, &compare.comparators[0])?;
    Some(compare_version_values(&left, compare.ops[0], &right))
}

fn compare_version_values(left: &VersionValue, op: ast::CmpOp, right: &VersionValue) -> bool {
    match op {
        ast::CmpOp::Eq => left == right,
        ast::CmpOp::NotEq => left != right,
        ast::CmpOp::Lt => left < right,
        ast::CmpOp::LtE => left <= right,
        ast::CmpOp::Gt => left > right,
        ast::CmpOp::GtE => left >= right,
        _ => false,
    }
}

fn version_value(serializer: &Serializer<'_>, expression: &ast::Expr) -> Option<VersionValue> {
    match expression {
        ast::Expr::NumberLiteral(number) => match &number.value {
            ast::Number::Int(value) => value.as_i64().map(VersionValue::Int),
            _ => None,
        },
        ast::Expr::Tuple(tuple) => tuple
            .elts
            .iter()
            .map(version_int)
            .collect::<Option<Vec<_>>>()
            .map(VersionValue::Tuple),
        ast::Expr::Attribute(attribute)
            if dotted_name(expression).as_deref() == Some("sys.version_info") =>
        {
            let _ = attribute;
            Some(VersionValue::Tuple(vec![
                serializer.python_version.0,
                serializer.python_version.1,
            ]))
        }
        ast::Expr::Subscript(subscript)
            if dotted_name(&subscript.value).as_deref() == Some("sys.version_info") =>
        {
            version_subscript(serializer, &subscript.slice)
        }
        _ => None,
    }
}

fn version_int(expression: &ast::Expr) -> Option<i64> {
    match expression {
        ast::Expr::NumberLiteral(number) => match &number.value {
            ast::Number::Int(value) => value.as_i64(),
            _ => None,
        },
        _ => None,
    }
}

fn version_subscript(serializer: &Serializer<'_>, slice: &ast::Expr) -> Option<VersionValue> {
    let version = [serializer.python_version.0, serializer.python_version.1];
    match slice {
        ast::Expr::NumberLiteral(_) => {
            let index = version_int(slice)?;
            version.get(index as usize).copied().map(VersionValue::Int)
        }
        ast::Expr::Slice(slice) => {
            let start = slice.lower.as_deref().and_then(version_int).unwrap_or(0);
            let end = slice.upper.as_deref().and_then(version_int).unwrap_or(2);
            if start < 0 || end < start {
                return None;
            }
            let start = usize::try_from(start).ok()?;
            let end = usize::try_from(end).ok()?.min(version.len());
            Some(VersionValue::Tuple(version[start..end].to_vec()))
        }
        _ => None,
    }
}

fn dotted_name(expression: &ast::Expr) -> Option<String> {
    match expression {
        ast::Expr::Name(name) => Some(name.id.as_str().to_owned()),
        ast::Expr::Attribute(attribute) => {
            let mut base = dotted_name(&attribute.value)?;
            base.push('.');
            base.push_str(attribute.attr.as_str());
            Some(base)
        }
        _ => None,
    }
}

fn escaped_bytes(bytes: impl IntoIterator<Item = u8>) -> String {
    let mut value = String::new();
    for byte in bytes {
        match byte {
            b'\r' => value.push_str("\\r"),
            b'\n' => value.push_str("\\n"),
            b'\t' => value.push_str("\\t"),
            b'\'' => value.push_str("\\'"),
            b'\\' => value.push_str("\\\\"),
            0x20..=0x7e => value.push(char::from(byte)),
            _ => value.push_str(&format!("\\x{byte:02x}")),
        }
    }
    value
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in source.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

fn collect_comment_directives(
    source: &str,
    tokens: &ruff_python_ast::token::Tokens,
) -> (Vec<(i64, Vec<String>)>, HashMap<i64, String>) {
    let line_starts = line_starts(source);
    let mut ignores = Vec::new();
    let mut type_comments = HashMap::new();
    for token in tokens.iter() {
        if token.kind() != TokenKind::Comment {
            continue;
        }
        let range = token.range();
        let comment = &source[range.start().to_usize()..range.end().to_usize()];
        let line_index =
            line_starts.partition_point(|start| *start <= range.start().to_usize()) - 1;
        let line = (line_index + 1) as i64;
        if let Some(codes) = parse_type_ignore_comment(comment) {
            ignores.push((line, codes));
        }
        if let Some(type_comment) = parse_assignment_type_comment(comment) {
            type_comments.insert(line, type_comment);
        }
    }
    (ignores, type_comments)
}

fn parse_type_ignore_comment(comment: &str) -> Option<Vec<String>> {
    for segment in comment.split('#').skip(1) {
        let after_hash = segment.trim_start();
        if let Some(after_type) = after_hash.strip_prefix("type:") {
            if let Some(tag) = after_type.trim_start().strip_prefix("ignore") {
                return parse_type_ignore_tag(tag);
            }
        }
    }
    None
}

fn parse_assignment_type_comment(comment: &str) -> Option<String> {
    let text = comment.trim_start();
    let after_hash = text.strip_prefix('#')?.trim_start();
    let type_comment = after_hash.strip_prefix("type:")?.trim_start();
    if type_comment.starts_with("ignore") {
        return None;
    }
    let type_comment =
        strip_trailing_comment(strip_trailing_type_ignore_comment(type_comment)).trim();
    (!type_comment.is_empty()).then(|| type_comment.to_owned())
}

fn strip_trailing_type_ignore_comment(type_comment: &str) -> &str {
    type_comment
        .find("# type: ignore")
        .or_else(|| type_comment.find("#type:ignore"))
        .map_or(type_comment, |index| &type_comment[..index])
}

fn strip_trailing_comment(type_comment: &str) -> &str {
    type_comment
        .split_once("  #")
        .map_or(type_comment, |(typ, _)| typ)
}

fn parse_type_ignore_tag(tag: &str) -> Option<Vec<String>> {
    let trimmed = tag.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Some(Vec::new());
    }

    let after_open = trimmed.strip_prefix('[')?;
    let close = after_open.find(']')?;
    let (codes, rest) = after_open.split_at(close);
    let rest = rest.strip_prefix(']').unwrap().trim_start();
    if !(rest.is_empty() || rest.starts_with('#')) {
        return None;
    }
    Some(
        codes
            .split(',')
            .filter_map(|code| {
                let code = code.trim();
                (!code.is_empty()).then(|| code.to_owned())
            })
            .collect(),
    )
}

fn source_hash(source: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(source.as_bytes());
    format!("{:x}", hash.finalize())
}

#[pymodule]
fn ast_serialize(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_trivial_call_like_existing_binary_contract() {
        let suite = parse_module("print('hello')").unwrap().into_suite();
        let (bytes, imports) =
            serialize_suite(&suite, "print('hello')", (3, 10), false, HashMap::new()).unwrap();
        assert!(imports.is_empty());
        assert_eq!(
            bytes,
            [
                LITERAL_INT,
                22,
                EXPR_STMT,
                CALL_EXPR,
                NAME_EXPR,
                LITERAL_STR,
                30,
                b'p',
                b'r',
                b'i',
                b'n',
                b't',
                LOCATION,
                22,
                20,
                20,
                30,
                END_TAG,
                LIST_GEN,
                22,
                STR_EXPR,
                LITERAL_STR,
                30,
                b'h',
                b'e',
                b'l',
                b'l',
                b'o',
                LOCATION,
                22,
                32,
                20,
                34,
                END_TAG,
                LIST_INT,
                22,
                20,
                LIST_GEN,
                22,
                LITERAL_NONE,
                LOCATION,
                22,
                20,
                20,
                48,
                END_TAG,
                END_TAG,
            ]
        );
    }

    #[test]
    fn collects_type_ignore_comments_from_comment_tokens() {
        let source = "\
x = 1  # type: ignore
y = (
   2  # type: ignore  # Comment
)
y = 1 # foo: ignore
z = 1  #type: ignore[x]
zz = 1  #type: ignore [ foo, arg-type ]
a = []  # type: list[int]
b = []  # type: list[str]  # trailing comment
";
        let parsed = parse_module(source).unwrap();
        let (ignores, type_comments) = collect_comment_directives(source, parsed.tokens());

        assert_eq!(
            ignores,
            vec![
                (1, vec![]),
                (3, vec![]),
                (6, vec!["x".to_owned()]),
                (7, vec!["foo".to_owned(), "arg-type".to_owned()]),
            ]
        );
        assert_eq!(type_comments.get(&8).unwrap(), "list[int]");
        assert_eq!(type_comments.get(&9).unwrap(), "list[str]");
    }
}
