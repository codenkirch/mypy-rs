//! Byte writer for the `ExprNode` record tree.
//!
//! Emits exactly the record layout the old direct `serialize_expr` family
//! wrote. Emitting through the enum keeps one source of truth for expression
//! record shapes; the wire format is unchanged.

use crate::ast_node::{
    f_string_items_wire_len, ExprNode, FStringFormatSpecNode, FStringItemNode, FStringPartNode,
    TStringItemNode,
};
use crate::{
    Writer, ASSIGNMENT_EXPR, AWAIT_EXPR, BIG_INT_EXPR, BLOCK, BOOL_OP_EXPR, BYTES_EXPR, CALL_EXPR,
    COMPARISON_EXPR, COMPLEX_EXPR, CONDITIONAL_EXPR, DICT_COMPREHENSION, DICT_EXPR, ELLIPSIS_EXPR,
    END_TAG, FLOAT_EXPR, FSTRING_EXPR, FSTRING_INTERPOLATION, GENERATOR_EXPR, INDEX_EXPR, INT_EXPR,
    LAMBDA_EXPR, LIST_COMPREHENSION, LIST_EXPR, LIST_GEN, MEMBER_EXPR, NAME_EXPR, OP_EXPR,
    RETURN_STMT, SET_COMPREHENSION, SET_EXPR, SLICE_EXPR, STAR_EXPR, STR_EXPR, TSTRING_EXPR,
    TUPLE_EXPR, UNARY_EXPR, YIELD_EXPR, YIELD_FROM_EXPR,
};

/// Write one expression record.
pub(crate) fn write_expr(writer: &mut Writer, node: &ExprNode) {
    match node {
        ExprNode::Call {
            func,
            args,
            arg_kinds,
            arg_names,
            loc,
        } => {
            writer.tag(CALL_EXPR);
            write_expr(writer, func);
            writer.tag(LIST_GEN);
            writer.bare_int(args.len() as i64);
            for arg in args {
                write_expr(writer, arg);
            }
            writer.int_list(arg_kinds);
            writer.opt_str_list(arg_names);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Name { name, loc } => write_name_expr(writer, name, loc),
        ExprNode::Ellipsis { loc } => {
            writer.tag(ELLIPSIS_EXPR);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Compare {
            left,
            ops,
            comparators,
            loc,
        } => {
            writer.tag(COMPARISON_EXPR);
            write_expr(writer, left);
            writer.int_list(ops);
            writer.expr_list(comparators.len());
            for comparator in comparators {
                write_expr(writer, comparator);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::BoolOp { op, values, loc } => {
            writer.tag(BOOL_OP_EXPR);
            writer.int(*op);
            writer.expr_list(values.len());
            for value in values {
                write_expr(writer, value);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::UnaryOp { op, operand, loc } => {
            writer.tag(UNARY_EXPR);
            writer.int(*op);
            write_expr(writer, operand);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Await { value, loc } => {
            writer.tag(AWAIT_EXPR);
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Yield { value, loc } => {
            writer.tag(YIELD_EXPR);
            writer.bool(value.is_some());
            if let Some(value) = value {
                write_expr(writer, value);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::YieldFrom { value, loc } => {
            writer.tag(YIELD_FROM_EXPR);
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::FString { parts, loc } => {
            writer.tag(FSTRING_EXPR);
            writer.int(parts.len() as i64);
            for part in parts {
                match part {
                    FStringPartNode::Literal {
                        value,
                        corrupted,
                        raw,
                        loc,
                    } => {
                        writer.bool(false);
                        write_string_literal(writer, value, *corrupted, raw.as_deref(), loc);
                    }
                    FStringPartNode::Items { items } => {
                        writer.bool(true);
                        write_f_string_items(writer, items);
                    }
                }
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::TString { items, loc } => {
            writer.tag(TSTRING_EXPR);
            writer.int(items.len() as i64);
            for item in items {
                write_t_string_item(writer, item);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Conditional {
            body,
            test,
            orelse,
            loc,
        } => {
            writer.tag(CONDITIONAL_EXPR);
            write_expr(writer, body);
            write_expr(writer, test);
            write_expr(writer, orelse);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Assignment { target, value, loc } => {
            writer.tag(ASSIGNMENT_EXPR);
            write_expr(writer, target);
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Lambda {
            parameters,
            body,
            body_loc,
            loc,
        } => {
            writer.tag(LAMBDA_EXPR);
            writer.raw_bytes(parameters);
            writer.tag(BLOCK);
            writer.tag(LIST_GEN);
            writer.bare_int(1);
            writer.bool(false);
            writer.tag(RETURN_STMT);
            writer.bool(true);
            write_expr(writer, body);
            writer.loc(body_loc);
            writer.tag(END_TAG);
            writer.tag(END_TAG);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Starred { value, loc } => {
            writer.tag(STAR_EXPR);
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Member { value, attr, loc } => {
            writer.tag(MEMBER_EXPR);
            write_expr(writer, value);
            writer.string(attr);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::BinaryOp { op, left, right } => {
            writer.tag(OP_EXPR);
            writer.int(*op);
            write_expr(writer, left);
            write_expr(writer, right);
            writer.tag(END_TAG);
        }
        ExprNode::Int { value, loc } => {
            writer.tag(INT_EXPR);
            writer.int(*value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::BigInt { value, loc } => {
            writer.tag(BIG_INT_EXPR);
            writer.string(value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Float { value, loc } => {
            writer.tag(FLOAT_EXPR);
            writer.float(*value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Complex { real, imag, loc } => {
            writer.tag(COMPLEX_EXPR);
            writer.float(*real);
            writer.float(*imag);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Tuple { items, loc } => {
            writer.tag(TUPLE_EXPR);
            writer.expr_list(items.len());
            for item in items {
                write_expr(writer, item);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::List { items, loc } => {
            writer.tag(LIST_EXPR);
            writer.expr_list(items.len());
            for item in items {
                write_expr(writer, item);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::ListComp {
            elt,
            generators,
            loc,
        } => {
            writer.tag(LIST_COMPREHENSION);
            write_expr(writer, elt);
            write_comprehensions(writer, generators);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Set { items, loc } => {
            writer.tag(SET_EXPR);
            writer.expr_list(items.len());
            for item in items {
                write_expr(writer, item);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::SetComp {
            elt,
            generators,
            loc,
        } => {
            writer.tag(SET_COMPREHENSION);
            write_expr(writer, elt);
            write_comprehensions(writer, generators);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Dict { items, loc } => {
            writer.tag(DICT_EXPR);
            writer.expr_list(items.len());
            for item in items {
                writer.bool(item.key.is_some());
                if let Some(key) = &item.key {
                    write_expr(writer, key);
                }
            }
            writer.expr_list(items.len());
            for item in items {
                write_expr(writer, &item.value);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::DictComp {
            key,
            value,
            generators,
            loc,
        } => {
            writer.tag(DICT_COMPREHENSION);
            write_expr(writer, key);
            write_expr(writer, value);
            write_comprehensions(writer, generators);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Generator {
            elt,
            generators,
            loc,
        } => {
            writer.tag(GENERATOR_EXPR);
            write_expr(writer, elt);
            write_comprehensions(writer, generators);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::StringLiteral {
            value,
            corrupted,
            raw,
            loc,
        } => {
            writer.tag(STR_EXPR);
            write_string_literal(writer, value, *corrupted, raw.as_deref(), loc);
            writer.tag(END_TAG);
        }
        ExprNode::Bytes { value, loc } => {
            writer.tag(BYTES_EXPR);
            writer.string(value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Subscript { value, slice, loc } => {
            writer.tag(INDEX_EXPR);
            write_expr(writer, value);
            write_expr(writer, slice);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        ExprNode::Slice {
            lower,
            upper,
            step,
            loc,
        } => {
            writer.tag(SLICE_EXPR);
            writer.bool(lower.is_some());
            if let Some(lower) = lower {
                write_expr(writer, lower);
            }
            writer.bool(upper.is_some());
            if let Some(upper) = upper {
                write_expr(writer, upper);
            }
            writer.bool(step.is_some());
            if let Some(step) = step {
                write_expr(writer, step);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
    }
}

fn write_name_expr(writer: &mut Writer, name: &str, loc: &crate::SourceLocation) {
    writer.tag(NAME_EXPR);
    writer.string(name);
    writer.loc(loc);
    writer.tag(END_TAG);
}

/// The shared `str; bool corrupted; [raw]; loc` literal body.
fn write_string_literal(
    writer: &mut Writer,
    value: &str,
    corrupted: bool,
    raw: Option<&str>,
    loc: &crate::SourceLocation,
) {
    writer.string(value);
    writer.bool(corrupted);
    if let Some(raw) = raw {
        writer.string(raw);
    }
    writer.loc(loc);
}

/// `int n; items` tail shared by f-string lists and format specs.
fn write_f_string_items(writer: &mut Writer, items: &[FStringItemNode]) {
    writer.int(f_string_items_wire_len(items) as i64);
    for item in items {
        match item {
            FStringItemNode::Literal {
                value,
                corrupted,
                raw,
                loc,
            } => {
                write_string_literal(writer, value, *corrupted, raw.as_deref(), loc);
            }
            FStringItemNode::Interpolation {
                debug,
                expression,
                conversion,
                format_spec,
            } => {
                if let Some(debug) = debug {
                    writer.string(&debug.text);
                    writer.bool(false);
                    writer.loc(&debug.loc);
                }
                writer.tag(FSTRING_INTERPOLATION);
                write_expr(writer, expression);
                writer.bool(conversion.is_some());
                if let Some(conversion) = conversion {
                    writer.string(conversion);
                }
                writer.bool(format_spec.is_some());
                if let Some(format_spec) = format_spec {
                    write_format_spec(writer, format_spec);
                }
                writer.tag(END_TAG);
            }
        }
    }
}

fn write_format_spec(writer: &mut Writer, format_spec: &FStringFormatSpecNode) {
    write_f_string_items(writer, &format_spec.items);
    writer.loc(&format_spec.loc);
}

fn write_t_string_item(writer: &mut Writer, item: &TStringItemNode) {
    match item {
        TStringItemNode::Interpolation {
            expression,
            source,
            conversion,
            format_spec,
        } => {
            writer.bool(true);
            write_expr(writer, expression);
            writer.string(source);
            writer.bool(conversion.is_some());
            if let Some(conversion) = conversion {
                writer.string(conversion);
            }
            writer.bool(format_spec.is_some());
            if let Some(format_spec) = format_spec {
                write_format_spec(writer, format_spec);
            }
        }
        TStringItemNode::Literal {
            value,
            corrupted,
            raw,
            loc,
        } => {
            writer.bool(false);
            write_string_literal(writer, value, *corrupted, raw.as_deref(), loc);
        }
    }
}

fn write_comprehensions(writer: &mut Writer, generators: &[crate::ast_node::ComprehensionNode]) {
    writer.int(generators.len() as i64);
    for generator in generators {
        write_expr(writer, &generator.target);
    }
    for generator in generators {
        write_expr(writer, &generator.iter);
    }
    for generator in generators {
        writer.expr_list(generator.ifs.len());
        for condition in &generator.ifs {
            write_expr(writer, condition);
        }
    }
    for generator in generators {
        writer.bool(generator.is_async);
    }
}
