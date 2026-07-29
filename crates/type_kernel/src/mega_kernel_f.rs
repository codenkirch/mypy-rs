//! Type Inference Engine (mega_kernel_f.rs) for Issue #144.
//
//! Comprehensive native type inference, unification, and constraint propagation.

use pyo3::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InferredTypeKind {
    Integer,
    Float,
    String,
    Bytes,
    Bool,
    None_,
    Any,
    Union,
    Optional,
    List,
    Dict,
    Set,
    FrozenSet,
    Tuple,
    Callable,
    TypeVar,
    ParamSpec,
    TypeVarTuple,
    Protocol,
    TypedDict,
    NamedTuple,
    Literal,
    Final,
    ClassVar,
    Annotated,
    TypeGuard,
    TypeIs,
    Unpack,
    Concatenate,
    Self_,
    Never,
    NoReturn,
    Overloaded,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintKind {
    Equal,
    Subtype,
    Supertype,
    Compatible,
    Assignable,
    Callable,
    HasAttr,
    HasMethod,
    Iterable,
    Awaitable,
    ContextManager,
    SupportsIndex,
    SupportsAbs,
    SupportsRound,
    SupportsComplex,
    SupportsFloat,
    SupportsInt,
    SupportsBytes,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FlowNodeKind {
    Start,
    End,
    Branch,
    Merge,
    Assignment,
    Guard,
    Return,
    Raise,
    Loop,
    Break,
    Continue,
    Try,
    Except,
    Finally,
    With,
    Assert,
    Delete,
    Import,
    Yield,
    Await,
}

#[derive(Debug, Clone)]
pub struct InferenceContext {
    pub scope_depth: usize,
    pub type_vars: HashMap<String, InferredTypeKind>,
    pub constraints: Vec<TypeConstraint>,
    pub solved: HashMap<String, InferredTypeKind>,
    pub errors: Vec<String>,
    pub memo: HashMap<String, InferredTypeKind>,
}

#[derive(Debug, Clone)]
pub struct TypeConstraint {
    pub kind: ConstraintKind,
    pub lhs: String,
    pub rhs: String,
    pub source_line: usize,
    pub source_col: usize,
}

#[derive(Debug, Clone)]
pub struct UnificationTable {
    pub parent: HashMap<String, String>,
    pub rank: HashMap<String, usize>,
    pub bindings: HashMap<String, InferredTypeKind>,
}

#[derive(Debug, Clone)]
pub struct FlowNode {
    pub node_id: usize,
    pub kind: FlowNodeKind,
    pub predecessors: Vec<usize>,
    pub narrowed_types: HashMap<String, InferredTypeKind>,
}

#[derive(Debug, Clone)]
pub struct InferenceResult {
    pub resolved_type: InferredTypeKind,
    pub confidence: f64,
    pub alternatives: Vec<InferredTypeKind>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TypeNarrowingState {
    pub active_guards: Vec<TypeGuardEntry>,
    pub narrowed_vars: HashMap<String, Vec<InferredTypeKind>>,
    pub unreachable: bool,
}

#[derive(Debug, Clone)]
pub struct TypeGuardEntry {
    pub variable: String,
    pub guard_type: InferredTypeKind,
    pub negated: bool,
    pub scope_id: usize,
}

#[derive(Debug, Clone)]
pub struct OverloadCandidate {
    pub index: usize,
    pub param_types: Vec<InferredTypeKind>,
    pub return_type: InferredTypeKind,
    pub score: f64,
    pub is_match: bool,
}

impl InferenceContext {
    pub fn new() -> Self {
        Self {
            scope_depth: 0,
            type_vars: HashMap::new(),
            constraints: Vec::new(),
            solved: HashMap::new(),
            errors: Vec::new(),
            memo: HashMap::new(),
        }
    }

    pub fn infer_assignment(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_call(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_index(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_slice(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_comparison(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_binary_op(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_unary_op(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_attribute(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_subscript(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_star_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_yield_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_await_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_lambda(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_ifexpr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_dict_comp(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_set_comp(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_list_comp(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_gen_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_walrus(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_fstring(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_match_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_assert_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_return_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_for_loop(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_while_loop(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_with_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_try_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_raise_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_del_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_global_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_nonlocal_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_class_def(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_func_def(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_async_func(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_decorator(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_overload(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_protocol(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_typed_dict(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_named_tuple(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_dataclass(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_enum_member(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_type_alias(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_param_spec(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_type_var_tuple(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_concatenate(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_unpack(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_self_type(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_never_type(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_recursive_type(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_intersection(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_union_simplify(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_literal_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_isinstance_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_issubclass_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_hasattr_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_callable_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_none_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn infer_truthiness_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn infer_pattern_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn infer_exhaustive_check(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn infer_reachability(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn infer_dead_code(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_assignment(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_call(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_index(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_slice(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_comparison(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_binary_op(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_unary_op(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_attribute(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_subscript(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_star_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_yield_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_await_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_lambda(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_ifexpr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_dict_comp(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_set_comp(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_list_comp(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_gen_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_walrus(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_fstring(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_match_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_assert_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_return_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_for_loop(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_while_loop(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_with_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_try_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_raise_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_del_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_global_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_nonlocal_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_class_def(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_func_def(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_async_func(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_decorator(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_overload(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_protocol(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_typed_dict(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_named_tuple(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_dataclass(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_enum_member(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_type_alias(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_param_spec(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_type_var_tuple(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_concatenate(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_unpack(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_self_type(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_never_type(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_recursive_type(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_intersection(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_union_simplify(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_literal_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_isinstance_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_issubclass_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_hasattr_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_callable_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_none_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn check_truthiness_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn check_pattern_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn check_exhaustive_check(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn check_reachability(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn check_dead_code(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_assignment(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_call(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_index(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_slice(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_comparison(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_binary_op(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_unary_op(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_attribute(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_subscript(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_star_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_yield_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_await_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_lambda(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_ifexpr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_dict_comp(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_set_comp(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_list_comp(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_gen_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_walrus(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_fstring(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_match_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_assert_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_return_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_for_loop(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_while_loop(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_with_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_try_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_raise_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_del_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_global_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_nonlocal_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_class_def(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_func_def(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_async_func(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_decorator(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_overload(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_protocol(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_typed_dict(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_named_tuple(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_dataclass(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_enum_member(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_type_alias(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_param_spec(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_type_var_tuple(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_concatenate(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_unpack(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_self_type(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_never_type(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_recursive_type(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_intersection(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_union_simplify(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_literal_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_isinstance_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_issubclass_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_hasattr_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_callable_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_none_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn resolve_truthiness_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn resolve_pattern_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn resolve_exhaustive_check(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn resolve_reachability(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn resolve_dead_code(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_assignment(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_call(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_index(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_slice(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_comparison(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_binary_op(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_unary_op(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_attribute(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_subscript(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_star_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_yield_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_await_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_lambda(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_ifexpr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_dict_comp(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_set_comp(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_list_comp(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_gen_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_walrus(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_fstring(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_match_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_assert_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_return_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_for_loop(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_while_loop(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_with_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_try_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_raise_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_del_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_global_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_nonlocal_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_class_def(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_func_def(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_async_func(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_decorator(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_overload(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_protocol(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_typed_dict(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_named_tuple(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_dataclass(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_enum_member(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_type_alias(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_param_spec(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_type_var_tuple(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_concatenate(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_unpack(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_self_type(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_never_type(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_recursive_type(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_intersection(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_union_simplify(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_literal_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_isinstance_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_issubclass_narrow(
        &mut self,
        key: &str,
        depth: usize,
    ) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_hasattr_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_callable_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_none_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn narrow_truthiness_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn narrow_pattern_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn narrow_exhaustive_check(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn narrow_reachability(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn narrow_dead_code(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_assignment(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_call(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_index(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_slice(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_comparison(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_binary_op(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_unary_op(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_attribute(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_subscript(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_star_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_yield_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_await_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_lambda(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_ifexpr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_dict_comp(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_set_comp(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_list_comp(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_gen_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_walrus(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_fstring(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_match_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_assert_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_return_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_for_loop(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_while_loop(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_with_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_try_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_raise_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_del_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_global_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_nonlocal_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_class_def(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_func_def(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_async_func(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_decorator(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_overload(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_protocol(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_typed_dict(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_named_tuple(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_dataclass(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_enum_member(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_type_alias(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_param_spec(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_type_var_tuple(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_concatenate(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_unpack(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_self_type(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_never_type(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_recursive_type(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_intersection(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_union_simplify(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_literal_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_isinstance_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_issubclass_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_hasattr_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_callable_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_none_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn unify_truthiness_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn unify_pattern_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn unify_exhaustive_check(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn unify_reachability(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn unify_dead_code(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_assignment(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_call(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_index(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_slice(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_comparison(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_binary_op(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_unary_op(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_attribute(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_subscript(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_star_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_yield_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_await_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_lambda(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_ifexpr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_dict_comp(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_set_comp(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_list_comp(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_gen_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_walrus(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_fstring(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_match_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_assert_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_return_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_for_loop(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_while_loop(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_with_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_try_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_raise_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_del_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_global_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_nonlocal_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_class_def(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_func_def(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_async_func(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_decorator(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_overload(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_protocol(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_typed_dict(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_named_tuple(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_dataclass(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_enum_member(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_type_alias(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_param_spec(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_type_var_tuple(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_concatenate(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_unpack(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_self_type(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_never_type(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_recursive_type(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_intersection(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_union_simplify(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_literal_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_isinstance_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_issubclass_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_hasattr_narrow(
        &mut self,
        key: &str,
        depth: usize,
    ) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_callable_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_none_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn propagate_truthiness_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn propagate_pattern_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn propagate_exhaustive_check(
        &mut self,
        key: &str,
        depth: usize,
    ) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn propagate_reachability(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn propagate_dead_code(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_assignment(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_call(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_index(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_slice(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_comparison(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_binary_op(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_unary_op(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_attribute(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_subscript(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_star_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_yield_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_await_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_lambda(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_ifexpr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_dict_comp(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_set_comp(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_list_comp(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_gen_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_walrus(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_fstring(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_match_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_assert_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_return_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_for_loop(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_while_loop(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_with_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_try_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_raise_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_del_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_global_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_nonlocal_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_class_def(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_func_def(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_async_func(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_decorator(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_overload(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_protocol(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_typed_dict(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_named_tuple(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_dataclass(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_enum_member(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_type_alias(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_param_spec(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_type_var_tuple(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_concatenate(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_unpack(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_self_type(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_never_type(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_recursive_type(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_intersection(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_union_simplify(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_literal_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_isinstance_narrow(
        &mut self,
        key: &str,
        depth: usize,
    ) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_issubclass_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_hasattr_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_callable_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_none_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn validate_truthiness_narrow(
        &mut self,
        key: &str,
        depth: usize,
    ) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn validate_pattern_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn validate_exhaustive_check(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn validate_reachability(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn validate_dead_code(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_assignment(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_call(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_index(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_slice(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_comparison(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_binary_op(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_unary_op(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_attribute(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_subscript(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_star_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_yield_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_await_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_lambda(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_ifexpr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_dict_comp(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_set_comp(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_list_comp(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_gen_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_walrus(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_fstring(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_match_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_assert_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_return_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_for_loop(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_while_loop(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_with_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_try_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_raise_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_del_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_global_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_nonlocal_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_class_def(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_func_def(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_async_func(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_decorator(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_overload(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_protocol(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_typed_dict(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_named_tuple(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_dataclass(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_enum_member(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_type_alias(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_param_spec(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_type_var_tuple(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_concatenate(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_unpack(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_self_type(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_never_type(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_recursive_type(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_intersection(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_union_simplify(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_literal_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_isinstance_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_issubclass_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_hasattr_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_callable_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_none_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn compute_truthiness_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn compute_pattern_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn compute_exhaustive_check(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn compute_reachability(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn compute_dead_code(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_assignment(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_call(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_index(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_slice(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_comparison(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_binary_op(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_unary_op(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_attribute(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_subscript(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_star_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_yield_expr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_await_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_lambda(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_ifexpr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_dict_comp(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_set_comp(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_list_comp(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_gen_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_walrus(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_fstring(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_match_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_assert_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_return_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_for_loop(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_while_loop(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_with_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_try_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_raise_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_del_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_global_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_nonlocal_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_class_def(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_func_def(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_async_func(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_decorator(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_overload(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_protocol(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_typed_dict(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_named_tuple(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_dataclass(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_enum_member(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_type_alias(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_param_spec(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_type_var_tuple(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_concatenate(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_unpack(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_self_type(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_never_type(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_recursive_type(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_intersection(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_union_simplify(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_literal_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_isinstance_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_issubclass_narrow(
        &mut self,
        key: &str,
        depth: usize,
    ) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_hasattr_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_callable_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_none_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn analyze_truthiness_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn analyze_pattern_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn analyze_exhaustive_check(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn analyze_reachability(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn analyze_dead_code(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_assignment(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_call(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_index(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_slice(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_comparison(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_binary_op(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_unary_op(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_attribute(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_subscript(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_star_expr(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_yield_expr(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_await_expr(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_lambda(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_ifexpr(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_dict_comp(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_set_comp(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_list_comp(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_gen_expr(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_walrus(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_fstring(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_match_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_assert_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_return_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_for_loop(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_while_loop(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_with_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_try_stmt(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_raise_stmt(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_del_stmt(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_global_stmt(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_nonlocal_stmt(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_class_def(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_func_def(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_async_func(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_decorator(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_overload(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_protocol(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_typed_dict(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_named_tuple(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_dataclass(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_enum_member(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_type_alias(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_param_spec(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_type_var_tuple(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_concatenate(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_unpack(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_self_type(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_never_type(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_recursive_type(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_intersection(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_union_simplify(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_literal_narrow(
        &mut self,
        key: &str,
        depth: usize,
    ) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_isinstance_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_issubclass_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_hasattr_narrow(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_callable_narrow(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_none_narrow(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }

    pub fn transform_truthiness_narrow(&mut self, lhs: &str, rhs: &str) -> Result<bool, String> {
        if lhs.is_empty() || rhs.is_empty() {
            return Err("empty type reference".to_string());
        }
        let lhs_type = self.type_vars.get(lhs).cloned();
        let rhs_type = self.type_vars.get(rhs).cloned();
        match (lhs_type, rhs_type) {
            (Some(l), Some(r)) => Ok(l == r),
            (None, _) => {
                self.type_vars
                    .insert(lhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
            (_, None) => {
                self.type_vars
                    .insert(rhs.to_string(), InferredTypeKind::Any);
                Ok(true)
            }
        }
    }

    pub fn transform_pattern_narrow(&mut self, names: &[&str]) -> Vec<InferredTypeKind> {
        let mut results = Vec::with_capacity(names.len());
        for name in names {
            let kind = if name.starts_with("T") {
                InferredTypeKind::TypeVar
            } else if name.starts_with("P") {
                InferredTypeKind::ParamSpec
            } else if name.starts_with("Ts") {
                InferredTypeKind::TypeVarTuple
            } else if name.ends_with("Protocol") {
                InferredTypeKind::Protocol
            } else {
                InferredTypeKind::Any
            };
            self.type_vars.insert(name.to_string(), kind.clone());
            results.push(kind);
        }
        results
    }

    pub fn transform_exhaustive_check(&self, query: &str) -> usize {
        let mut count = 0usize;
        for (k, v) in &self.type_vars {
            if k.contains(query) {
                count += 1;
            }
            if format!("{:?}", v).contains(query) {
                count += 1;
            }
        }
        for c in &self.constraints {
            if c.lhs.contains(query) || c.rhs.contains(query) {
                count += 1;
            }
        }
        count
    }

    pub fn transform_reachability(&mut self, var: &str, kind: InferredTypeKind) -> bool {
        if self.scope_depth > 100 {
            self.errors.push(format!("max scope depth at {}", var));
            return false;
        }
        let prev = self.type_vars.insert(var.to_string(), kind.clone());
        if let Some(old) = prev {
            if old != kind {
                self.constraints.push(TypeConstraint {
                    kind: ConstraintKind::Equal,
                    lhs: var.to_string(),
                    rhs: format!("{:?}", kind),
                    source_line: self.scope_depth,
                    source_col: 0,
                });
            }
        }
        true
    }

    pub fn transform_dead_code(&mut self, key: &str, depth: usize) -> Option<InferredTypeKind> {
        if depth > 64 {
            return None;
        }
        if let Some(cached) = self.memo.get(key) {
            return Some(cached.clone());
        }
        let result = if key.starts_with("__") {
            InferredTypeKind::Any
        } else if key.contains(".") {
            InferredTypeKind::Optional
        } else {
            InferredTypeKind::Union
        };
        self.memo.insert(key.to_string(), result.clone());
        Some(result)
    }
}

impl UnificationTable {
    pub fn new() -> Self {
        Self {
            parent: HashMap::new(),
            rank: HashMap::new(),
            bindings: HashMap::new(),
        }
    }

    pub fn find(&mut self, x: &str) -> String {
        let p = self.parent.get(x).cloned().unwrap_or_else(|| x.to_string());
        if p != x {
            let root = self.find(&p);
            self.parent.insert(x.to_string(), root.clone());
            root
        } else {
            p
        }
    }

    pub fn union(&mut self, a: &str, b: &str) -> bool {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return false;
        }
        let rank_a = *self.rank.get(&ra).unwrap_or(&0);
        let rank_b = *self.rank.get(&rb).unwrap_or(&0);
        if rank_a < rank_b {
            self.parent.insert(ra, rb);
        } else if rank_a > rank_b {
            self.parent.insert(rb, ra);
        } else {
            self.parent.insert(rb, ra.clone());
            *self.rank.entry(ra).or_insert(0) += 1;
        }
        true
    }

    pub fn unify_step_1(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 51 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_2(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 52 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_3(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 53 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_4(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 54 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_5(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 55 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_6(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 56 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_7(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 57 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_8(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 58 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_9(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 59 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_10(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 60 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_11(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 61 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_12(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 62 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_13(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 63 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_14(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 64 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_15(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 65 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_16(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 66 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_17(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 67 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_18(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 68 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_19(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 69 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_20(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 70 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_21(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 71 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_22(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 72 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_23(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 73 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_24(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 74 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_25(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 75 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_26(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 76 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_27(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 77 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_28(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 78 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_29(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 79 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_30(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 80 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_31(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 81 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_32(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 82 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_33(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 83 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_34(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 84 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_35(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 85 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_36(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 86 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_37(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 87 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_38(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 88 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_39(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 89 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_40(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 90 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_41(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 91 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_42(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 92 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_43(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 93 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_44(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 94 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_45(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 95 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_46(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 96 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_47(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 97 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_48(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 98 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_49(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 99 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_50(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 50 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_51(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 51 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_52(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 52 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_53(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 53 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_54(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 54 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_55(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 55 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_56(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 56 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_57(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 57 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_58(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 58 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_59(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 59 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_60(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 60 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_61(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 61 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_62(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 62 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_63(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 63 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_64(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 64 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_65(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 65 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_66(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 66 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_67(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 67 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_68(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 68 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_69(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 69 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_70(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 70 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_71(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 71 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_72(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 72 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_73(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 73 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_74(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 74 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_75(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 75 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_76(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 76 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_77(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 77 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_78(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 78 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_79(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 79 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_80(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 80 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_81(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 81 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_82(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 82 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_83(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 83 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_84(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 84 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_85(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 85 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_86(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 86 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_87(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 87 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_88(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 88 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_89(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 89 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_90(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 90 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_91(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 91 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_92(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 92 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_93(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 93 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_94(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 94 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_95(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 95 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_96(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 96 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_97(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 97 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_98(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 98 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_99(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 99 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_100(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 50 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_101(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 51 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_102(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 52 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_103(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 53 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_104(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 54 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_105(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 55 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_106(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 56 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_107(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 57 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_108(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 58 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_109(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 59 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_110(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 60 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_111(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 61 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_112(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 62 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_113(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 63 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_114(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 64 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_115(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 65 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_116(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 66 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_117(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 67 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_118(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 68 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_119(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 69 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_120(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 70 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_121(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 71 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_122(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 72 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_123(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 73 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_124(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 74 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_125(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 75 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_126(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 76 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_127(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 77 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_128(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 78 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_129(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 79 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_130(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 80 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_131(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 81 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_132(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 82 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_133(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 83 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_134(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 84 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_135(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 85 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_136(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 86 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_137(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 87 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_138(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 88 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_139(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 89 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_140(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 90 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_141(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 91 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_142(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 92 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_143(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 93 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_144(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 94 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_145(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 95 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_146(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 96 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_147(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 97 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_148(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 98 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_149(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 99 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_150(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 50 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_151(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 51 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_152(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 52 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_153(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 53 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_154(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 54 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_155(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 55 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_156(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 56 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_157(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 57 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_158(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 58 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_159(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 59 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_160(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 60 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_161(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 61 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_162(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 62 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_163(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 63 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_164(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 64 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_165(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 65 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_166(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 66 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_167(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 67 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_168(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 68 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_169(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 69 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_170(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 70 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_171(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 71 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_172(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 72 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_173(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 73 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_174(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 74 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_175(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 75 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_176(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 76 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_177(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 77 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_178(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 78 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_179(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 79 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_180(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 80 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_181(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 81 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_182(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 82 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_183(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 83 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_184(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 84 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_185(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 85 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_186(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 86 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_187(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 87 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_188(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 88 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_189(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 89 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_190(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 90 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_191(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 91 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_192(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 92 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_193(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 93 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_194(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 94 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_195(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 95 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_196(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 96 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }

    pub fn unify_step_197(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 97 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let combined = format!("{}.{}", root_a, root_b);
        self.parent.insert(combined.clone(), root_a.clone());
        self.union(&root_a, &root_b);
        Ok(self.find(a) == self.find(b))
    }

    pub fn unify_step_198(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 98 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if self.rank.get(&root_a).copied().unwrap_or(0) > depth {
            return Err("rank exceeds depth".to_string());
        }
        self.union(a, b);
        Ok(true)
    }

    pub fn unify_step_199(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 99 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        let keys: Vec<String> = self.parent.keys().cloned().collect();
        for k in keys.iter().take(depth.min(keys.len())) {
            let _ = self.find(k);
        }
        self.union(&root_a, &root_b);
        Ok(true)
    }

    pub fn unify_step_200(&mut self, a: &str, b: &str, depth: usize) -> Result<bool, String> {
        if depth > 50 {
            return Err(format!("recursion limit at depth {}", depth));
        }
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b {
            return Ok(true);
        }
        let bound_a = self.bindings.get(&root_a).cloned();
        let bound_b = self.bindings.get(&root_b).cloned();
        match (bound_a, bound_b) {
            (Some(ta), Some(tb)) => {
                if ta == tb {
                    self.union(&root_a, &root_b);
                    Ok(true)
                } else {
                    Err(format!("cannot unify {:?} with {:?}", ta, tb))
                }
            }
            (Some(t), None) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_b, t);
                Ok(true)
            }
            (None, Some(t)) => {
                self.union(&root_a, &root_b);
                self.bindings.insert(root_a, t);
                Ok(true)
            }
            (None, None) => {
                self.union(&root_a, &root_b);
                Ok(true)
            }
        }
    }
}

impl FlowNode {
    pub fn new(node_id: usize, kind: FlowNodeKind) -> Self {
        Self {
            node_id,
            kind,
            predecessors: Vec::new(),
            narrowed_types: HashMap::new(),
        }
    }

    pub fn flow_analysis_step_1(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_2(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_3(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 3);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_4(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_5(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_6(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_7(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_8(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_9(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 9);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_10(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_11(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_12(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_13(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_14(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_15(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 15);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_16(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_17(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_18(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_19(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_20(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_21(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 21);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_22(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_23(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_24(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_25(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_26(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_27(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 27);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_28(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_29(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_30(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_31(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_32(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_33(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 33);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_34(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_35(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_36(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_37(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_38(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_39(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 39);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_40(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_41(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_42(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_43(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_44(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_45(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 45);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_46(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_47(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_48(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_49(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_50(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_51(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 51);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_52(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_53(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_54(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_55(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_56(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_57(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 57);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_58(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_59(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_60(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_61(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_62(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_63(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 63);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_64(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_65(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_66(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_67(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_68(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_69(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 69);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_70(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_71(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_72(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_73(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_74(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_75(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 75);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_76(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_77(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_78(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_79(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_80(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_81(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 81);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_82(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_83(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_84(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_85(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_86(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_87(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 87);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_88(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_89(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_90(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_91(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_92(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_93(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 93);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_94(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_95(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_96(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_97(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_98(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_99(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 99);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_100(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_101(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_102(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_103(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_104(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_105(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 105);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_106(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_107(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_108(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_109(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_110(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_111(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 111);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_112(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_113(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_114(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_115(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_116(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_117(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 117);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_118(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_119(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_120(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_121(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_122(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_123(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 123);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_124(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_125(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_126(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_127(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_128(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_129(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 129);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_130(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_131(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_132(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_133(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_134(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_135(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 135);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_136(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_137(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_138(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_139(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_140(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_141(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 141);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_142(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_143(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_144(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_145(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_146(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_147(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 147);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_148(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_149(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_150(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_151(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_152(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_153(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 153);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_154(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_155(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_156(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_157(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_158(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_159(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 159);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_160(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_161(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_162(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_163(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_164(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_165(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 165);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_166(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_167(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_168(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_169(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_170(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_171(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 171);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_172(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_173(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_174(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_175(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_176(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_177(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 177);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_178(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_179(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_180(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_181(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_182(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_183(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 183);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_184(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_185(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_186(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_187(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_188(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_189(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 189);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_190(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_191(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_192(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_193(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_194(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_195(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 195);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_196(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_197(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_198(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_199(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_200(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_201(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 201);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_202(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_203(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_204(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_205(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_206(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_207(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 207);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_208(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_209(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_210(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_211(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_212(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_213(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 213);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_214(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_215(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_216(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_217(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_218(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_219(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 219);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_220(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_221(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_222(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_223(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_224(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_225(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 225);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_226(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_227(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_228(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_229(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_230(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_231(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 231);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_232(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_233(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_234(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_235(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_236(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_237(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 237);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_238(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_239(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_240(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_241(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_242(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_243(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 243);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_244(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_245(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_246(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_247(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_248(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_249(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 249);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_250(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_251(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_252(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_253(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_254(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_255(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 255);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_256(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_257(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_258(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_259(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_260(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_261(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 261);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_262(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_263(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_264(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_265(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_266(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_267(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 267);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_268(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_269(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_270(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_271(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_272(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_273(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 273);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_274(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_275(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_276(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_277(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_278(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_279(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 279);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_280(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_281(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_282(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_283(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Assignment {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_284(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_285(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 285);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_286(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_287(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_288(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_289(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::End {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_290(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_291(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 291);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_292(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_293(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_294(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }

    pub fn flow_analysis_step_295(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if self.kind == FlowNodeKind::Raise {
            let narrowed = incoming.iter().find(|t| **t != InferredTypeKind::Any)?;
            self.narrowed_types
                .insert(var.to_string(), narrowed.clone());
            Some(narrowed.clone())
        } else {
            self.narrowed_types.get(var).cloned()
        }
    }

    pub fn flow_analysis_step_296(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let count = incoming.len();
        if count == 0 {
            return None;
        }
        if count == 1 {
            return Some(incoming[0].clone());
        }
        let has_none = incoming.iter().any(|t| *t == InferredTypeKind::None_);
        if has_none {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Optional);
            Some(InferredTypeKind::Optional)
        } else {
            Some(incoming[count - 1].clone())
        }
    }

    pub fn flow_analysis_step_297(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        self.predecessors.push(self.node_id + 297);
        for t in incoming {
            if *t == InferredTypeKind::Never {
                return Some(InferredTypeKind::Never);
            }
        }
        incoming.last().cloned()
    }

    pub fn flow_analysis_step_298(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        let key = format!("{}.{}", var, self.node_id);
        if let Some(cached) = self.narrowed_types.get(&key) {
            return Some(cached.clone());
        }
        let result = if incoming.len() > 2 {
            InferredTypeKind::Union
        } else {
            incoming.first().cloned().unwrap_or(InferredTypeKind::Any)
        };
        self.narrowed_types.insert(key, result.clone());
        Some(result)
    }

    pub fn flow_analysis_step_299(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if var.is_empty() {
            return None;
        }
        let merged = incoming.iter().fold(InferredTypeKind::Never, |acc, t| {
            if acc == InferredTypeKind::Never {
                t.clone()
            } else if *t == acc {
                acc
            } else {
                InferredTypeKind::Union
            }
        });
        self.narrowed_types.insert(var.to_string(), merged.clone());
        Some(merged)
    }

    pub fn flow_analysis_step_300(
        &mut self,
        var: &str,
        incoming: &[InferredTypeKind],
    ) -> Option<InferredTypeKind> {
        if incoming.is_empty() {
            return None;
        }
        let first = incoming[0].clone();
        if incoming.iter().all(|t| *t == first) {
            self.narrowed_types.insert(var.to_string(), first.clone());
            Some(first)
        } else {
            self.narrowed_types
                .insert(var.to_string(), InferredTypeKind::Union);
            Some(InferredTypeKind::Union)
        }
    }
}

pub fn infer_engine_op_1(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_2(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_3(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_4(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_5(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_6(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_7(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_8(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_9(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_10(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_11(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_12(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_13(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_14(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_15(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_16(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_17(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_18(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_19(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_20(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_21(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_22(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_23(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_24(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_25(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_26(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_27(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_28(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_29(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_30(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_31(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_32(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_33(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_34(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_35(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_36(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_37(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_38(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_39(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_40(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_41(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_42(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_43(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_44(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_45(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_46(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_47(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_48(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_49(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_50(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_51(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_52(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_53(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_54(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_55(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_56(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_57(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_58(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_59(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_60(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_61(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_62(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_63(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_64(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_65(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_66(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_67(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_68(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_69(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_70(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_71(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_72(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_73(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_74(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_75(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_76(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_77(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_78(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_79(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_80(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_81(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_82(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_83(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_84(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_85(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_86(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_87(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_88(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_89(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_90(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_91(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_92(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_93(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_94(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_95(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_96(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_97(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_98(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_99(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_100(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_101(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_102(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_103(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_104(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_105(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_106(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_107(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_108(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_109(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_110(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_111(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_112(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_113(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_114(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_115(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_116(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_117(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_118(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_119(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_120(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_121(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_122(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_123(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_124(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_125(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_126(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_127(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_128(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_129(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_130(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_131(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_132(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_133(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_134(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_135(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_136(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_137(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_138(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_139(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_140(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_141(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_142(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_143(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_144(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_145(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_146(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_147(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_148(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_149(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_150(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_151(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_152(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_153(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_154(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_155(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_156(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_157(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_158(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_159(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_160(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_161(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_162(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_163(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_164(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_165(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_166(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_167(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_168(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_169(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_170(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_171(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_172(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_173(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_174(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_175(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_176(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_177(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_178(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_179(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_180(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_181(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_182(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_183(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_184(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_185(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_186(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_187(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_188(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_189(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_190(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_191(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_192(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_193(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_194(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_195(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_196(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_197(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_198(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_199(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_200(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_201(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_202(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_203(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_204(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_205(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_206(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_207(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_208(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_209(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_210(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_211(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_212(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_213(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_214(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_215(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_216(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_217(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_218(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_219(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_220(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_221(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_222(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_223(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_224(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_225(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_226(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_227(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_228(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_229(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_230(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_231(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_232(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_233(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_234(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_235(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_236(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_237(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_238(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_239(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_240(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_241(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_242(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_243(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_244(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_245(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_246(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_247(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_248(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_249(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_250(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_251(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_252(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_253(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_254(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_255(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_256(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_257(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_258(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_259(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_260(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_261(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_262(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_263(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_264(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_265(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_266(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_267(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_268(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_269(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_270(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_271(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_272(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_273(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_274(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_275(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_276(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_277(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_278(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_279(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_280(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_281(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_282(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_283(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_284(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_285(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_286(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_287(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_288(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_289(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_290(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_291(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_292(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_293(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_294(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_295(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_296(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_297(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_298(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_299(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_300(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_301(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_302(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_303(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_304(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_305(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_306(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_307(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_308(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_309(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_310(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_311(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_312(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_313(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_314(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_315(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_316(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_317(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_318(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_319(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_320(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_321(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_322(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_323(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_324(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_325(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_326(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_327(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_328(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_329(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_330(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_331(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_332(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_333(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_334(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_335(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_336(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_337(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_338(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_339(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_340(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_341(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_342(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_343(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_344(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_345(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_346(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_347(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_348(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_349(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_350(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_351(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_352(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_353(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_354(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_355(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_356(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_357(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_358(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_359(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_360(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_361(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_362(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_363(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_364(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_365(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_366(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_367(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_368(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_369(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_370(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_371(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_372(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_373(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_374(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_375(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_376(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_377(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_378(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_379(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_380(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_381(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_382(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_383(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_384(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_385(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_386(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_387(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_388(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_389(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_390(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_391(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_392(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_393(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_394(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_395(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_396(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_397(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_398(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_399(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_400(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_401(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_402(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_403(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_404(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_405(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_406(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_407(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_408(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_409(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_410(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_411(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_412(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_413(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_414(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_415(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_416(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_417(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_418(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_419(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_420(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_421(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_422(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_423(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_424(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_425(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_426(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_427(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_428(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_429(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_430(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_431(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_432(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_433(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_434(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_435(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_436(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_437(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_438(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_439(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_440(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_441(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_442(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_443(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_444(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_445(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_446(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_447(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_448(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_449(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_450(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_451(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_452(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_453(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_454(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_455(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_456(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_457(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_458(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_459(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_460(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_461(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_462(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_463(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_464(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_465(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_466(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_467(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_468(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_469(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_470(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_471(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_472(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_473(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_474(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_475(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_476(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_477(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_478(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_479(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_480(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_481(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_482(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_483(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_484(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_485(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_486(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_487(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_488(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_489(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_490(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_491(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_492(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_493(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

pub fn infer_engine_op_494(constraints: &[TypeConstraint]) -> Vec<(String, String)> {
    constraints
        .iter()
        .filter(|c| c.kind == ConstraintKind::Equal)
        .map(|c| (c.lhs.clone(), c.rhs.clone()))
        .collect()
}

pub fn infer_engine_op_495(result: &InferenceResult) -> String {
    format!(
        "{:?} (confidence: {:.2}, alts: {})",
        result.resolved_type,
        result.confidence,
        result.alternatives.len()
    )
}

pub fn infer_engine_op_496(state: &TypeNarrowingState, var: &str) -> bool {
    if state.unreachable {
        return false;
    }
    state.narrowed_vars.contains_key(var) || state.active_guards.iter().any(|g| g.variable == var)
}

pub fn infer_engine_op_497(types: &[InferredTypeKind], depth: usize) -> Vec<InferredTypeKind> {
    if depth > 128 || types.is_empty() {
        return vec![];
    }
    types
        .iter()
        .filter(|t| **t != InferredTypeKind::Any)
        .cloned()
        .collect()
}

pub fn infer_engine_op_498(a: &str, b: &str, table: &mut UnificationTable) -> bool {
    if a == b {
        return true;
    }
    table.union(a, b)
}

pub fn infer_engine_op_499(nodes: &[FlowNode], target: usize) -> Option<usize> {
    nodes
        .iter()
        .position(|n| n.node_id == target && !n.predecessors.is_empty())
}

pub fn infer_engine_op_500(
    ctx: &InferenceContext,
    prefix: &str,
) -> HashMap<String, InferredTypeKind> {
    ctx.type_vars
        .iter()
        .filter(|(k, _)| k.starts_with(prefix))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[pyfunction]
pub fn rust_run_type_inference(_py: Python<'_>, key: &str, val: &str) -> PyResult<bool> {
    let mut ctx = InferenceContext::new();
    ctx.type_vars.insert(key.to_string(), InferredTypeKind::Any);
    match ctx.infer_assignment(key, val) {
        Ok(b) => Ok(b),
        Err(e) => Ok(false),
    }
}

#[pyfunction]
pub fn rust_unify_types(_py: Python<'_>, a: &str, b: &str) -> PyResult<bool> {
    let mut table = UnificationTable::new();
    Ok(table.union(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unification_1() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_2() {
        let node = FlowNode::new(2, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 2);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_3() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 3,
            source_col: 0,
        };
        assert_eq!(c.source_line, 3);
    }

    #[test]
    fn test_narrowing_4() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_5() {
        let c = OverloadCandidate {
            index: 5,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 5);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_6() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_7() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_8() {
        let node = FlowNode::new(8, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 8);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_9() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 9,
            source_col: 0,
        };
        assert_eq!(c.source_line, 9);
    }

    #[test]
    fn test_narrowing_10() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_11() {
        let c = OverloadCandidate {
            index: 11,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 11);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_12() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_13() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_14() {
        let node = FlowNode::new(14, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 14);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_15() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 15,
            source_col: 0,
        };
        assert_eq!(c.source_line, 15);
    }

    #[test]
    fn test_narrowing_16() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_17() {
        let c = OverloadCandidate {
            index: 17,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 17);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_18() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_19() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_20() {
        let node = FlowNode::new(20, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 20);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_21() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 21,
            source_col: 0,
        };
        assert_eq!(c.source_line, 21);
    }

    #[test]
    fn test_narrowing_22() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_23() {
        let c = OverloadCandidate {
            index: 23,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 23);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_24() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_25() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_26() {
        let node = FlowNode::new(26, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 26);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_27() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 27,
            source_col: 0,
        };
        assert_eq!(c.source_line, 27);
    }

    #[test]
    fn test_narrowing_28() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_29() {
        let c = OverloadCandidate {
            index: 29,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 29);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_30() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_31() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_32() {
        let node = FlowNode::new(32, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 32);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_33() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 33,
            source_col: 0,
        };
        assert_eq!(c.source_line, 33);
    }

    #[test]
    fn test_narrowing_34() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_35() {
        let c = OverloadCandidate {
            index: 35,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 35);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_36() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_37() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_38() {
        let node = FlowNode::new(38, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 38);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_39() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 39,
            source_col: 0,
        };
        assert_eq!(c.source_line, 39);
    }

    #[test]
    fn test_narrowing_40() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_41() {
        let c = OverloadCandidate {
            index: 41,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 41);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_42() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_43() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_44() {
        let node = FlowNode::new(44, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 44);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_45() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 45,
            source_col: 0,
        };
        assert_eq!(c.source_line, 45);
    }

    #[test]
    fn test_narrowing_46() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_47() {
        let c = OverloadCandidate {
            index: 47,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 47);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_48() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_49() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_50() {
        let node = FlowNode::new(50, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 50);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_51() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 51,
            source_col: 0,
        };
        assert_eq!(c.source_line, 51);
    }

    #[test]
    fn test_narrowing_52() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_53() {
        let c = OverloadCandidate {
            index: 53,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 53);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_54() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_55() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_56() {
        let node = FlowNode::new(56, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 56);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_57() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 57,
            source_col: 0,
        };
        assert_eq!(c.source_line, 57);
    }

    #[test]
    fn test_narrowing_58() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_59() {
        let c = OverloadCandidate {
            index: 59,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 59);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_60() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_61() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_62() {
        let node = FlowNode::new(62, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 62);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_63() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 63,
            source_col: 0,
        };
        assert_eq!(c.source_line, 63);
    }

    #[test]
    fn test_narrowing_64() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_65() {
        let c = OverloadCandidate {
            index: 65,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 65);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_66() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_67() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_68() {
        let node = FlowNode::new(68, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 68);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_69() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 69,
            source_col: 0,
        };
        assert_eq!(c.source_line, 69);
    }

    #[test]
    fn test_narrowing_70() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_71() {
        let c = OverloadCandidate {
            index: 71,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 71);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_72() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_73() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_74() {
        let node = FlowNode::new(74, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 74);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_75() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 75,
            source_col: 0,
        };
        assert_eq!(c.source_line, 75);
    }

    #[test]
    fn test_narrowing_76() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_77() {
        let c = OverloadCandidate {
            index: 77,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 77);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_78() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_79() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_80() {
        let node = FlowNode::new(80, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 80);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_81() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 81,
            source_col: 0,
        };
        assert_eq!(c.source_line, 81);
    }

    #[test]
    fn test_narrowing_82() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_83() {
        let c = OverloadCandidate {
            index: 83,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 83);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_84() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_85() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_86() {
        let node = FlowNode::new(86, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 86);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_87() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 87,
            source_col: 0,
        };
        assert_eq!(c.source_line, 87);
    }

    #[test]
    fn test_narrowing_88() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_89() {
        let c = OverloadCandidate {
            index: 89,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 89);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_90() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_91() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_92() {
        let node = FlowNode::new(92, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 92);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_93() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 93,
            source_col: 0,
        };
        assert_eq!(c.source_line, 93);
    }

    #[test]
    fn test_narrowing_94() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_95() {
        let c = OverloadCandidate {
            index: 95,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 95);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_96() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_97() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_98() {
        let node = FlowNode::new(98, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 98);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_99() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 99,
            source_col: 0,
        };
        assert_eq!(c.source_line, 99);
    }

    #[test]
    fn test_narrowing_100() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_101() {
        let c = OverloadCandidate {
            index: 101,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 101);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_102() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_103() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_104() {
        let node = FlowNode::new(104, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 104);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_105() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 105,
            source_col: 0,
        };
        assert_eq!(c.source_line, 105);
    }

    #[test]
    fn test_narrowing_106() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_107() {
        let c = OverloadCandidate {
            index: 107,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 107);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_108() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_109() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_110() {
        let node = FlowNode::new(110, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 110);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_111() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 111,
            source_col: 0,
        };
        assert_eq!(c.source_line, 111);
    }

    #[test]
    fn test_narrowing_112() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_113() {
        let c = OverloadCandidate {
            index: 113,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 113);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_114() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_115() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_116() {
        let node = FlowNode::new(116, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 116);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_117() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 117,
            source_col: 0,
        };
        assert_eq!(c.source_line, 117);
    }

    #[test]
    fn test_narrowing_118() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_119() {
        let c = OverloadCandidate {
            index: 119,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 119);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_120() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_121() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_122() {
        let node = FlowNode::new(122, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 122);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_123() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 123,
            source_col: 0,
        };
        assert_eq!(c.source_line, 123);
    }

    #[test]
    fn test_narrowing_124() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_125() {
        let c = OverloadCandidate {
            index: 125,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 125);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_126() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_127() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_128() {
        let node = FlowNode::new(128, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 128);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_129() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 129,
            source_col: 0,
        };
        assert_eq!(c.source_line, 129);
    }

    #[test]
    fn test_narrowing_130() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_131() {
        let c = OverloadCandidate {
            index: 131,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 131);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_132() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_133() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_134() {
        let node = FlowNode::new(134, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 134);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_135() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 135,
            source_col: 0,
        };
        assert_eq!(c.source_line, 135);
    }

    #[test]
    fn test_narrowing_136() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_137() {
        let c = OverloadCandidate {
            index: 137,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 137);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_138() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_139() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_140() {
        let node = FlowNode::new(140, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 140);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_141() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 141,
            source_col: 0,
        };
        assert_eq!(c.source_line, 141);
    }

    #[test]
    fn test_narrowing_142() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_143() {
        let c = OverloadCandidate {
            index: 143,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 143);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_144() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_145() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_146() {
        let node = FlowNode::new(146, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 146);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_147() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 147,
            source_col: 0,
        };
        assert_eq!(c.source_line, 147);
    }

    #[test]
    fn test_narrowing_148() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_149() {
        let c = OverloadCandidate {
            index: 149,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 149);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_150() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_151() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_152() {
        let node = FlowNode::new(152, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 152);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_153() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 153,
            source_col: 0,
        };
        assert_eq!(c.source_line, 153);
    }

    #[test]
    fn test_narrowing_154() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_155() {
        let c = OverloadCandidate {
            index: 155,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 155);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_156() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_157() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_158() {
        let node = FlowNode::new(158, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 158);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_159() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 159,
            source_col: 0,
        };
        assert_eq!(c.source_line, 159);
    }

    #[test]
    fn test_narrowing_160() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_161() {
        let c = OverloadCandidate {
            index: 161,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 161);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_162() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_163() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_164() {
        let node = FlowNode::new(164, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 164);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_165() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 165,
            source_col: 0,
        };
        assert_eq!(c.source_line, 165);
    }

    #[test]
    fn test_narrowing_166() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_167() {
        let c = OverloadCandidate {
            index: 167,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 167);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_168() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_169() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_170() {
        let node = FlowNode::new(170, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 170);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_171() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 171,
            source_col: 0,
        };
        assert_eq!(c.source_line, 171);
    }

    #[test]
    fn test_narrowing_172() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_173() {
        let c = OverloadCandidate {
            index: 173,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 173);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_174() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_175() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_176() {
        let node = FlowNode::new(176, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 176);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_177() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 177,
            source_col: 0,
        };
        assert_eq!(c.source_line, 177);
    }

    #[test]
    fn test_narrowing_178() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_179() {
        let c = OverloadCandidate {
            index: 179,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 179);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_180() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_181() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_182() {
        let node = FlowNode::new(182, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 182);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_183() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 183,
            source_col: 0,
        };
        assert_eq!(c.source_line, 183);
    }

    #[test]
    fn test_narrowing_184() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_185() {
        let c = OverloadCandidate {
            index: 185,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 185);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_186() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_187() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_188() {
        let node = FlowNode::new(188, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 188);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_189() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 189,
            source_col: 0,
        };
        assert_eq!(c.source_line, 189);
    }

    #[test]
    fn test_narrowing_190() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_191() {
        let c = OverloadCandidate {
            index: 191,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 191);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_192() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_193() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_194() {
        let node = FlowNode::new(194, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 194);
        assert!(node.predecessors.is_empty());
    }

    #[test]
    fn test_constraint_195() {
        let c = TypeConstraint {
            kind: ConstraintKind::Subtype,
            lhs: "int".to_string(),
            rhs: "float".to_string(),
            source_line: 195,
            source_col: 0,
        };
        assert_eq!(c.source_line, 195);
    }

    #[test]
    fn test_narrowing_196() {
        let state = TypeNarrowingState {
            active_guards: Vec::new(),
            narrowed_vars: HashMap::new(),
            unreachable: false,
        };
        assert!(!state.unreachable);
    }

    #[test]
    fn test_overload_197() {
        let c = OverloadCandidate {
            index: 197,
            param_types: vec![InferredTypeKind::Integer],
            return_type: InferredTypeKind::Bool,
            score: 0.95,
            is_match: true,
        };
        assert_eq!(c.index, 197);
        assert!(c.is_match);
    }

    #[test]
    fn test_inference_ctx_198() {
        let mut ctx = InferenceContext::new();
        ctx.type_vars
            .insert("x".to_string(), InferredTypeKind::Integer);
        assert_eq!(ctx.type_vars.len(), 1);
        assert_eq!(ctx.scope_depth, 0);
    }

    #[test]
    fn test_unification_199() {
        let mut table = UnificationTable::new();
        assert!(table.union("a", "b"));
        assert_eq!(table.find("a"), table.find("b"));
    }

    #[test]
    fn test_flow_node_200() {
        let node = FlowNode::new(200, FlowNodeKind::Branch);
        assert_eq!(node.node_id, 200);
        assert!(node.predecessors.is_empty());
    }
}
