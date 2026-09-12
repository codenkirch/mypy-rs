//! Byte writer for the `ExprNode` record tree.
//!
//! Emits exactly the record layout the old direct `serialize_expr` family
//! wrote. Emitting through the enum keeps one source of truth for expression
//! record shapes; the wire format is unchanged.

use crate::ast_node::{
    f_string_items_wire_len, BlockNode, DocstringNode, ExprNode, FStringFormatSpecNode,
    FStringItemNode, FStringPartNode, PatternNode, SingletonNode, StmtNode, TStringItemNode,
};
use crate::{
    Writer, ASSERT_STMT, ASSIGNMENT_EXPR, ASSIGNMENT_STMT, AS_PATTERN, AWAIT_EXPR, BIG_INT_EXPR,
    BLOCK, BOOL_OP_EXPR, BREAK_STMT, BYTES_EXPR, CALL_EXPR, CLASS_DEF, CLASS_PATTERN,
    COMPARISON_EXPR, COMPLEX_EXPR, CONDITIONAL_EXPR, CONTINUE_STMT, DECORATOR, DEL_STMT,
    DICT_COMPREHENSION, DICT_EXPR, DICT_STR_GEN, ELLIPSIS_EXPR, END_TAG, EXPR_STMT, FLOAT_EXPR,
    FOR_STMT, FSTRING_EXPR, FSTRING_INTERPOLATION, FUNC_DEF_STMT, GENERATOR_EXPR, GLOBAL_DECL,
    IF_STMT, IMPORT, IMPORT_ALL, IMPORT_FROM, INDEX_EXPR, INT_EXPR, LAMBDA_EXPR,
    LIST_COMPREHENSION, LIST_EXPR, LIST_GEN, MAPPING_PATTERN, MATCH_STMT, MEMBER_EXPR, NAME_EXPR,
    NONLOCAL_DECL, OPERATOR_ASSIGNMENT_STMT, OP_EXPR, OR_PATTERN, PASS_STMT, RAISE_STMT,
    RETURN_STMT, SEQUENCE_PATTERN, SET_COMPREHENSION, SET_EXPR, SINGLETON_PATTERN, SLICE_EXPR,
    STARRED_PATTERN, STAR_EXPR, STR_EXPR, TEMP_NODE, TRY_STMT, TSTRING_EXPR, TUPLE_EXPR,
    TYPE_ALIAS_STMT, UNARY_EXPR, VALUE_PATTERN, WHILE_STMT, WITH_STMT, YIELD_EXPR, YIELD_FROM_EXPR,
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

/// Write one statement record.
pub(crate) fn write_stmt(writer: &mut Writer, node: &StmtNode) {
    match node {
        StmtNode::ExprStmt { value, loc } => {
            writer.tag(EXPR_STMT);
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::Assignment {
            targets,
            value,
            annotation,
            new_syntax,
            loc,
        } => {
            writer.tag(ASSIGNMENT_STMT);
            writer.expr_list(targets.len());
            for target in targets {
                write_expr(writer, target);
            }
            match value {
                Some(value) => write_expr(writer, value),
                None => {
                    writer.tag(TEMP_NODE);
                    writer.tag(END_TAG);
                }
            }
            match annotation {
                Some(bytes) => {
                    writer.bool(true);
                    writer.raw_bytes(bytes);
                }
                None => writer.bool(false),
            }
            writer.bool(*new_syntax);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::OperatorAssignment {
            op,
            target,
            value,
            loc,
        } => {
            writer.tag(OPERATOR_ASSIGNMENT_STMT);
            writer.string(op);
            write_expr(writer, target);
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::ReturnStmt { value, loc } => {
            writer.tag(RETURN_STMT);
            writer.bool(value.is_some());
            if let Some(value) = value {
                write_expr(writer, value);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::RaiseStmt { exc, cause, loc } => {
            writer.tag(RAISE_STMT);
            writer.bool(exc.is_some());
            if let Some(exc) = exc {
                write_expr(writer, exc);
            }
            writer.bool(cause.is_some());
            if let Some(cause) = cause {
                write_expr(writer, cause);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::AssertStmt { test, msg, loc } => {
            writer.tag(ASSERT_STMT);
            write_expr(writer, test);
            writer.bool(msg.is_some());
            if let Some(msg) = msg {
                write_expr(writer, msg);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::DeleteStmt { targets, loc } => {
            writer.tag(DEL_STMT);
            if targets.len() == 1 {
                write_expr(writer, &targets[0]);
            } else {
                writer.tag(TUPLE_EXPR);
                writer.expr_list(targets.len());
                for target in targets {
                    write_expr(writer, target);
                }
                writer.loc(loc);
                writer.tag(END_TAG);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::FuncDef {
            name,
            docstring,
            parameters,
            body,
            is_async,
            type_params,
            returns,
            loc,
        } => {
            writer.tag(FUNC_DEF_STMT);
            writer.string(name);
            write_docstring(writer, docstring);
            writer.raw_bytes(parameters);
            write_block(writer, body);
            writer.bool(*is_async);
            writer.bool(type_params.is_some());
            if let Some(type_params) = type_params {
                writer.raw_bytes(type_params);
            }
            writer.bool(returns.is_some());
            if let Some(returns) = returns {
                writer.raw_bytes(returns);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::DecoratedFuncDef {
            decorators,
            line,
            column,
            function,
        } => {
            writer.tag(DECORATOR);
            writer.tag(LIST_GEN);
            writer.bare_int(decorators.len() as i64);
            for decorator in decorators {
                write_expr(writer, decorator);
            }
            writer.int(*line);
            writer.int(*column);
            write_stmt(writer, function);
            writer.tag(END_TAG);
        }
        StmtNode::ClassDef {
            name,
            docstring,
            body,
            bases,
            decorators,
            type_params,
            keywords,
            loc,
        } => {
            writer.tag(CLASS_DEF);
            writer.string(name);
            write_docstring(writer, docstring);
            write_block(writer, body);
            writer.expr_list(bases.len());
            for base in bases {
                write_expr(writer, base);
            }
            writer.tag(LIST_GEN);
            writer.bare_int(decorators.len() as i64);
            for decorator in decorators {
                write_expr(writer, decorator);
            }
            writer.bool(type_params.is_some());
            if let Some(type_params) = type_params {
                writer.raw_bytes(type_params);
            }
            writer.tag(DICT_STR_GEN);
            writer.bare_int(keywords.len() as i64);
            for (keyword, value) in keywords {
                writer.string(keyword);
                write_expr(writer, value);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::IfStmt {
            test,
            body,
            elifs,
            else_body,
            loc,
        } => {
            writer.tag(IF_STMT);
            write_expr(writer, test);
            write_block(writer, body);
            writer.int(elifs.len() as i64);
            for clause in elifs {
                write_expr(writer, &clause.test);
                write_block(writer, &clause.body);
            }
            writer.bool(else_body.is_some());
            if let Some(else_body) = else_body {
                write_block(writer, else_body);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::ForStmt {
            target,
            iter,
            body,
            orelse,
            is_async,
            type_comment,
            loc,
        } => {
            writer.tag(FOR_STMT);
            write_expr(writer, target);
            write_expr(writer, iter);
            write_block(writer, body);
            write_optional_block(writer, orelse);
            writer.bool(*is_async);
            writer.bool(type_comment.is_some());
            if let Some(type_comment) = type_comment {
                writer.raw_bytes(type_comment);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::WhileStmt {
            test,
            body,
            orelse,
            loc,
        } => {
            writer.tag(WHILE_STMT);
            write_expr(writer, test);
            write_block(writer, body);
            write_optional_block(writer, orelse);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::WithStmt {
            items,
            body,
            is_async,
            type_comment,
            loc,
        } => {
            writer.tag(WITH_STMT);
            writer.int(items.len() as i64);
            for item in items {
                write_expr(writer, &item.context_expr);
                writer.bool(item.optional_vars.is_some());
                if let Some(target) = &item.optional_vars {
                    write_expr(writer, target);
                }
            }
            write_block(writer, body);
            writer.bool(*is_async);
            writer.bool(type_comment.is_some());
            if let Some(type_comment) = type_comment {
                writer.raw_bytes(type_comment);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::TryStmt {
            body,
            handlers,
            orelse,
            finalbody,
            is_star,
            loc,
        } => {
            writer.tag(TRY_STMT);
            write_block(writer, body);
            writer.int(handlers.len() as i64);
            for handler in handlers {
                writer.bool(handler.type_.is_some());
                if let Some(exc_type) = &handler.type_ {
                    write_expr(writer, exc_type);
                }
            }
            for handler in handlers {
                writer.bool(handler.name.is_some());
                if let Some((name, name_loc)) = &handler.name {
                    writer.string(name);
                    writer.loc(name_loc);
                }
            }
            for handler in handlers {
                write_block(writer, &handler.body);
            }
            writer.bool(orelse.is_some());
            if let Some(orelse) = orelse {
                write_optional_block(writer, orelse);
            }
            writer.bool(finalbody.is_some());
            if let Some(finalbody) = finalbody {
                write_optional_block(writer, finalbody);
            }
            writer.bool(*is_star);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::MatchStmt {
            subject,
            cases,
            loc,
        } => {
            writer.tag(MATCH_STMT);
            write_expr(writer, subject);
            writer.int(cases.len() as i64);
            for case in cases {
                write_pattern(writer, &case.pattern);
                writer.bool(case.guard.is_some());
                if let Some(guard) = &case.guard {
                    write_expr(writer, guard);
                }
                write_block(writer, &case.body);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::TypeAliasStmt {
            name,
            type_params,
            value,
            loc,
        } => {
            writer.tag(TYPE_ALIAS_STMT);
            write_expr(writer, name);
            match type_params {
                Some(type_params) => writer.raw_bytes(type_params),
                None => writer.bare_int(0),
            }
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::Import { names, loc, flags } => {
            writer.tag(IMPORT);
            writer.int(names.len() as i64);
            for (name, asname) in names {
                writer.string(name);
                writer.bool(asname.is_some());
                if let Some(asname) = asname {
                    writer.string(asname);
                }
            }
            writer.loc(loc);
            writer.int(*flags);
            writer.tag(END_TAG);
        }
        StmtNode::ImportFrom {
            relative,
            module,
            names,
            loc,
            flags,
        } => {
            writer.tag(IMPORT_FROM);
            writer.int(*relative);
            writer.string(module);
            writer.int(names.len() as i64);
            for (name, asname) in names {
                writer.string(name);
                writer.bool(asname.is_some());
                if let Some(asname) = asname {
                    writer.string(asname);
                }
            }
            writer.loc(loc);
            writer.int(*flags);
            writer.tag(END_TAG);
        }
        StmtNode::ImportAll {
            module,
            relative,
            loc,
            flags,
        } => {
            writer.tag(IMPORT_ALL);
            writer.string(module);
            writer.int(*relative);
            writer.loc(loc);
            writer.int(*flags);
            writer.tag(END_TAG);
        }
        StmtNode::GlobalDecl { names, loc } => write_name_decl(writer, GLOBAL_DECL, names, loc),
        StmtNode::NonlocalDecl { names, loc } => write_name_decl(writer, NONLOCAL_DECL, names, loc),
        StmtNode::PassStmt { loc } => {
            writer.tag(PASS_STMT);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::BreakStmt { loc } => {
            writer.tag(BREAK_STMT);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        StmtNode::ContinueStmt { loc } => {
            writer.tag(CONTINUE_STMT);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
    }
}

/// Write one pattern record.
pub(crate) fn write_pattern(writer: &mut Writer, node: &PatternNode) {
    match node {
        PatternNode::As { pattern, name, loc } => {
            writer.tag(AS_PATTERN);
            writer.bool(pattern.is_some());
            if let Some(pattern) = pattern {
                write_pattern(writer, pattern);
            }
            writer.bool(name.is_some());
            if let Some((name, name_loc)) = name {
                writer.string(name);
                writer.loc(name_loc);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        PatternNode::Or { patterns, loc } => {
            writer.tag(OR_PATTERN);
            writer.int(patterns.len() as i64);
            for pattern in patterns {
                write_pattern(writer, pattern);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        PatternNode::Value { value, loc } => {
            writer.tag(VALUE_PATTERN);
            write_expr(writer, value);
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        PatternNode::Singleton { value, loc } => {
            writer.tag(SINGLETON_PATTERN);
            match value {
                SingletonNode::None => writer.none(),
                SingletonNode::Bool(value) => writer.bool(*value),
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        PatternNode::Sequence { patterns, loc } => {
            writer.tag(SEQUENCE_PATTERN);
            writer.int(patterns.len() as i64);
            for pattern in patterns {
                write_pattern(writer, pattern);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        PatternNode::Star { name, loc } => {
            writer.tag(STARRED_PATTERN);
            writer.bool(name.is_some());
            if let Some((name, name_loc)) = name {
                writer.string(name);
                writer.loc(name_loc);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        PatternNode::Mapping { entries, rest, loc } => {
            writer.tag(MAPPING_PATTERN);
            writer.int(entries.len() as i64);
            for (key, value) in entries {
                write_expr(writer, key);
                write_pattern(writer, value);
            }
            writer.bool(rest.is_some());
            if let Some((rest, rest_loc)) = rest {
                writer.string(rest);
                writer.loc(rest_loc);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
        PatternNode::Class {
            cls,
            positionals,
            keywords,
            loc,
        } => {
            writer.tag(CLASS_PATTERN);
            write_expr(writer, cls);
            writer.int(positionals.len() as i64);
            for positional in positionals {
                write_pattern(writer, positional);
            }
            writer.int(keywords.len() as i64);
            for (keyword, pattern) in keywords {
                writer.string(keyword);
                write_pattern(writer, pattern);
            }
            writer.loc(loc);
            writer.tag(END_TAG);
        }
    }
}

fn write_docstring(writer: &mut Writer, docstring: &Option<DocstringNode>) {
    match docstring {
        None => writer.bool(false),
        Some(docstring) => {
            writer.bool(true);
            writer.string(&docstring.value);
            writer.bool(docstring.corrupted);
            if let Some(raw) = &docstring.raw {
                writer.string(raw);
            }
        }
    }
}

/// A `BLOCK` record that carries its empty-body location.
fn write_block(writer: &mut Writer, block: &BlockNode) {
    writer.tag(BLOCK);
    writer.tag(LIST_GEN);
    writer.bare_int(block.body.len() as i64);
    writer.bool(false);
    if block.body.is_empty() {
        if let Some(loc) = &block.empty_loc {
            writer.loc(loc);
        }
    } else {
        for statement in &block.body {
            write_stmt(writer, statement);
        }
    }
    writer.tag(END_TAG);
}

/// A `BLOCK` record whose empty body emits no location (`for`/`while` else,
/// `try` else/finally).
fn write_optional_block(writer: &mut Writer, block: &BlockNode) {
    writer.tag(BLOCK);
    writer.tag(LIST_GEN);
    writer.bare_int(block.body.len() as i64);
    writer.bool(false);
    for statement in &block.body {
        write_stmt(writer, statement);
    }
    writer.tag(END_TAG);
}

fn write_name_decl(writer: &mut Writer, tag: u8, names: &[String], loc: &crate::SourceLocation) {
    writer.tag(tag);
    writer.int(names.len() as i64);
    for name in names {
        writer.string(name);
    }
    writer.loc(loc);
    writer.tag(END_TAG);
}
