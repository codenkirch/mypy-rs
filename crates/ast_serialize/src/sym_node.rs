//! Cache-payload symbol records + writer (G0.5, issue #1566).
//!
//! `SymNode` mirrors the `mypy/nodes.py` fixed-format `.write()` field
//! sets one-to-one: `MypyFile`, `SymbolTable` / `SymbolTableNode`,
//! `TypeInfo`, `ClassDef`, `Var`, `FuncDef`, `Decorator`,
//! `OverloadedFuncDef`, `TypeVarExpr`, `ParamSpecExpr`,
//! `TypeVarTupleExpr`, `TypeAlias` and `DataclassTransformSpec`. Records
//! are built from live Python objects via PyO3; type-valued fields stay
//! opaque byte payloads captured through Python callbacks (the G0.3
//! lambda-parameter pattern), so the type layer is not duplicated here.
//! The writer is pure Rust and byte-identical to the Python writer; the
//! Python-side differential tests are the frozen-legacy A/B reference
//! (there was no pre-existing Rust writer to freeze).
//!
//! Deferral contract: an unsupported node shape returns `None` and the
//! Python `MypyFile.write` fallback runs, so the payload can never
//! diverge silently. `PyAttributeError` / `PyAssertionError` /
//! `PyNotImplementedError` map to the same defer; other Python errors
//! propagate so real bugs stay visible (#1466 pattern).

use pyo3::exceptions::{PyAssertionError, PyAttributeError, PyNotImplementedError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyTuple};

use crate::{Writer, DECORATOR, DICT_STR_GEN, END_TAG, LITERAL_NONE};

/// Tag values shared with `mypy/nodes.py` (see the tag table there).
const MYPY_FILE: u8 = 50;
const OVERLOADED_FUNC_DEF: u8 = 51;
const FUNC_DEF: u8 = 52;
const VAR: u8 = 54;
const TYPE_VAR_EXPR: u8 = 55;
const PARAM_SPEC_EXPR: u8 = 56;
const TYPE_VAR_TUPLE_EXPR: u8 = 57;
const TYPE_INFO: u8 = 58;
const TYPE_ALIAS: u8 = 59;
const SYMBOL_TABLE_NODE: u8 = 61;
const DT_SPEC: u8 = 151;

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

pub(crate) struct MypyFileRecord {
    fullname: String,
    names: SymbolTableRecord,
    is_stub: bool,
    path: String,
    is_partial_stub_package: bool,
    future_import_flags: Vec<String>,
}

pub(crate) struct SymbolTableRecord {
    entries: Vec<(String, SymbolTableNodeRecord)>,
}

pub(crate) struct SymbolTableNodeRecord {
    kind: i64,
    module_hidden: bool,
    module_public: bool,
    implicit: bool,
    plugin_generated: bool,
    cross_ref: Option<String>,
    node: Option<SymNode>,
}

pub(crate) enum SymNode {
    TypeInfo(Box<TypeInfoRecord>),
    Var(Box<VarRecord>),
    FuncDef(Box<FuncDefRecord>),
    Decorator(Box<DecoratorRecord>),
    OverloadedFuncDef(Box<OverloadedFuncDefRecord>),
    TypeVarExpr(Box<TypeVarExprRecord>),
    ParamSpecExpr(Box<ParamSpecExprRecord>),
    TypeVarTupleExpr(Box<TypeVarTupleExprRecord>),
    TypeAlias(Box<TypeAliasRecord>),
}

pub(crate) struct TypeInfoRecord {
    names: SymbolTableRecord,
    defn: ClassDefRecord,
    module_name: String,
    fullname: String,
    abstract_names: Vec<String>,
    abstract_statuses: Vec<i64>,
    type_vars: Vec<String>,
    has_param_spec_type: bool,
    bases: Vec<u8>,
    mro: Vec<String>,
    promote: Vec<u8>,
    alt_promote: Vec<u8>,
    declared_metaclass: Vec<u8>,
    metaclass_type: Vec<u8>,
    tuple_type: Vec<u8>,
    typeddict_type: Vec<u8>,
    flags: [bool; 11],
    metadata: Vec<u8>,
    slots: Option<Vec<String>>,
    deletable_attributes: Vec<String>,
    self_type: Vec<u8>,
    dataclass_transform_spec: Option<DataclassTransformSpecRecord>,
    deprecated: Option<String>,
}

pub(crate) struct ClassDefRecord {
    name: String,
    type_vars: Vec<u8>,
    fullname: String,
}

pub(crate) struct VarRecord {
    name: String,
    type_: Vec<u8>,
    setter_type: Vec<u8>,
    fullname: String,
    flags: [bool; 19],
    final_value: Vec<u8>,
}

pub(crate) struct FuncDefRecord {
    name: String,
    type_: Vec<u8>,
    fullname: String,
    flags: [bool; 14],
    arg_names: Vec<Option<String>>,
    arg_kinds: Vec<i64>,
    abstract_status: i64,
    dataclass_transform_spec: Option<DataclassTransformSpecRecord>,
    deprecated: Option<String>,
    original_first_arg: Option<String>,
}

pub(crate) struct DecoratorRecord {
    func: SymNode,
    var: SymNode,
    is_overload: bool,
}

pub(crate) struct OverloadedFuncDefRecord {
    items: Vec<SymNode>,
    type_: Vec<u8>,
    fullname: String,
    impl_: Option<SymNode>,
    flags: [bool; 4],
    deprecated: Option<String>,
    setter_index: Option<i64>,
}

pub(crate) struct TypeVarExprRecord {
    name: String,
    fullname: String,
    values: Vec<u8>,
    upper_bound: Vec<u8>,
    default: Vec<u8>,
    variance: i64,
}

pub(crate) struct ParamSpecExprRecord {
    name: String,
    fullname: String,
    upper_bound: Vec<u8>,
    default: Vec<u8>,
    variance: i64,
}

pub(crate) struct TypeVarTupleExprRecord {
    tuple_fallback: Vec<u8>,
    name: String,
    fullname: String,
    upper_bound: Vec<u8>,
    default: Vec<u8>,
    variance: i64,
}

pub(crate) struct TypeAliasRecord {
    fullname: String,
    module: String,
    target: Vec<u8>,
    alias_tvars: Vec<u8>,
    no_args: bool,
    normalized: bool,
    python_3_12_type_alias: bool,
}

pub(crate) struct DataclassTransformSpecRecord {
    eq_default: bool,
    order_default: bool,
    kw_only_default: bool,
    frozen_default: bool,
    field_specifiers: Vec<String>,
}

// ---------------------------------------------------------------------------
// Writer (pure Rust, mirrors the Python `.write()` field order)
// ---------------------------------------------------------------------------

fn write_sym_node(writer: &mut Writer, node: &SymNode) {
    match node {
        SymNode::TypeInfo(record) => write_type_info(writer, record),
        SymNode::Var(record) => write_var(writer, record),
        SymNode::FuncDef(record) => write_func_def(writer, record),
        SymNode::Decorator(record) => write_decorator(writer, record),
        SymNode::OverloadedFuncDef(record) => write_overloaded(writer, record),
        SymNode::TypeVarExpr(record) => write_typevar_expr(writer, record),
        SymNode::ParamSpecExpr(record) => write_paramspec_expr(writer, record),
        SymNode::TypeVarTupleExpr(record) => write_typevartuple_expr(writer, record),
        SymNode::TypeAlias(record) => write_type_alias(writer, record),
    }
}

fn write_mypy_file(writer: &mut Writer, record: &MypyFileRecord) {
    writer.tag(MYPY_FILE);
    writer.string(&record.fullname);
    write_symbol_table(writer, &record.names);
    writer.bool(record.is_stub);
    writer.string(&record.path);
    writer.bool(record.is_partial_stub_package);
    writer.str_list(&record.future_import_flags);
    writer.tag(END_TAG);
}

fn write_symbol_table(writer: &mut Writer, table: &SymbolTableRecord) {
    writer.tag(DICT_STR_GEN);
    writer.bare_int(table.entries.len() as i64);
    for (key, stn) in &table.entries {
        writer.str_bare(key);
        write_symbol_table_node(writer, stn);
    }
}

fn write_symbol_table_node(writer: &mut Writer, record: &SymbolTableNodeRecord) {
    writer.tag(SYMBOL_TABLE_NODE);
    writer.int(record.kind);
    writer.bool(record.module_hidden);
    writer.bool(record.module_public);
    writer.bool(record.implicit);
    writer.bool(record.plugin_generated);
    writer.str_opt(record.cross_ref.as_deref());
    if let Some(node) = &record.node {
        write_sym_node(writer, node);
    }
    writer.tag(END_TAG);
}

fn write_type_info(writer: &mut Writer, record: &TypeInfoRecord) {
    writer.tag(TYPE_INFO);
    write_symbol_table(writer, &record.names);
    write_class_def(writer, &record.defn);
    writer.string(&record.module_name);
    writer.string(&record.fullname);
    writer.str_list(&record.abstract_names);
    writer.int_list(&record.abstract_statuses);
    writer.str_list(&record.type_vars);
    writer.bool(record.has_param_spec_type);
    writer.raw_bytes(&record.bases);
    writer.str_list(&record.mro);
    writer.raw_bytes(&record.promote);
    writer.raw_bytes(&record.alt_promote);
    writer.raw_bytes(&record.declared_metaclass);
    writer.raw_bytes(&record.metaclass_type);
    writer.raw_bytes(&record.tuple_type);
    writer.raw_bytes(&record.typeddict_type);
    writer.flags(&record.flags);
    writer.raw_bytes(&record.metadata);
    match &record.slots {
        Some(slots) => writer.str_list(slots),
        None => writer.tag(LITERAL_NONE),
    }
    writer.str_list(&record.deletable_attributes);
    writer.raw_bytes(&record.self_type);
    match &record.dataclass_transform_spec {
        Some(spec) => write_dt_spec(writer, spec),
        None => writer.tag(LITERAL_NONE),
    }
    writer.str_opt(record.deprecated.as_deref());
    writer.tag(END_TAG);
}

fn write_class_def(writer: &mut Writer, record: &ClassDefRecord) {
    writer.tag(crate::CLASS_DEF);
    writer.string(&record.name);
    writer.raw_bytes(&record.type_vars);
    writer.string(&record.fullname);
    writer.tag(END_TAG);
}

fn write_var(writer: &mut Writer, record: &VarRecord) {
    writer.tag(VAR);
    writer.string(&record.name);
    writer.raw_bytes(&record.type_);
    writer.raw_bytes(&record.setter_type);
    writer.string(&record.fullname);
    writer.flags(&record.flags);
    writer.raw_bytes(&record.final_value);
    writer.tag(END_TAG);
}

fn write_func_def(writer: &mut Writer, record: &FuncDefRecord) {
    writer.tag(FUNC_DEF);
    writer.string(&record.name);
    writer.raw_bytes(&record.type_);
    writer.string(&record.fullname);
    writer.flags(&record.flags);
    writer.opt_str_list(&record.arg_names);
    writer.int_list(&record.arg_kinds);
    writer.int(record.abstract_status);
    match &record.dataclass_transform_spec {
        Some(spec) => write_dt_spec(writer, spec),
        None => writer.tag(LITERAL_NONE),
    }
    writer.str_opt(record.deprecated.as_deref());
    writer.str_opt(record.original_first_arg.as_deref());
    writer.tag(END_TAG);
}

fn write_decorator(writer: &mut Writer, record: &DecoratorRecord) {
    writer.tag(DECORATOR);
    write_sym_node(writer, &record.func);
    write_sym_node(writer, &record.var);
    writer.bool(record.is_overload);
    writer.tag(END_TAG);
}

fn write_overloaded(writer: &mut Writer, record: &OverloadedFuncDefRecord) {
    writer.tag(OVERLOADED_FUNC_DEF);
    writer.tag(crate::LIST_GEN);
    writer.bare_int(record.items.len() as i64);
    for item in &record.items {
        write_sym_node(writer, item);
    }
    writer.raw_bytes(&record.type_);
    writer.string(&record.fullname);
    match &record.impl_ {
        Some(impl_) => write_sym_node(writer, impl_),
        None => writer.tag(LITERAL_NONE),
    }
    writer.flags(&record.flags);
    writer.str_opt(record.deprecated.as_deref());
    writer.int_opt(record.setter_index);
    writer.tag(END_TAG);
}

fn write_typevar_expr(writer: &mut Writer, record: &TypeVarExprRecord) {
    writer.tag(TYPE_VAR_EXPR);
    writer.string(&record.name);
    writer.string(&record.fullname);
    writer.raw_bytes(&record.values);
    writer.raw_bytes(&record.upper_bound);
    writer.raw_bytes(&record.default);
    writer.int(record.variance);
    writer.tag(END_TAG);
}

fn write_paramspec_expr(writer: &mut Writer, record: &ParamSpecExprRecord) {
    writer.tag(PARAM_SPEC_EXPR);
    writer.string(&record.name);
    writer.string(&record.fullname);
    writer.raw_bytes(&record.upper_bound);
    writer.raw_bytes(&record.default);
    writer.int(record.variance);
    writer.tag(END_TAG);
}

fn write_typevartuple_expr(writer: &mut Writer, record: &TypeVarTupleExprRecord) {
    writer.tag(TYPE_VAR_TUPLE_EXPR);
    writer.raw_bytes(&record.tuple_fallback);
    writer.string(&record.name);
    writer.string(&record.fullname);
    writer.raw_bytes(&record.upper_bound);
    writer.raw_bytes(&record.default);
    writer.int(record.variance);
    writer.tag(END_TAG);
}

fn write_type_alias(writer: &mut Writer, record: &TypeAliasRecord) {
    writer.tag(TYPE_ALIAS);
    writer.string(&record.fullname);
    writer.string(&record.module);
    writer.raw_bytes(&record.target);
    writer.raw_bytes(&record.alias_tvars);
    writer.bool(record.no_args);
    writer.bool(record.normalized);
    writer.bool(record.python_3_12_type_alias);
    writer.tag(END_TAG);
}

fn write_dt_spec(writer: &mut Writer, record: &DataclassTransformSpecRecord) {
    writer.tag(DT_SPEC);
    writer.bool(record.eq_default);
    writer.bool(record.order_default);
    writer.bool(record.kw_only_default);
    writer.bool(record.frozen_default);
    writer.str_list(&record.field_specifiers);
    writer.tag(END_TAG);
}

// ---------------------------------------------------------------------------
// Builder (live Python objects -> records)
// ---------------------------------------------------------------------------

struct Classes<'py> {
    mypy_file: &'py PyAny,
    placeholder_node: &'py PyAny,
    type_info: &'py PyAny,
    var: &'py PyAny,
    func_def: &'py PyAny,
    decorator: &'py PyAny,
    overloaded_func_def: &'py PyAny,
    type_var_expr: &'py PyAny,
    param_spec_expr: &'py PyAny,
    type_var_tuple_expr: &'py PyAny,
    type_alias: &'py PyAny,
    class_def: &'py PyAny,
    dataclass_transform_spec: &'py PyAny,
}

impl<'py> Classes<'py> {
    fn load(py: Python<'py>) -> PyResult<Self> {
        let nodes = py.import("mypy.nodes")?;
        Ok(Self {
            mypy_file: nodes.getattr("MypyFile")?,
            placeholder_node: nodes.getattr("PlaceholderNode")?,
            type_info: nodes.getattr("TypeInfo")?,
            var: nodes.getattr("Var")?,
            func_def: nodes.getattr("FuncDef")?,
            decorator: nodes.getattr("Decorator")?,
            overloaded_func_def: nodes.getattr("OverloadedFuncDef")?,
            type_var_expr: nodes.getattr("TypeVarExpr")?,
            param_spec_expr: nodes.getattr("ParamSpecExpr")?,
            type_var_tuple_expr: nodes.getattr("TypeVarTupleExpr")?,
            type_alias: nodes.getattr("TypeAlias")?,
            class_def: nodes.getattr("ClassDef")?,
            dataclass_transform_spec: nodes.getattr("DataclassTransformSpec")?,
        })
    }
}

struct Capture<'py> {
    type_opt: &'py PyAny,
    type_list: &'py PyAny,
    literal: &'py PyAny,
    json: &'py PyAny,
}

impl<'py> Capture<'py> {
    /// Serialize one type payload; `None` becomes the `LITERAL_NONE` tag.
    fn type_opt(&self, value: &'py PyAny) -> PyResult<Vec<u8>> {
        if value.is_none() {
            return Ok(vec![LITERAL_NONE]);
        }
        self.call_bytes(self.type_opt, value)
    }

    fn type_list(&self, values: &'py PyAny) -> PyResult<Vec<u8>> {
        self.call_bytes(self.type_list, values)
    }

    fn literal(&self, value: &'py PyAny) -> PyResult<Vec<u8>> {
        self.call_bytes(self.literal, value)
    }

    fn json(&self, value: &'py PyAny) -> PyResult<Vec<u8>> {
        self.call_bytes(self.json, value)
    }

    fn call_bytes(&self, func: &'py PyAny, value: &'py PyAny) -> PyResult<Vec<u8>> {
        let out = func.call1((value,))?;
        match out.downcast::<PyBytes>() {
            Ok(bytes) => Ok(bytes.as_bytes().to_vec()),
            Err(_) => Err(PyTypeError::new_err(
                "cache data capture callback must return bytes",
            )),
        }
    }
}

fn get_str(obj: &PyAny, name: &str) -> PyResult<String> {
    obj.getattr(name)?.extract()
}

fn get_bool(obj: &PyAny, name: &str) -> PyResult<bool> {
    obj.getattr(name)?.is_true()
}

fn get_int(obj: &PyAny, name: &str) -> PyResult<i64> {
    obj.getattr(name)?.extract()
}

fn get_str_opt(obj: &PyAny, name: &str) -> PyResult<Option<String>> {
    let value = obj.getattr(name)?;
    if value.is_none() {
        Ok(None)
    } else {
        Ok(Some(value.extract()?))
    }
}

fn get_int_opt(obj: &PyAny, name: &str) -> PyResult<Option<i64>> {
    let value = obj.getattr(name)?;
    if value.is_none() {
        Ok(None)
    } else {
        Ok(Some(value.extract()?))
    }
}

fn get_str_vec(obj: &PyAny, name: &str) -> PyResult<Vec<String>> {
    let mut values = Vec::new();
    for item in obj.getattr(name)?.iter()? {
        values.push(item?.extract::<String>()?);
    }
    Ok(values)
}

fn build_root<'py>(
    tree: &'py PyAny,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<MypyFileRecord>> {
    if !tree.is_instance(classes.mypy_file)? {
        return Ok(None);
    }
    let fullname = get_str(tree, "_fullname")?;
    let Some(names) = build_symbol_table(tree.getattr("names")?, &fullname, classes, cap)? else {
        return Ok(None);
    };
    let mut future_import_flags = get_str_vec(tree, "future_import_flags")?;
    future_import_flags.sort();
    Ok(Some(MypyFileRecord {
        fullname,
        names,
        is_stub: get_bool(tree, "is_stub")?,
        path: get_str(tree, "path")?,
        is_partial_stub_package: get_bool(tree, "is_partial_stub_package")?,
        future_import_flags,
    }))
}

fn build_symbol_table<'py>(
    names: &'py PyAny,
    prefix: &str,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<SymbolTableRecord>> {
    let dict = names.downcast::<PyDict>().map_err(PyErr::from)?;
    let mut entries = Vec::new();
    for (key, value) in dict.iter() {
        let key: String = key.extract()?;
        if key == "__builtins__" || get_bool(value, "no_serialize")? {
            continue;
        }
        let Some(stn) = build_symbol_table_node(value, prefix, &key, classes, cap)? else {
            return Ok(None);
        };
        entries.push((key, stn));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(Some(SymbolTableRecord { entries }))
}

fn build_symbol_table_node<'py>(
    obj: &'py PyAny,
    prefix: &str,
    name: &str,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<SymbolTableNodeRecord>> {
    let kind = get_int(obj, "kind")?;
    let module_hidden = get_bool(obj, "module_hidden")?;
    let module_public = get_bool(obj, "module_public")?;
    let implicit = get_bool(obj, "implicit")?;
    let plugin_generated = get_bool(obj, "plugin_generated")?;
    let node = obj.getattr("node")?;
    let mut cross_ref = None;
    if node.is_instance(classes.mypy_file)? {
        cross_ref = Some(get_str(node, "fullname")?);
    } else if !node.is_none() {
        let fullname = get_str(node, "fullname")?;
        let from_module_getattr = if node.is_instance(classes.var)? {
            get_bool(node, "from_module_getattr")?
        } else {
            false
        };
        if fullname.contains('.') && fullname != format!("{prefix}.{name}") && !from_module_getattr
        {
            if node.is_instance(classes.placeholder_node)? {
                // Python asserts here; defer so the fallback raises.
                return Ok(None);
            }
            cross_ref = Some(fullname);
        }
    }
    let node_record = if cross_ref.is_none() {
        if node.is_none() {
            // Python asserts `self.node is not None`; defer to the fallback.
            return Ok(None);
        }
        match build_sym_node(node, classes, cap)? {
            Some(record) => Some(record),
            None => return Ok(None),
        }
    } else {
        None
    };
    Ok(Some(SymbolTableNodeRecord {
        kind,
        module_hidden,
        module_public,
        implicit,
        plugin_generated,
        cross_ref,
        node: node_record,
    }))
}

fn build_sym_node<'py>(
    node: &'py PyAny,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<SymNode>> {
    if node.is_instance(classes.type_info)? {
        Ok(build_type_info(node, classes, cap)?.map(|r| SymNode::TypeInfo(Box::new(r))))
    } else if node.is_instance(classes.var)? {
        Ok(build_var(node, cap)?.map(|r| SymNode::Var(Box::new(r))))
    } else if node.is_instance(classes.func_def)? {
        Ok(build_func_def(node, classes, cap)?.map(|r| SymNode::FuncDef(Box::new(r))))
    } else if node.is_instance(classes.decorator)? {
        Ok(build_decorator(node, classes, cap)?.map(|r| SymNode::Decorator(Box::new(r))))
    } else if node.is_instance(classes.overloaded_func_def)? {
        Ok(build_overloaded(node, classes, cap)?.map(|r| SymNode::OverloadedFuncDef(Box::new(r))))
    } else if node.is_instance(classes.type_var_expr)? {
        Ok(build_typevar_expr(node, cap)?.map(|r| SymNode::TypeVarExpr(Box::new(r))))
    } else if node.is_instance(classes.param_spec_expr)? {
        Ok(build_paramspec_expr(node, cap)?.map(|r| SymNode::ParamSpecExpr(Box::new(r))))
    } else if node.is_instance(classes.type_var_tuple_expr)? {
        Ok(build_typevartuple_expr(node, cap)?.map(|r| SymNode::TypeVarTupleExpr(Box::new(r))))
    } else if node.is_instance(classes.type_alias)? {
        Ok(build_type_alias(node, cap)?.map(|r| SymNode::TypeAlias(Box::new(r))))
    } else {
        Ok(None)
    }
}

fn build_type_info<'py>(
    obj: &'py PyAny,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<TypeInfoRecord>> {
    let fullname = get_str(obj, "fullname")?;
    let Some(names) = build_symbol_table(obj.getattr("names")?, &fullname, classes, cap)? else {
        return Ok(None);
    };
    let Some(defn) = build_class_def(obj.getattr("defn")?, classes, cap)? else {
        return Ok(None);
    };
    let mut abstract_names = Vec::new();
    let mut abstract_statuses = Vec::new();
    for item in obj.getattr("abstract_attributes")?.iter()? {
        let item = item?;
        let pair = item.downcast::<PyTuple>().map_err(PyErr::from)?;
        abstract_names.push(pair.get_item(0)?.extract::<String>()?);
        abstract_statuses.push(pair.get_item(1)?.extract::<i64>()?);
    }
    let type_vars = get_str_vec(obj, "type_vars")?;
    let mut mro = Vec::new();
    for base in obj.getattr("mro")?.iter()? {
        mro.push(get_str(base?, "fullname")?);
    }
    let slots_obj = obj.getattr("slots")?;
    let slots = if slots_obj.is_none() {
        None
    } else {
        let mut slots: Vec<String> = Vec::new();
        for slot in slots_obj.iter()? {
            slots.push(slot?.extract::<String>()?);
        }
        slots.sort();
        Some(slots)
    };
    let dts_obj = obj.getattr("dataclass_transform_spec")?;
    let dataclass_transform_spec = if dts_obj.is_none() {
        None
    } else {
        let Some(spec) = build_dt_spec(dts_obj, classes)? else {
            return Ok(None);
        };
        Some(spec)
    };
    let flags = [
        get_bool(obj, "is_abstract")?,
        get_bool(obj, "is_enum")?,
        get_bool(obj, "fallback_to_any")?,
        get_bool(obj, "meta_fallback_to_any")?,
        get_bool(obj, "is_named_tuple")?,
        get_bool(obj, "is_newtype")?,
        get_bool(obj, "is_protocol")?,
        get_bool(obj, "runtime_protocol")?,
        get_bool(obj, "is_final")?,
        get_bool(obj, "is_disjoint_base")?,
        get_bool(obj, "is_intersection")?,
    ];
    Ok(Some(TypeInfoRecord {
        names,
        defn,
        module_name: get_str(obj, "module_name")?,
        fullname,
        abstract_names,
        abstract_statuses,
        type_vars,
        has_param_spec_type: get_bool(obj, "has_param_spec_type")?,
        bases: cap.type_list(obj.getattr("bases")?)?,
        mro,
        promote: cap.type_list(obj.getattr("_promote")?)?,
        alt_promote: cap.type_opt(obj.getattr("alt_promote")?)?,
        declared_metaclass: cap.type_opt(obj.getattr("declared_metaclass")?)?,
        metaclass_type: cap.type_opt(obj.getattr("metaclass_type")?)?,
        tuple_type: cap.type_opt(obj.getattr("tuple_type")?)?,
        typeddict_type: cap.type_opt(obj.getattr("typeddict_type")?)?,
        flags,
        metadata: cap.json(obj.getattr("metadata")?)?,
        slots,
        deletable_attributes: get_str_vec(obj, "deletable_attributes")?,
        self_type: cap.type_opt(obj.getattr("self_type")?)?,
        dataclass_transform_spec,
        deprecated: get_str_opt(obj, "deprecated")?,
    }))
}

fn build_class_def<'py>(
    obj: &'py PyAny,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<ClassDefRecord>> {
    if !obj.is_instance(classes.class_def)? {
        return Ok(None);
    }
    Ok(Some(ClassDefRecord {
        name: get_str(obj, "name")?,
        type_vars: cap.type_list(obj.getattr("type_vars")?)?,
        fullname: get_str(obj, "fullname")?,
    }))
}

fn build_var<'py>(obj: &'py PyAny, cap: &Capture<'py>) -> PyResult<Option<VarRecord>> {
    let flags = [
        get_bool(obj, "is_initialized_in_class")?,
        get_bool(obj, "is_staticmethod")?,
        get_bool(obj, "is_classmethod")?,
        get_bool(obj, "is_property")?,
        get_bool(obj, "is_settable_property")?,
        get_bool(obj, "is_suppressed_import")?,
        get_bool(obj, "is_classvar")?,
        get_bool(obj, "is_abstract_var")?,
        get_bool(obj, "is_final")?,
        get_bool(obj, "is_index_var")?,
        get_bool(obj, "final_unset_in_class")?,
        get_bool(obj, "final_set_in_init")?,
        get_bool(obj, "explicit_self_type")?,
        get_bool(obj, "is_ready")?,
        get_bool(obj, "is_inferred")?,
        get_bool(obj, "invalid_partial_type")?,
        get_bool(obj, "from_module_getattr")?,
        get_bool(obj, "has_explicit_value")?,
        get_bool(obj, "allow_incompatible_override")?,
    ];
    Ok(Some(VarRecord {
        name: get_str(obj, "_name")?,
        type_: cap.type_opt(obj.getattr("type")?)?,
        setter_type: cap.type_opt(obj.getattr("setter_type")?)?,
        fullname: get_str(obj, "_fullname")?,
        flags,
        final_value: cap.literal(obj.getattr("final_value")?)?,
    }))
}

fn build_func_def<'py>(
    obj: &'py PyAny,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<FuncDefRecord>> {
    let flags = [
        get_bool(obj, "is_property")?,
        get_bool(obj, "is_class")?,
        get_bool(obj, "is_static")?,
        get_bool(obj, "is_final")?,
        get_bool(obj, "is_overload")?,
        get_bool(obj, "is_generator")?,
        get_bool(obj, "is_coroutine")?,
        get_bool(obj, "is_async_generator")?,
        get_bool(obj, "is_awaitable_coroutine")?,
        get_bool(obj, "is_decorated")?,
        get_bool(obj, "is_conditional")?,
        get_bool(obj, "is_trivial_body")?,
        get_bool(obj, "is_trivial_self")?,
        get_bool(obj, "is_mypy_only")?,
    ];
    let mut arg_names = Vec::new();
    for item in obj.getattr("arg_names")?.iter()? {
        let item = item?;
        if item.is_none() {
            arg_names.push(None);
        } else {
            arg_names.push(Some(item.extract::<String>()?));
        }
    }
    let mut arg_kinds = Vec::new();
    for item in obj.getattr("arg_kinds")?.iter()? {
        arg_kinds.push(item?.getattr("value")?.extract::<i64>()?);
    }
    let dts_obj = obj.getattr("dataclass_transform_spec")?;
    let dataclass_transform_spec = if dts_obj.is_none() {
        None
    } else {
        let Some(spec) = build_dt_spec(dts_obj, classes)? else {
            return Ok(None);
        };
        Some(spec)
    };
    Ok(Some(FuncDefRecord {
        name: get_str(obj, "_name")?,
        type_: cap.type_opt(obj.getattr("type")?)?,
        fullname: get_str(obj, "_fullname")?,
        flags,
        arg_names,
        arg_kinds,
        abstract_status: get_int(obj, "abstract_status")?,
        dataclass_transform_spec,
        deprecated: get_str_opt(obj, "deprecated")?,
        original_first_arg: get_str_opt(obj, "original_first_arg")?,
    }))
}

fn build_decorator<'py>(
    obj: &'py PyAny,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<DecoratorRecord>> {
    let Some(func) = build_sym_node(obj.getattr("func")?, classes, cap)? else {
        return Ok(None);
    };
    let Some(var) = build_sym_node(obj.getattr("var")?, classes, cap)? else {
        return Ok(None);
    };
    Ok(Some(DecoratorRecord {
        func,
        var,
        is_overload: get_bool(obj, "is_overload")?,
    }))
}

fn build_overloaded<'py>(
    obj: &'py PyAny,
    classes: &Classes<'py>,
    cap: &Capture<'py>,
) -> PyResult<Option<OverloadedFuncDefRecord>> {
    let mut items = Vec::new();
    for item in obj.getattr("items")?.iter()? {
        let Some(record) = build_sym_node(item?, classes, cap)? else {
            return Ok(None);
        };
        items.push(record);
    }
    let impl_obj = obj.getattr("impl")?;
    let impl_ = if impl_obj.is_none() {
        None
    } else {
        let Some(record) = build_sym_node(impl_obj, classes, cap)? else {
            return Ok(None);
        };
        Some(record)
    };
    let flags = [
        get_bool(obj, "is_property")?,
        get_bool(obj, "is_class")?,
        get_bool(obj, "is_static")?,
        get_bool(obj, "is_final")?,
    ];
    Ok(Some(OverloadedFuncDefRecord {
        items,
        type_: cap.type_opt(obj.getattr("type")?)?,
        fullname: get_str(obj, "_fullname")?,
        impl_,
        flags,
        deprecated: get_str_opt(obj, "deprecated")?,
        setter_index: get_int_opt(obj, "setter_index")?,
    }))
}

fn build_typevar_expr<'py>(
    obj: &'py PyAny,
    cap: &Capture<'py>,
) -> PyResult<Option<TypeVarExprRecord>> {
    Ok(Some(TypeVarExprRecord {
        name: get_str(obj, "_name")?,
        fullname: get_str(obj, "_fullname")?,
        values: cap.type_list(obj.getattr("values")?)?,
        upper_bound: cap.type_opt(obj.getattr("upper_bound")?)?,
        default: cap.type_opt(obj.getattr("default")?)?,
        variance: get_int(obj, "variance")?,
    }))
}

fn build_paramspec_expr<'py>(
    obj: &'py PyAny,
    cap: &Capture<'py>,
) -> PyResult<Option<ParamSpecExprRecord>> {
    Ok(Some(ParamSpecExprRecord {
        name: get_str(obj, "_name")?,
        fullname: get_str(obj, "_fullname")?,
        upper_bound: cap.type_opt(obj.getattr("upper_bound")?)?,
        default: cap.type_opt(obj.getattr("default")?)?,
        variance: get_int(obj, "variance")?,
    }))
}

fn build_typevartuple_expr<'py>(
    obj: &'py PyAny,
    cap: &Capture<'py>,
) -> PyResult<Option<TypeVarTupleExprRecord>> {
    Ok(Some(TypeVarTupleExprRecord {
        tuple_fallback: cap.type_opt(obj.getattr("tuple_fallback")?)?,
        name: get_str(obj, "_name")?,
        fullname: get_str(obj, "_fullname")?,
        upper_bound: cap.type_opt(obj.getattr("upper_bound")?)?,
        default: cap.type_opt(obj.getattr("default")?)?,
        variance: get_int(obj, "variance")?,
    }))
}

fn build_type_alias<'py>(obj: &'py PyAny, cap: &Capture<'py>) -> PyResult<Option<TypeAliasRecord>> {
    Ok(Some(TypeAliasRecord {
        fullname: get_str(obj, "_fullname")?,
        module: get_str(obj, "module")?,
        target: cap.type_opt(obj.getattr("target")?)?,
        alias_tvars: cap.type_list(obj.getattr("alias_tvars")?)?,
        no_args: get_bool(obj, "no_args")?,
        normalized: get_bool(obj, "normalized")?,
        python_3_12_type_alias: get_bool(obj, "python_3_12_type_alias")?,
    }))
}

fn build_dt_spec<'py>(
    obj: &'py PyAny,
    classes: &Classes<'py>,
) -> PyResult<Option<DataclassTransformSpecRecord>> {
    if !obj.is_instance(classes.dataclass_transform_spec)? {
        return Ok(None);
    }
    let mut field_specifiers = Vec::new();
    for item in obj.getattr("field_specifiers")?.iter()? {
        field_specifiers.push(item?.extract::<String>()?);
    }
    Ok(Some(DataclassTransformSpecRecord {
        eq_default: get_bool(obj, "eq_default")?,
        order_default: get_bool(obj, "order_default")?,
        kw_only_default: get_bool(obj, "kw_only_default")?,
        frozen_default: get_bool(obj, "frozen_default")?,
        field_specifiers,
    }))
}

fn is_defer_error(py: Python<'_>, err: &PyErr) -> bool {
    err.is_instance_of::<PyAttributeError>(py)
        || err.is_instance_of::<PyAssertionError>(py)
        || err.is_instance_of::<PyNotImplementedError>(py)
}

/// Serialize a checked module tree to the fixed-format cache payload.
///
/// Returns `None` when the tree contains a shape the Rust writer does not
/// implement; the caller then runs `MypyFile.write` so the bytes can never
/// diverge. The capture callbacks serialize the Python-side field payloads
/// (types, literals, JSON metadata) with the legacy Python writers.
#[pyfunction]
#[pyo3(signature = (tree, capture_type_opt, capture_type_list, capture_literal, capture_json))]
pub(crate) fn write_cache_data<'py>(
    py: Python<'py>,
    tree: &'py PyAny,
    capture_type_opt: &'py PyAny,
    capture_type_list: &'py PyAny,
    capture_literal: &'py PyAny,
    capture_json: &'py PyAny,
) -> PyResult<Option<PyObject>> {
    let classes = Classes::load(py)?;
    let capture = Capture {
        type_opt: capture_type_opt,
        type_list: capture_type_list,
        literal: capture_literal,
        json: capture_json,
    };
    match build_root(tree, &classes, &capture) {
        Ok(None) => Ok(None),
        Ok(Some(record)) => {
            let mut writer = Writer::default();
            write_mypy_file(&mut writer, &record);
            Ok(Some(PyBytes::new(py, &writer.into_bytes()).into()))
        }
        Err(err) if is_defer_error(py, &err) => Ok(None),
        Err(err) => Err(err),
    }
}

// ---------------------------------------------------------------------------
// Unit tests: writer bytes are pinned independently of the Python writer.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn stn(kind: i64, cross_ref: Option<&str>) -> SymbolTableNodeRecord {
        SymbolTableNodeRecord {
            kind,
            module_hidden: false,
            module_public: true,
            implicit: false,
            plugin_generated: false,
            cross_ref: cross_ref.map(str::to_owned),
            node: None,
        }
    }

    fn stn_with_node(kind: i64, node: SymNode) -> SymbolTableNodeRecord {
        SymbolTableNodeRecord {
            kind,
            module_hidden: false,
            module_public: true,
            implicit: false,
            plugin_generated: false,
            cross_ref: None,
            node: Some(node),
        }
    }

    fn bytes_of(node: &SymNode) -> Vec<u8> {
        let mut writer = Writer::default();
        write_sym_node(&mut writer, node);
        writer.into_bytes()
    }

    #[test]
    fn tag_constants_match_nodes_py() {
        assert_eq!(MYPY_FILE, 50);
        assert_eq!(OVERLOADED_FUNC_DEF, 51);
        assert_eq!(FUNC_DEF, 52);
        assert_eq!(DECORATOR, 53);
        assert_eq!(VAR, 54);
        assert_eq!(TYPE_VAR_EXPR, 55);
        assert_eq!(PARAM_SPEC_EXPR, 56);
        assert_eq!(TYPE_VAR_TUPLE_EXPR, 57);
        assert_eq!(TYPE_INFO, 58);
        assert_eq!(TYPE_ALIAS, 59);
        assert_eq!(crate::CLASS_DEF, 60);
        assert_eq!(SYMBOL_TABLE_NODE, 61);
        assert_eq!(DT_SPEC, 151);
        assert_eq!(crate::LIST_STR, 22);
    }

    #[test]
    fn var_minimal_payload_bytes() {
        let record = VarRecord {
            name: "x".to_owned(),
            type_: vec![LITERAL_NONE],
            setter_type: vec![LITERAL_NONE],
            fullname: "mod.x".to_owned(),
            flags: [false; 19],
            final_value: vec![LITERAL_NONE],
        };
        assert_eq!(
            bytes_of(&SymNode::Var(Box::new(record))),
            vec![
                VAR, // 54
                4, 22, b'x', // write_str("x")
                2, 2, // write_type_opt(None) x2
                4, 30, b'm', b'o', b'd', b'.', b'x', // write_str("mod.x")
                3, 20,  // write_flags(all false) = tagged int 0
                2,   // write_literal(None)
                255, // END_TAG
            ]
        );
    }

    #[test]
    fn flags_pack_into_tagged_int() {
        let mut flags = [false; 19];
        flags[0] = true;
        flags[2] = true;
        let record = VarRecord {
            name: "x".to_owned(),
            type_: vec![LITERAL_NONE],
            setter_type: vec![LITERAL_NONE],
            fullname: "m.x".to_owned(),
            flags,
            final_value: vec![LITERAL_NONE],
        };
        let bytes = bytes_of(&SymNode::Var(Box::new(record)));
        // packed = 1 << 0 | 1 << 2 = 5 -> tag 3 + bare (5 + 10) << 1 = 30.
        assert!(bytes.windows(2).any(|window| window == [3, 30]));
    }

    #[test]
    fn symbol_table_node_cross_ref_bytes() {
        let record = stn(1, Some("a.b"));
        let mut writer = Writer::default();
        write_symbol_table_node(&mut writer, &record);
        assert_eq!(
            writer.into_bytes(),
            vec![
                SYMBOL_TABLE_NODE,
                3,
                22, // write_int(1) = tagged bare (1 + 10) << 1 = 22
                0,
                1,
                0,
                0, // module_hidden=false, public=true, implicit, plugin
                4,
                26,
                b'a',
                b'.',
                b'b', // write_str_opt("a.b")
                255,
            ]
        );
    }

    #[test]
    fn symbol_table_emits_entries_in_record_order() {
        let table = SymbolTableRecord {
            entries: vec![
                ("b".to_owned(), stn(1, Some("m.b"))),
                ("a".to_owned(), stn(1, Some("m.a"))),
            ],
        };
        let mut writer = Writer::default();
        write_symbol_table(&mut writer, &table);
        assert_eq!(
            writer.into_bytes(),
            vec![
                DICT_STR_GEN,
                24, // bare size 2
                22,
                b'b', // bare key "b"
                SYMBOL_TABLE_NODE,
                3,
                22, // write_int(kind=1)
                0,
                1,
                0,
                0, // module flags
                4,
                26,
                b'm',
                b'.',
                b'b', // cross_ref
                END_TAG,
                22,
                b'a', // bare key "a"
                SYMBOL_TABLE_NODE,
                3,
                22,
                0,
                1,
                0,
                0,
                4,
                26,
                b'm',
                b'.',
                b'a',
                END_TAG,
            ]
        );
    }

    #[test]
    fn dataclass_transform_spec_bytes() {
        let record = DataclassTransformSpecRecord {
            eq_default: true,
            order_default: false,
            kw_only_default: false,
            frozen_default: false,
            field_specifiers: Vec::new(),
        };
        let mut writer = Writer::default();
        write_dt_spec(&mut writer, &record);
        assert_eq!(
            writer.into_bytes(),
            vec![DT_SPEC, 1, 0, 0, 0, crate::LIST_STR, 20, END_TAG]
        );
    }

    #[test]
    fn local_symbol_skips_are_writer_independent() {
        // The writer emits exactly the entries it is given; the builder owns
        // `__builtins__` / `no_serialize` filtering and pre-sorts keys.
        let table = SymbolTableRecord {
            entries: vec![(
                "a".to_owned(),
                stn_with_node(1, SymNode::Var(Box::new(minimal_var()))),
            )],
        };
        let mut writer = Writer::default();
        write_symbol_table(&mut writer, &table);
        let bytes = writer.into_bytes();
        assert_eq!(bytes[0], DICT_STR_GEN);
        assert_eq!(bytes[1], 22); // bare size 1
        assert_eq!(bytes[2], 22); // bare len("a") = 1
        assert_eq!(bytes[3], b'a');
        assert_eq!(bytes[4], SYMBOL_TABLE_NODE);
    }

    fn minimal_var() -> VarRecord {
        VarRecord {
            name: "x".to_owned(),
            type_: vec![LITERAL_NONE],
            setter_type: vec![LITERAL_NONE],
            fullname: "m.x".to_owned(),
            flags: [false; 19],
            final_value: vec![LITERAL_NONE],
        }
    }
}
