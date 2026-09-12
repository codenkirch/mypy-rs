//! Frozen pre-enum expression writers, test-only A/B reference.
//!
//! These are the direct byte emitters that `serialize_expr` used before the
//! G0.3 enum conversion, kept verbatim so the corpus parity test can compare
//! both paths byte-for-byte. Do not extend: production lives in `ast_node`
//! and `ast_writer`.

#![allow(dead_code)]

use ruff_python_ast::ArgOrKeyword;

use super::*;

pub(crate) fn serialize_expr(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
) -> PyResult<()> {
    match expression {
        ast::Expr::Call(call) => {
            let loc = serializer.loc(expression);
            serializer.writer.tag(CALL_EXPR);
            serialize_expr(serializer, &call.func)?;

            let arg_count = call.arguments.args.len() + call.arguments.keywords.len();
            serializer.writer.tag(LIST_GEN);
            serializer.writer.bare_int(arg_count as i64);
            for arg in &call.arguments.args {
                serialize_call_arg_value(serializer, &ArgOrKeyword::Arg(arg))?;
            }
            for keyword in &call.arguments.keywords {
                serialize_call_arg_value(serializer, &ArgOrKeyword::Keyword(keyword))?;
            }

            let mut arg_kinds = Vec::with_capacity(arg_count);
            let mut arg_names = Vec::with_capacity(arg_count);
            for arg in &call.arguments.args {
                arg_kinds.push(if matches!(arg, ast::Expr::Starred(_)) {
                    ARG_STAR
                } else {
                    ARG_POS
                });
                arg_names.push(None);
            }
            for keyword in &call.arguments.keywords {
                if let Some(name) = &keyword.arg {
                    arg_kinds.push(ARG_NAMED);
                    arg_names.push(Some(name.as_str().to_owned()));
                } else {
                    arg_kinds.push(ARG_STAR2);
                    arg_names.push(None);
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
        ast::Expr::TString(t_string) => serialize_t_string_expr(serializer, t_string),
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
            let (value, corrupted, raw) = string_literal_parts(serializer.source, string);
            serializer.writer.tag(STR_EXPR);
            serializer.writer.string(value);
            serializer.writer.bool(corrupted);
            if let Some(raw) = raw {
                serializer.writer.string(&raw);
            }
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

pub(crate) fn serialize_lvalue(
    serializer: &mut Serializer<'_>,
    expression: &ast::Expr,
) -> PyResult<()> {
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
                // Same lone-surrogate repair as plain string literals: the
                // value is lossy when the token has such an escape.
                let range = literal.range;
                let raw = &serializer.source[range.start().to_usize()..range.end().to_usize()];
                let corrupted = !literal.flags.prefix().is_raw() && has_surrogate_escape(raw);
                let loc = serializer.loc(literal);
                serializer.writer.bool(false);
                serializer.writer.string(literal.as_str());
                serializer.writer.bool(corrupted);
                if corrupted {
                    serializer.writer.string(raw);
                }
                serializer.writer.loc(&loc);
            }
            ast::FStringPart::FString(part) => {
                serializer.writer.bool(true);
                serialize_f_string_items(serializer, &part.elements, part.flags.prefix().is_raw())?;
            }
        }
    }
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
}

fn serialize_t_string_expr(
    serializer: &mut Serializer<'_>,
    t_string: &ast::ExprTString,
) -> PyResult<()> {
    serializer.uses_template_strings = true;
    let loc = serializer.loc(t_string);
    serializer.writer.tag(TSTRING_EXPR);
    // Wire format flattens all TString parts' elements into one list.
    let nparts: usize = t_string
        .value
        .as_slice()
        .iter()
        .map(|t| t.elements.iter().count())
        .sum();
    serializer.writer.int(nparts as i64);
    for tstring in t_string.value.as_slice() {
        let is_raw = tstring.flags.prefix().is_raw();
        for element in tstring.elements.iter() {
            match element {
                ast::InterpolatedStringElement::Interpolation(interpolation) => {
                    serializer.writer.bool(true);
                    serialize_expr(serializer, &interpolation.expression)?;
                    // Raw interpolation source text (CPython's Interpolation.str).
                    let source = &serializer.source[interpolation.range.start().to_usize()
                        ..interpolation.range.end().to_usize()];
                    serializer.writer.string(source);

                    let conversion = interpolation.conversion.to_char();
                    serializer.writer.bool(conversion.is_some());
                    if let Some(conversion) = conversion {
                        serializer.writer.string(&format!("!{conversion}"));
                    }

                    serializer.writer.bool(interpolation.format_spec.is_some());
                    if let Some(format_spec) = &interpolation.format_spec {
                        serialize_f_string_items(serializer, &format_spec.elements, is_raw)?;
                        let loc = serializer.loc(&**format_spec);
                        serializer.writer.loc(&loc);
                    }
                }
                ast::InterpolatedStringElement::Literal(literal) => {
                    let range = literal.range;
                    let raw = &serializer.source[range.start().to_usize()..range.end().to_usize()];
                    let corrupted = !is_raw && has_surrogate_escape(raw);
                    serializer.writer.bool(false);
                    serializer.writer.string(&literal.value);
                    serializer.writer.bool(corrupted);
                    if corrupted {
                        serializer.writer.string(raw);
                    }
                    serializer.writer.loc(&serializer.loc(literal));
                }
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
    is_raw: bool,
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
                let range = literal.range;
                let raw = &serializer.source[range.start().to_usize()..range.end().to_usize()];
                let corrupted = !is_raw && has_surrogate_escape(raw);
                let loc = serializer.loc(literal);
                serializer.writer.string(&literal.value);
                serializer.writer.bool(corrupted);
                if corrupted {
                    serializer.writer.string(raw);
                }
                serializer.writer.loc(&loc);
            }
            ast::InterpolatedStringElement::Interpolation(interpolation) => {
                if let Some(debug_text) = &interpolation.debug_text {
                    let loc = serializer.loc(interpolation);
                    // Debug text is raw source spelling, never escape-decoded.
                    serializer.writer.string(debug_text.as_str());
                    serializer.writer.bool(false);
                    serializer.writer.loc(&loc);
                }
                serialize_f_string_interpolation(serializer, interpolation, is_raw)?;
            }
        }
    }
    Ok(())
}

fn serialize_f_string_interpolation(
    serializer: &mut Serializer<'_>,
    interpolation: &ast::InterpolatedElement,
    is_raw: bool,
) -> PyResult<()> {
    serializer.writer.tag(FSTRING_INTERPOLATION);
    serialize_expr(serializer, &interpolation.expression)?;

    // CPython defaults the !r conversion for the debug form only when no
    // format spec is applied (with a spec it formats str(expr)).
    let conversion = if interpolation.debug_text.is_some()
        && interpolation.conversion == ast::ConversionFlag::None
        && interpolation.format_spec.is_none()
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
        serialize_f_string_items(serializer, &format_spec.elements, is_raw)?;
        let loc = serializer.loc(&**format_spec);
        serializer.writer.loc(&loc);
    }

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
    // CPython (type_comments=True) never attaches `# type:` comments to
    // lambda parameters; a per-line statement comment must not leak into
    // the lambda's argument annotations.
    serializer.lambda_depth += 1;
    if let Some(parameters) = &lambda.parameters {
        serialize_parameters(serializer, parameters, None)?;
    } else {
        serialize_empty_parameters(serializer);
    }
    serializer.lambda_depth -= 1;
    serialize_lambda_body(serializer, &lambda.body)?;
    serializer.writer.loc(&loc);
    serializer.writer.tag(END_TAG);
    Ok(())
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

fn serialize_name_expr(writer: &mut Writer, name: &str, loc: &SourceLocation) {
    writer.tag(NAME_EXPR);
    writer.string(name);
    writer.loc(loc);
    writer.tag(END_TAG);
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
