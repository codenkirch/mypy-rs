//! Frozen pre-enum statement/pattern writers, test-only A/B reference.
//!
//! These are the direct byte emitters that `serialize_stmt`/`serialize_suite`
//! used before the G0.4 enum conversion, kept verbatim so the corpus parity
//! test can compare both paths byte-for-byte. Do not extend: production lives
//! in `ast_node` and `ast_writer`.
//!
//! Nested statement/pattern calls stay inside this module (the local
//! `serialize_stmt`/`serialize_pattern` shadow the dispatchers imported by the
//! glob); expression and lvalue calls route through the `super::` dispatchers
//! so the `legacy_exprs` switch keeps working for both A/B arms.

#![allow(dead_code)]

use super::*;

pub(crate) fn serialize_stmt(
    serializer: &mut Serializer<'_>,
    statement: &ast::Stmt,
) -> PyResult<()> {
    match statement {
        ast::Stmt::Expr(expr) => {
            let loc = serializer.loc(statement);
            serializer.writer.tag(EXPR_STMT);
            serialize_expr(serializer, &expr.value)?;
            serializer.writer.loc(&loc);
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
                serialize_type_with_forced_loc(serializer, &parsed_type, &loc)?;
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
    let names = translated_import_names(&import.names, serializer.custom_typing_module.as_deref());
    serializer.writer.tag(IMPORT);
    serializer.writer.int(names.len() as i64);
    for (name, asname) in &names {
        serializer.writer.string(name);
        write_optional_string(&mut serializer.writer, asname.as_deref());
    }
    write_import_metadata_tail(&mut serializer.writer, &loc, flags);
    serializer.writer.tag(END_TAG);

    for (name, asname) in names {
        serializer.imports.push(ImportMetadata {
            tag: IMPORT_METADATA,
            module: name,
            relative: 0,
            asname,
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
    let raw_module = import
        .module
        .as_ref()
        .map_or_else(String::new, |module| module.as_str().to_owned());
    let relative = i64::from(import.level);

    if import.names.len() == 1 && import.names[0].name.as_str() == "*" {
        // fastparse's `visit_ImportFrom` leaves the module untranslated for
        // star imports; mirror that for parity.
        serializer.writer.tag(IMPORT_ALL);
        serializer.writer.string(&raw_module);
        serializer.writer.int(relative);
        write_import_metadata_tail(&mut serializer.writer, &loc, flags);
        serializer.writer.tag(END_TAG);
        serializer.imports.push(ImportMetadata {
            tag: IMPORTALL_METADATA,
            module: raw_module,
            relative,
            asname: None,
            names: Vec::new(),
            loc,
            flags,
        });
        return Ok(());
    }

    let module = translate_module_name(&raw_module, serializer.custom_typing_module.as_deref());
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
    serializer.write_docstring(&class_def.body);
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
    // Type comment on the for-loop variable (e.g. `for x in l:  # type: object`).
    // Keyed by the line of the comment, which is the for-statement's line.
    if let Some(type_comment) = serializer.type_comments.get(&loc.line).cloned() {
        let parsed_type = parse_expression(&type_comment)
            .map_err(to_parse_error)?
            .into_expr();
        serializer.writer.bool(true);
        serialize_type_with_forced_loc(serializer, &parsed_type, &loc)?;
    } else {
        serializer.writer.bool(false);
    }
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
    // Type comment on the with-statement target
    // (e.g. `with open(f) as d:  # type: io.TextIO`).
    if let Some(type_comment) = serializer.type_comments.get(&loc.line).cloned() {
        let parsed_type = parse_expression(&type_comment)
            .map_err(to_parse_error)?
            .into_expr();
        serializer.writer.bool(true);
        serialize_type_with_forced_loc(serializer, &parsed_type, &loc)?;
    } else {
        serializer.writer.bool(false);
    }
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

pub(crate) fn serialize_pattern(
    serializer: &mut Serializer<'_>,
    pattern: &ast::Pattern,
) -> PyResult<()> {
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
    serializer.write_docstring(&function.body);
    let type_comment = function_type_comment(serializer, function, &loc);
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
        serialize_type_with_forced_loc(serializer, &parsed_type.into_expr(), &loc)?;
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
    serializer.writer.int(loc.column + 1);
    serialize_function_def(serializer, function)?;
    serializer.writer.tag(END_TAG);
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
