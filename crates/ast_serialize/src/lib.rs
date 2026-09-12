use pyo3::exceptions::{PyNotImplementedError, PyRuntimeError, PyUnicodeDecodeError};
use pyo3::prelude::*;
use ruff_python_ast::{self as ast, token::TokenKind, AnyParameterRef, PySourceType};
#[cfg(test)]
use ruff_python_parser::parse_module;
use ruff_python_parser::{parse_expression, parse_unchecked_source};
use ruff_text_size::Ranged;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;

mod ast_node;
mod ast_writer;
#[cfg(test)]
mod expr_legacy;
#[cfg(test)]
mod stmt_legacy;

/// Wire format version for the serialized AST returned by `parse`.
/// Bump when a record layout changes; `parse` rejects any other value so a
/// stale extension fails at the entry instead of mid-deserialization.
const AST_WIRE_VERSION: i64 = 5;

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
const TSTRING_EXPR: u8 = 229;

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
const IMPORT_FLAG_UNREACHABLE: i64 = 0x02;
const IMPORT_FLAG_MYPY_ONLY: i64 = 0x04;
const IMPORT_FLAG_OMIT_FROM_DEPENDENCIES: i64 = 0x08;

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

    /// Append a pre-built sub-record verbatim (lambda parameter payloads).
    fn raw_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
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
    platform: String,
    always_true: HashSet<String>,
    always_false: HashSet<String>,
    skip_function_bodies: bool,
    class_depth: usize,
    parse_errors: Vec<NativeParseError>,
    forced_type_loc: Option<SourceLocation>,
    uses_template_strings: bool,
    callable_arg_list_depth: usize,
    lambda_depth: usize,
    include_docstrings: bool,
    custom_typing_module: Option<String>,
    /// Test-only A/B switch: route expression records through the frozen
    /// pre-enum writer so both paths can be byte-compared on one corpus.
    #[cfg(test)]
    legacy_exprs: bool,
    /// Test-only A/B switch: route statement/pattern records through the
    /// frozen pre-enum writer so both paths can be byte-compared.
    #[cfg(test)]
    legacy_stmts: bool,
}

impl<'a> Serializer<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        source: &'a str,
        python_version: (i64, i64),
        platform: String,
        always_true: HashSet<String>,
        always_false: HashSet<String>,
        skip_function_bodies: bool,
        type_comments: HashMap<i64, String>,
        include_docstrings: bool,
        custom_typing_module: Option<String>,
    ) -> Self {
        Self {
            writer: Writer::default(),
            imports: ImportCollector::default(),
            type_comments,
            line_starts: line_starts(source),
            source,
            python_version,
            platform,
            always_true,
            always_false,
            skip_function_bodies,
            class_depth: 0,
            parse_errors: Vec::new(),
            forced_type_loc: None,
            callable_arg_list_depth: 0,
            lambda_depth: 0,
            uses_template_strings: false,
            include_docstrings,
            custom_typing_module,
            #[cfg(test)]
            legacy_exprs: false,
            #[cfg(test)]
            legacy_stmts: false,
        }
    }

    /// `ast.get_docstring(node, clean=False)` fields for a class/function body:
    /// None when the first statement is not a string expression, else the
    /// evaluated value plus the surrogate-corruption repair data.
    fn docstring_fields(&self, body: &ast::Suite) -> Option<(String, bool, Option<String>)> {
        if !self.include_docstrings {
            return None;
        }
        let ast::Stmt::Expr(first) = body.first()? else {
            return None;
        };
        let ast::Expr::StringLiteral(literal) = &*first.value else {
            return None;
        };
        let (value, corrupted, raw) = string_literal_parts(self.source, literal);
        Some((value.to_owned(), corrupted, raw))
    }

    #[cfg(test)]
    fn write_docstring(&mut self, body: &ast::Suite) {
        match self.docstring_fields(body) {
            None => self.writer.bool(false),
            Some((value, corrupted, raw)) => {
                self.writer.bool(true);
                self.writer.string(&value);
                self.writer.bool(corrupted);
                if let Some(raw) = raw {
                    self.writer.string(&raw);
                }
            }
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

#[derive(Clone, Debug)]
struct NativeParseError {
    line: i64,
    column: i64,
    message: String,
    blocker: bool,
    code: &'static str,
}

#[derive(Default)]
struct ImportCollector {
    imports: Vec<ImportMetadata>,
    function_depth: usize,
    unreachable_depth: usize,
    mypy_only_depth: usize,
    omit_from_dependencies_depth: usize,
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
        self.imports.push(import);
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
        if self.unreachable_depth > 0 {
            flags |= IMPORT_FLAG_UNREACHABLE;
        }
        if self.omit_from_dependencies_depth > 0 {
            flags |= IMPORT_FLAG_OMIT_FROM_DEPENDENCIES;
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

    fn enter_omit_from_dependencies(&mut self) {
        self.omit_from_dependencies_depth += 1;
    }

    fn leave_omit_from_dependencies(&mut self) {
        self.omit_from_dependencies_depth -= 1;
    }
}

#[allow(clippy::too_many_arguments)] // Signature mirrors the Python `parse` entry point.
#[pyfunction]
#[pyo3(signature = (
    fnam,
    source = None,
    skip_function_bodies = false,
    python_version = None,
    platform = None,
    always_true = None,
    always_false = None,
    cache_version = 0,
    include_docstrings = false,
    custom_typing_module = None
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
    include_docstrings: bool,
    custom_typing_module: Option<String>,
) -> PyResult<PyObject> {
    if cache_version != AST_WIRE_VERSION {
        return Err(PyRuntimeError::new_err(format!(
            "ast_serialize extension wire version mismatch: caller expects AST wire v{cache_version}, \
             this extension writes v{AST_WIRE_VERSION}; rebuild the ast_serialize extension"
        )));
    }
    let source = read_source(py, source, fnam)?;
    let parsed = parse_unchecked_source(&source, PySourceType::Python);
    let mut errors = parse_errors_to_py(py, &source, parsed.errors())?;
    let (type_ignores, type_comments) = collect_comment_directives(&source, parsed.tokens());
    let module = parsed.into_syntax();
    let is_partial_package = is_partial_stub_package(fnam, &module.body);
    let python_version = python_version.unwrap_or((3, 10));
    let python_version = (i64::from(python_version.0), i64::from(python_version.1));
    let (ast_bytes, imports, native_errors, uses_template_strings) = serialize_suite(
        &module.body,
        &source,
        python_version,
        platform.unwrap_or_default(),
        always_true.unwrap_or_default(),
        always_false.unwrap_or_default(),
        skip_function_bodies,
        type_comments,
        include_docstrings,
        custom_typing_module,
    )?;
    errors.extend(native_parse_errors_to_py(py, native_errors)?);
    let import_bytes = serialize_import_metadata(&imports);
    let data = pyo3::types::PyDict::new(py);
    data.set_item("ast_wire_version", AST_WIRE_VERSION)?;
    data.set_item("is_partial_package", is_partial_package)?;
    data.set_item("uses_template_strings", uses_template_strings)?;
    data.set_item("mypy_ignores", type_ignores.clone())?;
    data.set_item("source_hash", source_hash(&source))?;
    data.set_item("mypy_comments", collect_mypy_comments(&source))?;
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

    // Ruff produces more specific syntax error messages than CPython.
    // The strangler-fig contract requires byte-identical output to Python.
    // Re-parse with CPython's ast.parse on ruff syntax errors to match.

    // message and location.

    // Syntax errors are rare in production (users fix them), so the
    // double-parse cost is negligible.
    let cpython_error = cpython_syntax_error(py, source).ok();

    // CPython's ast.parse always stops at the first syntax error, so the
    // Python path reports exactly one; emit more and we diverge from parity.
    let errors = if cpython_error.is_some() && !errors.is_empty() {
        &errors[..1]
    } else {
        errors
    };

    let mut result = Vec::with_capacity(errors.len());
    for error in errors {
        let ruff_offset = error.location.start().to_usize();
        let ruff_line_index = line_starts.partition_point(|start| *start <= ruff_offset) - 1;
        let ruff_line_start = line_starts[ruff_line_index];
        let ruff_line_end = line_starts
            .get(ruff_line_index + 1)
            .copied()
            .unwrap_or(source.len());
        let source_line = &source[ruff_line_start..ruff_line_end];

        let (line, column, message) = match &cpython_error {
            Some(cpe) => {
                let line = cpe.lineno.unwrap_or(ruff_line_index as i64 + 1);
                let column = cpe
                    .offset
                    .unwrap_or((source[ruff_line_start..ruff_offset].chars().count() + 1) as i64);
                (line, column, standardize_message(&cpe.message))
            }
            None => {
                // Fallback: translate via the TypedDict special-case, else
                // use ruff's message and location.
                let column = source[ruff_line_start..ruff_offset].chars().count() + 1;
                let msg = translate_parse_error_message(&error.error.to_string(), source_line);
                (ruff_line_index as i64 + 1, column as i64, msg)
            }
        };

        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("line", line)?;
        dict.set_item("column", column)?;
        dict.set_item("message", message)?;
        dict.set_item("blocker", true)?;
        dict.set_item("code", "syntax")?;
        result.push(dict.into_py(py));
    }
    Ok(result)
}

/// The information CPython's `ast.parse` reports in its SyntaxError.
struct CpythonSyntaxError {
    message: String,
    lineno: Option<i64>,
    offset: Option<i64>,
}

/// Re-parse `source` with CPython's `ast.parse` to obtain the exact
/// `SyntaxError` CPython would raise. Returns `None` if CPython does not
/// report a syntax error (shouldn't happen if ruff found one, but we guard
/// against it so the caller can fall back to ruff's message/location).
fn cpython_syntax_error(py: Python<'_>, source: &str) -> PyResult<CpythonSyntaxError> {
    let ast_module = py.import("ast")?;
    let result: PyResult<PyObject> = ast_module
        .call_method1("parse", (source, "<native-parser>", "exec"))
        .map(|obj| obj.into_py(py));
    match result {
        Ok(_) => {
            // CPython parsed successfully but ruff didn't — fall back.
            Err(pyo3::exceptions::PyValueError::new_err(
                "CPython accepted source that ruff rejected",
            ))
        }
        Err(err) if err.is_instance_of::<pyo3::exceptions::PySyntaxError>(py) => {
            let value = err.value(py);
            let message: String = value.getattr("msg")?.extract()?;
            let lineno: Option<i64> = value.getattr("lineno").ok().and_then(|a| a.extract().ok());
            let offset: Option<i64> = value.getattr("offset").ok().and_then(|a| a.extract().ok());
            Ok(CpythonSyntaxError {
                message,
                lineno,
                offset,
            })
        }
        Err(err) => Err(err),
    }
}

/// Capitalize the first word of the message, matching fastparse.py's
/// `re.sub(r"^(\s*\w)", upper, message)` standardization so native and
/// Python parser paths produce byte-identical error strings.
fn standardize_message(message: &str) -> String {
    let mut chars = message.char_indices();
    for (idx, ch) in chars.by_ref() {
        if ch.is_whitespace() {
            continue;
        }
        if ch.is_alphabetic() {
            let mut out = String::with_capacity(message.len());
            out.push_str(&message[..idx]);
            for up in ch.to_uppercase() {
                out.push(up);
            }
            out.push_str(&message[idx + ch.len_utf8()..]);
            return out;
        }
        break;
    }
    message.to_owned()
}

fn translate_parse_error_message(message: &str, source_line: &str) -> String {
    if source_line.contains("TypedDict") {
        if let Some(keyword) = message
            .strip_prefix("Duplicate keyword argument \"")
            .and_then(|rest| rest.strip_suffix('"'))
        {
            return format!("Repeated keyword argument \"{keyword}\" for \"TypedDict\"");
        }
    }
    message.to_owned()
}

fn native_parse_errors_to_py(
    py: Python<'_>,
    errors: Vec<NativeParseError>,
) -> PyResult<Vec<PyObject>> {
    let mut result = Vec::with_capacity(errors.len());
    for error in errors {
        let dict = pyo3::types::PyDict::new(py);
        dict.set_item("line", error.line)?;
        dict.set_item("column", error.column)?;
        dict.set_item("message", error.message)?;
        dict.set_item("blocker", error.blocker)?;
        dict.set_item("code", error.code)?;
        result.push(dict.into_py(py));
    }
    Ok(result)
}

type SerializeSuiteResult = (Vec<u8>, Vec<ImportMetadata>, Vec<NativeParseError>, bool);

#[allow(clippy::too_many_arguments)]
fn serialize_suite(
    suite: &ast::Suite,
    source: &str,
    python_version: (i64, i64),
    platform: String,
    always_true: Vec<String>,
    always_false: Vec<String>,
    skip_function_bodies: bool,
    type_comments: HashMap<i64, String>,
    include_docstrings: bool,
    custom_typing_module: Option<String>,
) -> PyResult<SerializeSuiteResult> {
    let serializer = Serializer::new(
        source,
        python_version,
        platform,
        always_true.into_iter().collect(),
        always_false.into_iter().collect(),
        skip_function_bodies,
        type_comments,
        include_docstrings,
        custom_typing_module,
    );
    serialize_suite_with_serializer(serializer, suite)
}

fn serialize_suite_with_serializer(
    mut serializer: Serializer<'_>,
    suite: &ast::Suite,
) -> PyResult<SerializeSuiteResult> {
    serializer.writer.int(suite.len() as i64);
    let mut rest_unreachable = false;
    for statement in suite {
        if rest_unreachable {
            serializer.imports.enter_unreachable();
            serializer.imports.enter_omit_from_dependencies();
            let result = serialize_stmt(&mut serializer, statement);
            serializer.imports.leave_omit_from_dependencies();
            serializer.imports.leave_unreachable();
            result?;
        } else {
            serialize_stmt(&mut serializer, statement)?;
            rest_unreachable = top_level_assert_always_fails(&serializer, statement);
        }
    }
    let uses_template_strings = serializer.uses_template_strings;
    let imports = serializer.imports.imports.clone();
    let errors = serializer.parse_errors.clone();
    Ok((
        serializer.into_bytes(),
        imports,
        errors,
        uses_template_strings,
    ))
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

fn is_partial_stub_package(fnam: &str, suite: &ast::Suite) -> bool {
    if !(fnam.ends_with("/__init__.pyi") || fnam.ends_with("\\__init__.pyi")) {
        return false;
    }
    suite.iter().any(|statement| match statement {
        ast::Stmt::FunctionDef(function) => function.name.as_str() == "__getattr__",
        _ => false,
    })
}

/// The evaluated value of a string literal, whether lone surrogate escapes
/// were lost (Ruff decodes them to U+FFFD), and the raw source tokens to
/// repair them. The Python reader re-evaluates the raw tokens with CPython
/// semantics, matching `ast.get_docstring(node, clean=False)` exactly.
fn string_literal_parts<'a>(
    source: &'a str,
    string: &'a ast::ExprStringLiteral,
) -> (&'a str, bool, Option<String>) {
    let raw_tokens: Vec<&str> = string
        .value
        .as_slice()
        .iter()
        .map(|part| {
            let range = part.range;
            &source[range.start().to_usize()..range.end().to_usize()]
        })
        .collect();
    let corrupted = string
        .value
        .as_slice()
        .iter()
        .enumerate()
        .any(|(i, part)| !part.flags.prefix().is_raw() && has_surrogate_escape(raw_tokens[i]));
    let raw = corrupted.then(|| raw_tokens.join(" "));
    (string.value.to_str(), corrupted, raw)
}

/// Mirror fastparse's `visit_Import`: a module named like the custom typing
/// module becomes `typing`, with the original name as an implicit asname.
fn translated_import_names(
    aliases: &[ast::Alias],
    custom_typing_module: Option<&str>,
) -> Vec<(String, Option<String>)> {
    aliases
        .iter()
        .map(|alias| {
            let original = alias.name.as_str();
            let translated = translate_module_name(original, custom_typing_module);
            let asname = match &alias.asname {
                Some(asname) => Some(asname.as_str().to_owned()),
                None if translated != original => Some(original.to_owned()),
                None => None,
            };
            (translated, asname)
        })
        .collect()
}

fn translate_module_name(name: &str, custom_typing_module: Option<&str>) -> String {
    if custom_typing_module == Some(name) {
        "typing".to_owned()
    } else {
        name.to_owned()
    }
}

fn top_level_assert_always_fails(serializer: &Serializer<'_>, statement: &ast::Stmt) -> bool {
    matches!(
        statement,
        ast::Stmt::Assert(assert_stmt)
            if matches!(
                evaluate_condition(serializer, &assert_stmt.test),
                ConditionValue::AlwaysFalse | ConditionValue::MypyFalse
            )
    )
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

fn serialize_expr(serializer: &mut Serializer<'_>, expression: &ast::Expr) -> PyResult<()> {
    #[cfg(test)]
    if serializer.legacy_exprs {
        return expr_legacy::serialize_expr(serializer, expression);
    }
    let node = ast_node::build_expr(serializer, expression)?;
    ast_writer::write_expr(&mut serializer.writer, &node);
    Ok(())
}

#[cfg(test)]
fn serialize_lvalue(serializer: &mut Serializer<'_>, expression: &ast::Expr) -> PyResult<()> {
    if serializer.legacy_exprs {
        return expr_legacy::serialize_lvalue(serializer, expression);
    }
    let node = ast_node::build_lvalue(serializer, expression)?;
    ast_writer::write_expr(&mut serializer.writer, &node);
    Ok(())
}

fn serialize_stmt(serializer: &mut Serializer<'_>, statement: &ast::Stmt) -> PyResult<()> {
    #[cfg(test)]
    if serializer.legacy_stmts {
        return stmt_legacy::serialize_stmt(serializer, statement);
    }
    let node = ast_node::build_stmt(serializer, statement)?;
    ast_writer::write_stmt(&mut serializer.writer, &node);
    Ok(())
}

/// Capture a not-yet-enumed sub-record (type annotations, type params,
/// parameter lists) into bytes so the statement writer can re-emit it
/// verbatim. Wire bytes are unchanged; this keeps the G0.4 enum scope to
/// statements and patterns.
fn capture_bytes<F>(serializer: &mut Serializer<'_>, write: F) -> PyResult<Vec<u8>>
where
    F: FnOnce(&mut Serializer<'_>) -> PyResult<()>,
{
    let saved = std::mem::take(&mut serializer.writer);
    let result = write(serializer);
    let captured = std::mem::replace(&mut serializer.writer, saved);
    result?;
    Ok(captured.into_bytes())
}

/// True when a raw token has a `\u`/`\U` escape for a lone UTF-16 surrogate.
/// Ruff maps those to U+FFFD, so the decoded value is lossy; the wire carries
/// the raw token for the Python reader to re-decode (over-matches are safe).
fn has_surrogate_escape(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            i += 1;
            continue;
        }
        let hex_len = match bytes.get(i + 1) {
            Some(b'u') => 4,
            Some(b'U') => 8,
            _ => {
                i += 2;
                continue;
            }
        };
        let start = i + 2;
        if let Some(hex) = bytes.get(start..start + hex_len) {
            if hex.iter().all(u8::is_ascii_hexdigit) {
                let value = u32::from_str_radix(
                    std::str::from_utf8(hex).expect("ascii hex digits are utf-8"),
                    16,
                )
                .expect("valid hex digits");
                if (0xd800..=0xdfff).contains(&value) {
                    return true;
                }
            }
        }
        i += 2;
    }
    false
}

#[derive(Debug)]
struct ParsedFunctionTypeComment {
    arg_types: Option<Vec<Option<String>>>,
    return_type: String,
}

fn function_type_comment(
    serializer: &mut Serializer<'_>,
    function: &ast::StmtFunctionDef,
    function_loc: &SourceLocation,
) -> Option<ParsedFunctionTypeComment> {
    let def_line = function_loc.line;
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
            if comment.contains("->") {
                serializer
                    .parse_errors
                    .push(type_comment_syntax_error(function_loc, comment));
            }
            continue;
        };
        if function_type_comment_has_syntax_error(&parsed) {
            serializer
                .parse_errors
                .push(type_comment_syntax_error(function_loc, comment));
            return None;
        }
        if let Some(arg_types) = &mut parsed.arg_types {
            if serializer.class_depth > 0 && arg_types.len() + 1 == function.parameters.len() {
                arg_types.insert(0, None);
            }
            if function.returns.is_some() || has_parameter_annotation(function) {
                serializer
                    .parse_errors
                    .push(duplicate_type_signature_error(function_loc));
            }
            if let Some(parameter_loc) = first_parameter_type_comment_loc(serializer, function) {
                serializer
                    .parse_errors
                    .push(duplicate_type_signature_error(&parameter_loc));
            }
            if arg_types.len() > function.parameters.len() {
                serializer.parse_errors.push(NativeParseError {
                    line: function_loc.line,
                    column: function_loc.column,
                    message: "Type signature has too many parameters".to_owned(),
                    blocker: false,
                    code: "syntax",
                });
                return None;
            }
            if arg_types.len() < function.parameters.len() {
                serializer.parse_errors.push(NativeParseError {
                    line: function_loc.line,
                    column: function_loc.column,
                    message: "Type signature has too few parameters".to_owned(),
                    blocker: false,
                    code: "syntax",
                });
                return None;
            }
        } else if function.returns.is_some() {
            serializer
                .parse_errors
                .push(duplicate_type_signature_error(function_loc));
        }
        return Some(parsed);
    }
    None
}

fn function_type_comment_has_syntax_error(comment: &ParsedFunctionTypeComment) -> bool {
    comment
        .arg_types
        .as_ref()
        .into_iter()
        .flatten()
        .filter_map(|arg| arg.as_deref())
        .any(|arg| parse_expression(arg).is_err())
        || parse_expression(&comment.return_type).is_err()
}

fn type_comment_syntax_error(loc: &SourceLocation, comment: &str) -> NativeParseError {
    NativeParseError {
        line: loc.line,
        column: loc.column,
        message: format!(
            "Syntax error in type comment \"{}\"",
            comment.split('#').next().unwrap_or(comment).trim()
        ),
        blocker: false,
        code: "syntax",
    }
}

fn duplicate_type_signature_error(loc: &SourceLocation) -> NativeParseError {
    NativeParseError {
        line: loc.line,
        column: loc.column,
        message: "Function has duplicate type signatures".to_owned(),
        blocker: false,
        code: "syntax",
    }
}

fn has_parameter_annotation(function: &ast::StmtFunctionDef) -> bool {
    function
        .parameters
        .iter()
        .any(|parameter| parameter.annotation().is_some())
}

fn first_parameter_type_comment_loc(
    serializer: &Serializer<'_>,
    function: &ast::StmtFunctionDef,
) -> Option<SourceLocation> {
    function.parameters.iter().find_map(|parameter| {
        let loc = serializer.loc(&parameter);
        serializer
            .type_comments
            .get(&loc.end_line)
            .is_some_and(|comment| parse_function_type_comment(comment).is_none())
            .then_some(loc)
    })
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
    // CPython never populates arg.type_comment for lambda parameters, so a
    // line-matched statement comment must not become a lambda arg annotation.
    let inline_comment = if serializer.lambda_depth > 0 {
        None
    } else {
        serializer
            .type_comments
            .get(&loc.end_line)
            .filter(|comment| parse_function_type_comment(comment).is_none())
            .cloned()
    };
    if parameter.annotation().is_some() && inline_comment.is_some() {
        serializer
            .parse_errors
            .push(duplicate_type_signature_error(&loc));
    }
    let inline_type_comment = if comment_annotation.is_none() && parameter.annotation().is_none() {
        inline_comment
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
        serialize_type_with_forced_loc(serializer, &parsed_type.into_expr(), &loc)?;
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
    let loc = serializer
        .forced_type_loc
        .clone()
        .unwrap_or_else(|| serializer.loc(expression));
    serialize_type_with_loc(serializer, expression, &loc)
}

fn serialize_type_with_forced_loc(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
    loc: &SourceLocation,
) -> PyResult<()> {
    let previous = serializer.forced_type_loc.replace(loc.clone());
    let result = serialize_type_with_loc(serializer, expression, loc);
    serializer.forced_type_loc = previous;
    result
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
            serialize_unbound_type_with_originals(
                serializer,
                &name,
                args.as_slice(),
                matches!(&*subscript.slice, ast::Expr::Tuple(tuple) if tuple.elts.is_empty()),
                None,
                None,
                loc,
            )
        }
        ast::Expr::Call(call) => {
            if serializer.callable_arg_list_depth > 0 {
                serialize_call_type(serializer, call, loc)
            } else {
                serialize_invalid_raw_expression_type(
                    serializer,
                    loc,
                    invalid_call_type_note(call).as_deref(),
                )
            }
        }
        ast::Expr::List(list) => {
            serializer.writer.tag(LIST_TYPE);
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(list.elts.len() as i64);
            serializer.callable_arg_list_depth += 1;
            for item in &list.elts {
                serialize_type(serializer, item)?;
            }
            serializer.callable_arg_list_depth -= 1;
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
            let previous = serializer.forced_type_loc.replace(loc.clone());
            let serialized = serialize_type_with_original_string(
                serializer,
                &expression,
                text,
                fallback_name,
                loc,
            );
            serializer.forced_type_loc = previous;
            if serialized? {
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

fn invalid_call_type_note(call: &ast::ExprCall) -> Option<String> {
    let constructor = dotted_name(&call.func)?;
    if !call.arguments.keywords.is_empty() {
        Some("Cannot use a function call in a type annotation".to_owned())
    } else {
        Some(format!(
            "Suggestion: use {constructor}[...] instead of {constructor}(...)"
        ))
    }
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
        ast::Expr::Name(name) => evaluate_special_name(serializer, name.id.as_str()),
        ast::Expr::Attribute(attribute) => {
            evaluate_special_name(serializer, attribute.attr.as_str())
        }
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
        ast::Expr::Compare(compare) => evaluate_compare(serializer, compare)
            .map(bool_condition)
            .unwrap_or(ConditionValue::Unknown),
        _ => ConditionValue::Unknown,
    }
}

fn evaluate_special_name(serializer: &Serializer<'_>, name: &str) -> ConditionValue {
    match name {
        "PY3" => ConditionValue::AlwaysTrue,
        "PY2" => ConditionValue::AlwaysFalse,
        "MYPY" | "TYPE_CHECKING" => ConditionValue::MypyTrue,
        _ if serializer.always_true.contains(name) => ConditionValue::AlwaysTrue,
        _ if serializer.always_false.contains(name) => ConditionValue::AlwaysFalse,
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

fn evaluate_compare(serializer: &Serializer<'_>, compare: &ast::ExprCompare) -> Option<bool> {
    evaluate_version_compare(serializer, compare)
        .or_else(|| evaluate_platform_compare(serializer, compare))
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

fn evaluate_platform_compare(
    serializer: &Serializer<'_>,
    compare: &ast::ExprCompare,
) -> Option<bool> {
    if compare.ops.len() != 1 || compare.comparators.len() != 1 {
        return None;
    }
    if dotted_name(&compare.left).as_deref() != Some("sys.platform") {
        return None;
    }
    let ast::Expr::StringLiteral(right) = &compare.comparators[0] else {
        return None;
    };
    let right = right.value.to_str();
    match compare.ops[0] {
        ast::CmpOp::Eq => Some(serializer.platform == right),
        ast::CmpOp::NotEq => Some(serializer.platform != right),
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
    // Must match the Python path exactly: BytesExpr.value is
    // `bytes_to_human_readable_repr(val)` = repr(b)[2:-1], whose bytes
    // repr escapes a quote only when its quote wrapper forces it.
    let bytes_copy: Vec<u8> = bytes.into_iter().collect();
    let outer_double = bytes_copy.contains(&b'\'') && !bytes_copy.contains(&b'"');
    let mut value = String::new();
    for byte in bytes_copy {
        match byte {
            b'\r' => value.push_str("\\r"),
            b'\n' => value.push_str("\\n"),
            b'\t' => value.push_str("\\t"),
            b'\'' if !outer_double => value.push_str("\\'"),
            b'\'' => value.push('\''),
            b'"' if outer_double => value.push_str("\\\""),
            b'"' => value.push('"'),
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

type CommentDirectives = (Vec<(i64, Vec<String>)>, HashMap<i64, String>);

fn collect_comment_directives(
    source: &str,
    tokens: &ruff_python_ast::token::Tokens,
) -> CommentDirectives {
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
        if let Some((type_comment, type_ignore)) = parse_assignment_type_comment(comment) {
            type_comments.insert(line, type_comment);
            if let Some(codes) = type_ignore {
                ignores.push((line, codes));
            }
        } else if let Some(codes) = parse_type_ignore_comment(comment) {
            ignores.push((line, codes));
        }
    }
    (ignores, type_comments)
}

fn parse_type_ignore_comment(comment: &str) -> Option<Vec<String>> {
    let text = comment.trim_start();
    let after_hash = text.strip_prefix('#')?.trim_start();
    let after_type = after_hash.strip_prefix("type:")?;
    let tag = strip_type_ignore_keyword(after_type.trim_start())?;
    parse_type_ignore_tag(tag)
}

fn parse_assignment_type_comment(comment: &str) -> Option<(String, Option<Vec<String>>)> {
    let text = comment.trim_start();
    let after_hash = text.strip_prefix('#')?.trim_start();
    let type_comment = after_hash.strip_prefix("type:")?.trim_start();
    if strip_type_ignore_keyword(type_comment).is_some() {
        return None;
    }
    let (type_comment, type_ignore) = split_type_comment_ignore(type_comment);
    let type_comment = strip_trailing_comment(type_comment).trim();
    (!type_comment.is_empty()).then(|| (type_comment.to_owned(), type_ignore))
}

fn split_type_comment_ignore(type_comment: &str) -> (&str, Option<Vec<String>>) {
    let Some(hash_index) = type_comment.find('#') else {
        return (type_comment, None);
    };
    let after_hash = type_comment[hash_index + 1..].trim_start();
    let Some(after_type) = after_hash.strip_prefix("type:") else {
        return (type_comment, None);
    };
    let Some(tag) = strip_type_ignore_keyword(after_type.trim_start()) else {
        return (type_comment, None);
    };
    (&type_comment[..hash_index], parse_type_ignore_tag(tag))
}

fn strip_type_ignore_keyword(comment: &str) -> Option<&str> {
    let rest = comment.strip_prefix("ignore")?;
    if rest.is_empty()
        || rest.starts_with('[')
        || rest.starts_with('#')
        || rest.starts_with(char::is_whitespace)
    {
        Some(rest)
    } else {
        None
    }
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

fn collect_mypy_comments(source: &str) -> Vec<(i64, String)> {
    const PREFIX: &str = "# mypy: ";
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            line.strip_prefix(PREFIX)
                .map(|comment| ((index + 1) as i64, comment.to_owned()))
        })
        .collect()
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

    fn serialize_test_source(
        source: &str,
        include_docstrings: bool,
        custom_typing_module: Option<&str>,
    ) -> Vec<u8> {
        let suite = parse_module(source).unwrap().into_suite();
        let (bytes, _, _, _) = serialize_suite(
            &suite,
            source,
            (3, 10),
            String::new(),
            Vec::new(),
            Vec::new(),
            false,
            HashMap::new(),
            include_docstrings,
            custom_typing_module.map(str::to_owned),
        )
        .unwrap();
        bytes
    }

    /// Serialize one source through the production enum paths or the frozen
    /// direct-writer references, for byte-parity comparison.
    fn serialize_source_bytes(source: &str, legacy_stmts: bool, legacy_exprs: bool) -> Vec<u8> {
        serialize_source_bytes_ex(source, legacy_stmts, legacy_exprs, false, true, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn serialize_source_bytes_ex(
        source: &str,
        legacy_stmts: bool,
        legacy_exprs: bool,
        skip_function_bodies: bool,
        include_docstrings: bool,
        custom_typing_module: Option<&str>,
    ) -> Vec<u8> {
        let suite = parse_module(source).unwrap().into_suite();
        let mut serializer = Serializer::new(
            source,
            (3, 10),
            String::new(),
            HashSet::new(),
            HashSet::new(),
            skip_function_bodies,
            HashMap::new(),
            include_docstrings,
            custom_typing_module.map(str::to_owned),
        );
        serializer.legacy_stmts = legacy_stmts;
        serializer.legacy_exprs = legacy_exprs;
        serialize_suite_with_serializer(serializer, &suite)
            .unwrap()
            .0
    }

    /// Every `[case ...]` source in the native-parser data files.
    fn read_native_parser_cases() -> Vec<(String, String)> {
        const FILES: &[&str] = &[
            "native-parser.test",
            "native-parser-python311.test",
            "native-parser-python312.test",
            "native-parser-imports.test",
        ];
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut cases = Vec::new();
        for file in FILES {
            let path = root.join("test-data/unit").join(file);
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            let mut name: Option<String> = None;
            let mut source: Vec<&str> = Vec::new();
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("[case ") {
                    name = Some(rest.trim_end_matches(']').to_owned());
                    source.clear();
                } else if line == "[out]" {
                    if let Some(case_name) = name.take() {
                        cases.push((case_name, source.join("\n")));
                    }
                    source.clear();
                } else if name.is_some() {
                    source.push(line);
                }
            }
            assert!(name.is_none(), "case without [out] in {file}");
        }
        cases
    }

    #[test]
    fn expression_enum_matches_direct_writer_for_tstrings_and_debug_fstrings() {
        let source = "value = t\"a{x!r:>{width}}b\"\ndebug = f\"{value=}\"\n";
        assert!(
            parse_module(source).is_ok(),
            "corpus source must stay parsable"
        );
        assert_eq!(
            serialize_source_bytes(source, false, false),
            serialize_source_bytes(source, false, true)
        );
    }

    #[test]
    fn statement_enum_matches_direct_writer_for_language_surface() {
        let source = "\
import os as _os
from collections import OrderedDict

def deco(arg):
    return lambda f: f

@deco
async def f(a, /, b: int = 1, *args, c, **kwargs) -> str:
    global G
    del a
    async with open('x') as fp:
        await fp.read()
    async for item in fp:
        pass
    else:
        return 'empty'
    while c:
        break
    else:
        continue
    try:
        raise ValueError('x') from None
    except ValueError as exc:
        assert exc, 'boom'
    except (TypeError, KeyError):
        pass
    else:
        return
    finally:
        nonlocal_target = 1

class C(Base, metaclass=Meta, **kw):
    \"\"\"doc\"\"\"
    field: int = 1
    other: str
    x = y = 1
    x += 2
    type Alias[T] = list[T]

match value:
    case [1, *rest]:
        pass
    case {'k': v, **extra} if v:
        pass
    case C(a, b=1) | D(a=2):
        pass
    case None | True:
        pass
    case _:
        pass
";

        for legacy_exprs in [false, true] {
            assert_eq!(
                serialize_source_bytes(source, false, legacy_exprs),
                serialize_source_bytes(source, true, legacy_exprs),
                "statement enum/direct writer byte parity failed (legacy_exprs={legacy_exprs})"
            );
        }
    }

    #[test]
    fn corpus_expression_enum_matches_direct_writer_bytes() {
        let cases = read_native_parser_cases();
        assert!(
            cases.len() >= 250,
            "native-parser corpus shrank: {} cases",
            cases.len()
        );
        let mut checked = 0;
        let mut skipped = 0;
        for (name, source) in &cases {
            if parse_module(source).is_err() {
                skipped += 1;
                continue;
            }
            let enum_bytes = serialize_source_bytes(source, false, false);
            let legacy_bytes = serialize_source_bytes(source, false, true);
            assert_eq!(
                enum_bytes, legacy_bytes,
                "expression enum/direct writer byte parity failed for case {name}"
            );
            checked += 1;
        }
        assert!(checked >= 230, "too few parsable corpus cases: {checked}");
        assert!(skipped <= 20, "too many unparsable corpus cases: {skipped}");
    }

    #[test]
    fn corpus_statement_enum_matches_direct_writer_bytes() {
        let cases = read_native_parser_cases();
        assert!(
            cases.len() >= 250,
            "native-parser corpus shrank: {} cases",
            cases.len()
        );
        let mut checked = 0;
        let mut skipped = 0;
        for (name, source) in &cases {
            if parse_module(source).is_err() {
                skipped += 1;
                continue;
            }
            for legacy_exprs in [false, true] {
                let enum_bytes = serialize_source_bytes(source, false, legacy_exprs);
                let legacy_bytes = serialize_source_bytes(source, true, legacy_exprs);
                assert_eq!(
                    enum_bytes, legacy_bytes,
                    "statement enum/direct writer byte parity failed for case {name} \
                     (legacy_exprs={legacy_exprs})"
                );
            }
            checked += 1;
        }
        assert!(checked >= 230, "too few parsable corpus cases: {checked}");
        assert!(skipped <= 20, "too many unparsable corpus cases: {skipped}");
    }

    #[test]
    fn statement_enum_parity_with_parser_options() {
        let sources = [
            "def f():\n    \"\"\"doc\"\"\"\n    import a.b\n    return 1\n",
            "from foo import (x, y as z)\nfrom foo import *\nimport foo, foo.bar as fb\n",
            "class C:\n    \"\"\"doc\"\"\"\n    x: int = 1\n    def m(self):\n        return 2\n",
        ];
        for source in sources {
            for legacy_exprs in [false, true] {
                for custom_typing_module in [None, Some("foo")] {
                    for skip_function_bodies in [false, true] {
                        for include_docstrings in [false, true] {
                            assert_eq!(
                                serialize_source_bytes_ex(
                                    source,
                                    false,
                                    legacy_exprs,
                                    skip_function_bodies,
                                    include_docstrings,
                                    custom_typing_module,
                                ),
                                serialize_source_bytes_ex(
                                    source,
                                    true,
                                    legacy_exprs,
                                    skip_function_bodies,
                                    include_docstrings,
                                    custom_typing_module,
                                ),
                                "statement enum parity failed for source {source:?} \
                                 (legacy_exprs={legacy_exprs}, skip_bodies={skip_function_bodies}, \
                                 docstrings={include_docstrings}, custom_typing={custom_typing_module:?})"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Frozen wire bytes for a statement surface covering import, def, an
    /// if/elif/else chain, try/except-as/finally, and match with sequence,
    /// star, and wildcard patterns. Both writers must reproduce them.
    #[test]
    fn statement_pattern_wire_golden_blob_is_stable() {
        let source = "\
import os

def f(x):
    if x:
        try:
            pass
        except ValueError as e:
            pass
        finally:
            pass
    elif y:
        pass
    else:
        pass
    match x:
        case [1, *rest]:
            pass
        case _:
            pass
";
        let expected: &[u8] = &[
            3, 24, 164, 3, 22, 4, 24, 111, 115, 0, 152, 22, 20, 20, 38, 3, 22, 255, 179, 4, 22,
            102, 0, 20, 22, 4, 22, 120, 3, 20, 0, 0, 0, 152, 26, 32, 20, 22, 171, 20, 24, 0, 168,
            162, 4, 22, 120, 152, 28, 34, 20, 22, 255, 171, 20, 22, 0, 201, 171, 20, 22, 0, 180,
            152, 32, 44, 20, 28, 255, 255, 3, 22, 1, 162, 4, 40, 86, 97, 108, 117, 101, 69, 114,
            114, 111, 114, 152, 34, 50, 20, 40, 255, 1, 4, 22, 101, 152, 34, 78, 20, 22, 171, 20,
            22, 0, 180, 152, 36, 44, 20, 28, 255, 255, 0, 1, 171, 20, 22, 0, 180, 152, 40, 44, 20,
            28, 255, 255, 0, 152, 30, 36, 30, 36, 255, 255, 3, 22, 162, 4, 22, 121, 152, 42, 38,
            20, 22, 255, 171, 20, 22, 0, 180, 152, 44, 36, 20, 28, 255, 255, 1, 171, 20, 22, 0,
            180, 152, 48, 36, 20, 28, 255, 255, 152, 28, 28, 40, 36, 255, 216, 162, 4, 22, 120,
            152, 50, 40, 20, 22, 255, 3, 24, 221, 3, 24, 219, 167, 3, 22, 152, 52, 48, 20, 22, 255,
            152, 52, 48, 20, 22, 255, 222, 1, 4, 28, 114, 101, 115, 116, 152, 52, 56, 20, 28, 152,
            52, 54, 20, 30, 255, 152, 52, 46, 20, 40, 255, 0, 171, 20, 22, 0, 180, 152, 54, 44, 20,
            28, 255, 255, 217, 0, 0, 152, 56, 46, 20, 22, 255, 0, 171, 20, 22, 0, 180, 152, 58, 44,
            20, 28, 255, 255, 152, 50, 28, 28, 44, 255, 255, 0, 0, 0, 152, 26, 20, 52, 52, 255,
        ];
        assert_eq!(
            serialize_source_bytes(source, true, true),
            expected,
            "frozen legacy writer drifted from the golden blob"
        );
        assert_eq!(
            serialize_source_bytes(source, false, false),
            expected,
            "statement enum writer emitted different bytes than the golden blob"
        );
    }

    #[test]
    fn func_def_docstring_field_only_when_enabled() {
        let source = "def f():\n    \"d\"\n";
        let with_doc = serialize_test_source(source, true, None);
        let without_doc = serialize_test_source(source, false, None);
        // Layout: LITERAL_INT len, FUNC_DEF_STMT, LITERAL_STR len name.
        let name_end = 6;
        assert_eq!(with_doc[..name_end], without_doc[..name_end]);
        // Present flag, value, corrupted flag.
        assert_eq!(
            with_doc[name_end..name_end + 5],
            [1, LITERAL_STR, 22, b'd', 0]
        );
        // Without the option only the absent flag is written.
        assert_eq!(without_doc[name_end], 0);
        assert_eq!(with_doc[name_end + 5..], without_doc[name_end + 1..]);
    }

    #[test]
    fn class_def_docstring_field_only_when_enabled() {
        let source = "class C:\n    \"d\"\n";
        let with_doc = serialize_test_source(source, true, None);
        let without_doc = serialize_test_source(source, false, None);
        let name_end = 6;
        assert_eq!(with_doc[..name_end], without_doc[..name_end]);
        assert_eq!(
            with_doc[name_end..name_end + 5],
            [1, LITERAL_STR, 22, b'd', 0]
        );
        assert_eq!(without_doc[name_end], 0);
        assert_eq!(with_doc[name_end + 5..], without_doc[name_end + 1..]);
    }

    #[test]
    fn docstring_surrogate_repair_data_written() {
        let bytes = serialize_test_source("class C:\n    \"\\ud800\"\n", true, None);
        // present flag, lossy U+FFFD value (len 3), corrupted flag, raw token.
        let marker = [1, LITERAL_STR, 26, 0xEF, 0xBF, 0xBD, 1, LITERAL_STR];
        assert!(bytes.windows(marker.len()).any(|window| window == marker));
    }

    #[test]
    fn custom_typing_module_translates_import() {
        let translated = serialize_test_source("import foo\n", false, Some("foo"));
        // IMPORT, alias count, module "typing", implicit asname "foo".
        let mut expected = vec![LITERAL_INT, 22, IMPORT, LITERAL_INT, 22, LITERAL_STR, 32];
        expected.extend_from_slice(b"typing");
        expected.extend_from_slice(&[1, LITERAL_STR, 26]);
        expected.extend_from_slice(b"foo");
        assert_eq!(&translated[..expected.len()], expected.as_slice());
    }

    #[test]
    fn custom_typing_module_translates_import_from() {
        let translated = serialize_test_source("from foo import X\n", false, Some("foo"));
        let mut expected = vec![
            LITERAL_INT,
            22,
            IMPORT_FROM,
            LITERAL_INT,
            20,
            LITERAL_STR,
            32,
        ];
        expected.extend_from_slice(b"typing");
        expected.extend_from_slice(&[LITERAL_INT, 22, LITERAL_STR, 22]);
        expected.extend_from_slice(b"X");
        assert_eq!(&translated[..expected.len()], expected.as_slice());
    }

    #[test]
    fn custom_typing_module_leaves_other_imports_alone() {
        let raw = serialize_test_source("import foo.bar\n", false, Some("foo"));
        let plain = serialize_test_source("import foo.bar\n", false, None);
        assert_eq!(raw, plain);
    }

    #[test]
    fn serializes_trivial_call_like_existing_binary_contract() {
        let suite = parse_module("print('hello')").unwrap().into_suite();
        let (bytes, imports, errors, _) = serialize_suite(
            &suite,
            "print('hello')",
            (3, 10),
            String::new(),
            Vec::new(),
            Vec::new(),
            false,
            HashMap::new(),
            false,
            None,
        )
        .unwrap();
        assert!(imports.is_empty());
        assert!(errors.is_empty());
        // Blob regenerated from actual writer output; it pins the STR_EXPR wire
        // contract including the corrupted-surrogate flag byte, which stays in
        // lockstep with the STR_EXPR reader arm in mypy/nativeparse.py.
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
                0, // corrupted-surrogate flag byte; read by nativeparse.py
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
                LOCATION,
                22,
                20,
                20,
                48,
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

    #[test]
    fn test_ast_tags_defined() {
        assert_eq!(EXPR_STMT, 160);
        assert_eq!(CALL_EXPR, 161);
        assert_eq!(NAME_EXPR, 162);
        assert_eq!(STR_EXPR, 163);
        assert_eq!(RETURN_STMT, 175);
    }
}
