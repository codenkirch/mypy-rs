//! Port of `mypy/message_registry.py` (Issue #535).
//!
//! Ports the `ErrorMessage` class as a `#[pyclass]` and all module-level
//! message constants as factory `#[pyfunction]`s. The `format()` method
//! delegates to Python's `str.format` for byte-identical behaviour.
//! Error-code objects are looked up from `mypy.errorcodes` so the `.code`
//! attribute returns the real `ErrorCode` object Python callers expect.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString, PyTuple};

// ---------------------------------------------------------------------------
// ErrorMessage class
// ---------------------------------------------------------------------------

/// Mirrors `mypy.message_registry.ErrorMessage`.
///
/// `value` is the message template (with `{}` placeholders).
/// `code` is an optional `mypy.errorcodes.ErrorCode` Python object,
/// stored as `PyObject` so callers get the real object back.
#[pyclass]
pub struct ErrorMessage {
    value: String,
    code: Option<PyObject>,
}

#[allow(non_local_definitions)]
#[pymethods]
impl ErrorMessage {
    #[new]
    #[pyo3(signature = (value, code=None))]
    pub fn new(value: String, code: Option<PyObject>) -> Self {
        Self { value, code }
    }

    #[getter]
    pub fn value(&self) -> String {
        self.value.clone()
    }

    #[getter]
    pub fn code(&self) -> Option<PyObject> {
        self.code.clone()
    }

    /// Format the template with the given args/kwargs, returning a new
    /// `ErrorMessage` with the same `code`. Delegates to Python's
    /// `str.format` for exact parity with the original.
    #[pyo3(signature = (*args, **kwargs))]
    pub fn format(
        &self,
        py: Python<'_>,
        args: &PyTuple,
        kwargs: Option<&PyDict>,
    ) -> PyResult<ErrorMessage> {
        let py_str = PyString::new(py, &self.value);
        let formatted = py_str.call_method("format", args, kwargs)?;
        let formatted_str: String = formatted.extract()?;
        Ok(ErrorMessage {
            value: formatted_str,
            code: self.code.clone(),
        })
    }

    /// Append `info` to the message, returning a new `ErrorMessage`
    /// with the same `code`.
    pub fn with_additional_msg(&self, info: String) -> ErrorMessage {
        ErrorMessage {
            value: self.value.clone() + &info,
            code: self.code.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Error-code lookup helper
// ---------------------------------------------------------------------------

/// Look up an `ErrorCode` object from `mypy.errorcodes.error_codes` by its
/// string code (e.g. `"return-value"`). Returns `None` for `"None"`.
fn lookup_error_code(py: Python<'_>, code_str: &str) -> PyResult<Option<PyObject>> {
    if code_str == "None" {
        return Ok(None);
    }
    let module = py.import("mypy.errorcodes")?;
    let codes = module.getattr("error_codes")?;
    let code = codes.get_item(code_str)?;
    Ok(Some(code.into()))
}

// ---------------------------------------------------------------------------
// Factory macros
// ---------------------------------------------------------------------------

/// Generate a factory `#[pyfunction]` returning an `ErrorMessage`
/// without an error code.
macro_rules! em_no_code {
    ($name:ident, $value:expr) => {
        #[pyfunction]
        pub fn $name() -> ErrorMessage {
            ErrorMessage::new($value.to_string(), None)
        }
    };
}

/// Generate a factory `#[pyfunction]` returning an `ErrorMessage`
/// with an error code, looked up at call time from `mypy.errorcodes`.
macro_rules! em_with_code {
    ($name:ident, $value:expr, $code:expr) => {
        #[pyfunction]
        pub fn $name(py: Python<'_>) -> PyResult<ErrorMessage> {
            let code = lookup_error_code(py, $code)?;
            Ok(ErrorMessage::new($value.to_string(), code))
        }
    };
}

/// Generate a factory `#[pyfunction]` returning a plain `String`.
macro_rules! str_const {
    ($name:ident, $value:expr) => {
        #[pyfunction]
        pub fn $name() -> String {
            $value.to_string()
        }
    };
}

// ---------------------------------------------------------------------------
// Invalid types
// ---------------------------------------------------------------------------

em_with_code!(
    invalid_type_raw_enum_value,
    "Invalid type: try using Literal[{}.{}] instead?",
    "valid-type"
);

// ---------------------------------------------------------------------------
// Type checker error message constants — ErrorMessage instances
// ---------------------------------------------------------------------------

em_with_code!(
    no_return_value_expected,
    "No return value expected",
    "return-value"
);
em_with_code!(
    missing_return_statement,
    "Missing return statement",
    "return"
);
em_with_code!(
    empty_body_abstract,
    "If the method is meant to be abstract, use @abc.abstractmethod",
    "empty-body"
);
em_no_code!(
    invalid_implicit_return,
    "Implicit return in function which does not return"
);
em_with_code!(
    incompatible_return_value_type,
    "Incompatible return value type",
    "return-value"
);
em_with_code!(
    return_value_expected,
    "Return value expected",
    "return-value"
);
em_no_code!(
    no_return_expected,
    "Return statement in function which does not return"
);
em_no_code!(
    invalid_exception,
    "Exception must be derived from BaseException"
);
em_no_code!(
    invalid_exception_type,
    "Exception type must be derived from BaseException (or be a tuple of exception classes)"
);
em_no_code!(
    invalid_exception_group,
    "Exception type in except* cannot derive from BaseExceptionGroup"
);
em_no_code!(
    return_in_async_generator,
    "\"return\" with value in async generator is not allowed"
);
em_no_code!(
    invalid_return_type_for_generator,
    "The return type of a generator function should be \"Generator\" or one of its supertypes"
);
em_no_code!(
    invalid_return_type_for_async_generator,
    concat!(
        "The return type of an async generator function should be \"AsyncGenerator\" or one of its ",
        "supertypes"
    )
);
em_no_code!(yield_value_expected, "Yield value expected");
em_no_code!(incompatible_types, "Incompatible types");
em_with_code!(
    incompatible_types_in_assignment,
    "Incompatible types in assignment",
    "assignment"
);
em_with_code!(
    covariant_override_of_mutable_attribute,
    "Covariant override of a mutable attribute",
    "mutable-override"
);
em_no_code!(
    incompatible_types_in_await,
    "Incompatible types in \"await\""
);
em_no_code!(incompatible_redefinition, "Incompatible redefinition");
em_no_code!(
    incompatible_types_in_yield,
    "Incompatible types in \"yield\""
);
em_no_code!(
    incompatible_types_in_yield_from,
    "Incompatible types in \"yield from\""
);
em_no_code!(
    incompatible_types_in_capture,
    "Incompatible types in capture pattern"
);
em_no_code!(
    must_have_none_return_type,
    "The return type of \"{}\" must be None"
);
em_no_code!(tuple_index_out_of_range, "Tuple index out of range");
em_no_code!(
    ambiguous_slice_of_variadic_tuple,
    "Ambiguous slice of a variadic tuple"
);
em_no_code!(
    too_many_targets_for_variadic_unpack,
    "Too many assignment targets for variadic unpack"
);
em_no_code!(cannot_infer_lambda_type, "Cannot infer type of lambda");
em_no_code!(
    non_instance_new_type,
    "\"__new__\" must return a class instance (got {})"
);
em_no_code!(invalid_new_type, "Incompatible return type for \"__new__\"");
em_no_code!(
    bad_constructor_type,
    "Unsupported decorated constructor type"
);
em_no_code!(
    inconsistent_abstract_overload,
    "Overloaded method has both abstract and non-abstract variants"
);
em_no_code!(
    multiple_overloads_required,
    "Single overload definition, multiple required"
);
em_no_code!(
    read_only_property_overrides_read_write,
    "Read-only property cannot override read-write property"
);
em_no_code!(
    return_type_cannot_be_contravariant,
    "Cannot use a contravariant type variable as return type"
);
em_no_code!(
    function_parameter_cannot_be_covariant,
    "Cannot use a covariant type variable as a parameter"
);
em_with_code!(
    incompatible_import_of,
    "Incompatible import of \"{}\"",
    "assignment"
);
em_with_code!(
    function_type_expected,
    "Function is missing a type annotation",
    "no-untyped-def"
);
em_no_code!(
    only_class_application,
    "Type application is only supported for generic classes"
);
em_with_code!(
    return_type_expected,
    "Function is missing a return type annotation",
    "no-untyped-def"
);
em_with_code!(
    param_type_expected,
    "Function is missing a type annotation for one or more parameters",
    "no-untyped-def"
);
em_no_code!(
    keyword_argument_requires_str_key_type,
    "Keyword argument only valid with \"str\" key type in call to \"dict\""
);
em_no_code!(all_must_be_seq_str, "Type of __all__ must be {}, not {}");
em_no_code!(
    invalid_typeddict_args,
    "Expected keyword arguments, {...}, or dict(...) in TypedDict constructor"
);
em_no_code!(
    typeddict_key_must_be_string_literal,
    "Expected TypedDict key to be string literal"
);
em_no_code!(
    malformed_assert,
    "Assertion is always true, perhaps remove parentheses?"
);
em_no_code!(
    duplicate_type_signatures,
    "Function has duplicate type signatures"
);
em_no_code!(descriptor_set_not_callable, "{}.__set__ is not callable");
em_no_code!(
    module_level_getattribute,
    "__getattribute__ is not valid at the module level"
);
em_no_code!(
    name_not_in_slots,
    "Trying to assign name \"{}\" that is not in \"__slots__\" of type \"{}\""
);
em_with_code!(
    type_always_true,
    "{} which does not implement __bool__ or __len__ so it could always be true in boolean context",
    "truthy-bool"
);
em_with_code!(
    type_always_true_uniontype,
    "{} of which no members implement __bool__ or __len__ so it could always be true in boolean context",
    "truthy-bool"
);
em_with_code!(
    function_always_true,
    "Function {} could always be true in boolean context",
    "truthy-function"
);
em_with_code!(
    iterable_always_true,
    "{} which can always be true in boolean context. Consider using {} instead.",
    "truthy-iterable"
);

// ---------------------------------------------------------------------------
// Super
// ---------------------------------------------------------------------------

em_no_code!(too_many_args_for_super, "Too many arguments for \"super\"");
em_no_code!(
    super_with_single_arg_not_supported,
    "\"super\" with a single argument not supported"
);
em_no_code!(
    unsupported_arg_1_for_super,
    "Unsupported argument 1 for \"super\""
);
em_no_code!(
    unsupported_arg_2_for_super,
    "Unsupported argument 2 for \"super\""
);
em_no_code!(
    super_varargs_not_supported,
    "Varargs not supported with \"super\""
);
em_no_code!(
    super_positional_args_required,
    "\"super\" only accepts positional arguments"
);
em_no_code!(
    super_arg_2_not_instance_of_arg_1,
    "Argument 2 for \"super\" not an instance of argument 1"
);
em_no_code!(
    target_class_has_no_base_class,
    "Target class has no base class"
);
em_no_code!(
    super_outside_of_method_not_supported,
    "\"super()\" outside of a method is not supported"
);
em_no_code!(
    super_enclosing_positional_args_required,
    "\"super()\" requires one or two positional arguments in enclosing function"
);

// ---------------------------------------------------------------------------
// Self-type
// ---------------------------------------------------------------------------

em_no_code!(
    missing_or_invalid_self_type,
    "\"self\" parameter missing for a non-static method (or an invalid type for self)"
);
em_no_code!(
    erased_self_type_not_supertype,
    "The erased type of self \"{}\" is not a supertype of its class \"{}\""
);

// ---------------------------------------------------------------------------
// Final
// ---------------------------------------------------------------------------

em_no_code!(
    cannot_inherit_from_final,
    "Cannot inherit from final class \"{}\""
);
em_no_code!(
    dependent_final_in_class_body,
    "Final name declared in class body cannot depend on type variables"
);
em_no_code!(
    cannot_make_deletable_final,
    "Deletable attribute cannot be final"
);

// ---------------------------------------------------------------------------
// Disjoint bases
// ---------------------------------------------------------------------------

em_no_code!(
    incompatible_disjoint_bases,
    "Class \"{}\" has incompatible disjoint bases"
);

// ---------------------------------------------------------------------------
// Enum
// ---------------------------------------------------------------------------

em_no_code!(
    enum_members_attr_will_be_overridden,
    "Assigned \"__members__\" will be overridden by \"Enum\" internally"
);

// ---------------------------------------------------------------------------
// ClassVar
// ---------------------------------------------------------------------------

em_no_code!(
    cannot_override_instance_var,
    concat!(
        "Cannot override instance variable (previously declared on base class \"{}\") with class ",
        "variable"
    )
);
em_no_code!(
    cannot_override_class_var,
    concat!(
        "Cannot override class variable (previously declared on base class \"{}\") with instance ",
        "variable"
    )
);

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

em_no_code!(
    runtime_protocol_expected,
    "Only @runtime_checkable protocols can be used with instance and class checks"
);
em_no_code!(
    cannot_instantiate_protocol,
    "Cannot instantiate protocol class \"{}\""
);
em_no_code!(
    too_many_union_combinations,
    "Not all union combinations were tried because there are too many unions"
);
em_no_code!(
    contiguous_iterable_expected,
    "Contiguous iterable with same type expected"
);
em_no_code!(
    iterable_type_expected,
    "Invalid type '{}' for *expr (iterable expected)"
);
em_no_code!(
    type_guard_pos_arg_required,
    "Type {} requires positional argument"
);

// ---------------------------------------------------------------------------
// fastparse
// ---------------------------------------------------------------------------

em_no_code!(
    failed_to_merge_overloads,
    "Condition can't be inferred, unable to merge overloads"
);
em_with_code!(
    type_ignore_with_errcode_on_module,
    "Type ignore with error code is not supported for modules; use `# mypy: disable-error-code=\"{}\"`",
    "syntax"
);
em_with_code!(
    invalid_type_ignore,
    "Invalid \"type: ignore\" comment",
    "syntax"
);
em_with_code!(
    type_comment_syntax_error_value,
    "Syntax error in type comment \"{}\"",
    "syntax"
);
em_with_code!(
    ellipsis_with_other_typeparams,
    "Ellipses cannot accompany other parameter types in function type signature",
    "syntax"
);
em_with_code!(
    type_signature_too_many_params,
    "Type signature has too many parameters",
    "syntax"
);
em_with_code!(
    type_signature_too_few_params,
    "Type signature has too few parameters",
    "syntax"
);
em_with_code!(
    arg_constructor_name_expected,
    "Expected arg constructor name",
    "syntax"
);
em_with_code!(
    arg_constructor_too_many_args,
    "Too many arguments for argument constructor",
    "syntax"
);
em_with_code!(
    multiple_values_for_name_kwarg,
    "\"{}\" gets multiple values for keyword argument \"name\"",
    "syntax"
);
em_with_code!(
    multiple_values_for_type_kwarg,
    "\"{}\" gets multiple values for keyword argument \"type\"",
    "syntax"
);
em_with_code!(
    arg_constructor_unexpected_arg,
    "Unexpected argument \"{}\" for argument constructor",
    "syntax"
);
em_with_code!(
    arg_name_expected_string_literal,
    "Expected string literal for argument name, got {}",
    "syntax"
);
em_with_code!(
    narrowed_type_not_subtype,
    "Narrowed type {} is not a subtype of input type {}",
    "narrowed-type-not-subtype"
);

// ---------------------------------------------------------------------------
// TypeVar / TypeAlias (fastparse)
// ---------------------------------------------------------------------------

em_with_code!(
    type_var_too_few_constrained_types,
    "Type variable must have at least two constrained types",
    "misc"
);
em_with_code!(
    type_var_yield_expression_in_bound,
    "Yield expression cannot be used as a type variable bound",
    "syntax"
);
em_with_code!(
    type_var_named_expression_in_bound,
    "Named expression cannot be used as a type variable bound",
    "syntax"
);
em_with_code!(
    type_var_await_expression_in_bound,
    "Await expression cannot be used as a type variable bound",
    "syntax"
);
em_with_code!(
    type_var_generic_constraint_type,
    "TypeVar constraint type cannot be parametrized by type variables",
    "misc"
);
em_with_code!(
    type_var_redeclared_in_nested_class,
    "Type variable \"{}\" is bound by an outer class",
    "valid-type"
);
em_with_code!(
    type_alias_with_yield_expression,
    "Yield expression cannot be used within a type alias",
    "syntax"
);
em_with_code!(
    type_alias_with_named_expression,
    "Named expression cannot be used within a type alias",
    "syntax"
);
em_with_code!(
    type_alias_with_await_expression,
    "Await expression cannot be used within a type alias",
    "syntax"
);

// ---------------------------------------------------------------------------
// Plain string constants
// ---------------------------------------------------------------------------

str_const!(
    incompatible_types_in_async_with_aenter,
    "Incompatible types in \"async with\" for \"__aenter__\""
);
str_const!(
    incompatible_types_in_async_with_aexit,
    "Incompatible types in \"async with\" for \"__aexit__\""
);
str_const!(
    incompatible_types_in_async_for,
    "Incompatible types in \"async for\""
);
str_const!(invalid_type_for_slots, "Invalid type for \"__slots__\"");
str_const!(
    async_for_outside_coroutine,
    "\"async for\" outside async function"
);
str_const!(
    async_with_outside_coroutine,
    "\"async with\" outside async function"
);
str_const!(
    incompatible_types_in_str_interpolation,
    "Incompatible types in string interpolation"
);
str_const!(
    cannot_access_init,
    "Accessing \"__init__\" on an instance is unsound, since instance.__init__ could be from an incompatible subclass"
);
str_const!(cannot_assign_to_method, "Cannot assign to a method");
str_const!(cannot_assign_to_type, "Cannot assign to a type");
str_const!(format_requires_mapping, "Format requires a mapping");
str_const!(
    typeddict_override_merge,
    "Overwriting TypedDict field \"{}\" while merging"
);
str_const!(descriptor_get_not_callable, "{}.__get__ is not callable");
str_const!(
    class_var_conflicts_slots,
    "\"{}\" in __slots__ conflicts with class variable access"
);
str_const!(not_callable, "{} not callable");
str_const!(type_must_be_used, "Value of type {} must be used");
str_const!(
    generic_instance_var_class_access,
    "Access to generic instance variables via class is ambiguous"
);
str_const!(
    generic_class_var_access,
    "Access to generic class variables is ambiguous"
);
str_const!(bare_generic, "Missing type arguments for generic type {}");
str_const!(
    implicit_generic_any_builtin,
    "Implicit generic \"Any\". Use \"{}\" and specify generic parameters"
);
str_const!(
    no_cyclic_default,
    "Cyclic type variable defaults are not supported"
);
str_const!(
    no_default_after_typevar_tuple,
    "A type variable with default cannot follow TypeVarTuple"
);
str_const!(
    invalid_unpack,
    "{} cannot be unpacked (must be tuple or TypeVarTuple)"
);
str_const!(
    invalid_unpack_position,
    "Unpack is only valid in a variadic position"
);
str_const!(
    invalid_param_spec_location,
    "Invalid location for ParamSpec {}"
);
str_const!(
    invalid_param_spec_location_note,
    "You can use ParamSpec as the first argument to Callable, e.g., \"Callable[{}, int]\""
);
str_const!(
    incompatible_typevar_value,
    "Value of type variable \"{}\" of {} cannot be {}"
);
str_const!(
    invalid_typevar_as_typearg,
    "Type variable \"{}\" not valid as type argument value for \"{}\""
);
str_const!(
    invalid_typevar_arg_bound,
    "Type argument {} of \"{}\" must be a subtype of {}"
);
str_const!(
    invalid_typevar_arg_value,
    "Invalid type argument value for \"{}\""
);
str_const!(
    typevar_variance_def,
    "TypeVar \"{}\" may only be a literal bool"
);
str_const!(typevar_arg_must_be_type, "{} \"{}\" must be a type");
str_const!(
    typevar_unexpected_argument,
    "Unexpected argument to \"TypeVar()\""
);
str_const!(
    unbound_typevar,
    "A function returning TypeVar should receive at least one argument containing the same TypeVar"
);
str_const!(
    type_parameters_should_be_declared,
    "All type parameters should be declared ({} not declared)"
);
str_const!(
    cannot_access_final_instance_attr,
    "Cannot access final instance attribute \"{}\" on class object"
);
str_const!(
    cannot_access_instance_only_attr,
    "Cannot access instance-only attribute \"{}\" on class object"
);
str_const!(
    class_var_with_generic_self,
    "ClassVar cannot contain Self type in generic classes"
);
str_const!(
    class_var_outside_of_class,
    "ClassVar can only be used for assignments in class body"
);
str_const!(
    missing_match_args,
    "Class \"{}\" doesn't define \"__match_args__\""
);
str_const!(
    or_pattern_alternative_names,
    "Alternative patterns bind different names"
);
str_const!(
    class_pattern_generic_type_alias,
    "Class pattern class must not be a type alias with type parameters"
);
str_const!(
    class_pattern_type_required,
    "Expected type in class pattern; found \"{}\""
);
str_const!(
    class_pattern_too_many_positional_args,
    "Too many positional patterns for class pattern"
);
str_const!(
    class_pattern_keyword_matches_positional,
    "Keyword \"{}\" already matches a positional pattern"
);
str_const!(
    class_pattern_duplicate_keyword_pattern,
    "Duplicate keyword pattern \"{}\""
);
str_const!(
    class_pattern_unknown_keyword,
    "Class \"{}\" has no attribute \"{}\""
);
str_const!(
    class_pattern_class_or_static_method,
    "Cannot have both classmethod and staticmethod"
);
str_const!(
    multiple_assignments_in_pattern,
    "Multiple assignments to name \"{}\" in pattern"
);
str_const!(
    cannot_modify_match_args,
    "Cannot assign to \"__match_args__\""
);
str_const!(
    dataclass_field_alias_must_be_literal,
    "\"alias\" argument to dataclass field must be a string literal"
);
str_const!(
    dataclass_post_init_must_be_a_function,
    "\"__post_init__\" method must be an instance method"
);
