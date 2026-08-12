//! Native port of `mypy/partially_defined.py` (Issue #537).
//!
//! Walks a live Python mypy AST and detects variables that may be used
//! before definition or are only defined in some branches. Mirrors the
//! `PossiblyUndefinedVariableVisitor` (an `ExtendedTraverserVisitor`).
//!
//! Entry point: `rust_find_possibly_undefined(node, type_map, options,
//! names) -> Vec<String>`. Returns the names of variables flagged as
//! "possibly undefined" or "used before def", in traversal order.

use pyo3::prelude::*;
use pyo3::types::PyTuple;
use std::collections::{HashMap, HashSet};

// Names always defined at module load (mirrors `implicit_module_attrs`).
const IMPLICIT_MODULE_ATTRS: &[&str] = &[
    "__name__",
    "__doc__",
    "__path__",
    "__file__",
    "__package__",
    "__annotations__",
    "__spec__",
];

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn class_name(node: &PyAny) -> PyResult<String> {
    Ok(node.get_type().name()?.into())
}

/// Get an attribute that exists and is not Python `None`.
fn opt<'a>(node: &'a PyAny, name: &str) -> Option<&'a PyAny> {
    node.getattr(name).ok().filter(|v| !v.is_none())
}

/// `mypy.semanal.refers_to_fullname` for RefExpr (NameExpr/MemberExpr).
fn refers_to_fullname(node: &PyAny, fullname: &str) -> PyResult<bool> {
    let tn = class_name(node)?;
    if tn == "NameExpr" || tn == "MemberExpr" {
        if let Ok(fn_) = node.getattr("fullname").and_then(|v| v.extract::<String>()) {
            return Ok(fn_ == fullname);
        }
    }
    Ok(false)
}

/// `mypy.checker.is_false_literal`.
fn is_false_literal(node: &PyAny) -> PyResult<bool> {
    if refers_to_fullname(node, "builtins.False")? {
        return Ok(true);
    }
    if class_name(node)? == "IntExpr" {
        let val: i64 = node.getattr("value")?.extract()?;
        return Ok(val == 0);
    }
    Ok(false)
}

/// `mypy.checker.is_true_literal`.
fn is_true_literal(node: &PyAny) -> PyResult<bool> {
    if refers_to_fullname(node, "builtins.True")? {
        return Ok(true);
    }
    if class_name(node)? == "IntExpr" {
        let val: i64 = node.getattr("value")?.extract()?;
        return Ok(val != 0);
    }
    Ok(false)
}

/// `mypy.reachability.infer_pattern_value(pattern) == ALWAYS_TRUE`.
fn infer_pattern_always_true(pattern: &PyAny) -> PyResult<bool> {
    let tn = class_name(pattern)?;
    if tn == "AsPattern" {
        return Ok(pattern.getattr("pattern")?.is_none());
    }
    if tn == "OrPattern" {
        for p in pattern.getattr("patterns")?.iter()? {
            if infer_pattern_always_true(p?)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// BranchState / BranchStatement / Scope / Tracker / Loop
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct BranchState {
    must_be_defined: HashSet<String>,
    may_be_defined: HashSet<String>,
    skipped: bool,
}

#[derive(Clone)]
struct BranchStatement {
    initial_state: BranchState,
    branches: Vec<BranchState>,
}

impl BranchStatement {
    fn new(initial_state: Option<BranchState>) -> Self {
        let initial = initial_state.unwrap_or_default();
        Self {
            initial_state: initial.clone(),
            branches: vec![BranchState {
                must_be_defined: initial.must_be_defined.clone(),
                may_be_defined: initial.may_be_defined.clone(),
                skipped: false,
            }],
        }
    }

    fn next_branch(&mut self) {
        self.branches.push(BranchState {
            must_be_defined: self.initial_state.must_be_defined.clone(),
            may_be_defined: self.initial_state.may_be_defined.clone(),
            skipped: false,
        });
    }

    fn record_definition(&mut self, name: &str) {
        let b = self.branches.last_mut().unwrap();
        b.must_be_defined.insert(name.to_string());
        b.may_be_defined.remove(name);
    }

    fn delete_var(&mut self, name: &str) {
        let b = self.branches.last_mut().unwrap();
        b.must_be_defined.remove(name);
        b.may_be_defined.remove(name);
    }

    fn record_nested_branch(&mut self, state: BranchState) {
        let b = self.branches.last_mut().unwrap();
        if state.skipped {
            b.skipped = true;
            return;
        }
        for n in &state.must_be_defined {
            b.must_be_defined.insert(n.clone());
        }
        for n in &state.may_be_defined {
            b.may_be_defined.insert(n.clone());
        }
        b.may_be_defined.retain(|n| !b.must_be_defined.contains(n));
    }

    fn skip_branch(&mut self) {
        self.branches.last_mut().unwrap().skipped = true;
    }

    fn is_possibly_undefined(&self, name: &str) -> bool {
        self.branches.last().unwrap().may_be_defined.contains(name)
    }

    fn is_undefined(&self, name: &str) -> bool {
        let b = self.branches.last().unwrap();
        !b.may_be_defined.contains(name) && !b.must_be_defined.contains(name)
    }

    fn is_defined_in_a_branch(&self, name: &str) -> bool {
        self.branches
            .iter()
            .any(|b| b.must_be_defined.contains(name) || b.may_be_defined.contains(name))
    }

    fn done(self) -> BranchState {
        let mut all_vars = HashSet::new();
        for b in &self.branches {
            for n in &b.may_be_defined {
                all_vars.insert(n.clone());
            }
            for n in &b.must_be_defined {
                all_vars.insert(n.clone());
            }
        }
        let non_skipped: Vec<&BranchState> = self.branches.iter().filter(|b| !b.skipped).collect();
        let must_be_defined = if !non_skipped.is_empty() {
            let mut m = non_skipped[0].must_be_defined.clone();
            for b in &non_skipped[1..] {
                m.retain(|n| b.must_be_defined.contains(n));
            }
            m
        } else {
            HashSet::new()
        };
        let may_be_defined: HashSet<String> =
            all_vars.difference(&must_be_defined).cloned().collect();
        let skipped = non_skipped.is_empty();
        BranchState {
            must_be_defined,
            may_be_defined,
            skipped,
        }
    }
}

#[derive(Clone, PartialEq)]
enum ScopeType {
    Global,
    Class,
    Func,
    Generator,
}

#[derive(Clone)]
struct Scope {
    branch_stmts: Vec<BranchStatement>,
    scope_type: ScopeType,
    undefined_refs: HashMap<String, usize>,
}

impl Scope {
    fn new(scope_type: ScopeType, initial_state: Option<BranchState>) -> Self {
        Self {
            branch_stmts: vec![BranchStatement::new(initial_state)],
            scope_type,
            undefined_refs: HashMap::new(),
        }
    }

    fn record_undefined_ref(&mut self, name: &str) {
        *self.undefined_refs.entry(name.to_string()).or_insert(0) += 1;
    }

    fn pop_undefined_ref(&mut self, name: &str) -> usize {
        self.undefined_refs.remove(name).unwrap_or(0)
    }
}

#[derive(Clone)]
struct DefinedVariableTracker {
    scopes: Vec<Scope>,
    disable_branch_skip: bool,
    in_finally: bool,
}

impl Default for DefinedVariableTracker {
    fn default() -> Self {
        Self {
            scopes: vec![Scope::new(ScopeType::Global, None)],
            disable_branch_skip: false,
            in_finally: false,
        }
    }
}

impl DefinedVariableTracker {
    fn scope(&self) -> &Scope {
        self.scopes.last().unwrap()
    }

    fn scope_mut(&mut self) -> &mut Scope {
        self.scopes.last_mut().unwrap()
    }

    fn enter_scope(&mut self, scope_type: ScopeType) {
        let initial_state = if scope_type == ScopeType::Generator {
            Some(
                self.scope()
                    .branch_stmts
                    .last()
                    .unwrap()
                    .branches
                    .last()
                    .unwrap()
                    .clone(),
            )
        } else {
            None
        };
        self.scopes.push(Scope::new(scope_type, initial_state));
    }

    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn in_scope(&self, scope_type: ScopeType) -> bool {
        self.scope().scope_type == scope_type
    }

    fn start_branch_statement(&mut self) {
        let initial = self
            .scope()
            .branch_stmts
            .last()
            .unwrap()
            .branches
            .last()
            .unwrap()
            .clone();
        self.scope_mut()
            .branch_stmts
            .push(BranchStatement::new(Some(initial)));
    }

    fn next_branch(&mut self) {
        self.scope_mut()
            .branch_stmts
            .last_mut()
            .unwrap()
            .next_branch();
    }

    fn end_branch_statement(&mut self) {
        let result = self.scope_mut().branch_stmts.pop().unwrap().done();
        self.scope_mut()
            .branch_stmts
            .last_mut()
            .unwrap()
            .record_nested_branch(result);
    }

    fn skip_branch(&mut self) {
        if self.scope().branch_stmts.len() > 1 && !self.disable_branch_skip {
            self.scope_mut()
                .branch_stmts
                .last_mut()
                .unwrap()
                .skip_branch();
        }
    }

    fn record_definition(&mut self, name: &str) {
        self.scope_mut()
            .branch_stmts
            .last_mut()
            .unwrap()
            .record_definition(name);
    }

    fn delete_var(&mut self, name: &str) {
        self.scope_mut()
            .branch_stmts
            .last_mut()
            .unwrap()
            .delete_var(name);
    }

    fn record_undefined_ref(&mut self, name: &str) {
        self.scope_mut().record_undefined_ref(name);
    }

    fn pop_undefined_ref(&mut self, name: &str) -> usize {
        self.scope_mut().pop_undefined_ref(name)
    }

    fn is_possibly_undefined(&self, name: &str) -> bool {
        self.scope()
            .branch_stmts
            .last()
            .unwrap()
            .is_possibly_undefined(name)
    }

    fn is_undefined(&self, name: &str) -> bool {
        self.scope().branch_stmts.last().unwrap().is_undefined(name)
    }

    fn is_defined_in_different_branch(&self, name: &str) -> bool {
        let stmts = &self.scope().branch_stmts;
        let last = stmts.last().unwrap();
        if !last.is_undefined(name) {
            return false;
        }
        for stmt in stmts {
            if stmt.is_defined_in_a_branch(name) {
                return true;
            }
        }
        false
    }

    fn current_must_be_defined(&self) -> HashSet<String> {
        self.scope()
            .branch_stmts
            .last()
            .unwrap()
            .branches
            .last()
            .unwrap()
            .must_be_defined
            .clone()
    }
}

#[derive(Clone)]
struct Loop {
    has_break: bool,
    break_vars: Option<HashSet<String>>,
}

// ---------------------------------------------------------------------------
// Visitor
// ---------------------------------------------------------------------------

struct Visitor<'py> {
    py: Python<'py>,
    tracker: DefinedVariableTracker,
    loops: Vec<Loop>,
    try_depth: usize,
    builtins: HashSet<String>,
    type_map: Option<&'py PyAny>,
    options: Option<&'py PyAny>,
    result: Vec<String>,
}

impl<'py> Visitor<'py> {
    fn func_is_dynamic(&self, node: &PyAny) -> PyResult<bool> {
        node.getattr("is_dynamic")?.call0()?.extract::<bool>()
    }

    fn check_untyped_defs(&self) -> bool {
        self.options
            .and_then(|o| o.getattr("check_untyped_defs").ok())
            .and_then(|v| v.extract::<bool>().ok())
            .unwrap_or(false)
    }

    // -- definition / lvalue handling -------------------------------

    fn process_definition(&mut self, name: &str) -> PyResult<()> {
        if !self.tracker.in_scope(ScopeType::Class) {
            let count = self.tracker.pop_undefined_ref(name);
            for _ in 0..count {
                // variable_may_be_undefined (in loops) or var_used_before_def.
                self.result.push(name.to_string());
            }
        }
        self.tracker.record_definition(name);
        Ok(())
    }

    fn process_lvalue(&mut self, lvalue: &PyAny) -> PyResult<()> {
        if lvalue.is_none() {
            return Ok(());
        }
        let tn = class_name(lvalue)?;
        match tn.as_str() {
            "NameExpr" => {
                let name: String = lvalue.getattr("name")?.extract()?;
                self.process_definition(&name)?;
            }
            "StarExpr" => {
                self.process_lvalue(lvalue.getattr("expr")?)?;
            }
            "ListExpr" | "TupleExpr" => {
                for item in lvalue.getattr("items")?.iter()? {
                    self.process_lvalue(item?)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    // -- dispatch ----------------------------------------------------

    fn visit_node(&mut self, node: &PyAny) -> PyResult<()> {
        let tn = class_name(node)?;
        match tn.as_str() {
            "MypyFile" => self.visit_mypy_file(node),
            "Block" => self.visit_block(node),
            "FuncDef" => self.visit_func_def(node),
            "FuncItem" => self.visit_func(node),
            "OverloadedFuncDef" => self.visit_overloaded_func_def(node),
            "ClassDef" => self.visit_class_def(node),
            "Decorator" => self.visit_decorator(node),
            "ExpressionStmt" => self.visit_expression_stmt(node),
            "AssignmentStmt" => self.visit_assignment_stmt(node),
            "OperatorAssignmentStmt" => self.visit_operator_assignment_stmt(node),
            "WhileStmt" => self.visit_while_stmt(node),
            "ForStmt" => self.visit_for_stmt(node),
            "ReturnStmt" => self.visit_return_stmt(node),
            "AssertStmt" => self.visit_assert_stmt(node),
            "DelStmt" => self.visit_del_stmt(node),
            "IfStmt" => self.visit_if_stmt(node),
            "RaiseStmt" => self.visit_raise_stmt(node),
            "TryStmt" => self.visit_try_stmt(node),
            "WithStmt" => self.visit_with_stmt(node),
            "MatchStmt" => self.visit_match_stmt(node),
            "TypeAliasStmt" => self.visit_type_alias_stmt(node),
            "Import" => self.visit_import(node),
            "ImportFrom" => self.visit_import_from(node),
            "ImportAll" => Ok(()),
            "GlobalDecl" => self.visit_global_decl(node),
            "NonlocalDecl" => self.visit_nonlocal_decl(node),
            "ContinueStmt" => self.visit_continue_stmt(node),
            "PassStmt" => Ok(()),
            "BreakStmt" => self.visit_break_stmt(node),
            "Var" => Ok(()),
            "MemberExpr" => self.visit_member_expr(node),
            "YieldFromExpr" => self.visit_yield_from_expr(node),
            "YieldExpr" => self.visit_yield_expr(node),
            "CallExpr" => self.visit_call_expr(node),
            "OpExpr" => self.visit_op_expr(node),
            "ComparisonExpr" => self.visit_comparison_expr(node),
            "SliceExpr" => self.visit_slice_expr(node),
            "CastExpr" => self.visit_cast_expr(node),
            "TypeFormExpr" => Ok(()),
            "AssertTypeExpr" => self.visit_assert_type_expr(node),
            "RevealExpr" => self.visit_reveal_expr(node),
            "AssignmentExpr" => self.visit_assignment_expr(node),
            "UnaryExpr" => self.visit_unary_expr(node),
            "ListExpr" => self.visit_list_expr(node),
            "TupleExpr" => self.visit_tuple_expr(node),
            "DictExpr" => self.visit_dict_expr(node),
            "TemplateStrExpr" => self.visit_template_str_expr(node),
            "SetExpr" => self.visit_set_expr(node),
            "IndexExpr" => self.visit_index_expr(node),
            "GeneratorExpr" => self.visit_generator_expr(node),
            "DictionaryComprehension" => self.visit_dictionary_comprehension(node),
            "ListComprehension" => self.visit_list_comprehension(node),
            "SetComprehension" => self.visit_set_comprehension(node),
            "ConditionalExpr" => self.visit_conditional_expr(node),
            "TypeApplication" => self.visit_type_application(node),
            "LambdaExpr" => self.visit_lambda_expr(node),
            "StarExpr" => self.visit_star_expr(node),
            "AwaitExpr" => self.visit_await_expr(node),
            "SuperExpr" => self.visit_super_expr(node),
            "NameExpr" => self.visit_name_expr(node),
            "StrExpr" | "IntExpr" | "FloatExpr" | "BytesExpr" | "ComplexExpr" | "EllipsisExpr" => {
                Ok(())
            }
            "AsPattern" => self.visit_as_pattern(node),
            "OrPattern" => self.visit_or_pattern(node),
            "ValuePattern" => self.visit_value_pattern(node),
            "SequencePattern" => self.visit_sequence_pattern(node),
            "StarredPattern" => self.visit_starred_pattern(node),
            "MappingPattern" => self.visit_mapping_pattern(node),
            "ClassPattern" => self.visit_class_pattern(node),
            "SingletonPattern" => Ok(()),
            "TypeAlias" | "TypeVarExpr" | "ParamSpecExpr" | "TypeVarTupleExpr"
            | "TypeAliasExpr" | "NamedTupleExpr" | "TypedDictExpr" | "NewTypeExpr"
            | "EnumCallExpr" | "PromoteExpr" => Ok(()),
            _ => Ok(()),
        }
    }

    // -- statements / definitions ------------------------------------

    fn visit_mypy_file(&mut self, node: &PyAny) -> PyResult<()> {
        for d in node.getattr("defs")?.iter()? {
            self.visit_node(d?)?;
        }
        Ok(())
    }

    fn visit_block(&mut self, node: &PyAny) -> PyResult<()> {
        for s in node.getattr("body")?.iter()? {
            self.visit_node(s?)?;
        }
        Ok(())
    }

    fn visit_func_def(&mut self, node: &PyAny) -> PyResult<()> {
        let name: String = node.getattr("name")?.extract()?;
        self.process_definition(&name)?;
        self.visit_func(node)
    }

    fn visit_func(&mut self, node: &PyAny) -> PyResult<()> {
        if self.func_is_dynamic(node)? && !self.check_untyped_defs() {
            return Ok(());
        }
        let args_obj = node.getattr("arguments")?;
        let args: Vec<&PyAny> = if args_obj.is_none() {
            Vec::new()
        } else {
            args_obj.iter()?.collect::<PyResult<Vec<_>>>()?
        };
        for arg in &args {
            if let Some(init) = opt(arg, "initializer") {
                self.visit_node(init)?;
            }
        }
        self.tracker.enter_scope(ScopeType::Func);
        for arg in &args {
            let var = arg.getattr("variable")?;
            let name: String = var.getattr("name")?.extract()?;
            self.process_definition(&name)?;
        }
        self.visit_node(node.getattr("body")?)?;
        self.tracker.exit_scope();
        Ok(())
    }

    fn visit_overloaded_func_def(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("items")?.iter()? {
            self.visit_node(item?)?;
        }
        if let Some(impl_) = opt(node, "impl") {
            self.visit_node(impl_)?;
        }
        Ok(())
    }

    fn visit_class_def(&mut self, node: &PyAny) -> PyResult<()> {
        let name: String = node.getattr("name")?.extract()?;
        self.process_definition(&name)?;
        self.tracker.enter_scope(ScopeType::Class);
        for d in node.getattr("decorators")?.iter()? {
            self.visit_node(d?)?;
        }
        for b in node.getattr("base_type_exprs")?.iter()? {
            self.visit_node(b?)?;
        }
        if let Some(m) = opt(node, "metaclass") {
            self.visit_node(m)?;
        }
        let kw = node.getattr("keywords")?;
        for v in kw.call_method0("values")?.iter()? {
            self.visit_node(v?)?;
        }
        self.visit_node(node.getattr("defs")?)?;
        if let Some(a) = opt(node, "analyzed") {
            self.visit_node(a)?;
        }
        self.tracker.exit_scope();
        Ok(())
    }

    fn visit_decorator(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("func")?)?;
        self.visit_node(node.getattr("var")?)?;
        for d in node.getattr("decorators")?.iter()? {
            self.visit_node(d?)?;
        }
        Ok(())
    }

    fn visit_expression_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        let expr = node.getattr("expr")?;
        if let Some(tm) = self.type_map {
            let none = self.py.None().into_ref(self.py);
            let typ = tm.call_method1("get", (expr, none))?;
            let skip = if typ.is_none() {
                true
            } else {
                let gpt = self.py.import("mypy.types")?.getattr("get_proper_type")?;
                let proper = gpt.call1((typ,))?;
                class_name(proper)? == "UninhabitedType"
            };
            if skip {
                self.tracker.skip_branch();
            }
        }
        self.visit_node(expr)?;
        Ok(())
    }

    fn visit_assignment_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        for lv in node.getattr("lvalues")?.iter()? {
            self.process_lvalue(lv?)?;
        }
        self.visit_node(node.getattr("rvalue")?)?;
        for lv in node.getattr("lvalues")?.iter()? {
            self.visit_node(lv?)?;
        }
        Ok(())
    }

    fn visit_operator_assignment_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("rvalue")?)?;
        self.visit_node(node.getattr("lvalue")?)?;
        Ok(())
    }

    fn visit_if_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        for e in node.getattr("expr")?.iter()? {
            self.visit_node(e?)?;
        }
        self.tracker.start_branch_statement();
        for b in node.getattr("body")?.iter()? {
            let b = b?;
            if b.getattr("is_unreachable")?.extract::<bool>()? {
                continue;
            }
            self.visit_node(b)?;
            self.tracker.next_branch();
        }
        let unreachable_else: bool = node.getattr("unreachable_else")?.extract()?;
        if unreachable_else {
            self.tracker.skip_branch();
        } else if let Some(else_body) = opt(node, "else_body") {
            if else_body.getattr("is_unreachable")?.extract::<bool>()? {
                self.tracker.skip_branch();
            } else {
                self.visit_node(else_body)?;
            }
        }
        self.tracker.end_branch_statement();
        Ok(())
    }

    fn visit_match_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("subject")?)?;
        self.tracker.start_branch_statement();
        let patterns: Vec<&PyAny> = node.getattr("patterns")?.iter()?.collect::<PyResult<_>>()?;
        let guards: Vec<&PyAny> = node.getattr("guards")?.iter()?.collect::<PyResult<_>>()?;
        let bodies: Vec<&PyAny> = node.getattr("bodies")?.iter()?.collect::<PyResult<_>>()?;
        for i in 0..patterns.len() {
            let pattern = patterns[i];
            self.visit_node(pattern)?;
            if let Some(g) = guards.get(i) {
                if !g.is_none() {
                    self.visit_node(g)?;
                }
            }
            let body = bodies[i];
            if body.getattr("is_unreachable")?.extract::<bool>()? {
                self.tracker.skip_branch();
            } else {
                self.visit_node(body)?;
            }
            if !infer_pattern_always_true(pattern)? {
                self.tracker.next_branch();
            }
        }
        self.tracker.end_branch_statement();
        Ok(())
    }

    fn visit_for_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)?;
        self.process_lvalue(node.getattr("index")?)?;
        self.visit_node(node.getattr("index")?)?;
        self.tracker.start_branch_statement();
        self.loops.push(Loop {
            has_break: false,
            break_vars: None,
        });
        self.visit_node(node.getattr("body")?)?;
        self.tracker.next_branch();
        self.tracker.end_branch_statement();
        if let Some(else_body) = opt(node, "else_body") {
            let has_break = self.loops.last().unwrap().has_break;
            if has_break {
                self.tracker.start_branch_statement();
                let break_vars = self.loops.last().unwrap().break_vars.clone();
                if let Some(bv) = break_vars {
                    for name in &bv {
                        self.tracker.record_definition(name);
                    }
                }
                self.tracker.next_branch();
            }
            self.visit_node(else_body)?;
            if has_break {
                self.tracker.end_branch_statement();
            }
        }
        self.loops.pop();
        Ok(())
    }

    fn visit_return_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(e) = opt(node, "expr") {
            self.visit_node(e)?;
        }
        self.tracker.skip_branch();
        Ok(())
    }

    fn visit_assert_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(e) = opt(node, "expr") {
            self.visit_node(e)?;
        }
        if let Some(m) = opt(node, "msg") {
            self.visit_node(m)?;
        }
        let expr = node.getattr("expr")?;
        if !expr.is_none() && is_false_literal(expr)? {
            self.tracker.skip_branch();
        }
        Ok(())
    }

    fn visit_raise_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(e) = opt(node, "expr") {
            self.visit_node(e)?;
        }
        if let Some(f) = opt(node, "from_expr") {
            self.visit_node(f)?;
        }
        self.tracker.skip_branch();
        Ok(())
    }

    fn visit_continue_stmt(&mut self, _node: &PyAny) -> PyResult<()> {
        self.tracker.skip_branch();
        Ok(())
    }

    fn visit_break_stmt(&mut self, _node: &PyAny) -> PyResult<()> {
        if let Some(loop_el) = self.loops.last_mut() {
            loop_el.has_break = true;
            let must = self.tracker.current_must_be_defined();
            match &mut loop_el.break_vars {
                None => loop_el.break_vars = Some(must),
                Some(bv) => bv.retain(|n| must.contains(n)),
            }
        }
        self.tracker.skip_branch();
        Ok(())
    }

    fn visit_try_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        self.try_depth += 1;
        let has_finally = opt(node, "finally_body").is_some();
        if has_finally {
            let old_tracker = self.tracker.clone();
            self.tracker.disable_branch_skip = true;
            self.process_try_stmt(node)?;
            self.tracker = old_tracker;
        }
        self.process_try_stmt(node)?;
        self.try_depth -= 1;
        Ok(())
    }

    fn process_try_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        self.tracker.start_branch_statement();
        self.visit_node(node.getattr("body")?)?;
        if let Some(else_body) = opt(node, "else_body") {
            self.visit_node(else_body)?;
        }
        let handlers: Vec<&PyAny> = node.getattr("handlers")?.iter()?.collect::<PyResult<_>>()?;
        let types: Vec<&PyAny> = node.getattr("types")?.iter()?.collect::<PyResult<_>>()?;
        let vars: Vec<&PyAny> = node.getattr("vars")?.iter()?.collect::<PyResult<_>>()?;
        if !handlers.is_empty() {
            for (i, handler) in handlers.iter().enumerate() {
                self.tracker.next_branch();
                if let Some(et) = types.get(i) {
                    if !et.is_none() {
                        self.visit_node(et)?;
                    }
                }
                let var_opt = vars.get(i).copied();
                let had_var = var_opt.is_some() && !var_opt.unwrap().is_none();
                if had_var {
                    let v = var_opt.unwrap();
                    let name: String = v.getattr("name")?.extract()?;
                    self.process_definition(&name)?;
                    self.visit_node(v)?;
                }
                self.visit_node(handler)?;
                if had_var {
                    let v = var_opt.unwrap();
                    let name: String = v.getattr("name")?.extract()?;
                    self.tracker.delete_var(&name);
                }
            }
        }
        self.tracker.end_branch_statement();
        if let Some(finally_body) = opt(node, "finally_body") {
            self.tracker.in_finally = true;
            self.visit_node(finally_body)?;
            self.tracker.in_finally = false;
        }
        Ok(())
    }

    fn visit_while_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        let expr = node.getattr("expr")?;
        self.visit_node(expr)?;
        self.tracker.start_branch_statement();
        self.loops.push(Loop {
            has_break: false,
            break_vars: None,
        });
        self.visit_node(node.getattr("body")?)?;
        let has_break = self.loops.last().unwrap().has_break;
        if !is_true_literal(expr)? {
            self.tracker.next_branch();
        }
        self.tracker.end_branch_statement();
        if let Some(else_body) = opt(node, "else_body") {
            if has_break {
                self.tracker.start_branch_statement();
                self.tracker.next_branch();
            }
            self.visit_node(else_body)?;
            if has_break {
                self.tracker.end_branch_statement();
            }
        }
        self.loops.pop();
        Ok(())
    }

    fn visit_lambda_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.tracker.enter_scope(ScopeType::Func);
        self.visit_func(node)?;
        self.tracker.exit_scope();
        Ok(())
    }

    fn visit_generator_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.tracker.enter_scope(ScopeType::Generator);
        for idx in node.getattr("indices")?.iter()? {
            self.process_lvalue(idx?)?;
        }
        let sequences: Vec<&PyAny> = node
            .getattr("sequences")?
            .iter()?
            .collect::<PyResult<_>>()?;
        let indices: Vec<&PyAny> = node.getattr("indices")?.iter()?.collect::<PyResult<_>>()?;
        let condlists: Vec<&PyAny> = node
            .getattr("condlists")?
            .iter()?
            .collect::<PyResult<_>>()?;
        for i in 0..sequences.len() {
            self.visit_node(sequences[i])?;
            self.visit_node(indices[i])?;
            for c in condlists[i].iter()? {
                self.visit_node(c?)?;
            }
        }
        self.visit_node(node.getattr("left_expr")?)?;
        self.tracker.exit_scope();
        Ok(())
    }

    fn visit_dictionary_comprehension(&mut self, node: &PyAny) -> PyResult<()> {
        self.tracker.enter_scope(ScopeType::Generator);
        for idx in node.getattr("indices")?.iter()? {
            self.process_lvalue(idx?)?;
        }
        let sequences: Vec<&PyAny> = node
            .getattr("sequences")?
            .iter()?
            .collect::<PyResult<_>>()?;
        let indices: Vec<&PyAny> = node.getattr("indices")?.iter()?.collect::<PyResult<_>>()?;
        let condlists: Vec<&PyAny> = node
            .getattr("condlists")?
            .iter()?
            .collect::<PyResult<_>>()?;
        for i in 0..sequences.len() {
            self.visit_node(sequences[i])?;
            self.visit_node(indices[i])?;
            for c in condlists[i].iter()? {
                self.visit_node(c?)?;
            }
        }
        self.visit_node(node.getattr("key")?)?;
        self.visit_node(node.getattr("value")?)?;
        self.tracker.exit_scope();
        Ok(())
    }

    fn visit_with_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        let exprs: Vec<&PyAny> = node.getattr("expr")?.iter()?.collect::<PyResult<_>>()?;
        let targets: Vec<&PyAny> = node.getattr("target")?.iter()?.collect::<PyResult<_>>()?;
        for (i, expr) in exprs.iter().enumerate() {
            self.visit_node(expr)?;
            if let Some(t) = targets.get(i) {
                self.process_lvalue(t)?;
            }
        }
        self.visit_node(node.getattr("body")?)?;
        Ok(())
    }

    fn visit_global_decl(&mut self, node: &PyAny) -> PyResult<()> {
        for n in node.getattr("names")?.iter()? {
            let name: String = n?.extract()?;
            self.process_definition(&name)?;
        }
        Ok(())
    }

    fn visit_nonlocal_decl(&mut self, node: &PyAny) -> PyResult<()> {
        for n in node.getattr("names")?.iter()? {
            let name: String = n?.extract()?;
            self.process_definition(&name)?;
        }
        Ok(())
    }

    fn visit_import(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("ids")?.iter()? {
            let item = item?;
            let mod_name: String = item.get_item(0)?.extract()?;
            let alias_obj = item.get_item(1)?;
            if !alias_obj.is_none() {
                let alias: String = alias_obj.extract()?;
                self.tracker.record_definition(&alias);
            } else if let Some(top) = mod_name.split('.').next() {
                if !top.is_empty() {
                    self.tracker.record_definition(top);
                }
            }
        }
        for a in node.getattr("assignments")?.iter()? {
            self.visit_node(a?)?;
        }
        Ok(())
    }

    fn visit_import_from(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("names")?.iter()? {
            let item = item?;
            let mod_name: String = item.get_item(0)?.extract()?;
            let alias_obj = item.get_item(1)?;
            let name = if alias_obj.is_none() {
                mod_name
            } else {
                alias_obj.extract::<String>()?
            };
            self.tracker.record_definition(&name);
        }
        for a in node.getattr("assignments")?.iter()? {
            self.visit_node(a?)?;
        }
        Ok(())
    }

    fn visit_type_alias_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        let name_node = node.getattr("name")?;
        let name: String = name_node.getattr("name")?.extract()?;
        self.tracker.record_definition(&name);
        Ok(())
    }

    fn visit_name_expr(&mut self, node: &PyAny) -> PyResult<()> {
        let name: String = node.getattr("name")?.extract()?;
        if self.builtins.contains(&name) && self.tracker.in_scope(ScopeType::Global) {
            return Ok(());
        }
        if self.tracker.is_possibly_undefined(&name)
            && self.tracker.in_finally == self.tracker.disable_branch_skip
        {
            self.result.push(name.clone());
            self.tracker.record_definition(&name);
        } else if self.tracker.is_defined_in_different_branch(&name) {
            // In loops/try: may_be_undefined; otherwise used_before_def.
            // Both push the name to the result list.
            self.result.push(name.clone());
        } else if self.tracker.is_undefined(&name) {
            self.tracker.record_undefined_ref(&name);
        }
        Ok(())
    }

    fn visit_as_pattern(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(n) = opt(node, "name") {
            self.process_lvalue(n)?;
        }
        if let Some(p) = opt(node, "pattern") {
            self.visit_node(p)?;
        }
        if let Some(n) = opt(node, "name") {
            self.visit_node(n)?;
        }
        Ok(())
    }

    fn visit_starred_pattern(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(c) = opt(node, "capture") {
            self.process_lvalue(c)?;
        }
        if let Some(c) = opt(node, "capture") {
            self.visit_node(c)?;
        }
        Ok(())
    }

    fn visit_del_stmt(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(e) = opt(node, "expr") {
            self.visit_node(e)?;
        }
        Ok(())
    }

    // -- expression default traversals ------------------------------

    fn visit_member_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_yield_from_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_yield_expr(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(e) = opt(node, "expr") {
            self.visit_node(e)?;
        }
        Ok(())
    }

    fn visit_call_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("callee")?)?;
        for a in node.getattr("args")?.iter()? {
            self.visit_node(a?)?;
        }
        if let Some(a) = opt(node, "analyzed") {
            self.visit_node(a)?;
        }
        Ok(())
    }

    fn visit_op_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("left")?)?;
        self.visit_node(node.getattr("right")?)?;
        if let Some(a) = opt(node, "analyzed") {
            self.visit_node(a)?;
        }
        Ok(())
    }

    fn visit_comparison_expr(&mut self, node: &PyAny) -> PyResult<()> {
        for o in node.getattr("operands")?.iter()? {
            self.visit_node(o?)?;
        }
        Ok(())
    }

    fn visit_slice_expr(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(b) = opt(node, "begin_index") {
            self.visit_node(b)?;
        }
        if let Some(e) = opt(node, "end_index") {
            self.visit_node(e)?;
        }
        if let Some(s) = opt(node, "stride") {
            self.visit_node(s)?;
        }
        Ok(())
    }

    fn visit_cast_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_assert_type_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_reveal_expr(&mut self, node: &PyAny) -> PyResult<()> {
        if let Some(e) = opt(node, "expr") {
            self.visit_node(e)?;
        }
        Ok(())
    }

    fn visit_assignment_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("value")?)?;
        self.process_lvalue(node.getattr("target")?)?;
        Ok(())
    }

    fn visit_unary_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_list_expr(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("items")?.iter()? {
            self.visit_node(item?)?;
        }
        Ok(())
    }

    fn visit_set_expr(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("items")?.iter()? {
            self.visit_node(item?)?;
        }
        Ok(())
    }

    fn visit_tuple_expr(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("items")?.iter()? {
            self.visit_node(item?)?;
        }
        Ok(())
    }

    fn visit_dict_expr(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("items")?.iter()? {
            let item = item?;
            let key = item.get_item(0)?;
            if !key.is_none() {
                self.visit_node(key)?;
            }
            let value = item.get_item(1)?;
            self.visit_node(value)?;
        }
        Ok(())
    }

    fn visit_template_str_expr(&mut self, node: &PyAny) -> PyResult<()> {
        for item in node.getattr("items")?.iter()? {
            let item = item?;
            if let Ok(t) = item.downcast::<PyTuple>() {
                self.visit_node(t.get_item(0)?)?;
                let fourth = t.get_item(3)?;
                if !fourth.is_none() {
                    self.visit_node(fourth)?;
                }
            } else {
                self.visit_node(item)?;
            }
        }
        Ok(())
    }

    fn visit_index_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("base")?)?;
        self.visit_node(node.getattr("index")?)?;
        if let Some(a) = opt(node, "analyzed") {
            self.visit_node(a)?;
        }
        Ok(())
    }

    fn visit_list_comprehension(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("generator")?)
    }

    fn visit_set_comprehension(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("generator")?)
    }

    fn visit_conditional_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("cond")?)?;
        self.visit_node(node.getattr("if_expr")?)?;
        self.visit_node(node.getattr("else_expr")?)
    }

    fn visit_type_application(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_star_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_await_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_super_expr(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("call")?)
    }

    fn visit_or_pattern(&mut self, node: &PyAny) -> PyResult<()> {
        for p in node.getattr("patterns")?.iter()? {
            self.visit_node(p?)?;
        }
        Ok(())
    }

    fn visit_value_pattern(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("expr")?)
    }

    fn visit_sequence_pattern(&mut self, node: &PyAny) -> PyResult<()> {
        for p in node.getattr("patterns")?.iter()? {
            self.visit_node(p?)?;
        }
        Ok(())
    }

    fn visit_mapping_pattern(&mut self, node: &PyAny) -> PyResult<()> {
        for k in node.getattr("keys")?.iter()? {
            self.visit_node(k?)?;
        }
        for v in node.getattr("values")?.iter()? {
            self.visit_node(v?)?;
        }
        if let Some(r) = opt(node, "rest") {
            self.visit_node(r)?;
        }
        Ok(())
    }

    fn visit_class_pattern(&mut self, node: &PyAny) -> PyResult<()> {
        self.visit_node(node.getattr("class_ref")?)?;
        for p in node.getattr("positionals")?.iter()? {
            self.visit_node(p?)?;
        }
        for v in node.getattr("keyword_values")?.iter()? {
            self.visit_node(v?)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Builtins extraction + PyO3 entry point
// ---------------------------------------------------------------------------

fn extract_builtins(_py: Python<'_>, names: Option<&PyAny>) -> PyResult<HashSet<String>> {
    let mut set = HashSet::new();
    if let Some(names) = names {
        if let Ok(entry) = names.get_item("__builtins__") {
            if !entry.is_none() {
                let node = entry.getattr("node")?;
                if !node.is_none() {
                    let bdict = node.getattr("names")?;
                    for key in bdict.iter()? {
                        if let Ok(k) = key?.extract::<String>() {
                            set.insert(k);
                        }
                    }
                }
            }
        }
    }
    Ok(set)
}

/// Detect variables that may be used before definition or are only
/// defined in some branches. Mirrors `PossiblyUndefinedVariableVisitor`.
///
/// `node` is the AST to walk (typically an `MypyFile`). `type_map` and
/// `options` mirror the Python constructor args; `names` is the file's
/// `SymbolTable` (used to resolve builtins). All context args are
/// optional and degrade gracefully when absent.
#[pyfunction]
pub fn rust_find_possibly_undefined(
    py: Python<'_>,
    node: &PyAny,
    type_map: Option<&PyAny>,
    options: Option<&PyAny>,
    names: Option<&PyAny>,
) -> PyResult<Vec<String>> {
    let builtins = extract_builtins(py, names)?;
    let mut visitor = Visitor {
        py,
        tracker: DefinedVariableTracker::default(),
        loops: Vec::new(),
        try_depth: 0,
        builtins,
        type_map,
        options,
        result: Vec::new(),
    };
    for name in IMPLICIT_MODULE_ATTRS {
        visitor.tracker.record_definition(name);
    }
    visitor.visit_node(node)?;
    Ok(visitor.result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::PyDict;

    #[test]
    fn branch_state_default_is_empty() {
        let b = BranchState::default();
        assert!(b.must_be_defined.is_empty());
        assert!(b.may_be_defined.is_empty());
        assert!(!b.skipped);
    }

    #[test]
    fn branch_statement_record_and_done() {
        let mut bs = BranchStatement::new(None);
        bs.record_definition("x");
        let state = bs.done();
        assert!(state.must_be_defined.contains("x"));
        assert!(!state.skipped);
    }

    #[test]
    fn tracker_possibly_undefined_after_partial_branch() {
        let mut t = DefinedVariableTracker::default();
        t.start_branch_statement();
        t.record_definition("x");
        t.next_branch();
        t.end_branch_statement();
        assert!(t.is_possibly_undefined("x"));
    }

    #[test]
    fn infer_pattern_as_pattern_none_is_always_true() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
class AsPattern:
    def __init__(self):
        self.pattern = None
node = AsPattern()
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let node = locals.get_item("node").unwrap().unwrap();
            assert!(infer_pattern_always_true(node).unwrap());
        });
    }

    #[test]
    fn is_false_literal_int_zero() {
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(
                r#"
class IntExpr:
    def __init__(self, v):
        self.value = v
node = IntExpr(0)
"#,
                None,
                Some(locals),
            )
            .unwrap();
            let node = locals.get_item("node").unwrap().unwrap();
            assert!(is_false_literal(node).unwrap());
        });
    }

    #[test]
    fn visit_simple_if_partial_definition() {
        // if cond: x = 1; then use x -> x is possibly undefined.
        let code = r#"
class NameExpr:
    def __init__(self, name):
        self.name = name
class IntExpr:
    def __init__(self, v):
        self.value = v
class AssignmentStmt:
    def __init__(self, lvalues, rvalue):
        self.lvalues = lvalues
        self.rvalue = rvalue
class ExpressionStmt:
    def __init__(self, expr):
        self.expr = expr
class Block:
    def __init__(self, body, is_unreachable=False):
        self.body = body
        self.is_unreachable = is_unreachable
class IfStmt:
    def __init__(self, expr, body, else_body=None, unreachable_else=False):
        self.expr = expr
        self.body = body
        self.else_body = else_body
        self.unreachable_else = unreachable_else
class MypyFile:
    def __init__(self, defs):
        self.defs = defs
cond = NameExpr("cond")
body = Block([AssignmentStmt([NameExpr("x")], IntExpr(1))])
use_x = ExpressionStmt(NameExpr("x"))
node = MypyFile([IfStmt([cond], [body]), use_x])
"#;
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(code, None, Some(locals)).unwrap();
            let node = locals.get_item("node").unwrap().unwrap();
            let result = rust_find_possibly_undefined(py, node, None, None, None).unwrap();
            assert_eq!(result, vec!["x".to_string()]);
        });
    }

    #[test]
    fn visit_no_false_positives_for_defined_var() {
        let code = r#"
class NameExpr:
    def __init__(self, name):
        self.name = name
class IntExpr:
    def __init__(self, v):
        self.value = v
class AssignmentStmt:
    def __init__(self, lvalues, rvalue):
        self.lvalues = lvalues
        self.rvalue = rvalue
class ExpressionStmt:
    def __init__(self, expr):
        self.expr = expr
class MypyFile:
    def __init__(self, defs):
        self.defs = defs
node = MypyFile([
    AssignmentStmt([NameExpr("x")], IntExpr(1)),
    ExpressionStmt(NameExpr("x")),
])
"#;
        pyo3::prepare_freethreaded_python();
        Python::with_gil(|py| {
            let locals = PyDict::new(py);
            py.run(code, None, Some(locals)).unwrap();
            let node = locals.get_item("node").unwrap().unwrap();
            let result = rust_find_possibly_undefined(py, node, None, None, None).unwrap();
            assert!(result.is_empty(), "got {:?}", result);
        });
    }
}
