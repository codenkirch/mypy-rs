//! Issue #1632: native fine-grained dependency walk (`DependencyVisitor`).
//!
//! Ports `mypy/server/deps.py::DependencyVisitor` (52 `visit_*` methods plus
//! `visit_instance` via the shared type-trigger helpers) to Rust behind the
//! existing server-deps gate (`_native_server_deps_active`, production-live
//! via `Options.native_type_kernel`).
//!
//! Interface: `rust_walk_dependency_visitor` reads live AST/type objects via
//! PyO3 (no wire bytes, like `_rust_get_type_triggers`) and returns the deps
//! map (`dict[str, set[str]]`). Any unreadable fact defers (`None`) so the
//! pure-Python visitor re-runs; a defer therefore preserves behavior exactly,
//! including re-raising Python `assert`s.
//!
//! Stays Python (per the slice): the public `Deps`/`get_dependencies` API,
//! `MypyFile.plugin_deps` reads are consumed here but owned there, the
//! `alias_deps` defaultdict produced by semanal, and `TraverserVisitor`
//! itself (a subclass surface).

use std::collections::{HashMap, HashSet};

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple, PyType};

use crate::refs::{is_instance, TypeRefs};
use crate::serverdeps::{
    attribute_triggers_walk, class_name_is, collect_triggers, get_attr_or_defer, AttrTriggerCtx,
    DeferError, SeenAliases, TriggerCtx,
};

/// Cached Python references fetched once per walk.
struct NodeRefs<'py> {
    types: TypeRefs<'py>,
    ref_expr: &'py PyType,
    type_info: &'py PyType,
    var: &'py PyType,
    mypy_file: &'py PyType,
    func_def: &'py PyType,
    func_base: &'py PyType,
    partial_type: &'py PyType,
    type_var_expr: &'py PyType,
    named_tuple_expr: &'py PyType,
    typed_dict_expr: &'py PyType,
    enum_call_expr: &'py PyType,
    type_alias_expr: &'py PyType,
    symbol_funcbase_types: Py<PyAny>,
    get_proper_type: &'py PyAny,
    bind_self: &'py PyAny,
    correct_relative_import: &'py PyAny,
    op_methods: &'py PyDict,
    reverse_op_methods: &'py PyDict,
    inplace_methods: &'py PySet,
    unary_op_methods: &'py PyDict,
    ldef: i64,
    gdef: i64,
    mdef: i64,
    reveal_type: i64,
    from_another_any: i64,
    from_unimported: i64,
    make_trigger: &'py PyAny,
    make_wildcard_trigger: &'py PyAny,
}

impl<'py> NodeRefs<'py> {
    fn try_new(py: Python<'py>) -> Result<Self, DeferError> {
        let nodes = py.import("mypy.nodes").map_err(|_| DeferError)?;
        let types_mod = py.import("mypy.types").map_err(|_| DeferError)?;
        let operators = py.import("mypy.operators").map_err(|_| DeferError)?;
        let typeops = py.import("mypy.typeops").map_err(|_| DeferError)?;
        let util = py.import("mypy.util").map_err(|_| DeferError)?;
        let trigger_mod = py.import("mypy.server.trigger").map_err(|_| DeferError)?;
        macro_rules! cls {
            ($mod:expr, $name:literal) => {{
                $mod.getattr($name)
                    .map_err(|_| DeferError)?
                    .downcast::<PyType>()
                    .map_err(|_| DeferError)?
            }};
        }
        let int_const = |mod_: &PyAny, name: &str| -> Result<i64, DeferError> {
            mod_.getattr(name)
                .map_err(|_| DeferError)?
                .extract::<i64>()
                .map_err(|_| DeferError)
        };
        let type_of_any = types_mod.getattr("TypeOfAny").map_err(|_| DeferError)?;
        Ok(NodeRefs {
            types: TypeRefs::try_new(py).map_err(|_| DeferError)?,
            ref_expr: cls!(nodes, "RefExpr"),
            type_info: cls!(nodes, "TypeInfo"),
            var: cls!(nodes, "Var"),
            mypy_file: cls!(nodes, "MypyFile"),
            func_def: cls!(nodes, "FuncDef"),
            func_base: cls!(nodes, "FuncBase"),
            partial_type: cls!(types_mod, "PartialType"),
            type_var_expr: cls!(nodes, "TypeVarExpr"),
            named_tuple_expr: cls!(nodes, "NamedTupleExpr"),
            typed_dict_expr: cls!(nodes, "TypedDictExpr"),
            enum_call_expr: cls!(nodes, "EnumCallExpr"),
            type_alias_expr: cls!(nodes, "TypeAliasExpr"),
            symbol_funcbase_types: nodes
                .getattr("SYMBOL_FUNCBASE_TYPES")
                .map_err(|_| DeferError)?
                .into(),
            get_proper_type: types_mod
                .getattr("get_proper_type")
                .map_err(|_| DeferError)?,
            bind_self: typeops.getattr("bind_self").map_err(|_| DeferError)?,
            correct_relative_import: util
                .getattr("correct_relative_import")
                .map_err(|_| DeferError)?,
            op_methods: operators
                .getattr("op_methods")
                .map_err(|_| DeferError)?
                .downcast::<PyDict>()
                .map_err(|_| DeferError)?,
            reverse_op_methods: operators
                .getattr("reverse_op_methods")
                .map_err(|_| DeferError)?
                .downcast::<PyDict>()
                .map_err(|_| DeferError)?,
            inplace_methods: operators
                .getattr("ops_with_inplace_method")
                .map_err(|_| DeferError)?
                .downcast::<PySet>()
                .map_err(|_| DeferError)?,
            unary_op_methods: operators
                .getattr("unary_op_methods")
                .map_err(|_| DeferError)?
                .downcast::<PyDict>()
                .map_err(|_| DeferError)?,
            ldef: int_const(nodes, "LDEF")?,
            gdef: int_const(nodes, "GDEF")?,
            mdef: int_const(nodes, "MDEF")?,
            reveal_type: int_const(nodes, "REVEAL_TYPE")?,
            from_another_any: type_of_any
                .getattr("from_another_any")
                .map_err(|_| DeferError)?
                .extract::<i64>()
                .map_err(|_| DeferError)?,
            from_unimported: type_of_any
                .getattr("from_unimported_type")
                .map_err(|_| DeferError)?
                .extract::<i64>()
                .map_err(|_| DeferError)?,
            make_trigger: trigger_mod
                .getattr("make_trigger")
                .map_err(|_| DeferError)?,
            make_wildcard_trigger: trigger_mod
                .getattr("make_wildcard_trigger")
                .map_err(|_| DeferError)?,
        })
    }
}

/// Read `obj.name` as a trigger-embeddable string. Mirrors an f-string
/// embedding: `None` formats as `"None"` instead of deferring.
fn fstr_attr(obj: &PyAny, name: &str) -> Result<String, DeferError> {
    let v = get_attr_or_defer(obj, name)?;
    if v.is_none() {
        return Ok("None".to_string());
    }
    v.extract::<String>().map_err(|_| DeferError)
}

/// Read `obj.name` as `Option<String>` (`None` stays `None`).
fn opt_str_attr(obj: &PyAny, name: &str) -> Result<Option<String>, DeferError> {
    let v = get_attr_or_defer(obj, name)?;
    if v.is_none() {
        return Ok(None);
    }
    Ok(Some(v.extract::<String>().map_err(|_| DeferError)?))
}

/// Python `if obj.name:` truthiness (calls `__bool__`, e.g. `TypeInfo`).
fn truthy_attr(obj: &PyAny, name: &str) -> Result<bool, DeferError> {
    get_attr_or_defer(obj, name)?
        .is_true()
        .map_err(|_| DeferError)
}

fn int_attr(obj: &PyAny, name: &str) -> Result<i64, DeferError> {
    get_attr_or_defer(obj, name)?
        .extract::<i64>()
        .map_err(|_| DeferError)
}

fn opt_int_attr(obj: &PyAny, name: &str) -> Result<Option<i64>, DeferError> {
    let v = get_attr_or_defer(obj, name)?;
    if v.is_none() {
        return Ok(None);
    }
    Ok(Some(v.extract::<i64>().map_err(|_| DeferError)?))
}

fn str_attr(obj: &PyAny, name: &str) -> Result<String, DeferError> {
    get_attr_or_defer(obj, name)?
        .extract::<String>()
        .map_err(|_| DeferError)
}

fn list_attr<'a>(obj: &'a PyAny, name: &str) -> Result<&'a PyList, DeferError> {
    get_attr_or_defer(obj, name)?
        .downcast::<PyList>()
        .map_err(|_| DeferError)
}

fn dict_attr<'a>(obj: &'a PyAny, name: &str) -> Result<&'a PyDict, DeferError> {
    get_attr_or_defer(obj, name)?
        .downcast::<PyDict>()
        .map_err(|_| DeferError)
}

/// Index a tuple or list element (mirrors tuple unpacking in the walk).
fn seq_get(obj: &PyAny, idx: usize) -> Result<&PyAny, DeferError> {
    if let Ok(t) = obj.downcast::<PyTuple>() {
        return t.get_item(idx).map_err(|_| DeferError);
    }
    if let Ok(l) = obj.downcast::<PyList>() {
        return l.get_item(idx).map_err(|_| DeferError);
    }
    Err(DeferError)
}

fn class_name_owned(obj: &PyAny) -> Result<String, DeferError> {
    let cls = get_attr_or_defer(obj, "__class__")?;
    let name = get_attr_or_defer(cls, "__name__")?;
    name.downcast::<PyString>()
        .map_err(|_| DeferError)?
        .to_str()
        .map(|s| s.to_string())
        .map_err(|_| DeferError)
}

/// The native `DependencyVisitor` walk state: scope stacks, the deps map,
/// and the read-only inputs (`type_map`, `alias_deps`).
struct DepsWalker<'py> {
    py: Python<'py>,
    refs: NodeRefs<'py>,
    type_map: &'py PyDict,
    alias_deps: &'py PyAny,
    use_logical_deps: bool,
    is_package_init_file: bool,
    scope_module: Option<String>,
    scope_classes: Vec<Py<PyAny>>,
    scope_function: Option<Py<PyAny>>,
    scope_functions: Vec<Py<PyAny>>,
    scope_ignored: usize,
    is_class: bool,
    map: HashMap<String, HashSet<String>>,
}

impl<'py> DepsWalker<'py> {
    fn new(
        py: Python<'py>,
        type_map: &'py PyDict,
        alias_deps: &'py PyAny,
        use_logical_deps: bool,
    ) -> Result<Self, DeferError> {
        Ok(DepsWalker {
            py,
            refs: NodeRefs::try_new(py)?,
            type_map,
            alias_deps,
            use_logical_deps,
            is_package_init_file: false,
            scope_module: None,
            scope_classes: Vec::new(),
            scope_function: None,
            scope_functions: Vec::new(),
            scope_ignored: 0,
            is_class: false,
            map: HashMap::new(),
        })
    }

    // -- Scope (mirrors mypy/scope.py) --

    fn current_module_id(&self) -> Result<String, DeferError> {
        self.scope_module.clone().ok_or(DeferError)
    }

    fn current_target(&self) -> Result<String, DeferError> {
        let module = self.scope_module.clone().ok_or(DeferError)?;
        if let Some(f) = &self.scope_function {
            let fb = f.as_ref(self.py);
            let v = get_attr_or_defer(fb, "fullname")?;
            if v.is_none() {
                return Ok(String::new());
            }
            let s: String = v.extract().map_err(|_| DeferError)?;
            if s.is_empty() {
                return Ok(String::new());
            }
            return Ok(s);
        }
        Ok(module)
    }

    fn current_full_target(&self) -> Result<String, DeferError> {
        let module = self.scope_module.clone().ok_or(DeferError)?;
        if let Some(f) = &self.scope_function {
            return fstr_attr(f.as_ref(self.py), "fullname");
        }
        if let Some(c) = self.scope_classes.last() {
            return fstr_attr(c.as_ref(self.py), "fullname");
        }
        Ok(module)
    }

    fn current_function_name(&self) -> Result<Option<String>, DeferError> {
        match &self.scope_function {
            None => Ok(None),
            Some(f) => Ok(Some(str_attr(f.as_ref(self.py), "name")?)),
        }
    }

    fn enter_function_scope(&mut self, fdef: &PyAny) {
        self.scope_functions.push(fdef.into());
        if self.scope_function.is_none() {
            self.scope_function = Some(fdef.into());
        } else {
            self.scope_ignored += 1;
        }
    }

    fn leave_function_scope(&mut self) {
        self.scope_functions.pop();
        if self.scope_ignored > 0 {
            self.scope_ignored -= 1;
        } else {
            self.scope_function = None;
        }
    }

    fn enter_class_scope(&mut self, info: &PyAny) {
        if self.scope_function.is_none() {
            self.scope_classes.push(info.into());
        } else {
            self.scope_ignored += 1;
        }
    }

    fn leave_class_scope(&mut self) {
        if self.scope_ignored > 0 {
            self.scope_ignored -= 1;
        } else {
            self.scope_classes.pop();
        }
    }

    // -- Trigger / map helpers (mirror deps.py module fns) --

    fn make_trigger(&self, name: &str) -> Result<String, DeferError> {
        let r = self
            .refs
            .make_trigger
            .call1((name,))
            .map_err(|_| DeferError)?;
        r.downcast::<PyString>()
            .map_err(|_| DeferError)?
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| DeferError)
    }

    fn make_wildcard_trigger(&self, module: &str) -> Result<String, DeferError> {
        let r = self
            .refs
            .make_wildcard_trigger
            .call1((module,))
            .map_err(|_| DeferError)?;
        r.downcast::<PyString>()
            .map_err(|_| DeferError)?
            .to_str()
            .map(|s| s.to_string())
            .map_err(|_| DeferError)
    }

    fn add_dependency(
        &mut self,
        trigger: String,
        target: Option<String>,
    ) -> Result<(), DeferError> {
        if trigger.starts_with("<builtins.")
            || trigger.starts_with("<typing.")
            || trigger.starts_with("<mypy_extensions.")
            || trigger.starts_with("<typing_extensions.")
        {
            return Ok(());
        }
        let t = match target {
            Some(t) => t,
            None => self.current_target()?,
        };
        self.map.entry(trigger).or_default().insert(t);
        Ok(())
    }

    fn proper(&self, typ: &PyAny) -> Result<Option<Py<PyAny>>, DeferError> {
        let r = self
            .refs
            .get_proper_type
            .call1((typ,))
            .map_err(|_| DeferError)?;
        if r.is_none() {
            return Ok(None);
        }
        Ok(Some(r.into()))
    }

    fn type_triggers(&self, typ: &PyAny) -> Result<Vec<String>, DeferError> {
        let mut ctx = TriggerCtx {
            use_logical_deps: self.use_logical_deps,
            refs: &self.refs.types,
            seen: SeenAliases::default(),
            make_trigger: self.refs.make_trigger,
            make_wildcard_trigger: self.refs.make_wildcard_trigger,
        };
        let mut out = Vec::new();
        collect_triggers(self.py, typ, &mut ctx, &mut out)?;
        Ok(out)
    }

    fn attr_triggers(&self, typ: &PyAny, name: &str) -> Result<Vec<String>, DeferError> {
        let ctx = AttrTriggerCtx {
            name,
            refs: &self.refs.types,
            make_trigger: self.refs.make_trigger,
            get_proper_type: self.refs.get_proper_type,
        };
        attribute_triggers_walk(self.py, typ, &ctx)
    }

    fn add_type_dependencies(
        &mut self,
        typ: &PyAny,
        target: Option<String>,
    ) -> Result<(), DeferError> {
        for trigger in self.type_triggers(typ)? {
            self.add_dependency(trigger, target.clone())?;
        }
        Ok(())
    }

    fn add_attribute_dependency(&mut self, typ: &PyAny, name: &str) -> Result<(), DeferError> {
        for trigger in self.attr_triggers(typ, name)? {
            self.add_dependency(trigger, None)?;
        }
        Ok(())
    }

    fn add_attribute_dependency_for_expr(
        &mut self,
        e: &PyAny,
        name: &str,
    ) -> Result<(), DeferError> {
        if let Some(typ) = self.type_map.get_item(e).map_err(|_| DeferError)? {
            self.add_attribute_dependency(typ, name)?;
        }
        Ok(())
    }

    fn add_iter_dependency(&mut self, node: &PyAny) -> Result<(), DeferError> {
        if let Some(typ) = self.type_map.get_item(node).map_err(|_| DeferError)? {
            if typ.is_true().map_err(|_| DeferError)? {
                self.add_attribute_dependency(typ, "__iter__")?;
            }
        }
        Ok(())
    }

    fn add_type_alias_deps(&mut self, target: &str) -> Result<(), DeferError> {
        // `PyDict::get_item` bypasses `defaultdict.__missing__`, so probing
        // never inserts empty entries (mirrors `if target in self.alias_deps`).
        let dict = self
            .alias_deps
            .downcast::<PyDict>()
            .map_err(|_| DeferError)?;
        if let Some(aliases) = dict.get_item(target).map_err(|_| DeferError)? {
            for alias in aliases.iter().map_err(|_| DeferError)? {
                let name: String = alias
                    .map_err(|_| DeferError)?
                    .extract()
                    .map_err(|_| DeferError)?;
                let trigger = self.make_trigger(&name)?;
                self.add_dependency(trigger, None)?;
            }
        }
        Ok(())
    }

    fn non_trivial_bases(&self, info: &PyAny) -> Result<Vec<Py<PyAny>>, DeferError> {
        let mro = list_attr(info, "mro")?;
        let mut out = Vec::new();
        for (i, base) in mro.iter().enumerate() {
            if i == 0 {
                continue;
            }
            if str_attr(base, "fullname")? != "builtins.object" {
                out.push(base.into());
            }
        }
        Ok(out)
    }

    fn has_user_bases(&self, info: &PyAny) -> Result<bool, DeferError> {
        let mro = list_attr(info, "mro")?;
        for (i, base) in mro.iter().enumerate() {
            if i == 0 {
                continue;
            }
            let module = str_attr(base, "module_name")?;
            if module != "builtins" && module != "typing" && module != "enum" {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn correct_relative_import(
        &self,
        cur_mod: &str,
        relative: i64,
        target: &str,
    ) -> Result<String, DeferError> {
        let r = self
            .refs
            .correct_relative_import
            .call1((cur_mod, relative, target, self.is_package_init_file))
            .map_err(|_| DeferError)?;
        let tup = r.downcast::<PyTuple>().map_err(|_| DeferError)?;
        tup.get_item(0)
            .map_err(|_| DeferError)?
            .extract::<String>()
            .map_err(|_| DeferError)
    }

    // -- Main dispatch --

    fn walk(&mut self, obj: &PyAny) -> Result<(), DeferError> {
        let name = class_name_owned(obj)?;
        match name.as_str() {
            "MypyFile" => self.visit_mypy_file(obj),
            "FuncDef" => self.visit_func_def(obj),
            "Decorator" => self.visit_decorator(obj),
            "ClassDef" => self.visit_class_def(obj),
            "NewTypeExpr" => self.visit_newtype_expr(obj),
            "Import" => self.visit_import(obj),
            "ImportFrom" => self.visit_import_from(obj),
            "ImportAll" => self.visit_import_all(obj),
            "Block" => self.visit_block(obj),
            "AssignmentStmt" => self.visit_assignment_stmt(obj),
            "OperatorAssignmentStmt" => self.visit_operator_assignment_stmt(obj),
            "ForStmt" => self.visit_for_stmt(obj),
            "WithStmt" => self.visit_with_stmt(obj),
            "DelStmt" => self.visit_del_stmt(obj),
            "NameExpr" => self.visit_name_expr(obj),
            "MemberExpr" => self.visit_member_expr(obj),
            "SuperExpr" => self.visit_super_expr(obj),
            "CallExpr" => self.visit_call_expr(obj),
            "CastExpr" => self.visit_cast_expr(obj),
            "TypeFormExpr" => self.visit_type_form_expr(obj),
            "AssertTypeExpr" => self.visit_assert_type_expr(obj),
            "TypeApplication" => self.visit_type_application(obj),
            "IndexExpr" => self.visit_index_expr(obj),
            "UnaryExpr" => self.visit_unary_expr(obj),
            "OpExpr" => self.visit_op_expr(obj),
            "ComparisonExpr" => self.visit_comparison_expr(obj),
            "GeneratorExpr" => self.visit_generator_expr(obj),
            "DictionaryComprehension" => self.visit_dictionary_comprehension(obj),
            "StarExpr" => self.visit_star_expr(obj),
            "YieldFromExpr" => self.visit_yield_from_expr(obj),
            "AwaitExpr" => self.visit_await_expr(obj),
            // Pure-traversal nodes (mirror TraverserVisitor exactly).
            "OverloadedFuncDef" => self.traverse_overloaded_func_def(obj),
            "ExpressionStmt" => self.traverse_single_child(obj, "expr"),
            "IfStmt" => self.traverse_if_stmt(obj),
            "WhileStmt" => self.traverse_while_stmt(obj),
            "ReturnStmt" => self.traverse_optional_child(obj, "expr"),
            "AssertStmt" => self.traverse_assert_stmt(obj),
            "RaiseStmt" => self.traverse_raise_stmt(obj),
            "TryStmt" => self.traverse_try_stmt(obj),
            "MatchStmt" => self.traverse_match_stmt(obj),
            "TypeAliasStmt" => self.traverse_two_children(obj, "name", "value"),
            "YieldExpr" => self.traverse_truthy_child(obj, "expr"),
            "SliceExpr" => self.traverse_slice_expr(obj),
            "RevealExpr" => self.traverse_reveal_expr(obj),
            "AssignmentExpr" => self.traverse_two_children(obj, "target", "value"),
            "ListExpr" | "TupleExpr" | "SetExpr" => self.traverse_list_field(obj, "items"),
            "DictExpr" => self.traverse_dict_expr(obj),
            "TemplateStrExpr" => self.traverse_template_str_expr(obj),
            "ConditionalExpr" => self.traverse_conditional_expr(obj),
            "LambdaExpr" => self.visit_func(obj),
            "ListComprehension" => self.traverse_single_child(obj, "generator"),
            "SetComprehension" => self.traverse_single_child(obj, "generator"),
            "AsPattern" => self.traverse_as_pattern(obj),
            "OrPattern" | "SequencePattern" => self.traverse_list_field(obj, "patterns"),
            "ValuePattern" => self.traverse_single_child(obj, "expr"),
            "StarredPattern" => self.traverse_optional_child(obj, "capture"),
            "MappingPattern" => self.traverse_mapping_pattern(obj),
            "ClassPattern" => self.traverse_class_pattern(obj),
            // Leaves (TraverserVisitor no-ops).
            "StrExpr" | "IntExpr" | "FloatExpr" | "BytesExpr" | "ComplexExpr" | "EllipsisExpr"
            | "Var" | "ContinueStmt" | "PassStmt" | "BreakStmt" | "TempNode" | "NonlocalDecl"
            | "GlobalDecl" | "TypeVarExpr" | "ParamSpecExpr" | "TypeVarTupleExpr"
            | "TypeAliasExpr" | "TypeAlias" | "NamedTupleExpr" | "TypedDictExpr"
            | "EnumCallExpr" | "SingletonPattern" | "PromoteExpr" => Ok(()),
            _ => Err(DeferError),
        }
    }

    // -- DependencyVisitor methods (mirror deps.py) --

    fn visit_mypy_file(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.scope_module = Some(str_attr(o, "fullname")?);
        self.scope_classes.clear();
        self.scope_function = None;
        self.scope_functions.clear();
        self.scope_ignored = 0;
        self.is_package_init_file = o
            .call_method0("is_package_init_file")
            .map_err(|_| DeferError)?
            .is_true()
            .map_err(|_| DeferError)?;
        let target = self.current_target()?;
        self.add_type_alias_deps(&target)?;
        let plugin_deps = dict_attr(o, "plugin_deps")?;
        for (trigger, targets) in plugin_deps.iter() {
            let t: String = trigger.extract().map_err(|_| DeferError)?;
            for v in targets.iter().map_err(|_| DeferError)? {
                let s: String = v
                    .map_err(|_| DeferError)?
                    .extract()
                    .map_err(|_| DeferError)?;
                self.map.entry(t.clone()).or_default().insert(s);
            }
        }
        for d in list_attr(o, "defs")?.iter() {
            self.walk(d)?;
        }
        self.scope_module = None;
        Ok(())
    }

    /// Mirror of TraverserVisitor.visit_func (the `super()` body).
    fn visit_func(&mut self, o: &PyAny) -> Result<(), DeferError> {
        if !get_attr_or_defer(o, "arguments")?.is_none() {
            let args = list_attr(o, "arguments")?;
            for arg in args.iter() {
                let init = get_attr_or_defer(arg, "initializer")?;
                if !init.is_none() {
                    self.walk(init)?;
                }
            }
        }
        self.walk(get_attr_or_defer(o, "body")?)?;
        Ok(())
    }

    fn visit_func_def(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.enter_function_scope(o);
        let target = self.current_target()?;
        if truthy_attr(o, "type")? {
            let typ = get_attr_or_defer(o, "type")?;
            let bound: Option<Py<PyAny>>;
            let signature: &PyAny =
                if self.is_class && is_instance(typ, self.refs.types.function_like) {
                    let b = self.refs.bind_self.call1((typ,)).map_err(|_| DeferError)?;
                    bound = Some(b.into());
                    bound
                        .as_ref()
                        .map(|b| b.as_ref(self.py))
                        .ok_or(DeferError)?
                } else {
                    typ
                };
            for trigger in self.type_triggers(signature)? {
                self.add_dependency(trigger.clone(), None)?;
                let wt = self.make_trigger(&target)?;
                self.add_dependency(trigger, Some(wt))?;
            }
        }
        if truthy_attr(o, "info")? {
            let info = get_attr_or_defer(o, "info")?;
            let o_name = str_attr(o, "name")?;
            for base in self.non_trivial_bases(info)? {
                if !self.use_logical_deps || (o_name != "__init__" && o_name != "__new__") {
                    let bfn = str_attr(base.as_ref(self.py), "fullname")?;
                    let t = self.make_trigger(&format!("{bfn}.{o_name}"))?;
                    self.add_dependency(t, None)?;
                }
            }
        }
        let cur = self.current_target()?;
        self.add_type_alias_deps(&cur)?;
        self.visit_func(o)?;
        for ex in list_attr(o, "expanded")?.iter() {
            if ex.as_ptr() == o.as_ptr() {
                continue;
            }
            if is_instance(ex, self.refs.func_def) {
                self.visit_func(ex)?;
            }
        }
        self.leave_function_scope();
        Ok(())
    }

    fn visit_decorator(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let func = get_attr_or_defer(o, "func")?;
        if !self.use_logical_deps {
            let is_overload = truthy_attr(func, "is_overload")?;
            if !is_overload && self.current_function_name()?.is_none() {
                let t = self.make_trigger(&fstr_attr(func, "fullname")?)?;
                self.add_dependency(t, None)?;
            }
        } else {
            let func_fullname = fstr_attr(func, "fullname")?;
            for d in list_attr(o, "decorators")?.iter() {
                let mut tname: Option<String> = None;
                if is_instance(d, self.refs.ref_expr) {
                    tname = opt_str_attr(d, "fullname")?.filter(|s| !s.is_empty());
                }
                if class_name_is(d, "CallExpr") {
                    let callee = get_attr_or_defer(d, "callee")?;
                    if is_instance(callee, self.refs.ref_expr) {
                        tname = opt_str_attr(callee, "fullname")?.filter(|s| !s.is_empty());
                    }
                }
                if let Some(tn) = tname {
                    let a = self.make_trigger(&tn)?;
                    let b = self.make_trigger(&func_fullname)?;
                    self.add_dependency(a, Some(b))?;
                }
            }
        }
        self.walk(func)?;
        for d in list_attr(o, "decorators")?.iter() {
            self.walk(d)?;
        }
        Ok(())
    }

    fn visit_class_def(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let info = get_attr_or_defer(o, "info")?;
        if info.is_none() {
            return Err(DeferError);
        }
        self.enter_class_scope(info);
        let target = self.current_full_target()?;
        let wt = self.make_trigger(&target)?;
        self.add_dependency(wt, Some(target.clone()))?;
        let old_is_class = self.is_class;
        self.is_class = true;
        for tv in list_attr(o, "type_vars")?.iter() {
            let t = self.make_trigger(&fstr_attr(tv, "fullname")?)?;
            self.add_dependency(t, Some(target.clone()))?;
        }
        self.process_type_info(info)?;
        for d in list_attr(o, "decorators")?.iter() {
            self.walk(d)?;
        }
        for b in list_attr(o, "base_type_exprs")?.iter() {
            self.walk(b)?;
        }
        if truthy_attr(o, "metaclass")? {
            self.walk(get_attr_or_defer(o, "metaclass")?)?;
        }
        for (_, v) in dict_attr(o, "keywords")?.iter() {
            self.walk(v)?;
        }
        self.walk(get_attr_or_defer(o, "defs")?)?;
        if truthy_attr(o, "analyzed")? {
            self.walk(get_attr_or_defer(o, "analyzed")?)?;
        }
        self.is_class = old_is_class;
        self.leave_class_scope();
        Ok(())
    }

    fn visit_newtype_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        if !get_attr_or_defer(o, "info")?.is_none() {
            let info = get_attr_or_defer(o, "info")?;
            self.enter_class_scope(info);
            self.process_type_info(info)?;
            self.leave_class_scope();
        }
        Ok(())
    }

    fn process_type_info(&mut self, info: &PyAny) -> Result<(), DeferError> {
        let target = self.current_full_target()?;
        for base in list_attr(info, "bases")?.iter() {
            self.add_type_dependencies(base, Some(target.clone()))?;
        }
        for field in ["tuple_type", "typeddict_type", "declared_metaclass"] {
            if !get_attr_or_defer(info, field)?.is_none() {
                let wt = self.make_trigger(&target)?;
                self.add_type_dependencies(get_attr_or_defer(info, field)?, Some(wt))?;
            }
        }
        if truthy_attr(info, "is_protocol")? {
            let mro = list_attr(info, "mro")?;
            let n = mro.len();
            for base_info in mro.iter().take(n.saturating_sub(1)) {
                let wt = self.make_wildcard_trigger(&str_attr(base_info, "fullname")?)?;
                let tgt = self.make_trigger(&target)?;
                self.add_dependency(wt, Some(tgt))?;
            }
        }
        let cur = self.current_target()?;
        self.add_type_alias_deps(&cur)?;
        let info_fullname = str_attr(info, "fullname")?;
        let names = dict_attr(info, "names")?;
        for (name_any, symnode) in names.iter() {
            let name: String = name_any.extract().map_err(|_| DeferError)?;
            let inner = get_attr_or_defer(symnode, "node")?;
            if is_instance(inner, self.refs.var) {
                if truthy_attr(inner, "is_initialized_in_class")? && self.has_user_bases(info)? {
                    let t = self.make_trigger(&format!("{info_fullname}.{name}"))?;
                    self.add_dependency(t, None)?;
                }
                for base in self.non_trivial_bases(info)? {
                    let bfn = str_attr(base.as_ref(self.py), "fullname")?;
                    let a = self.make_trigger(&format!("{bfn}.{name}"))?;
                    let b = self.make_trigger(&format!("{info_fullname}.{name}"))?;
                    self.add_dependency(a, Some(b))?;
                }
            }
        }
        for base in self.non_trivial_bases(info)? {
            let bpy = base.as_ref(self.py);
            let bfn = str_attr(bpy, "fullname")?;
            let bnames = dict_attr(bpy, "names")?;
            for (bname_any, _) in bnames.iter() {
                let bname: String = bname_any.extract().map_err(|_| DeferError)?;
                if self.use_logical_deps {
                    if !names.contains(&bname).map_err(|_| DeferError)? {
                        continue;
                    }
                    if bname == "__init__" || bname == "__new__" {
                        continue;
                    }
                }
                let a = self.make_trigger(&format!("{bfn}.{bname}"))?;
                let b = self.make_trigger(&format!("{info_fullname}.{bname}"))?;
                self.add_dependency(a, Some(b))?;
            }
            if !self.use_logical_deps {
                let init_a = self.make_trigger(&format!("{bfn}.__init__"))?;
                let init_b = self.make_trigger(&format!("{info_fullname}.__init__"))?;
                self.add_dependency(init_a, Some(init_b.clone()))?;
                let new_a = self.make_trigger(&format!("{bfn}.__new__"))?;
                let new_b = self.make_trigger(&format!("{info_fullname}.__new__"))?;
                self.add_dependency(new_a, Some(new_b))?;
                let abs_a = self.make_trigger(&format!("{bfn}.(abstract)"))?;
                self.add_dependency(abs_a, Some(init_b.clone()))?;
                let abs_self = self.make_trigger(&format!("{bfn}.(abstract)"))?;
                self.add_dependency(abs_self, None)?;
            }
        }
        Ok(())
    }

    fn visit_import(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let target = self.current_target()?;
        for id_pair in list_attr(o, "ids")?.iter() {
            let id: String = seq_get(id_pair, 0)
                .map_err(|_| DeferError)?
                .extract()
                .map_err(|_| DeferError)?;
            let t = self.make_trigger(&id)?;
            self.add_dependency(t, Some(target.clone()))?;
        }
        Ok(())
    }

    fn visit_import_from(&mut self, o: &PyAny) -> Result<(), DeferError> {
        if self.use_logical_deps {
            return Ok(());
        }
        let module_id = self.current_module_id()?;
        let relative = int_attr(o, "relative")?;
        let id = str_attr(o, "id")?;
        let mod_id = self.correct_relative_import(&module_id, relative, &id)?;
        let t = self.make_trigger(&mod_id)?;
        self.add_dependency(t, None)?;
        for name_pair in list_attr(o, "names")?.iter() {
            let nm: String = seq_get(name_pair, 0)
                .map_err(|_| DeferError)?
                .extract()
                .map_err(|_| DeferError)?;
            let t2 = self.make_trigger(&format!("{mod_id}.{nm}"))?;
            self.add_dependency(t2, None)?;
        }
        Ok(())
    }

    fn visit_import_all(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let module_id = self.current_module_id()?;
        let relative = int_attr(o, "relative")?;
        let id = str_attr(o, "id")?;
        let mod_id = self.correct_relative_import(&module_id, relative, &id)?;
        let t = self.make_wildcard_trigger(&mod_id)?;
        self.add_dependency(t, None)?;
        Ok(())
    }

    fn visit_block(&mut self, o: &PyAny) -> Result<(), DeferError> {
        if !truthy_attr(o, "is_unreachable")? {
            for s in list_attr(o, "body")?.iter() {
                self.walk(s)?;
            }
        }
        Ok(())
    }

    fn visit_call_children(&mut self, e: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(e, "callee")?)?;
        for a in list_attr(e, "args")?.iter() {
            self.walk(a)?;
        }
        if truthy_attr(e, "analyzed")? {
            self.walk(get_attr_or_defer(e, "analyzed")?)?;
        }
        Ok(())
    }

    fn visit_assignment_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let rvalue = get_attr_or_defer(o, "rvalue")?;
        let is_call = class_name_is(rvalue, "CallExpr");
        let analyzed: Option<&PyAny> = if is_call {
            let a = get_attr_or_defer(rvalue, "analyzed")?;
            if a.is_none() {
                None
            } else {
                Some(a)
            }
        } else {
            None
        };
        if let Some(a) = analyzed.filter(|a| is_instance(a, self.refs.type_var_expr)) {
            let t = self.make_trigger(&fstr_attr(a, "fullname")?)?;
            self.add_type_dependencies(get_attr_or_defer(a, "upper_bound")?, Some(t.clone()))?;
            for val in list_attr(a, "values")?.iter() {
                self.add_type_dependencies(val, Some(t.clone()))?;
            }
            self.visit_call_children(rvalue)?;
        } else if let Some(a) = analyzed.filter(|a| is_instance(a, self.refs.named_tuple_expr)) {
            let info = get_attr_or_defer(a, "info")?;
            let prefix = format!(
                "{}.{}",
                self.current_full_target()?,
                fstr_attr(info, "name")?
            );
            for (name_any, symnode) in dict_attr(info, "names")?.iter() {
                let sname: String = name_any.extract().map_err(|_| DeferError)?;
                let inner = get_attr_or_defer(symnode, "node")?;
                if !sname.starts_with('_') && is_instance(inner, self.refs.var) {
                    let typ = get_attr_or_defer(inner, "type")?;
                    if !typ.is_none() {
                        self.add_type_dependencies(typ, None)?;
                        let wt = self.make_trigger(&prefix)?;
                        self.add_type_dependencies(typ, Some(wt))?;
                        let at = self.make_trigger(&format!("{prefix}.{sname}"))?;
                        self.add_type_dependencies(typ, Some(at))?;
                    }
                }
            }
        } else if let Some(a) = analyzed.filter(|a| is_instance(a, self.refs.typed_dict_expr)) {
            let info = get_attr_or_defer(a, "info")?;
            let tt = get_attr_or_defer(info, "typeddict_type")?;
            if tt.is_none() {
                return Err(DeferError);
            }
            let prefix = format!(
                "{}.{}",
                self.current_full_target()?,
                fstr_attr(info, "name")?
            );
            let wt = self.make_trigger(&prefix)?;
            self.add_type_dependencies(tt, Some(wt))?;
        } else if let Some(a) = analyzed.filter(|a| is_instance(a, self.refs.enum_call_expr)) {
            let info = get_attr_or_defer(a, "info")?;
            for (_, symnode) in dict_attr(info, "names")?.iter() {
                let inner = get_attr_or_defer(symnode, "node")?;
                if is_instance(inner, self.refs.var) {
                    let typ = get_attr_or_defer(inner, "type")?;
                    if !typ.is_none() {
                        self.add_type_dependencies(typ, None)?;
                    }
                }
            }
        } else if truthy_attr(o, "is_alias_def")? {
            let lvalues = list_attr(o, "lvalues")?;
            if lvalues.len() != 1 {
                return Err(DeferError);
            }
            let lvalue = lvalues.get_item(0).map_err(|_| DeferError)?;
            if !class_name_is(lvalue, "NameExpr") {
                return Err(DeferError);
            }
            let raw = self.type_map.get_item(lvalue).map_err(|_| DeferError)?;
            let proper: Option<Py<PyAny>> = match raw {
                None => None,
                Some(t) => self.proper(t)?,
            };
            if let Some(ref p) = proper {
                let pb = p.as_ref(self.py);
                if is_instance(pb, self.refs.types.function_like)
                    && pb
                        .call_method0("is_type_obj")
                        .map_err(|_| DeferError)?
                        .is_true()
                        .map_err(|_| DeferError)?
                {
                    let fullname = str_attr(
                        pb.call_method0("type_object").map_err(|_| DeferError)?,
                        "fullname",
                    )?;
                    let t1 = self.make_trigger(&format!("{fullname}.__init__"))?;
                    self.add_dependency(t1, None)?;
                    let t2 = self.make_trigger(&format!("{fullname}.__new__"))?;
                    self.add_dependency(t2, None)?;
                }
            }
            if class_name_is(rvalue, "IndexExpr")
                && !get_attr_or_defer(rvalue, "analyzed")?.is_none()
                && is_instance(
                    get_attr_or_defer(rvalue, "analyzed")?,
                    self.refs.type_alias_expr,
                )
            {
                let alias_node = get_attr_or_defer(get_attr_or_defer(rvalue, "analyzed")?, "node")?;
                self.add_type_dependencies(get_attr_or_defer(alias_node, "target")?, None)?;
            } else if let Some(ref p) = proper {
                if p.as_ref(self.py).is_true().map_err(|_| DeferError)? {
                    let q = p.clone();
                    self.add_type_dependencies(q.as_ref(self.py), None)?;
                }
            }
        } else {
            self.walk(rvalue)?;
            let lvalues = list_attr(o, "lvalues")?;
            for l in lvalues.iter() {
                self.walk(l)?;
            }
            for lvalue in lvalues.iter() {
                self.process_lvalue(lvalue)?;
            }
            let n = lvalues.len();
            for i in 0..n {
                let lvalue = lvalues.get_item(i).map_err(|_| DeferError)?;
                let next: &PyAny = if i + 1 < n {
                    lvalues.get_item(i + 1).map_err(|_| DeferError)?
                } else {
                    rvalue
                };
                if class_name_is(lvalue, "TupleExpr") {
                    self.add_attribute_dependency_for_expr(next, "__iter__")?;
                }
            }
            if truthy_attr(o, "type")? {
                self.add_type_dependencies(get_attr_or_defer(o, "type")?, None)?;
            }
        }
        // Logical-deps tail (runs for every branch above).
        if self.use_logical_deps
            && get_attr_or_defer(o, "unanalyzed_type")?.is_none()
            && class_name_is(rvalue, "CallExpr")
        {
            let callee = get_attr_or_defer(rvalue, "callee")?;
            if is_instance(callee, self.refs.ref_expr)
                && opt_str_attr(callee, "fullname")?
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
            {
                let node = get_attr_or_defer(callee, "node")?;
                let fname: Option<String> =
                    if !node.is_none() && is_instance(node, self.refs.type_info) {
                        let init = node
                            .call_method1("get", ("__init__",))
                            .map_err(|_| DeferError)?;
                        if init.is_none() {
                            None
                        } else if init.is_true().map_err(|_| DeferError)?
                            && get_attr_or_defer(init, "node")?
                                .is_instance(self.refs.symbol_funcbase_types.as_ref(self.py))
                                .map_err(|_| DeferError)?
                        {
                            opt_str_attr(get_attr_or_defer(init, "node")?, "fullname")?
                        } else {
                            None
                        }
                    } else {
                        opt_str_attr(callee, "fullname")?
                    };
                let fname = match fname {
                    Some(s) if !s.is_empty() => s,
                    _ => return Ok(()),
                };
                for lv in list_attr(o, "lvalues")?.iter() {
                    if is_instance(lv, self.refs.ref_expr)
                        && opt_str_attr(lv, "fullname")?
                            .map(|s| !s.is_empty())
                            .unwrap_or(false)
                        && truthy_attr(lv, "is_new_def")?
                    {
                        if opt_int_attr(lv, "kind")? == Some(self.refs.ldef) {
                            return Ok(());
                        }
                        let a = self.make_trigger(&fname)?;
                        let lvfn = fstr_attr(lv, "fullname")?;
                        let b = self.make_trigger(&lvfn)?;
                        self.add_dependency(a, Some(b))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn process_lvalue(&mut self, lvalue: &PyAny) -> Result<(), DeferError> {
        let name = class_name_owned(lvalue)?;
        match name.as_str() {
            "IndexExpr" => {
                self.add_operator_method_dependency(
                    get_attr_or_defer(lvalue, "base")?,
                    "__setitem__",
                )?;
            }
            "NameExpr" => {
                if matches!(opt_int_attr(lvalue, "kind")?, Some(k) if k == self.refs.mdef || k == self.refs.gdef)
                {
                    let triggers = self
                        .non_partial_lvalue_triggers(lvalue)?
                        .unwrap_or_default();
                    let at = self.make_trigger(&format!(
                        "{}.{}",
                        self.current_full_target()?,
                        fstr_attr(lvalue, "name")?
                    ))?;
                    for tt in triggers {
                        self.add_dependency(tt, Some(at.clone()))?;
                    }
                }
            }
            "MemberExpr" => {
                if self.is_self_member_ref(lvalue)? && truthy_attr(lvalue, "is_new_def")? {
                    let node = get_attr_or_defer(lvalue, "node")?;
                    if is_instance(node, self.refs.var) {
                        let info = get_attr_or_defer(node, "info")?;
                        if truthy_attr(node, "info")? && self.has_user_bases(info)? {
                            let t = self.make_trigger(&format!(
                                "{}.{}",
                                str_attr(info, "fullname")?,
                                fstr_attr(lvalue, "name")?
                            ))?;
                            self.add_dependency(t, None)?;
                        }
                    }
                }
                if get_attr_or_defer(lvalue, "kind")?.is_none() {
                    let obj = get_attr_or_defer(lvalue, "expr")?;
                    let raw = match self.type_map.get_item(obj).map_err(|_| DeferError)? {
                        Some(t) => t,
                        None => return Ok(()),
                    };
                    let object_type = self.proper(raw)?;
                    let type_triggers = self
                        .non_partial_lvalue_triggers(lvalue)?
                        .unwrap_or_default();
                    let lname = fstr_attr(lvalue, "name")?;
                    let attr_triggers = match object_type {
                        Some(ref p) => self.attr_triggers(p.as_ref(self.py), &lname)?,
                        None => Vec::new(),
                    };
                    for at in attr_triggers {
                        for tt in type_triggers.clone() {
                            self.add_dependency(tt, Some(at.clone()))?;
                        }
                    }
                }
            }
            "TupleExpr" => {
                for item in list_attr(lvalue, "items")?.iter() {
                    self.process_lvalue(item)?;
                }
            }
            "StarExpr" => {
                self.process_lvalue(get_attr_or_defer(lvalue, "expr")?)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn is_self_member_ref(&self, e: &PyAny) -> Result<bool, DeferError> {
        let inner = get_attr_or_defer(e, "expr")?;
        if !class_name_is(inner, "NameExpr") {
            return Ok(false);
        }
        let node = get_attr_or_defer(inner, "node")?;
        if node.is_none() || !is_instance(node, self.refs.var) {
            return Ok(false);
        }
        truthy_attr(node, "is_self")
    }

    /// Mirror of `get_non_partial_lvalue_type` projected to trigger lists.
    /// `None` means "no triggers" (the `UninhabitedType` cases).
    fn non_partial_lvalue_triggers(
        &self,
        lvalue: &PyAny,
    ) -> Result<Option<Vec<String>>, DeferError> {
        let raw = match self.type_map.get_item(lvalue).map_err(|_| DeferError)? {
            Some(t) => t,
            None => return Ok(None),
        };
        let proper = self.proper(raw)?;
        let proper_ref: Option<Py<PyAny>> = proper;
        if let Some(ref p) = proper_ref {
            if is_instance(p.as_ref(self.py), self.refs.partial_type) {
                let node = get_attr_or_defer(lvalue, "node")?;
                if is_instance(node, self.refs.var) {
                    let nt = get_attr_or_defer(node, "type")?;
                    if nt.is_none() {
                        return Ok(None);
                    }
                    match self.proper(nt)? {
                        Some(q) => return Ok(Some(self.type_triggers(q.as_ref(self.py))?)),
                        None => return Ok(None),
                    }
                } else {
                    if truthy_attr(lvalue, "is_new_def")? {
                        return Err(DeferError);
                    }
                    return Ok(None);
                }
            }
        }
        match proper_ref {
            Some(p) => Ok(Some(self.type_triggers(p.as_ref(self.py))?)),
            None => Ok(Some(Vec::new())),
        }
    }

    fn visit_operator_assignment_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, "rvalue")?)?;
        self.walk(get_attr_or_defer(o, "lvalue")?)?;
        let lvalue = get_attr_or_defer(o, "lvalue")?;
        self.process_lvalue(lvalue)?;
        let op = str_attr(o, "op")?;
        let method: String = match self.refs.op_methods.get_item(&op).map_err(|_| DeferError)? {
            Some(m) => m.extract().map_err(|_| DeferError)?,
            None => return Err(DeferError),
        };
        self.add_attribute_dependency_for_expr(lvalue, &method)?;
        if self
            .refs
            .inplace_methods
            .contains(&op)
            .map_err(|_| DeferError)?
        {
            let rest = method.get(2..).ok_or(DeferError)?;
            self.add_attribute_dependency_for_expr(lvalue, &format!("__i{rest}"))?;
        }
        Ok(())
    }

    fn visit_for_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let index = get_attr_or_defer(o, "index")?;
        let expr = get_attr_or_defer(o, "expr")?;
        self.walk(index)?;
        self.walk(expr)?;
        self.walk(get_attr_or_defer(o, "body")?)?;
        if truthy_attr(o, "else_body")? {
            self.walk(get_attr_or_defer(o, "else_body")?)?;
        }
        if !truthy_attr(o, "is_async")? {
            self.add_attribute_dependency_for_expr(expr, "__iter__")?;
            self.add_attribute_dependency_for_expr(expr, "__getitem__")?;
            if truthy_attr(o, "inferred_iterator_type")? {
                let it = get_attr_or_defer(o, "inferred_iterator_type")?;
                self.add_attribute_dependency(it, "__next__")?;
            }
        } else {
            self.add_attribute_dependency_for_expr(expr, "__aiter__")?;
            if truthy_attr(o, "inferred_iterator_type")? {
                let it = get_attr_or_defer(o, "inferred_iterator_type")?;
                self.add_attribute_dependency(it, "__anext__")?;
            }
        }
        self.process_lvalue(index)?;
        if class_name_is(index, "TupleExpr") && truthy_attr(o, "inferred_item_type")? {
            let item_type = get_attr_or_defer(o, "inferred_item_type")?;
            self.add_attribute_dependency(item_type, "__iter__")?;
            self.add_attribute_dependency(item_type, "__getitem__")?;
        }
        if truthy_attr(o, "index_type")? {
            self.add_type_dependencies(get_attr_or_defer(o, "index_type")?, None)?;
        }
        Ok(())
    }

    fn visit_with_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let exprs = list_attr(o, "expr")?;
        let targets = list_attr(o, "target")?;
        if exprs.len() != targets.len() {
            return Err(DeferError);
        }
        for i in 0..exprs.len() {
            let e = exprs.get_item(i).map_err(|_| DeferError)?;
            self.walk(e)?;
            let targ = targets.get_item(i).map_err(|_| DeferError)?;
            if !targ.is_none() {
                self.walk(targ)?;
            }
        }
        self.walk(get_attr_or_defer(o, "body")?)?;
        let is_async = truthy_attr(o, "is_async")?;
        for e in exprs.iter() {
            if !is_async {
                self.add_attribute_dependency_for_expr(e, "__enter__")?;
                self.add_attribute_dependency_for_expr(e, "__exit__")?;
            } else {
                self.add_attribute_dependency_for_expr(e, "__aenter__")?;
                self.add_attribute_dependency_for_expr(e, "__aexit__")?;
            }
        }
        for typ in list_attr(o, "analyzed_types")?.iter() {
            self.add_type_dependencies(typ, None)?;
        }
        Ok(())
    }

    fn visit_del_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        if !get_attr_or_defer(o, "expr")?.is_none() {
            self.walk(get_attr_or_defer(o, "expr")?)?;
        }
        if class_name_is(get_attr_or_defer(o, "expr")?, "IndexExpr") {
            let base = get_attr_or_defer(get_attr_or_defer(o, "expr")?, "base")?;
            self.add_attribute_dependency_for_expr(base, "__delitem__")?;
        }
        Ok(())
    }

    fn process_global_ref_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        if let Some(fullname) = opt_str_attr(o, "fullname")? {
            if !fullname.is_empty() {
                let t = self.make_trigger(&fullname)?;
                self.add_dependency(t, None)?;
            }
        }
        if let Some(raw) = self.type_map.get_item(o).map_err(|_| DeferError)? {
            if let Some(p) = self.proper(raw)? {
                let pb = p.as_ref(self.py);
                if is_instance(pb, self.refs.types.function_like)
                    && pb
                        .call_method0("is_type_obj")
                        .map_err(|_| DeferError)?
                        .is_true()
                        .map_err(|_| DeferError)?
                {
                    let fullname = str_attr(
                        pb.call_method0("type_object").map_err(|_| DeferError)?,
                        "fullname",
                    )?;
                    let t1 = self.make_trigger(&format!("{fullname}.__init__"))?;
                    self.add_dependency(t1, None)?;
                    let t2 = self.make_trigger(&format!("{fullname}.__new__"))?;
                    self.add_dependency(t2, None)?;
                }
            }
        }
        Ok(())
    }

    fn visit_name_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        let kind = opt_int_attr(o, "kind")?;
        if kind == Some(self.refs.ldef) || kind == Some(self.refs.mdef) {
            return Ok(());
        }
        self.process_global_ref_expr(o)
    }

    fn visit_member_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let inner = get_attr_or_defer(e, "expr")?;
        if is_instance(inner, self.refs.ref_expr) {
            let node = get_attr_or_defer(inner, "node")?;
            if !node.is_none() && is_instance(node, self.refs.type_info) {
                let t = self.make_trigger(&str_attr(node, "fullname")?)?;
                self.add_dependency(t, None)?;
            } else {
                self.walk(inner)?;
            }
        } else {
            self.walk(inner)?;
        }
        if !get_attr_or_defer(e, "kind")?.is_none() {
            self.process_global_ref_expr(e)?;
        } else {
            let raw = match self.type_map.get_item(inner).map_err(|_| DeferError)? {
                Some(t) => t,
                None => return Ok(()),
            };
            if is_instance(inner, self.refs.ref_expr) {
                let onode = get_attr_or_defer(inner, "node")?;
                if !onode.is_none() && is_instance(onode, self.refs.mypy_file) {
                    let t = self.make_trigger(&format!(
                        "{}.{}",
                        str_attr(onode, "fullname")?,
                        fstr_attr(e, "name")?
                    ))?;
                    self.add_dependency(t, None)?;
                    return Ok(());
                }
            }
            let proper = self.proper(raw)?;
            let lname = fstr_attr(e, "name")?;
            let triggers = match proper {
                Some(ref p) => self.attr_triggers(p.as_ref(self.py), &lname)?,
                None => Vec::new(),
            };
            for t in triggers {
                self.add_dependency(t, None)?;
            }
            if self.use_logical_deps {
                if let Some(ref p) = proper {
                    let pb = p.as_ref(self.py);
                    if is_instance(pb, self.refs.types.any_type) {
                        if let Some(nm) = self.get_unimported_fullname(e, pb)? {
                            let t = self.make_trigger(&nm)?;
                            self.add_dependency(t, None)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn get_unimported_fullname(
        &self,
        e: &PyAny,
        typ: &PyAny,
    ) -> Result<Option<String>, DeferError> {
        let mut suffix = String::new();
        let mut cur_e = e;
        let mut cur_typ: Py<PyAny> = typ.into();
        loop {
            let toa: i64 = get_attr_or_defer(cur_typ.as_ref(self.py), "type_of_any")?
                .extract()
                .map_err(|_| DeferError)?;
            if toa != self.refs.from_another_any {
                break;
            }
            let inner = get_attr_or_defer(cur_e, "expr")?;
            if !class_name_is(inner, "MemberExpr") {
                break;
            }
            suffix = format!(".{}{}", fstr_attr(cur_e, "name")?, suffix);
            cur_e = inner;
            let obj = get_attr_or_defer(cur_e, "expr")?;
            let raw = match self.type_map.get_item(obj).map_err(|_| DeferError)? {
                Some(t) => t,
                None => return Ok(None),
            };
            match self.proper(raw)? {
                Some(p) if is_instance(p.as_ref(self.py), self.refs.types.any_type) => {
                    cur_typ = p;
                }
                _ => return Ok(None),
            }
        }
        let cur_pb = cur_typ.as_ref(self.py);
        let toa: i64 = get_attr_or_defer(cur_pb, "type_of_any")?
            .extract()
            .map_err(|_| DeferError)?;
        if toa == self.refs.from_unimported {
            if let Some(mi) = opt_str_attr(cur_pb, "missing_import_name")? {
                return Ok(Some(format!(
                    "{mi}.{}{}",
                    fstr_attr(cur_e, "name")?,
                    suffix
                )));
            }
        }
        Ok(None)
    }

    fn visit_super_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        if !self.use_logical_deps {
            self.walk(get_attr_or_defer(e, "call")?)?;
        }
        if !get_attr_or_defer(e, "info")?.is_none() {
            let info = get_attr_or_defer(e, "info")?;
            let ename = str_attr(e, "name")?;
            for base in self.non_trivial_bases(info)? {
                let bpy = base.as_ref(self.py);
                let bfn = str_attr(bpy, "fullname")?;
                let t = self.make_trigger(&format!("{bfn}.{ename}"))?;
                self.add_dependency(t, None)?;
                if dict_attr(bpy, "names")?
                    .contains(&ename)
                    .map_err(|_| DeferError)?
                {
                    break;
                }
            }
        }
        Ok(())
    }

    fn visit_call_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let callee = get_attr_or_defer(e, "callee")?;
        if is_instance(callee, self.refs.ref_expr)
            && opt_str_attr(callee, "fullname")?.as_deref() == Some("builtins.isinstance")
        {
            self.process_isinstance_call(e)?;
        } else {
            self.visit_call_children(e)?;
            if let Some(raw) = self.type_map.get_item(callee).map_err(|_| DeferError)? {
                if let Some(p) = self.proper(raw)? {
                    if !is_instance(p.as_ref(self.py), self.refs.types.function_like) {
                        self.add_attribute_dependency(p.as_ref(self.py), "__call__")?;
                    }
                }
            }
        }
        Ok(())
    }

    fn process_isinstance_call(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let args = list_attr(e, "args")?;
        if args.len() == 2 {
            let arg = args.get_item(1).map_err(|_| DeferError)?;
            if is_instance(arg, self.refs.ref_expr)
                && opt_int_attr(arg, "kind")? == Some(self.refs.gdef)
                && {
                    let n = get_attr_or_defer(arg, "node")?;
                    !n.is_none() && is_instance(n, self.refs.type_info)
                }
                && opt_str_attr(arg, "fullname")?
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
            {
                let t = self.make_trigger(&fstr_attr(arg, "fullname")?)?;
                self.add_dependency(t, None)?;
                return Ok(());
            }
        }
        self.visit_call_children(e)
    }

    fn visit_cast_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(e, "expr")?)?;
        self.add_type_dependencies(get_attr_or_defer(e, "type")?, None)?;
        Ok(())
    }

    fn visit_type_form_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        self.add_type_dependencies(get_attr_or_defer(e, "type")?, None)?;
        Ok(())
    }

    fn visit_assert_type_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(e, "expr")?)?;
        self.add_type_dependencies(get_attr_or_defer(e, "type")?, None)?;
        Ok(())
    }

    fn visit_type_application(&mut self, e: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(e, "expr")?)?;
        for typ in list_attr(e, "types")?.iter() {
            self.add_type_dependencies(typ, None)?;
        }
        Ok(())
    }

    fn visit_index_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let base = get_attr_or_defer(e, "base")?;
        self.walk(base)?;
        self.walk(get_attr_or_defer(e, "index")?)?;
        if truthy_attr(e, "analyzed")? {
            self.walk(get_attr_or_defer(e, "analyzed")?)?;
        }
        self.add_operator_method_dependency(base, "__getitem__")?;
        Ok(())
    }

    fn visit_unary_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let inner = get_attr_or_defer(e, "expr")?;
        self.walk(inner)?;
        let op = str_attr(e, "op")?;
        let method: Option<String> = self
            .refs
            .unary_op_methods
            .get_item(&op)
            .map_err(|_| DeferError)?
            .map(|m| m.extract().map_err(|_| DeferError))
            .transpose()?;
        if let Some(m) = method {
            self.add_operator_method_dependency(inner, &m)?;
        }
        Ok(())
    }

    fn visit_op_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let left = get_attr_or_defer(e, "left")?;
        let right = get_attr_or_defer(e, "right")?;
        self.walk(left)?;
        self.walk(right)?;
        if !get_attr_or_defer(e, "analyzed")?.is_none() {
            self.walk(get_attr_or_defer(e, "analyzed")?)?;
        }
        self.process_binary_op(&str_attr(e, "op")?, left, right)?;
        Ok(())
    }

    fn visit_comparison_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let operands = list_attr(e, "operands")?;
        for item in operands.iter() {
            self.walk(item)?;
        }
        let operators = list_attr(e, "operators")?;
        let n = operands.len();
        for (i, op_any) in operators.iter().enumerate() {
            let op: String = op_any.extract().map_err(|_| DeferError)?;
            let left = operands.get_item(i).map_err(|_| DeferError)?;
            let right = operands.get_item(i + 1).map_err(|_| DeferError)?;
            let _ = n;
            self.process_binary_op(&op, left, right)?;
        }
        Ok(())
    }

    fn process_binary_op(
        &mut self,
        op: &str,
        left: &PyAny,
        right: &PyAny,
    ) -> Result<(), DeferError> {
        let method: Option<String> = self
            .refs
            .op_methods
            .get_item(op)
            .map_err(|_| DeferError)?
            .map(|m| m.extract().map_err(|_| DeferError))
            .transpose()?;
        if let Some(m) = method {
            if op == "in" {
                self.add_operator_method_dependency(right, &m)?;
            } else {
                self.add_operator_method_dependency(left, &m)?;
                let rev: Option<String> = self
                    .refs
                    .reverse_op_methods
                    .get_item(&m)
                    .map_err(|_| DeferError)?
                    .map(|r| r.extract().map_err(|_| DeferError))
                    .transpose()?;
                if let Some(r) = rev {
                    self.add_operator_method_dependency(right, &r)?;
                }
            }
        }
        Ok(())
    }

    fn add_operator_method_dependency(
        &mut self,
        e: &PyAny,
        method: &str,
    ) -> Result<(), DeferError> {
        if let Some(raw) = self.type_map.get_item(e).map_err(|_| DeferError)? {
            if let Some(p) = self.proper(raw)? {
                self.add_operator_method_dependency_for_type(p.as_ref(self.py), method)?;
            }
        }
        Ok(())
    }

    fn add_operator_method_dependency_for_type(
        &mut self,
        typ: &PyAny,
        method: &str,
    ) -> Result<(), DeferError> {
        let mut cur: Option<Py<PyAny>> = Some(typ.into());
        if let Some(ref c) = cur.clone() {
            if is_instance(c.as_ref(self.py), self.refs.types.type_var_type) {
                let ub = get_attr_or_defer(c.as_ref(self.py), "upper_bound")?;
                cur = self.proper(ub)?;
            }
        }
        if let Some(ref c) = cur.clone() {
            if is_instance(c.as_ref(self.py), self.refs.types.tuple_type) {
                let pf = get_attr_or_defer(c.as_ref(self.py), "partial_fallback")?;
                cur = Some(pf.into());
            }
        }
        let owned = match cur {
            Some(c) => c,
            None => return Ok(()),
        };
        let obj = owned.as_ref(self.py);
        if is_instance(obj, self.refs.types.instance) {
            let t = self.make_trigger(&format!(
                "{}.{}",
                str_attr(get_attr_or_defer(obj, "type")?, "fullname")?,
                method
            ))?;
            self.add_dependency(t, None)?;
        } else if is_instance(obj, self.refs.types.union_type) {
            for item in list_attr(obj, "items")?.iter() {
                if let Some(p) = self.proper(item)? {
                    self.add_operator_method_dependency_for_type(p.as_ref(self.py), method)?;
                }
            }
        } else if is_instance(obj, self.refs.types.function_like)
            && obj
                .call_method0("is_type_obj")
                .map_err(|_| DeferError)?
                .is_true()
                .map_err(|_| DeferError)?
        {
            let fb = get_attr_or_defer(obj, "fallback")?;
            if let Some(p) = self.proper(fb)? {
                self.add_operator_method_dependency_for_type(p.as_ref(self.py), method)?;
            }
        } else if is_instance(obj, self.refs.types.type_type) {
            let item = get_attr_or_defer(obj, "item")?;
            if let Some(p) = self.proper(item)? {
                let pb = p.as_ref(self.py);
                if is_instance(pb, self.refs.types.instance) {
                    let mt = get_attr_or_defer(get_attr_or_defer(pb, "type")?, "metaclass_type")?;
                    if !mt.is_none() {
                        if let Some(q) = self.proper(mt)? {
                            self.add_operator_method_dependency_for_type(
                                q.as_ref(self.py),
                                method,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn visit_generator_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let indices = list_attr(e, "indices")?;
        let sequences = list_attr(e, "sequences")?;
        let condlists = list_attr(e, "condlists")?;
        let n = indices.len().min(sequences.len()).min(condlists.len());
        for i in 0..n {
            let sequence = sequences.get_item(i).map_err(|_| DeferError)?;
            self.walk(sequence)?;
            self.walk(indices.get_item(i).map_err(|_| DeferError)?)?;
            let conds = condlists.get_item(i).map_err(|_| DeferError)?;
            for c in conds.downcast::<PyList>().map_err(|_| DeferError)?.iter() {
                self.walk(c)?;
            }
        }
        self.walk(get_attr_or_defer(e, "left_expr")?)?;
        for seq in sequences.iter() {
            self.add_iter_dependency(seq)?;
        }
        Ok(())
    }

    fn visit_dictionary_comprehension(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let indices = list_attr(e, "indices")?;
        let sequences = list_attr(e, "sequences")?;
        let condlists = list_attr(e, "condlists")?;
        let n = indices.len().min(sequences.len()).min(condlists.len());
        for i in 0..n {
            let sequence = sequences.get_item(i).map_err(|_| DeferError)?;
            self.walk(sequence)?;
            self.walk(indices.get_item(i).map_err(|_| DeferError)?)?;
            let conds = condlists.get_item(i).map_err(|_| DeferError)?;
            for c in conds.downcast::<PyList>().map_err(|_| DeferError)?.iter() {
                self.walk(c)?;
            }
        }
        self.walk(get_attr_or_defer(e, "key")?)?;
        self.walk(get_attr_or_defer(e, "value")?)?;
        for seq in sequences.iter() {
            self.add_iter_dependency(seq)?;
        }
        Ok(())
    }

    fn visit_star_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let inner = get_attr_or_defer(e, "expr")?;
        self.walk(inner)?;
        self.add_iter_dependency(inner)?;
        Ok(())
    }

    fn visit_yield_from_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let inner = get_attr_or_defer(e, "expr")?;
        self.walk(inner)?;
        self.add_iter_dependency(inner)?;
        Ok(())
    }

    fn visit_await_expr(&mut self, e: &PyAny) -> Result<(), DeferError> {
        let inner = get_attr_or_defer(e, "expr")?;
        self.walk(inner)?;
        self.add_attribute_dependency_for_expr(inner, "__await__")?;
        Ok(())
    }

    // -- Pure-traversal nodes (mirror TraverserVisitor) --

    fn traverse_single_child(&mut self, o: &PyAny, field: &str) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, field)?)
    }

    fn traverse_optional_child(&mut self, o: &PyAny, field: &str) -> Result<(), DeferError> {
        if !get_attr_or_defer(o, field)?.is_none() {
            self.walk(get_attr_or_defer(o, field)?)?;
        }
        Ok(())
    }

    fn traverse_truthy_child(&mut self, o: &PyAny, field: &str) -> Result<(), DeferError> {
        if truthy_attr(o, field)? {
            self.walk(get_attr_or_defer(o, field)?)?;
        }
        Ok(())
    }

    fn traverse_two_children(
        &mut self,
        o: &PyAny,
        first: &str,
        second: &str,
    ) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, first)?)?;
        self.walk(get_attr_or_defer(o, second)?)?;
        Ok(())
    }

    fn traverse_list_field(&mut self, o: &PyAny, field: &str) -> Result<(), DeferError> {
        for item in list_attr(o, field)?.iter() {
            self.walk(item)?;
        }
        Ok(())
    }

    fn traverse_overloaded_func_def(&mut self, o: &PyAny) -> Result<(), DeferError> {
        for item in list_attr(o, "items")?.iter() {
            self.walk(item)?;
        }
        if truthy_attr(o, "impl")? {
            self.walk(get_attr_or_defer(o, "impl")?)?;
        }
        Ok(())
    }

    fn traverse_if_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        for e in list_attr(o, "expr")?.iter() {
            self.walk(e)?;
        }
        for b in list_attr(o, "body")?.iter() {
            self.walk(b)?;
        }
        if truthy_attr(o, "else_body")? {
            self.walk(get_attr_or_defer(o, "else_body")?)?;
        }
        Ok(())
    }

    fn traverse_while_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, "expr")?)?;
        self.walk(get_attr_or_defer(o, "body")?)?;
        if truthy_attr(o, "else_body")? {
            self.walk(get_attr_or_defer(o, "else_body")?)?;
        }
        Ok(())
    }

    fn traverse_assert_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.traverse_optional_child(o, "expr")?;
        self.traverse_optional_child(o, "msg")?;
        Ok(())
    }

    fn traverse_raise_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.traverse_optional_child(o, "expr")?;
        self.traverse_optional_child(o, "from_expr")?;
        Ok(())
    }

    fn traverse_try_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, "body")?)?;
        let types = list_attr(o, "types")?;
        let handlers = list_attr(o, "handlers")?;
        let vars = list_attr(o, "vars")?;
        if types.len() != handlers.len() {
            return Err(DeferError);
        }
        for i in 0..types.len() {
            let tp = types.get_item(i).map_err(|_| DeferError)?;
            if !tp.is_none() {
                self.walk(tp)?;
            }
            self.walk(handlers.get_item(i).map_err(|_| DeferError)?)?;
        }
        for v in vars.iter() {
            if !v.is_none() {
                self.walk(v)?;
            }
        }
        if !get_attr_or_defer(o, "else_body")?.is_none() {
            self.walk(get_attr_or_defer(o, "else_body")?)?;
        }
        if !get_attr_or_defer(o, "finally_body")?.is_none() {
            self.walk(get_attr_or_defer(o, "finally_body")?)?;
        }
        Ok(())
    }

    fn traverse_match_stmt(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, "subject")?)?;
        let patterns = list_attr(o, "patterns")?;
        let guards = list_attr(o, "guards")?;
        let bodies = list_attr(o, "bodies")?;
        if patterns.len() != guards.len() || patterns.len() != bodies.len() {
            return Err(DeferError);
        }
        for i in 0..patterns.len() {
            self.walk(patterns.get_item(i).map_err(|_| DeferError)?)?;
            let guard = guards.get_item(i).map_err(|_| DeferError)?;
            if !guard.is_none() {
                self.walk(guard)?;
            }
            self.walk(bodies.get_item(i).map_err(|_| DeferError)?)?;
        }
        Ok(())
    }

    fn traverse_slice_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.traverse_optional_child(o, "begin_index")?;
        self.traverse_optional_child(o, "end_index")?;
        self.traverse_optional_child(o, "stride")?;
        Ok(())
    }

    fn traverse_reveal_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        if int_attr(o, "kind")? == self.refs.reveal_type {
            let inner = get_attr_or_defer(o, "expr")?;
            if inner.is_none() {
                return Err(DeferError);
            }
            self.walk(inner)?;
        }
        Ok(())
    }

    fn traverse_dict_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        for pair in list_attr(o, "items")?.iter() {
            let k = seq_get(pair, 0)?;
            let v = seq_get(pair, 1)?;
            if !k.is_none() {
                self.walk(k)?;
            }
            self.walk(v)?;
        }
        Ok(())
    }

    fn traverse_template_str_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        for item in list_attr(o, "items")?.iter() {
            if let Ok(t) = item.downcast::<PyTuple>() {
                self.walk(t.get_item(0).map_err(|_| DeferError)?)?;
                let fmt = t.get_item(3).map_err(|_| DeferError)?;
                if !fmt.is_none() {
                    self.walk(fmt)?;
                }
            } else {
                self.walk(item)?;
            }
        }
        Ok(())
    }

    fn traverse_conditional_expr(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, "cond")?)?;
        self.walk(get_attr_or_defer(o, "if_expr")?)?;
        self.walk(get_attr_or_defer(o, "else_expr")?)?;
        Ok(())
    }

    fn traverse_as_pattern(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.traverse_optional_child(o, "pattern")?;
        self.traverse_optional_child(o, "name")?;
        Ok(())
    }

    fn traverse_mapping_pattern(&mut self, o: &PyAny) -> Result<(), DeferError> {
        for key in list_attr(o, "keys")?.iter() {
            self.walk(key)?;
        }
        for value in list_attr(o, "values")?.iter() {
            self.walk(value)?;
        }
        self.traverse_optional_child(o, "rest")?;
        Ok(())
    }

    fn traverse_class_pattern(&mut self, o: &PyAny) -> Result<(), DeferError> {
        self.walk(get_attr_or_defer(o, "class_ref")?)?;
        for p in list_attr(o, "positionals")?.iter() {
            self.walk(p)?;
        }
        for v in list_attr(o, "keyword_values")?.iter() {
            self.walk(v)?;
        }
        Ok(())
    }

    fn into_pydict(self) -> Result<PyObject, DeferError> {
        let py = self.py;
        let out = PyDict::new(py);
        for (trigger, targets) in &self.map {
            let set = PySet::new(py, targets.iter().collect::<Vec<_>>()).map_err(|_| DeferError)?;
            out.set_item(trigger, set).map_err(|_| DeferError)?;
        }
        Ok(out.into())
    }
}

/// Native `get_dependencies` walk over a `MypyFile`.
///
/// Returns the deps map (`dict[str, set[str]]`), or `None` when any node is
/// not handled, so the Python `DependencyVisitor` re-runs (strangler-fig
/// per-call gate). `use_logical_deps` mirrors `options.logical_deps`.
#[pyfunction]
pub(crate) fn rust_walk_dependency_visitor(
    py: Python<'_>,
    root: &PyAny,
    type_map: &PyDict,
    alias_deps: &PyAny,
    use_logical_deps: bool,
) -> PyResult<Option<PyObject>> {
    let mut walker = match DepsWalker::new(py, type_map, alias_deps, use_logical_deps) {
        Ok(w) => w,
        Err(_) => return Ok(None),
    };
    match walker.walk(root) {
        Ok(()) => match walker.into_pydict() {
            Ok(d) => Ok(Some(d)),
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

/// Native `get_dependencies_of_target` walk.
///
/// Mirrors the driver branching of `get_dependencies_of_target` (module
/// top-level skips nested targets; methods enter a class scope); the walk
/// itself is the same native visitor. Logical deps are off (the Python
/// driver constructs the visitor without options).
#[pyfunction]
pub(crate) fn rust_walk_dependency_target(
    py: Python<'_>,
    module_id: &str,
    module_tree: &PyAny,
    target: &PyAny,
    type_map: &PyDict,
) -> PyResult<Option<PyObject>> {
    let alias_deps = match module_tree.getattr("alias_deps") {
        Ok(a) => a,
        Err(_) => return Ok(None),
    };
    let mut walker = match DepsWalker::new(py, type_map, alias_deps, false) {
        Ok(w) => w,
        Err(_) => return Ok(None),
    };
    let result = (|| -> Result<(), DeferError> {
        walker.scope_module = Some(module_id.to_string());
        walker.scope_classes.clear();
        walker.scope_function = None;
        walker.scope_functions.clear();
        walker.scope_ignored = 0;
        if is_instance(target, walker.refs.mypy_file) {
            for defn in list_attr(target, "defs")?.iter() {
                let n = class_name_owned(defn)?;
                match n.as_str() {
                    "ClassDef" | "Decorator" | "FuncDef" | "OverloadedFuncDef" => {}
                    _ => walker.walk(defn)?,
                }
            }
        } else if is_instance(target, walker.refs.func_base) && truthy_attr(target, "info")? {
            let info = get_attr_or_defer(target, "info")?;
            walker.enter_class_scope(info);
            walker.walk(target)?;
            walker.leave_class_scope();
        } else {
            walker.walk(target)?;
        }
        walker.scope_module = None;
        Ok(())
    })();
    match result {
        Ok(()) => match walker.into_pydict() {
            Ok(d) => Ok(Some(d)),
            Err(_) => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

/// Register this module's Python-facing seam surface (#1677).
pub(crate) fn register_registry(m: &PyModule) -> PyResult<()> {
    // Issue #1632: native fine-grained dependency walk (DependencyVisitor).
    m.add_function(wrap_pyfunction!(rust_walk_dependency_visitor, m)?)?;

    m.add_function(wrap_pyfunction!(rust_walk_dependency_target, m)?)?;
    Ok(())
}
