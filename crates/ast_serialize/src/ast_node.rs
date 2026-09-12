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

/// A complete statement record tree in wire-field form.
///
/// One variant per statement wire tag. Variants whose wire record embeds a
/// not-yet-enumed family (type annotations, type params, parameter lists)
/// carry those sub-records as captured bytes and re-emit them verbatim.
#[derive(Debug, Clone)]
pub(crate) enum StmtNode {
    ExprStmt {
        value: ExprNode,
        loc: SourceLocation,
    },
    /// Covers `StmtAssign` (many targets, optional type comment, `new_syntax`
    /// false) and `StmtAnnAssign` (one target, always an annotation,
    /// `new_syntax` true); both share `ASSIGNMENT_STMT` on the wire.
    Assignment {
        targets: Vec<ExprNode>,
        /// `None` emits the `TEMP_NODE` placeholder (annotation without value).
        value: Option<ExprNode>,
        /// Captured type record; `None` emits the absent flag only.
        annotation: Option<Vec<u8>>,
        new_syntax: bool,
        loc: SourceLocation,
    },
    OperatorAssignment {
        op: String,
        target: ExprNode,
        value: ExprNode,
        loc: SourceLocation,
    },
    ReturnStmt {
        value: Option<ExprNode>,
        loc: SourceLocation,
    },
    RaiseStmt {
        exc: Option<ExprNode>,
        cause: Option<ExprNode>,
        loc: SourceLocation,
    },
    AssertStmt {
        test: ExprNode,
        msg: Option<ExprNode>,
        loc: SourceLocation,
    },
    DeleteStmt {
        targets: Vec<ExprNode>,
        loc: SourceLocation,
    },
    FuncDef {
        name: String,
        docstring: Option<DocstringNode>,
        /// Captured `LIST_GEN` parameter records (G0.5 scope).
        parameters: Vec<u8>,
        body: BlockNode,
        is_async: bool,
        type_params: Option<Vec<u8>>,
        returns: Option<Vec<u8>>,
        loc: SourceLocation,
    },
    DecoratedFuncDef {
        decorators: Vec<ExprNode>,
        line: i64,
        column: i64,
        function: Box<StmtNode>,
    },
    ClassDef {
        name: String,
        docstring: Option<DocstringNode>,
        body: BlockNode,
        bases: Vec<ExprNode>,
        decorators: Vec<ExprNode>,
        type_params: Option<Vec<u8>>,
        keywords: Vec<(String, ExprNode)>,
        loc: SourceLocation,
    },
    IfStmt {
        test: ExprNode,
        body: BlockNode,
        elifs: Vec<IfClauseNode>,
        else_body: Option<BlockNode>,
        loc: SourceLocation,
    },
    ForStmt {
        target: ExprNode,
        iter: ExprNode,
        body: BlockNode,
        /// Optional block: an empty body emits no location.
        orelse: BlockNode,
        is_async: bool,
        type_comment: Option<Vec<u8>>,
        loc: SourceLocation,
    },
    WhileStmt {
        test: ExprNode,
        body: BlockNode,
        orelse: BlockNode,
        loc: SourceLocation,
    },
    WithStmt {
        items: Vec<WithItemNode>,
        body: BlockNode,
        is_async: bool,
        type_comment: Option<Vec<u8>>,
        loc: SourceLocation,
    },
    TryStmt {
        body: BlockNode,
        handlers: Vec<ExceptHandlerNode>,
        orelse: Option<BlockNode>,
        finalbody: Option<BlockNode>,
        is_star: bool,
        loc: SourceLocation,
    },
    MatchStmt {
        subject: ExprNode,
        cases: Vec<MatchCaseNode>,
        loc: SourceLocation,
    },
    TypeAliasStmt {
        name: ExprNode,
        type_params: Option<Vec<u8>>,
        value: ExprNode,
        loc: SourceLocation,
    },
    Import {
        names: Vec<(String, Option<String>)>,
        loc: SourceLocation,
        flags: i64,
    },
    ImportFrom {
        relative: i64,
        module: String,
        names: Vec<(String, Option<String>)>,
        loc: SourceLocation,
        flags: i64,
    },
    ImportAll {
        module: String,
        relative: i64,
        loc: SourceLocation,
        flags: i64,
    },
    GlobalDecl {
        names: Vec<String>,
        loc: SourceLocation,
    },
    NonlocalDecl {
        names: Vec<String>,
        loc: SourceLocation,
    },
    PassStmt {
        loc: SourceLocation,
    },
    BreakStmt {
        loc: SourceLocation,
    },
    ContinueStmt {
        loc: SourceLocation,
    },
}

/// A `BLOCK` record: the statement list plus the location an empty body
/// carries. `serialize_block` writes it when the body is empty; optional
/// blocks (`for`/`while` else, `try` else/finally) never write a location.
#[derive(Debug, Clone)]
pub(crate) struct BlockNode {
    pub(crate) body: Vec<StmtNode>,
    pub(crate) empty_loc: Option<SourceLocation>,
}

/// One `if`/`elif` clause body.
#[derive(Debug, Clone)]
pub(crate) struct IfClauseNode {
    pub(crate) test: ExprNode,
    pub(crate) body: BlockNode,
}

/// One `with` item.
#[derive(Debug, Clone)]
pub(crate) struct WithItemNode {
    pub(crate) context_expr: ExprNode,
    pub(crate) optional_vars: Option<ExprNode>,
}

/// One `except` handler, in wire field order.
#[derive(Debug, Clone)]
pub(crate) struct ExceptHandlerNode {
    pub(crate) type_: Option<ExprNode>,
    pub(crate) name: Option<(String, SourceLocation)>,
    pub(crate) body: BlockNode,
}

/// One `match` case.
#[derive(Debug, Clone)]
pub(crate) struct MatchCaseNode {
    pub(crate) pattern: PatternNode,
    pub(crate) guard: Option<ExprNode>,
    pub(crate) body: BlockNode,
}

/// The captured `ast.get_docstring(node, clean=False)` fields.
#[derive(Debug, Clone)]
pub(crate) struct DocstringNode {
    pub(crate) value: String,
    pub(crate) corrupted: bool,
    pub(crate) raw: Option<String>,
}

/// A complete pattern record tree in wire-field form.
#[derive(Debug, Clone)]
pub(crate) enum PatternNode {
    As {
        pattern: Option<Box<PatternNode>>,
        name: Option<(String, SourceLocation)>,
        loc: SourceLocation,
    },
    Or {
        patterns: Vec<PatternNode>,
        loc: SourceLocation,
    },
    Value {
        value: ExprNode,
        loc: SourceLocation,
    },
    Singleton {
        value: SingletonNode,
        loc: SourceLocation,
    },
    Sequence {
        patterns: Vec<PatternNode>,
        loc: SourceLocation,
    },
    Star {
        name: Option<(String, SourceLocation)>,
        loc: SourceLocation,
    },
    Mapping {
        entries: Vec<(ExprNode, PatternNode)>,
        rest: Option<(String, SourceLocation)>,
        loc: SourceLocation,
    },
    Class {
        cls: ExprNode,
        positionals: Vec<PatternNode>,
        keywords: Vec<(String, PatternNode)>,
        loc: SourceLocation,
    },
}

/// The `None`/`True`/`False` singleton payload.
#[derive(Debug, Clone)]
pub(crate) enum SingletonNode {
    None,
    Bool(bool),
}

/// Build the wire record for a statement. All serializer side effects
/// (imports metadata, parse errors, branch modes, function/class depth) run
/// here, in the same order the pre-enum direct writer emitted them.
pub(crate) fn build_stmt(
    serializer: &mut Serializer<'_>,
    statement: &ast::Stmt,
) -> PyResult<StmtNode> {
    Ok(match statement {
        ast::Stmt::Expr(expr) => {
            let loc = serializer.loc(statement);
            let value = build_expr(serializer, &expr.value)?;
            StmtNode::ExprStmt { value, loc }
        }
        ast::Stmt::Assign(assign) => {
            let loc = serializer.loc(statement);
            let mut targets = Vec::with_capacity(assign.targets.len());
            for target in &assign.targets {
                targets.push(build_lvalue(serializer, target)?);
            }
            let value = build_expr(serializer, &assign.value)?;
            let type_comment = serializer.type_comments.get(&loc.end_line).cloned();
            let annotation = build_type_comment_bytes(serializer, &loc, &type_comment)?;
            StmtNode::Assignment {
                targets,
                value: Some(value),
                annotation,
                new_syntax: false,
                loc,
            }
        }
        ast::Stmt::AnnAssign(assign) => {
            let loc = serializer.loc(assign);
            let target = build_lvalue(serializer, &assign.target)?;
            let value = match &assign.value {
                Some(value) => Some(build_expr(serializer, value)?),
                None => None,
            };
            let annotation = Some(crate::capture_bytes(serializer, |s| {
                crate::serialize_type(s, &assign.annotation)
            })?);
            StmtNode::Assignment {
                targets: vec![target],
                value,
                annotation,
                new_syntax: true,
                loc,
            }
        }
        ast::Stmt::AugAssign(assign) => {
            let loc = serializer.loc(statement);
            let target = build_lvalue(serializer, &assign.target)?;
            let value = build_expr(serializer, &assign.value)?;
            StmtNode::OperatorAssignment {
                op: crate::operator_string(assign.op).to_owned(),
                target,
                value,
                loc,
            }
        }
        ast::Stmt::Return(return_stmt) => {
            let loc = serializer.loc(statement);
            let value = match &return_stmt.value {
                Some(value) => Some(build_expr(serializer, value)?),
                None => None,
            };
            StmtNode::ReturnStmt { value, loc }
        }
        ast::Stmt::Raise(raise) => {
            let loc = serializer.loc(statement);
            let exc = match &raise.exc {
                Some(exc) => Some(build_expr(serializer, exc)?),
                None => None,
            };
            let cause = match &raise.cause {
                Some(cause) => Some(build_expr(serializer, cause)?),
                None => None,
            };
            StmtNode::RaiseStmt { exc, cause, loc }
        }
        ast::Stmt::Assert(assert_stmt) => {
            let loc = serializer.loc(statement);
            let test = build_expr(serializer, &assert_stmt.test)?;
            let msg = match &assert_stmt.msg {
                Some(message) => Some(build_expr(serializer, message)?),
                None => None,
            };
            StmtNode::AssertStmt { test, msg, loc }
        }
        ast::Stmt::Delete(delete) => {
            let loc = serializer.loc(statement);
            let mut targets = Vec::with_capacity(delete.targets.len());
            for target in &delete.targets {
                targets.push(build_expr(serializer, target)?);
            }
            StmtNode::DeleteStmt { targets, loc }
        }
        ast::Stmt::FunctionDef(function) => {
            if function.decorator_list.is_empty() {
                build_function_def(serializer, function)?
            } else {
                let loc = serializer.loc(function);
                let mut decorators = Vec::with_capacity(function.decorator_list.len());
                for decorator in &function.decorator_list {
                    decorators.push(build_expr(serializer, &decorator.expression)?);
                }
                let function = Box::new(build_function_def(serializer, function)?);
                StmtNode::DecoratedFuncDef {
                    decorators,
                    line: loc.line,
                    column: loc.column + 1,
                    function,
                }
            }
        }
        ast::Stmt::ClassDef(class_def) => build_class_def(serializer, class_def)?,
        ast::Stmt::If(if_stmt) => build_if_stmt(serializer, if_stmt)?,
        ast::Stmt::For(for_stmt) => {
            let loc = serializer.loc(for_stmt);
            let target = build_lvalue(serializer, &for_stmt.target)?;
            let iter = build_expr(serializer, &for_stmt.iter)?;
            let body = build_block(serializer, &for_stmt.body, &loc)?;
            let orelse = build_optional_block(serializer, &for_stmt.orelse)?;
            let comment = serializer.type_comments.get(&loc.line).cloned();
            let type_comment = build_type_comment_bytes(serializer, &loc, &comment)?;
            StmtNode::ForStmt {
                target,
                iter,
                body,
                orelse,
                is_async: for_stmt.is_async,
                type_comment,
                loc,
            }
        }
        ast::Stmt::While(while_stmt) => {
            let loc = serializer.loc(while_stmt);
            let test = build_expr(serializer, &while_stmt.test)?;
            let body = build_block(serializer, &while_stmt.body, &loc)?;
            let orelse = build_optional_block(serializer, &while_stmt.orelse)?;
            StmtNode::WhileStmt {
                test,
                body,
                orelse,
                loc,
            }
        }
        ast::Stmt::With(with_stmt) => {
            let loc = serializer.loc(with_stmt);
            let mut items = Vec::with_capacity(with_stmt.items.len());
            for item in &with_stmt.items {
                let context_expr = build_expr(serializer, &item.context_expr)?;
                let optional_vars = match &item.optional_vars {
                    Some(target) => Some(build_lvalue(serializer, target)?),
                    None => None,
                };
                items.push(WithItemNode {
                    context_expr,
                    optional_vars,
                });
            }
            let body = build_block(serializer, &with_stmt.body, &loc)?;
            let comment = serializer.type_comments.get(&loc.line).cloned();
            let type_comment = build_type_comment_bytes(serializer, &loc, &comment)?;
            StmtNode::WithStmt {
                items,
                body,
                is_async: with_stmt.is_async,
                type_comment,
                loc,
            }
        }
        ast::Stmt::Try(try_stmt) => {
            let loc = serializer.loc(try_stmt);
            let body = build_block(serializer, &try_stmt.body, &loc)?;
            let mut handler_types = Vec::with_capacity(try_stmt.handlers.len());
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                handler_types.push(match &handler.type_ {
                    Some(exc_type) => Some(build_expr(serializer, exc_type)?),
                    None => None,
                });
            }
            let mut handler_names = Vec::with_capacity(try_stmt.handlers.len());
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                handler_names.push(
                    handler
                        .name
                        .as_ref()
                        .map(|name| (name.as_str().to_owned(), serializer.loc(name))),
                );
            }
            let mut handlers = Vec::with_capacity(try_stmt.handlers.len());
            for ((handler, type_), name) in try_stmt
                .handlers
                .iter()
                .zip(handler_types)
                .zip(handler_names)
            {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                let handler_loc = serializer.loc(handler);
                let body = build_block(serializer, &handler.body, &handler_loc)?;
                handlers.push(ExceptHandlerNode { type_, name, body });
            }
            let orelse = if try_stmt.orelse.is_empty() {
                None
            } else {
                Some(build_optional_block(serializer, &try_stmt.orelse)?)
            };
            let finalbody = if try_stmt.finalbody.is_empty() {
                None
            } else {
                Some(build_optional_block(serializer, &try_stmt.finalbody)?)
            };
            StmtNode::TryStmt {
                body,
                handlers,
                orelse,
                finalbody,
                is_star: try_stmt.is_star,
                loc,
            }
        }
        ast::Stmt::Match(match_stmt) => {
            let loc = serializer.loc(match_stmt);
            let subject = build_expr(serializer, &match_stmt.subject)?;
            let mut cases = Vec::with_capacity(match_stmt.cases.len());
            for case in &match_stmt.cases {
                let pattern = build_pattern(serializer, &case.pattern)?;
                let guard = match &case.guard {
                    Some(guard) => Some(build_expr(serializer, guard)?),
                    None => None,
                };
                let case_loc = serializer.loc(case);
                let body = build_block(serializer, &case.body, &case_loc)?;
                cases.push(MatchCaseNode {
                    pattern,
                    guard,
                    body,
                });
            }
            StmtNode::MatchStmt {
                subject,
                cases,
                loc,
            }
        }
        ast::Stmt::TypeAlias(type_alias) => {
            let loc = serializer.loc(type_alias);
            let name = build_expr(serializer, &type_alias.name)?;
            let type_params = match &type_alias.type_params {
                Some(type_params) => Some(crate::capture_bytes(serializer, |s| {
                    crate::serialize_type_params(s, type_params)
                })?),
                None => None,
            };
            let value = build_expr(serializer, &type_alias.value)?;
            StmtNode::TypeAliasStmt {
                name,
                type_params,
                value,
                loc,
            }
        }
        ast::Stmt::Import(import) => {
            let loc = serializer.loc(import);
            let flags = serializer.imports.flags();
            let names = crate::translated_import_names(
                &import.names,
                serializer.custom_typing_module.as_deref(),
            );
            for (name, asname) in &names {
                serializer.imports.push(crate::ImportMetadata {
                    tag: crate::IMPORT_METADATA,
                    module: name.clone(),
                    relative: 0,
                    asname: asname.clone(),
                    names: Vec::new(),
                    loc: loc.clone(),
                    flags,
                });
            }
            StmtNode::Import { names, loc, flags }
        }
        ast::Stmt::ImportFrom(import) => {
            let loc = serializer.loc(import);
            let flags = serializer.imports.flags();
            let raw_module = import
                .module
                .as_ref()
                .map_or_else(String::new, |module| module.as_str().to_owned());
            let relative = i64::from(import.level);
            if import.names.len() == 1 && import.names[0].name.as_str() == "*" {
                serializer.imports.push(crate::ImportMetadata {
                    tag: crate::IMPORTALL_METADATA,
                    module: raw_module.clone(),
                    relative,
                    asname: None,
                    names: Vec::new(),
                    loc: loc.clone(),
                    flags,
                });
                StmtNode::ImportAll {
                    module: raw_module,
                    relative,
                    loc,
                    flags,
                }
            } else {
                let module = crate::translate_module_name(
                    &raw_module,
                    serializer.custom_typing_module.as_deref(),
                );
                let names = crate::import_alias_names(&import.names);
                serializer.imports.push(crate::ImportMetadata {
                    tag: crate::IMPORTFROM_METADATA,
                    module: module.clone(),
                    relative,
                    asname: None,
                    names: names.clone(),
                    loc: loc.clone(),
                    flags,
                });
                StmtNode::ImportFrom {
                    relative,
                    module,
                    names,
                    loc,
                    flags,
                }
            }
        }
        ast::Stmt::Global(global) => {
            let loc = serializer.loc(statement);
            StmtNode::GlobalDecl {
                names: global
                    .names
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect(),
                loc,
            }
        }
        ast::Stmt::Nonlocal(nonlocal) => {
            let loc = serializer.loc(statement);
            StmtNode::NonlocalDecl {
                names: nonlocal
                    .names
                    .iter()
                    .map(|name| name.as_str().to_owned())
                    .collect(),
                loc,
            }
        }
        ast::Stmt::Pass(_) => StmtNode::PassStmt {
            loc: serializer.loc(statement),
        },
        ast::Stmt::Break(_) => StmtNode::BreakStmt {
            loc: serializer.loc(statement),
        },
        ast::Stmt::Continue(_) => StmtNode::ContinueStmt {
            loc: serializer.loc(statement),
        },
        _ => {
            return Err(PyNotImplementedError::new_err(format!(
                "mypy in-tree Rust parser does not serialize this statement yet: {statement:?}"
            )))
        }
    })
}

/// Build the `# type:` payload for assignment/for/with positions: parse the
/// comment and capture the type record, or `None` when no comment applies.
fn build_type_comment_bytes(
    serializer: &mut Serializer<'_>,
    loc: &SourceLocation,
    comment: &Option<String>,
) -> PyResult<Option<Vec<u8>>> {
    match comment {
        Some(comment) => {
            let parsed_type = crate::parse_expression(comment)
                .map_err(crate::to_parse_error)?
                .into_expr();
            Ok(Some(crate::capture_bytes(serializer, |s| {
                crate::serialize_type_with_forced_loc(s, &parsed_type, loc)
            })?))
        }
        None => Ok(None),
    }
}

fn build_function_def(
    serializer: &mut Serializer<'_>,
    function: &ast::StmtFunctionDef,
) -> PyResult<StmtNode> {
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
    let docstring = serializer
        .docstring_fields(&function.body)
        .map(|(value, corrupted, raw)| DocstringNode {
            value,
            corrupted,
            raw,
        });
    let type_comment = crate::function_type_comment(serializer, function, &loc);
    let comment_arg_types = type_comment
        .as_ref()
        .and_then(|comment| comment.arg_types.as_deref());
    let parameters = crate::capture_bytes(serializer, |s| {
        crate::serialize_parameters(s, &function.parameters, comment_arg_types)
    })?;
    serializer.imports.enter_function();
    let body_result = if serializer.skip_function_bodies
        && !crate::function_body_must_be_preserved(&function.body, serializer.class_depth > 0)
    {
        build_stripped_block(serializer, &function.body, &loc)
    } else {
        build_block(serializer, &function.body, &loc)
    };
    serializer.imports.leave_function();
    let body = body_result?;
    let type_params = match &function.type_params {
        Some(type_params) => Some(crate::capture_bytes(serializer, |s| {
            crate::serialize_type_params(s, type_params)
        })?),
        None => None,
    };
    let comment_return_type = type_comment
        .as_ref()
        .map(|comment| comment.return_type.as_str());
    let returns = if let Some(return_type) = &function.returns {
        Some(crate::capture_bytes(serializer, |s| {
            crate::serialize_type(s, return_type)
        })?)
    } else if let Some(return_type) = comment_return_type {
        let parsed_type = crate::parse_expression(return_type).map_err(|err| {
            PyNotImplementedError::new_err(format!(
                "mypy in-tree Rust parser does not parse this function return type comment yet: {err}"
            ))
        })?;
        Some(crate::capture_bytes(serializer, |s| {
            crate::serialize_type_with_forced_loc(s, &parsed_type.into_expr(), &loc)
        })?)
    } else {
        None
    };
    Ok(StmtNode::FuncDef {
        name: function.name.as_str().to_owned(),
        docstring,
        parameters,
        body,
        is_async: function.is_async,
        type_params,
        returns,
        loc,
    })
}

fn build_class_def(
    serializer: &mut Serializer<'_>,
    class_def: &ast::StmtClassDef,
) -> PyResult<StmtNode> {
    let full_loc = serializer.loc(class_def);
    let name_loc = serializer.loc(&class_def.name);
    let loc = SourceLocation {
        line: name_loc.line,
        column: (name_loc.column - 6).max(0),
        end_line: full_loc.end_line,
        end_column: full_loc.end_column,
    };
    let docstring = serializer
        .docstring_fields(&class_def.body)
        .map(|(value, corrupted, raw)| DocstringNode {
            value,
            corrupted,
            raw,
        });
    serializer.class_depth += 1;
    let body_result = build_block(serializer, &class_def.body, &loc);
    serializer.class_depth -= 1;
    let body = body_result?;

    let mut bases = Vec::with_capacity(
        class_def
            .arguments
            .as_ref()
            .map_or(0, |arguments| arguments.args.len()),
    );
    if let Some(arguments) = &class_def.arguments {
        for base_expr in &arguments.args {
            bases.push(build_expr(serializer, base_expr)?);
        }
    }
    let mut decorators = Vec::with_capacity(class_def.decorator_list.len());
    for decorator in &class_def.decorator_list {
        decorators.push(build_expr(serializer, &decorator.expression)?);
    }
    let type_params = match &class_def.type_params {
        Some(type_params) => Some(crate::capture_bytes(serializer, |s| {
            crate::serialize_type_params(s, type_params)
        })?),
        None => None,
    };
    let mut keywords = Vec::new();
    if let Some(arguments) = &class_def.arguments {
        for keyword in &arguments.keywords {
            let Some(name) = &keyword.arg else {
                continue;
            };
            keywords.push((
                name.as_str().to_owned(),
                build_expr(serializer, &keyword.value)?,
            ));
        }
    }
    Ok(StmtNode::ClassDef {
        name: class_def.name.as_str().to_owned(),
        docstring,
        body,
        bases,
        decorators,
        type_params,
        keywords,
        loc,
    })
}

fn build_if_stmt(serializer: &mut Serializer<'_>, if_stmt: &ast::StmtIf) -> PyResult<StmtNode> {
    let loc = serializer.loc(if_stmt);
    let mut remaining_mode = crate::BranchMode::Normal;
    let condition = crate::evaluate_condition(serializer, &if_stmt.test);
    let test = build_expr(serializer, &if_stmt.test)?;
    let (body_mode, is_closed, next_remaining_mode) =
        crate::branch_modes_for_condition(condition, remaining_mode);
    let body = build_block_with_import_mode(serializer, &if_stmt.body, &loc, body_mode)?;
    remaining_mode = next_remaining_mode;
    let mut previous_branch_is_exhaustive = is_closed;

    let mut elifs = Vec::new();
    let mut else_clause: Option<&ast::ElifElseClause> = None;
    for clause in &if_stmt.elif_else_clauses {
        if let Some(clause_test) = &clause.test {
            let clause_loc = serializer.loc(clause);
            let condition = crate::evaluate_condition(serializer, clause_test);
            let (body_mode, is_closed, next_remaining_mode) = if previous_branch_is_exhaustive {
                (
                    crate::BranchMode::Unreachable,
                    true,
                    crate::BranchMode::Unreachable,
                )
            } else {
                crate::branch_modes_for_condition(condition, remaining_mode)
            };
            let test = build_expr(serializer, clause_test)?;
            let clause_body =
                build_block_with_import_mode(serializer, &clause.body, &clause_loc, body_mode)?;
            remaining_mode = next_remaining_mode;
            previous_branch_is_exhaustive = previous_branch_is_exhaustive || is_closed;
            elifs.push(IfClauseNode {
                test,
                body: clause_body,
            });
        } else {
            else_clause = Some(clause);
        }
    }

    let else_body = match else_clause {
        Some(clause) => {
            let clause_loc = serializer.loc(clause);
            let mode = if previous_branch_is_exhaustive {
                crate::BranchMode::Unreachable
            } else {
                remaining_mode
            };
            Some(build_block_with_import_mode(
                serializer,
                &clause.body,
                &clause_loc,
                mode,
            )?)
        }
        None => None,
    };
    Ok(StmtNode::IfStmt {
        test,
        body,
        elifs,
        else_body,
        loc,
    })
}

/// Build a `BLOCK` record whose body list is non-empty; an empty body emits
/// `empty_loc` (the caller's fallback location).
pub(crate) fn build_block(
    serializer: &mut Serializer<'_>,
    body: &ast::Suite,
    fallback_loc: &SourceLocation,
) -> PyResult<BlockNode> {
    let mut statements = Vec::with_capacity(body.len());
    for statement in body {
        statements.push(build_stmt(serializer, statement)?);
    }
    Ok(BlockNode {
        body: statements,
        empty_loc: Some(fallback_loc.clone()),
    })
}

fn build_stripped_block(
    serializer: &mut Serializer<'_>,
    body: &ast::Suite,
    fallback_loc: &SourceLocation,
) -> PyResult<BlockNode> {
    let loc = body
        .first()
        .map(|statement| serializer.loc(statement))
        .unwrap_or_else(|| fallback_loc.clone());
    Ok(BlockNode {
        body: Vec::new(),
        empty_loc: Some(loc),
    })
}

/// Build an optional block: an empty body emits no location.
fn build_optional_block(serializer: &mut Serializer<'_>, body: &ast::Suite) -> PyResult<BlockNode> {
    let mut statements = Vec::with_capacity(body.len());
    for statement in body {
        statements.push(build_stmt(serializer, statement)?);
    }
    Ok(BlockNode {
        body: statements,
        empty_loc: None,
    })
}

fn build_block_with_import_mode(
    serializer: &mut Serializer<'_>,
    body: &ast::Suite,
    fallback_loc: &SourceLocation,
    mode: crate::BranchMode,
) -> PyResult<BlockNode> {
    match mode {
        crate::BranchMode::Normal => build_block(serializer, body, fallback_loc),
        crate::BranchMode::MypyOnly => {
            serializer.imports.enter_mypy_only();
            let result = build_block(serializer, body, fallback_loc);
            serializer.imports.leave_mypy_only();
            result
        }
        crate::BranchMode::Unreachable => {
            serializer.imports.enter_unreachable();
            let result = build_block(serializer, body, fallback_loc);
            serializer.imports.leave_unreachable();
            result
        }
    }
}

/// Build the wire record for a pattern.
pub(crate) fn build_pattern(
    serializer: &mut Serializer<'_>,
    pattern: &ast::Pattern,
) -> PyResult<PatternNode> {
    Ok(match pattern {
        ast::Pattern::MatchAs(as_pattern) => {
            let loc = serializer.loc(pattern);
            let inner = match &as_pattern.pattern {
                Some(inner) => Some(Box::new(build_pattern(serializer, inner)?)),
                None => None,
            };
            let name = as_pattern
                .name
                .as_ref()
                .map(|name| (name.as_str().to_owned(), serializer.loc(name)));
            PatternNode::As {
                pattern: inner,
                name,
                loc,
            }
        }
        ast::Pattern::MatchOr(or_pattern) => {
            let loc = serializer.loc(pattern);
            let mut patterns = Vec::with_capacity(or_pattern.patterns.len());
            for item in &or_pattern.patterns {
                patterns.push(build_pattern(serializer, item)?);
            }
            PatternNode::Or { patterns, loc }
        }
        ast::Pattern::MatchValue(value_pattern) => {
            let loc = serializer.loc(pattern);
            let value = build_expr(serializer, &value_pattern.value)?;
            PatternNode::Value { value, loc }
        }
        ast::Pattern::MatchSingleton(singleton_pattern) => {
            let loc = serializer.loc(pattern);
            let value = match singleton_pattern.value {
                ast::Singleton::None => SingletonNode::None,
                ast::Singleton::True => SingletonNode::Bool(true),
                ast::Singleton::False => SingletonNode::Bool(false),
            };
            PatternNode::Singleton { value, loc }
        }
        ast::Pattern::MatchSequence(sequence_pattern) => {
            let loc = serializer.loc(pattern);
            let mut patterns = Vec::with_capacity(sequence_pattern.patterns.len());
            for item in &sequence_pattern.patterns {
                patterns.push(build_pattern(serializer, item)?);
            }
            PatternNode::Sequence { patterns, loc }
        }
        ast::Pattern::MatchStar(star_pattern) => {
            let loc = serializer.loc(pattern);
            let name = star_pattern
                .name
                .as_ref()
                .map(|name| (name.as_str().to_owned(), serializer.loc(name)));
            PatternNode::Star { name, loc }
        }
        ast::Pattern::MatchMapping(mapping_pattern) => {
            let loc = serializer.loc(pattern);
            let mut entries = Vec::with_capacity(mapping_pattern.keys.len());
            for (key, value) in mapping_pattern
                .keys
                .iter()
                .zip(mapping_pattern.patterns.iter())
            {
                let key = build_expr(serializer, key)?;
                let value = build_pattern(serializer, value)?;
                entries.push((key, value));
            }
            let rest = mapping_pattern
                .rest
                .as_ref()
                .map(|rest| (rest.as_str().to_owned(), serializer.loc(rest)));
            PatternNode::Mapping { entries, rest, loc }
        }
        ast::Pattern::MatchClass(class_pattern) => {
            let loc = serializer.loc(pattern);
            let cls = build_expr(serializer, &class_pattern.cls)?;
            let mut positionals = Vec::with_capacity(class_pattern.arguments.patterns.len());
            for positional in &class_pattern.arguments.patterns {
                positionals.push(build_pattern(serializer, positional)?);
            }
            let mut keywords = Vec::with_capacity(class_pattern.arguments.keywords.len());
            for keyword in &class_pattern.arguments.keywords {
                keywords.push((
                    keyword.attr.as_str().to_owned(),
                    build_pattern(serializer, &keyword.pattern)?,
                ));
            }
            PatternNode::Class {
                cls,
                positionals,
                keywords,
                loc,
            }
        }
    })
}
