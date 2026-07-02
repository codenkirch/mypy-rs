use pyo3::exceptions::{PyNotImplementedError, PyUnicodeDecodeError};
use pyo3::prelude::*;
use rustpython_parser::{ast, Parse};
use sha2::{Digest, Sha256};
use std::fs;

const LITERAL_NONE: u8 = 2;
const LITERAL_INT: u8 = 3;
const LITERAL_STR: u8 = 4;
const LIST_GEN: u8 = 20;
const LIST_INT: u8 = 21;
const LOCATION: u8 = 152;
const END_TAG: u8 = 255;

const EXPR_STMT: u8 = 160;
const CALL_EXPR: u8 = 161;
const NAME_EXPR: u8 = 162;
const STR_EXPR: u8 = 163;

const ARG_POS: i64 = 0;

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
        let mut value = ((value + 10) << 1) as u64;
        while value >= 0x80 {
            self.bytes.push((value as u8) | 0x80);
            value >>= 7;
        }
        self.bytes.push(value as u8);
    }

    fn int(&mut self, value: i64) {
        self.tag(LITERAL_INT);
        self.bare_int(value);
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
}

struct Serializer<'a> {
    writer: Writer,
    line_starts: Vec<usize>,
    source: &'a str,
}

impl<'a> Serializer<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            writer: Writer::default(),
            line_starts: line_starts(source),
            source,
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        self.writer.into_bytes()
    }

    fn loc<T: ast::Ranged>(&self, node: &T) -> SourceLocation {
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
    let _ = (
        skip_function_bodies,
        python_version,
        platform,
        always_true,
        always_false,
        cache_version,
    );
    let source = read_source(py, source, fnam)?;
    let suite = ast::Suite::parse(&source, fnam).map_err(to_parse_error)?;
    let ast_bytes = serialize_suite(&suite, &source)?;
    let import_bytes = serialize_empty_imports();
    let data = pyo3::types::PyDict::new(py);
    data.set_item("is_partial_package", false)?;
    data.set_item("uses_template_strings", false)?;
    data.set_item("mypy_ignores", Vec::<(i64, Vec<String>)>::new())?;
    data.set_item("source_hash", source_hash(&source))?;
    data.set_item("mypy_comments", Vec::<(i64, String)>::new())?;
    Ok((
        pyo3::types::PyBytes::new(py, &ast_bytes),
        Vec::<PyObject>::new(),
        Vec::<(i64, Vec<String>)>::new(),
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

fn to_parse_error(error: rustpython_parser::ParseError) -> PyErr {
    pyo3::exceptions::PySyntaxError::new_err(error.to_string())
}

fn serialize_suite(suite: &ast::Suite, source: &str) -> PyResult<Vec<u8>> {
    let mut serializer = Serializer::new(source);
    serializer.writer.int(suite.len() as i64);
    for statement in suite {
        serialize_stmt(&mut serializer, statement)?;
    }
    Ok(serializer.into_bytes())
}

fn serialize_empty_imports() -> Vec<u8> {
    let mut writer = Writer::default();
    writer.bare_int(0);
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
        _ => Err(PyNotImplementedError::new_err(format!(
            "mypy in-tree Rust parser does not serialize this statement yet: {statement:?}"
        ))),
    }
}

fn serialize_expr(serializer: &mut Serializer<'_>, expression: &ast::Expr) -> PyResult<()> {
    match expression {
        ast::Expr::Call(call) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(CALL_EXPR);
            serialize_expr(serializer, &call.func)?;
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(call.args.len() as i64);
            for arg in &call.args {
                serialize_expr(serializer, arg)?;
            }
            serializer.writer.int_list(&vec![ARG_POS; call.args.len()]);
            serializer.writer.opt_str_list(&vec![None; call.args.len()]);
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Name(name) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(NAME_EXPR);
            serializer.writer.string(name.id.as_str());
            serializer.writer.loc(&loc);
            serializer.writer.tag(END_TAG);
            Ok(())
        }
        ast::Expr::Constant(constant) => match &constant.value {
            ast::Constant::Str(value) => {
                let loc = serializer.loc(expression);
                serializer.writer.tag(STR_EXPR);
                serializer.writer.string(value);
                serializer.writer.loc(&loc);
                serializer.writer.tag(END_TAG);
                Ok(())
            }
            _ => Err(PyNotImplementedError::new_err(format!(
                "mypy in-tree Rust parser does not serialize this constant yet: {constant:?}"
            ))),
        },
        _ => Err(PyNotImplementedError::new_err(format!(
            "mypy in-tree Rust parser does not serialize this expression yet: {expression:?}"
        ))),
    }
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
        let suite = ast::Suite::parse("print('hello')", "test.py").unwrap();
        let bytes = serialize_suite(&suite, "print('hello')").unwrap();
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
}
