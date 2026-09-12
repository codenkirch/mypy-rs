//! Typed expression records for the native parser wire format.
//!
//! `ExprNode` mirrors the old `serialize_expr` arms one-to-one: one variant
//! per expression wire tag with the exact fields the direct writer emitted.
//! `build_expr` converts a ruff expression into the enum, `ast_writer` turns
//! the enum back into bytes, and the wire format stays byte-identical (no
//! `AST_WIRE_VERSION` bump).

use ruff_python_ast as ast;

use crate::{
    bool_op_index, comparison_index, escaped_bytes, has_surrogate_escape, operator_index,
    string_literal_parts, unary_op_index, Serializer, SourceLocation, ARG_NAMED, ARG_POS, ARG_STAR,
    ARG_STAR2,
};

use pyo3::exceptions::PyNotImplementedError;
use pyo3::prelude::*;

/// A complete expression record tree in wire-field form.
#[derive(Debug, Clone)]
pub(crate) enum ExprNode {
    Call {
        func: Box<ExprNode>,
        args: Vec<ExprNode>,
        arg_kinds: Vec<i64>,
        arg_names: Vec<Option<String>>,
        loc: SourceLocation,
    },
    Name {
        name: String,
        loc: SourceLocation,
    },
    Ellipsis {
        loc: SourceLocation,
    },
    Compare {
        left: Box<ExprNode>,
        ops: Vec<i64>,
        comparators: Vec<ExprNode>,
        loc: SourceLocation,
    },
    BoolOp {
        op: i64,
        values: Vec<ExprNode>,
        loc: SourceLocation,
    },
    UnaryOp {
        op: i64,
        operand: Box<ExprNode>,
        loc: SourceLocation,
    },
    Await {
        value: Box<ExprNode>,
        loc: SourceLocation,
    },
    Yield {
        value: Option<Box<ExprNode>>,
        loc: SourceLocation,
    },
    YieldFrom {
        value: Box<ExprNode>,
        loc: SourceLocation,
    },
    FString {
        parts: Vec<FStringPartNode>,
        loc: SourceLocation,
    },
    TString {
        items: Vec<TStringItemNode>,
        loc: SourceLocation,
    },
    Conditional {
        body: Box<ExprNode>,
        test: Box<ExprNode>,
        orelse: Box<ExprNode>,
        loc: SourceLocation,
    },
    Assignment {
        target: Box<ExprNode>,
        value: Box<ExprNode>,
        loc: SourceLocation,
    },
    /// Lambda parameter records stay a captured byte payload for now: the
    /// parameter list is a def-family sub-record (G0.4), not an expression.
    Lambda {
        parameters: Vec<u8>,
        body: Box<ExprNode>,
        body_loc: SourceLocation,
        loc: SourceLocation,
    },
    Starred {
        value: Box<ExprNode>,
        loc: SourceLocation,
    },
    Member {
        value: Box<ExprNode>,
        attr: String,
        loc: SourceLocation,
    },
    /// `OP_EXPR` carries no location on the wire; consumers restamp from the
    /// operand bounds.
    BinaryOp {
        op: i64,
        left: Box<ExprNode>,
        right: Box<ExprNode>,
    },
    Int {
        value: i64,
        loc: SourceLocation,
    },
    BigInt {
        value: String,
        loc: SourceLocation,
    },
    Float {
        value: f64,
        loc: SourceLocation,
    },
    Complex {
        real: f64,
        imag: f64,
        loc: SourceLocation,
    },
    Tuple {
        items: Vec<ExprNode>,
        loc: SourceLocation,
    },
    List {
        items: Vec<ExprNode>,
        loc: SourceLocation,
    },
    ListComp {
        elt: Box<ExprNode>,
        generators: Vec<ComprehensionNode>,
        loc: SourceLocation,
    },
    Set {
        items: Vec<ExprNode>,
        loc: SourceLocation,
    },
    SetComp {
        elt: Box<ExprNode>,
        generators: Vec<ComprehensionNode>,
        loc: SourceLocation,
    },
    Dict {
        items: Vec<DictItemNode>,
        loc: SourceLocation,
    },
    DictComp {
        key: Box<ExprNode>,
        value: Box<ExprNode>,
        generators: Vec<ComprehensionNode>,
        loc: SourceLocation,
    },
    Generator {
        elt: Box<ExprNode>,
        generators: Vec<ComprehensionNode>,
        loc: SourceLocation,
    },
    StringLiteral {
        value: String,
        corrupted: bool,
        raw: Option<String>,
        loc: SourceLocation,
    },
    Bytes {
        value: String,
        loc: SourceLocation,
    },
    Subscript {
        value: Box<ExprNode>,
        slice: Box<ExprNode>,
        loc: SourceLocation,
    },
    Slice {
        lower: Option<Box<ExprNode>>,
        upper: Option<Box<ExprNode>>,
        step: Option<Box<ExprNode>>,
        loc: SourceLocation,
    },
}

/// One top-level f-string part: an outer literal or an item list.
#[derive(Debug, Clone)]
pub(crate) enum FStringPartNode {
    Literal {
        value: String,
        corrupted: bool,
        raw: Option<String>,
        loc: SourceLocation,
    },
    Items {
        items: Vec<FStringItemNode>,
    },
}

/// One element of an f-string item list.
#[derive(Debug, Clone)]
pub(crate) enum FStringItemNode {
    Literal {
        value: String,
        corrupted: bool,
        raw: Option<String>,
        loc: SourceLocation,
    },
    Interpolation {
        debug: Option<DebugTextNode>,
        expression: Box<ExprNode>,
        conversion: Option<String>,
        format_spec: Option<FStringFormatSpecNode>,
    },
}

/// The debug-form literal a `{x=}` interpolation emits before its record.
#[derive(Debug, Clone)]
pub(crate) struct DebugTextNode {
    pub(crate) text: String,
    pub(crate) loc: SourceLocation,
}

/// The `<items> loc` tail of an f-string/t-string format spec.
#[derive(Debug, Clone)]
pub(crate) struct FStringFormatSpecNode {
    pub(crate) items: Vec<FStringItemNode>,
    pub(crate) loc: SourceLocation,
}

/// One flattened t-string element.
#[derive(Debug, Clone)]
pub(crate) enum TStringItemNode {
    Interpolation {
        expression: Box<ExprNode>,
        source: String,
        conversion: Option<String>,
        format_spec: Option<FStringFormatSpecNode>,
    },
    Literal {
        value: String,
        corrupted: bool,
        raw: Option<String>,
        loc: SourceLocation,
    },
}

/// One comprehension generator, in wire field order.
#[derive(Debug, Clone)]
pub(crate) struct ComprehensionNode {
    pub(crate) target: ExprNode,
    pub(crate) iter: ExprNode,
    pub(crate) ifs: Vec<ExprNode>,
    pub(crate) is_async: bool,
}

/// One dict entry; `key` is `None` for a `**` unpack item.
#[derive(Debug, Clone)]
pub(crate) struct DictItemNode {
    pub(crate) key: Option<ExprNode>,
    pub(crate) value: ExprNode,
}

/// The wire length of an f-string item list, including the debug literals
/// that `{x=}` interpolations emit as standalone items.
pub(crate) fn f_string_items_wire_len(items: &[FStringItemNode]) -> usize {
    items.len()
        + items
            .iter()
            .filter(|item| matches!(item, FStringItemNode::Interpolation { debug: Some(_), .. }))
            .count()
}

/// Build the wire record for an expression.
pub(crate) fn build_expr(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
) -> PyResult<ExprNode> {
    Ok(match expression {
        ast::Expr::Call(call) => {
            let loc = serializer.loc(expression);
            let func = Box::new(build_expr(serializer, &call.func)?);
            let arg_count = call.arguments.args.len() + call.arguments.keywords.len();
            let mut args = Vec::with_capacity(arg_count);
            let mut arg_kinds = Vec::with_capacity(arg_count);
            let mut arg_names = Vec::with_capacity(arg_count);
            for arg in &call.arguments.args {
                let (value, kind): (&ast::Expr, i64) = match arg {
                    ast::Expr::Starred(starred) => (&starred.value, ARG_STAR),
                    other => (other, ARG_POS),
                };
                args.push(build_expr(serializer, value)?);
                arg_kinds.push(kind);
                arg_names.push(None);
            }
            for keyword in &call.arguments.keywords {
                args.push(build_expr(serializer, &keyword.value)?);
                if let Some(name) = &keyword.arg {
                    arg_kinds.push(ARG_NAMED);
                    arg_names.push(Some(name.as_str().to_owned()));
                } else {
                    arg_kinds.push(ARG_STAR2);
                    arg_names.push(None);
                }
            }
            ExprNode::Call {
                func,
                args,
                arg_kinds,
                arg_names,
                loc,
            }
        }
        ast::Expr::Name(name) => {
            let loc = serializer.loc(expression);
            ExprNode::Name {
                name: name.id.as_str().to_owned(),
                loc,
            }
        }
        ast::Expr::NoneLiteral(_) => {
            let loc = serializer.loc(expression);
            ExprNode::Name {
                name: "None".to_owned(),
                loc,
            }
        }
        ast::Expr::BooleanLiteral(boolean) => {
            let loc = serializer.loc(expression);
            ExprNode::Name {
                name: if boolean.value { "True" } else { "False" }.to_owned(),
                loc,
            }
        }
        ast::Expr::EllipsisLiteral(_) => {
            let loc = serializer.loc(expression);
            ExprNode::Ellipsis { loc }
        }
        ast::Expr::Compare(compare) => {
            let loc = serializer.loc(expression);
            let left = Box::new(build_expr(serializer, &compare.left)?);
            let ops = compare
                .ops
                .iter()
                .map(|op| comparison_index(*op))
                .collect::<Vec<_>>();
            let mut comparators = Vec::with_capacity(compare.comparators.len());
            for comparator in &compare.comparators {
                comparators.push(build_expr(serializer, comparator)?);
            }
            ExprNode::Compare {
                left,
                ops,
                comparators,
                loc,
            }
        }
        ast::Expr::BoolOp(bool_op) => {
            let loc = serializer.loc(expression);
            let op = bool_op_index(bool_op.op);
            let mut values = Vec::with_capacity(bool_op.values.len());
            for value in &bool_op.values {
                values.push(build_expr(serializer, value)?);
            }
            ExprNode::BoolOp { op, values, loc }
        }
        ast::Expr::UnaryOp(unary_op) => {
            let loc = serializer.loc(expression);
            let op = unary_op_index(unary_op.op);
            let operand = Box::new(build_expr(serializer, &unary_op.operand)?);
            ExprNode::UnaryOp { op, operand, loc }
        }
        ast::Expr::Await(await_expr) => {
            let loc = serializer.loc(expression);
            let value = Box::new(build_expr(serializer, &await_expr.value)?);
            ExprNode::Await { value, loc }
        }
        ast::Expr::Yield(yield_expr) => {
            let loc = serializer.loc(expression);
            let value = match &yield_expr.value {
                Some(value) => Some(Box::new(build_expr(serializer, value)?)),
                None => None,
            };
            ExprNode::Yield { value, loc }
        }
        ast::Expr::YieldFrom(yield_from) => {
            let loc = serializer.loc(expression);
            let value = Box::new(build_expr(serializer, &yield_from.value)?);
            ExprNode::YieldFrom { value, loc }
        }
        ast::Expr::FString(f_string) => build_f_string(serializer, f_string)?,
        ast::Expr::TString(t_string) => build_t_string(serializer, t_string)?,
        ast::Expr::If(if_expr) => {
            let loc = serializer.loc(expression);
            let body = Box::new(build_expr(serializer, &if_expr.body)?);
            let test = Box::new(build_expr(serializer, &if_expr.test)?);
            let orelse = Box::new(build_expr(serializer, &if_expr.orelse)?);
            ExprNode::Conditional {
                body,
                test,
                orelse,
                loc,
            }
        }
        ast::Expr::Named(named) => {
            let loc = serializer.loc(expression);
            let target = Box::new(build_expr(serializer, &named.target)?);
            let value = Box::new(build_expr(serializer, &named.value)?);
            ExprNode::Assignment { target, value, loc }
        }
        ast::Expr::Lambda(lambda) => {
            let loc = serializer.loc(expression);
            let parameters = build_lambda_parameters(serializer, lambda)?;
            let body = Box::new(build_expr(serializer, &lambda.body)?);
            let body_loc = serializer.loc(&*lambda.body);
            ExprNode::Lambda {
                parameters,
                body,
                body_loc,
                loc,
            }
        }
        ast::Expr::Starred(starred) => {
            let loc = serializer.loc(expression);
            let value = Box::new(build_expr(serializer, &starred.value)?);
            ExprNode::Starred { value, loc }
        }
        ast::Expr::Attribute(attribute) => {
            let loc = serializer.loc(expression);
            let value = Box::new(build_expr(serializer, &attribute.value)?);
            ExprNode::Member {
                value,
                attr: attribute.attr.as_str().to_owned(),
                loc,
            }
        }
        ast::Expr::BinOp(bin_op) => {
            let op = operator_index(bin_op.op)?;
            let left = Box::new(build_expr(serializer, &bin_op.left)?);
            let right = Box::new(build_expr(serializer, &bin_op.right)?);
            ExprNode::BinaryOp { op, left, right }
        }
        ast::Expr::NumberLiteral(number) => {
            let loc = serializer.loc(expression);
            match &number.value {
                ast::Number::Int(value) => match value.as_i64() {
                    Some(value) => ExprNode::Int { value, loc },
                    None => ExprNode::BigInt {
                        value: value.to_string(),
                        loc,
                    },
                },
                ast::Number::Float(value) => ExprNode::Float { value: *value, loc },
                ast::Number::Complex { real, imag } => ExprNode::Complex {
                    real: *real,
                    imag: *imag,
                    loc,
                },
            }
        }
        ast::Expr::Tuple(tuple) => {
            let loc = serializer.loc(expression);
            let mut items = Vec::with_capacity(tuple.elts.len());
            for item in &tuple.elts {
                items.push(build_expr(serializer, item)?);
            }
            ExprNode::Tuple { items, loc }
        }
        ast::Expr::List(list) => {
            let loc = serializer.loc(expression);
            let mut items = Vec::with_capacity(list.elts.len());
            for item in &list.elts {
                items.push(build_expr(serializer, item)?);
            }
            ExprNode::List { items, loc }
        }
        ast::Expr::ListComp(list_comp) => {
            let loc = serializer.loc(expression);
            let elt = Box::new(build_expr(serializer, &list_comp.elt)?);
            let generators = build_comprehensions(serializer, &list_comp.generators)?;
            ExprNode::ListComp {
                elt,
                generators,
                loc,
            }
        }
        ast::Expr::Set(set) => {
            let loc = serializer.loc(expression);
            let mut items = Vec::with_capacity(set.elts.len());
            for item in &set.elts {
                items.push(build_expr(serializer, item)?);
            }
            ExprNode::Set { items, loc }
        }
        ast::Expr::SetComp(set_comp) => {
            let loc = serializer.loc(expression);
            let elt = Box::new(build_expr(serializer, &set_comp.elt)?);
            let generators = build_comprehensions(serializer, &set_comp.generators)?;
            ExprNode::SetComp {
                elt,
                generators,
                loc,
            }
        }
        ast::Expr::Dict(dict) => {
            let loc = serializer.loc(expression);
            let mut keys = Vec::with_capacity(dict.items.len());
            for item in &dict.items {
                keys.push(match &item.key {
                    Some(key) => Some(build_expr(serializer, key)?),
                    None => None,
                });
            }
            let mut values = Vec::with_capacity(dict.items.len());
            for item in &dict.items {
                values.push(build_expr(serializer, &item.value)?);
            }
            let items = keys
                .into_iter()
                .zip(values)
                .map(|(key, value)| DictItemNode { key, value })
                .collect();
            ExprNode::Dict { items, loc }
        }
        ast::Expr::DictComp(dict_comp) => {
            let loc = serializer.loc(expression);
            let Some(key) = &dict_comp.key else {
                return Err(PyNotImplementedError::new_err(
                    "mypy in-tree Rust parser does not serialize dict unpack comprehensions yet",
                ));
            };
            let key = Box::new(build_expr(serializer, key)?);
            let value = Box::new(build_expr(serializer, &dict_comp.value)?);
            let generators = build_comprehensions(serializer, &dict_comp.generators)?;
            ExprNode::DictComp {
                key,
                value,
                generators,
                loc,
            }
        }
        ast::Expr::Generator(generator) => {
            let loc = serializer.loc(expression);
            let elt = Box::new(build_expr(serializer, &generator.elt)?);
            let generators = build_comprehensions(serializer, &generator.generators)?;
            ExprNode::Generator {
                elt,
                generators,
                loc,
            }
        }
        ast::Expr::StringLiteral(string) => {
            let loc = serializer.loc(expression);
            let (value, corrupted, raw) = string_literal_parts(serializer.source, string);
            ExprNode::StringLiteral {
                value: value.to_owned(),
                corrupted,
                raw,
                loc,
            }
        }
        ast::Expr::BytesLiteral(bytes) => {
            let loc = serializer.loc(expression);
            ExprNode::Bytes {
                value: escaped_bytes(bytes.value.bytes()),
                loc,
            }
        }
        ast::Expr::Subscript(subscript) => {
            let loc = serializer.loc(expression);
            let value = Box::new(build_expr(serializer, &subscript.value)?);
            let slice = Box::new(build_expr(serializer, &subscript.slice)?);
            ExprNode::Subscript { value, slice, loc }
        }
        ast::Expr::Slice(slice) => {
            let loc = serializer.loc(expression);
            let lower = match &slice.lower {
                Some(lower) => Some(Box::new(build_expr(serializer, lower)?)),
                None => None,
            };
            let upper = match &slice.upper {
                Some(upper) => Some(Box::new(build_expr(serializer, upper)?)),
                None => None,
            };
            let step = match &slice.step {
                Some(step) => Some(Box::new(build_expr(serializer, step)?)),
                None => None,
            };
            ExprNode::Slice {
                lower,
                upper,
                step,
                loc,
            }
        }
        _ => {
            return Err(PyNotImplementedError::new_err(format!(
                "mypy in-tree Rust parser does not serialize this expression yet: {expression:?}"
            )))
        }
    })
}

/// Build an lvalue record. Lists become `TUPLE_EXPR` on the wire, matching the
/// old `serialize_lvalue` flattening; every other shape is a plain expression.
pub(crate) fn build_lvalue(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
) -> PyResult<ExprNode> {
    match expression {
        ast::Expr::Tuple(tuple) => {
            let loc = serializer.loc(expression);
            let mut items = Vec::with_capacity(tuple.elts.len());
            for item in &tuple.elts {
                items.push(build_lvalue(serializer, item)?);
            }
            Ok(ExprNode::Tuple { items, loc })
        }
        ast::Expr::List(list) => {
            let loc = serializer.loc(expression);
            let mut items = Vec::with_capacity(list.elts.len());
            for item in &list.elts {
                items.push(build_lvalue(serializer, item)?);
            }
            Ok(ExprNode::Tuple { items, loc })
        }
        ast::Expr::Starred(starred) => {
            let loc = serializer.loc(expression);
            let value = Box::new(build_lvalue(serializer, &starred.value)?);
            Ok(ExprNode::Starred { value, loc })
        }
        _ => build_expr(serializer, expression),
    }
}

fn build_lambda_parameters(
    serializer: &mut Serializer<'_>,
    lambda: &ast::ExprLambda,
) -> PyResult<Vec<u8>> {
    let saved = std::mem::take(&mut serializer.writer);
    serializer.lambda_depth += 1;
    let result = match &lambda.parameters {
        Some(parameters) => crate::serialize_parameters(serializer, parameters, None),
        None => {
            crate::serialize_empty_parameters(serializer);
            Ok(())
        }
    };
    serializer.lambda_depth -= 1;
    let captured = std::mem::replace(&mut serializer.writer, saved);
    result?;
    Ok(captured.into_bytes())
}

fn build_f_string(
    serializer: &mut Serializer<'_>,
    f_string: &ast::ExprFString,
) -> PyResult<ExprNode> {
    let loc = serializer.loc(f_string);
    let parts = f_string.value.as_slice();
    let mut built = Vec::with_capacity(parts.len());
    for part in parts {
        match part {
            ast::FStringPart::Literal(literal) => {
                let range = literal.range;
                let raw = &serializer.source[range.start().to_usize()..range.end().to_usize()];
                let corrupted = !literal.flags.prefix().is_raw() && has_surrogate_escape(raw);
                built.push(FStringPartNode::Literal {
                    value: literal.as_str().to_owned(),
                    corrupted,
                    raw: corrupted.then(|| raw.to_owned()),
                    loc: serializer.loc(literal),
                });
            }
            ast::FStringPart::FString(part) => {
                let items =
                    build_f_string_items(serializer, &part.elements, part.flags.prefix().is_raw())?;
                built.push(FStringPartNode::Items { items });
            }
        }
    }
    Ok(ExprNode::FString { parts: built, loc })
}

fn build_f_string_items(
    serializer: &mut Serializer<'_>,
    elements: &ast::InterpolatedStringElements,
    is_raw: bool,
) -> PyResult<Vec<FStringItemNode>> {
    let mut items = Vec::with_capacity(elements.len());
    for element in elements.iter() {
        match element {
            ast::InterpolatedStringElement::Literal(literal) => {
                let range = literal.range;
                let raw = &serializer.source[range.start().to_usize()..range.end().to_usize()];
                let corrupted = !is_raw && has_surrogate_escape(raw);
                items.push(FStringItemNode::Literal {
                    value: literal.value.to_string(),
                    corrupted,
                    raw: corrupted.then(|| raw.to_owned()),
                    loc: serializer.loc(literal),
                });
            }
            ast::InterpolatedStringElement::Interpolation(interpolation) => {
                let debug = interpolation
                    .debug_text
                    .as_ref()
                    .map(|debug_text| DebugTextNode {
                        text: debug_text.as_str().to_owned(),
                        loc: serializer.loc(interpolation),
                    });
                // CPython defaults !r only for the debug form without a spec.
                let conversion = if interpolation.debug_text.is_some()
                    && interpolation.conversion == ast::ConversionFlag::None
                    && interpolation.format_spec.is_none()
                {
                    Some("!r".to_owned())
                } else {
                    interpolation.conversion.to_char().map(|c| format!("!{c}"))
                };
                let expression = Box::new(build_expr(serializer, &interpolation.expression)?);
                let format_spec = match &interpolation.format_spec {
                    Some(spec) => Some(FStringFormatSpecNode {
                        items: build_f_string_items(serializer, &spec.elements, is_raw)?,
                        loc: serializer.loc(&**spec),
                    }),
                    None => None,
                };
                items.push(FStringItemNode::Interpolation {
                    debug,
                    expression,
                    conversion,
                    format_spec,
                });
            }
        }
    }
    Ok(items)
}

/// Build a PEP 701 template string (`ExprTString`).
///
/// Wire format (read by `nativeparse.py:read_expression` TSTRING_EXPR):
/// ```text
/// TSTRING_EXPR; int nparts;
/// for each part:
///   bool is_interpolation
///   if interpolation:
///     expr; str (source); bool has_conv; [str conv]; bool has_format_spec;
///     [fstring_items]
///   else:
///     str; bool corrupted; [str raw]
/// loc; END_TAG
/// ```
/// Matches `fastparse.visit_TemplateStr`, which uses the same item shape as
/// f-strings plus the raw interpolation source string (dropped by the
/// checker, used only for debugging).
fn build_t_string(
    serializer: &mut Serializer<'_>,
    t_string: &ast::ExprTString,
) -> PyResult<ExprNode> {
    serializer.uses_template_strings = true;
    let loc = serializer.loc(t_string);
    let nparts: usize = t_string
        .value
        .as_slice()
        .iter()
        .map(|t| t.elements.iter().count())
        .sum();
    let mut items = Vec::with_capacity(nparts);
    for tstring in t_string.value.as_slice() {
        let is_raw = tstring.flags.prefix().is_raw();
        for element in tstring.elements.iter() {
            match element {
                ast::InterpolatedStringElement::Interpolation(interpolation) => {
                    let expression = Box::new(build_expr(serializer, &interpolation.expression)?);
                    let range = interpolation.range;
                    let source = serializer.source
                        [range.start().to_usize()..range.end().to_usize()]
                        .to_owned();
                    let conversion = interpolation.conversion.to_char().map(|c| format!("!{c}"));
                    let format_spec = match &interpolation.format_spec {
                        Some(spec) => Some(FStringFormatSpecNode {
                            items: build_f_string_items(serializer, &spec.elements, is_raw)?,
                            loc: serializer.loc(&**spec),
                        }),
                        None => None,
                    };
                    items.push(TStringItemNode::Interpolation {
                        expression,
                        source,
                        conversion,
                        format_spec,
                    });
                }
                ast::InterpolatedStringElement::Literal(literal) => {
                    let range = literal.range;
                    let raw = &serializer.source[range.start().to_usize()..range.end().to_usize()];
                    let corrupted = !is_raw && has_surrogate_escape(raw);
                    items.push(TStringItemNode::Literal {
                        value: literal.value.to_string(),
                        corrupted,
                        raw: corrupted.then(|| raw.to_owned()),
                        loc: serializer.loc(literal),
                    });
                }
            }
        }
    }
    Ok(ExprNode::TString { items, loc })
}

fn build_comprehensions(
    serializer: &mut Serializer<'_>,
    generators: &[ast::Comprehension],
) -> PyResult<Vec<ComprehensionNode>> {
    // Column-wise build order keeps diagnostic side effects identical to the
    // old writer, which emitted every target, then every iter, then every if.
    let mut targets = Vec::with_capacity(generators.len());
    for generator in generators {
        targets.push(build_lvalue(serializer, &generator.target)?);
    }
    let mut iters = Vec::with_capacity(generators.len());
    for generator in generators {
        iters.push(build_expr(serializer, &generator.iter)?);
    }
    let mut ifs = Vec::with_capacity(generators.len());
    for generator in generators {
        let mut built = Vec::with_capacity(generator.ifs.len());
        for condition in &generator.ifs {
            built.push(build_expr(serializer, condition)?);
        }
        ifs.push(built);
    }
    Ok(targets
        .into_iter()
        .zip(iters)
        .zip(ifs)
        .zip(generators)
        .map(|(((target, iter), ifs), generator)| ComprehensionNode {
            target,
            iter,
            ifs,
            is_async: generator.is_async,
        })
        .collect())
}
