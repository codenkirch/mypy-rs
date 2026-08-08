//! Stage 6 message formatting (messages.rs) for Issue #299.
//!
//! Ports the pure formatting functions from `mypy/messages.py`:
//! - `format_type` / `format_type_bare` / `format_type_inner`
//! - `format_type_distinctly`
//! - `quote_type_string`
//! - `format_callable_args`
//! - `pretty_callable`
//! - `pretty_seq`, `capitalize`, `format_string_list`, `format_item_name_list`
//! - `wrong_type_arg_count`, `callable_name`, `for_function`
//! - `extract_type`, `strip_quotes`, `variance_string`
//! - `append_invariance_notes`, `append_numbers_notes`, `append_union_note`
//! - `pretty_callable` (definition-free form)
//! - `rust_format_key_list` (pre-existing)
//!
//! The Rust path walks the wire-format `Type` enum (from `wire.rs`) and
//! applies the `format_type_inner` rules (not the `str(type)` rules from
//! `TypeStrVisitor`). Key differences from `str(type)`:
//! - Compact `Callable[[...], ret]` form (unless pretty_callable applies)
//! - Union coalescing (`Literal[...]` grouping, `Optional` detection)
//! - `verbosity` / `fullnames` / `module_names` controls
//! - TypedDict named-vs-anonymous distinction
//!
//! Returns `None` for any type the Rust path does not handle, so the
//! Python caller falls back to the pure-Python formatter.

use std::collections::HashSet;

use pyo3::prelude::*;

use crate::subtypes::{is_subtype, SubtypeContext};
use crate::typeinfo::{NativeTypeResolver, TypeResolver};
use crate::visitor::flatten_nested_unions_inner;
use crate::wire::{self, LiteralValue, ReadBuffer, Type};

const MAX_UNION_ITEMS: usize = 10;

/// `mypy/messages.py:UNSUPPORTED_NUMBERS_TYPES` (the "numeric tower").
const UNSUPPORTED_NUMBERS_TYPES: &[&str] = &[
    "numbers.Number",
    "numbers.Complex",
    "numbers.Real",
    "numbers.Rational",
    "numbers.Integral",
];

/// Types from `typing` that are always formatted with their full module
/// prefix in `find_type_overlaps` (mypy/messages.py:110-123).
const TYPES_FOR_UNIMPORTED_HINTS: &[&str] = &[
    "typing.Any",
    "typing.Callable",
    "typing.Dict",
    "typing.Iterable",
    "typing.Iterator",
    "typing.List",
    "typing.Optional",
    "typing.Set",
    "typing.Tuple",
    "typing.TypeVar",
    "typing.Union",
    "typing.cast",
];

/// Quote a type string for use in error messages (messages.py:2606).
#[pyfunction]
pub fn rust_quote_type_string(type_string: String) -> String {
    quote_type_string(&type_string)
}

/// Capitalize the first character (messages.py:3285).
#[pyfunction]
pub fn rust_capitalize(s: String) -> String {
    capitalize(&s)
}

/// Format a sequence with quotes and a conjunction (messages.py:3399).
#[pyfunction]
pub fn rust_pretty_seq(args: Vec<String>, conjunction: String) -> String {
    pretty_seq(&args, &conjunction)
}

/// Format a list of strings for error messages (messages.py:3309).
#[pyfunction]
pub fn rust_format_string_list(lst: Vec<String>) -> Option<String> {
    if lst.is_empty() {
        return None;
    }
    Some(format_string_list(&lst))
}

/// Format item names as a quoted list (messages.py:3323).
#[pyfunction]
pub fn rust_format_item_name_list(items: Vec<String>) -> String {
    format_item_name_list(&items)
}

/// Generate a wrong-type-arg-count message (messages.py:3345).
#[pyfunction]
pub fn rust_wrong_type_arg_count(low: i64, high: i64, act: String, name: String) -> String {
    wrong_type_arg_count(low, high, &act, &name)
}

/// Strip leading/trailing double quotes (messages.py:3302).
#[pyfunction]
pub fn rust_strip_quotes(s: String) -> String {
    strip_quotes(&s)
}

/// Extract the type portion from a method name (messages.py:3293).
#[pyfunction]
pub fn rust_extract_type(name: String) -> String {
    extract_type(&name)
}

/// Return the variance string for a variance int (messages.py:3174).
#[pyfunction]
pub fn rust_variance_string(variance: i64) -> String {
    variance_string(variance)
}

/// Format a list of keys for TypedDict error messages.
#[pyfunction]
pub fn rust_format_key_list(keys: Vec<String>, short: bool) -> String {
    format_key_list(&keys, short)
}

/// Format a type to its bare string (unquoted), using the native resolver.
///
/// Mirrors `format_type_bare(typ, options, verbosity, module_names)`.
/// Returns `None` if the type contains a variant the Rust path does not
/// handle, so the Python caller falls back.
#[pyfunction]
pub fn rust_format_type_bare(
    py: Python<'_>,
    bytes: &[u8],
    resolver: &mut NativeTypeResolver,
    verbosity: i64,
    module_names: bool,
    use_star_unpack: bool,
) -> Option<String> {
    let typ = wire::read_type(&mut ReadBuffer::new(bytes), None).ok()?;
    let fullnames = find_type_overlaps(&typ, resolver);
    format_type_inner(
        py,
        &typ,
        verbosity,
        module_names,
        &fullnames,
        resolver,
        true,
        use_star_unpack,
    )
}

/// Format a type to its quoted string, using the native resolver.
///
/// Mirrors `format_type(typ, options, verbosity, module_names)`.
#[pyfunction]
pub fn rust_format_type(
    py: Python<'_>,
    bytes: &[u8],
    resolver: &mut NativeTypeResolver,
    verbosity: i64,
    module_names: bool,
    use_star_unpack: bool,
) -> Option<String> {
    let bare = rust_format_type_bare(
        py,
        bytes,
        resolver,
        verbosity,
        module_names,
        use_star_unpack,
    )?;
    Some(quote_type_string(&bare))
}

/// Jointly format types to distinct strings (messages.py:2987).
///
/// Mirrors `format_type_distinctly(*types, options, bare)`.
/// Takes serialized type bytes for each type plus the resolver.
/// Returns `None` if any type cannot be formatted.
#[pyfunction]
pub fn rust_format_type_distinctly(
    py: Python<'_>,
    type_bytes_list: Vec<Vec<u8>>,
    resolver: &mut NativeTypeResolver,
    bare: bool,
    use_star_unpack: bool,
) -> Option<Vec<String>> {
    let mut types = Vec::with_capacity(type_bytes_list.len());
    for bytes in &type_bytes_list {
        types.push(wire::read_type(&mut ReadBuffer::new(bytes), None).ok()?);
    }

    // Collect fullnames from all types for overlap detection.
    let mut overlapping = HashSet::new();
    for t in &types {
        for fullname in find_type_overlaps_single(t, resolver) {
            overlapping.insert(fullname);
        }
    }
    // Also check cross-type overlaps.
    let mut all_short_names: std::collections::HashMap<String, HashSet<String>> =
        std::collections::HashMap::new();
    for t in &types {
        collect_named_types_for_overlap(t, &mut all_short_names);
    }
    for fullnames in all_short_names.values() {
        if fullnames.len() > 1 {
            for f in fullnames {
                overlapping.insert(f.clone());
            }
        }
    }

    // messages.py:3067-3083: min_verbosity bump when both are CallableType,
    // right has named args, and is_subtype(left, right, ignore_pos_arg_names).
    // We can't call is_subtype here, so defer to Python for this case.
    if types.len() == 2 {
        let left = &types[0];
        let right = &types[1];
        if matches!(left, Type::CallableType { .. }) && matches!(right, Type::CallableType { .. }) {
            if let Type::CallableType { arg_names, .. } = right {
                if arg_names.iter().any(|n| n.is_some()) {
                    return None;
                }
            }
        }
    }

    let min_verbosity = 0;

    let mut strs = Vec::with_capacity(types.len());
    for verbosity in min_verbosity..2 {
        strs.clear();
        let mut all_ok = true;
        for t in &types {
            match format_type_inner(
                py,
                t,
                verbosity,
                false,
                &overlapping,
                resolver,
                true,
                use_star_unpack,
            ) {
                Some(s) => strs.push(s),
                None => {
                    all_ok = false;
                    break;
                }
            }
        }
        if !all_ok {
            return None;
        }
        // Check if all strings are distinct.
        let unique: HashSet<&str> = strs.iter().map(|s| s.as_str()).collect();
        if unique.len() == strs.len() {
            break;
        }
    }

    if bare {
        Some(strs)
    } else {
        Some(strs.into_iter().map(|s| quote_type_string(&s)).collect())
    }
}

// ---------------------------------------------------------------------------
// Pure formatting helpers (no PyO3, no Python<'_>)
// ---------------------------------------------------------------------------

fn quote_type_string(type_string: &str) -> String {
    if type_string == "Module"
        || type_string == "overloaded function"
        || type_string == "<deleted>"
        || type_string.starts_with("Module ")
        || type_string.ends_with('?')
    {
        type_string.to_string()
    } else {
        format!("\"{type_string}\"")
    }
}

fn capitalize(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    first.to_uppercase().collect::<String>() + chars.as_str()
}

#[allow(dead_code)]
fn pretty_seq(args: &[String], conjunction: &str) -> String {
    let quoted: Vec<String> = args.iter().map(|a| format!("\"{a}\"")).collect();
    if quoted.len() == 1 {
        return quoted[0].clone();
    }
    if quoted.len() == 2 {
        return format!("{} {conjunction} {}", quoted[0], quoted[1]);
    }
    let last_sep = format!(", {conjunction} ");
    let head = quoted[..quoted.len() - 1].join(", ");
    format!("{head}{last_sep}{}", quoted.last().unwrap())
}

fn format_string_list(lst: &[String]) -> String {
    if lst.len() == 1 {
        return lst[0].clone();
    }
    if lst.len() <= 5 {
        let head = lst[..lst.len() - 1].join(", ");
        return format!("{head} and {}", lst[lst.len() - 1]);
    }
    let head = lst[..2].join(", ");
    let suppressed = lst.len() - 3;
    format!(
        "{head}, ... and {} ({} methods suppressed)",
        lst.last().unwrap(),
        suppressed
    )
}

fn format_item_name_list(items: &[String]) -> String {
    if items.len() <= 5 {
        let quoted: Vec<String> = items.iter().map(|n| format!("\"{n}\"")).collect();
        format!("({})", quoted.join(", "))
    } else {
        let quoted: Vec<String> = items[..5].iter().map(|n| format!("\"{n}\"")).collect();
        format!("({}, ...)", quoted.join(", "))
    }
}

fn wrong_type_arg_count(low: i64, high: i64, act: &str, name: &str) -> String {
    let s = if low == high {
        if low == 0 {
            "no type arguments".to_string()
        } else if low == 1 {
            "1 type argument".to_string()
        } else {
            format!("{low} type arguments")
        }
    } else {
        format!("between {low} and {high} type arguments")
    };
    let act_str = if act == "0" { "none" } else { act };
    format!("\"{name}\" expects {s}, but {act_str} given")
}

fn strip_quotes(s: &str) -> String {
    let s = s.strip_prefix('"').unwrap_or(s);
    s.strip_suffix('"').unwrap_or(s).to_string()
}

fn extract_type(name: &str) -> String {
    // messages.py:3298: re.sub('^"[a-zA-Z0-9_]+" of ', "", name)
    // Strips a leading `"identifier" of ` prefix.
    // The regex requires the quoted identifier to use only [a-zA-Z0-9_].
    if let Some(rest) = name.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let ident = &rest[..end];
            if ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                let after_quote = &rest[end + 1..];
                if let Some(stripped) = after_quote.strip_prefix(" of ") {
                    return stripped.to_string();
                }
            }
        }
    }
    name.to_string()
}

fn variance_string(variance: i64) -> String {
    match variance {
        1 => "covariant".to_string(),
        2 => "contravariant".to_string(),
        _ => "invariant".to_string(),
    }
}

/// Approximate Python's `repr(str)` for plain identifiers, which is
/// single-quoted (`'x'`). mypy's TypedDict keys and callable arg names
/// are identifiers, so backslashes/quotes are not expected; we still
/// escape single quotes to stay faithful.
fn py_str_repr(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('\'');
    out
}

fn format_key_list(keys: &[String], short: bool) -> String {
    let formatted_keys: Vec<String> = keys.iter().map(|k| format!("\"{k}\"")).collect();
    let td = if short { "" } else { "TypedDict " };
    if keys.is_empty() {
        format!("no {td}keys")
    } else if keys.len() == 1 {
        format!("{td}key {}", formatted_keys[0])
    } else {
        format!("{td}keys ({})", formatted_keys.join(", "))
    }
}

// ---------------------------------------------------------------------------
// format_type_inner — the core type formatter (messages.py:2649)
// ---------------------------------------------------------------------------

/// Format a `Type` to its bare string using `format_type_inner` rules.
///
/// Returns `None` for any type variant the Rust path does not handle,
/// so the Python caller falls back to the pure-Python formatter.
#[allow(clippy::too_many_arguments)]
fn format_type_inner(
    py: Python<'_>,
    typ: &Type,
    verbosity: i64,
    module_names: bool,
    fullnames: &HashSet<String>,
    resolver: &NativeTypeResolver,
    use_pretty_callable: bool,
    use_star_unpack: bool,
) -> Option<String> {
    // TypeAliasType recursive case (messages.py:2698-2708).
    if let Type::TypeAliasType { type_ref, args } = typ {
        // The wire format carries type_ref but no resolved alias node.
        // messages.py checks `typ.is_recursive` and `typ.alias`.
        // Without the resolved alias, we can't determine is_recursive
        // or alias.name. Return None to defer to Python.
        let _ = (type_ref, args, py);
        return None;
    }

    // get_proper_type: unwrap TypeAliasType (already handled above).
    // The wire format stores ProperType directly, so typ is already proper.

    match typ {
        Type::Instance {
            type_ref,
            args,
            last_known_value: _,
            extra_attrs,
        } => {
            let snap = resolver.resolver().get(type_ref);

            // types.ModuleType special case (messages.py:2716).
            if type_ref == "types.ModuleType" {
                if let Some(ea) = extra_attrs {
                    if let Some(mod_name) = &ea.mod_name {
                        if module_names {
                            return Some(format!("Module \"{mod_name}\""));
                        }
                    }
                }
                return Some("Module".to_string());
            }

            // typing._SpecialForm (messages.py:2722).
            if type_ref == "typing._SpecialForm" {
                return Some("<typing special form>".to_string());
            }

            // Determine the base name. messages.py:2725: `verbosity >= 2`
            // or overlap -> fullname; else `itype.type.name` (short).
            let base_str = if verbosity >= 2 || fullnames.contains(type_ref) {
                type_ref.clone()
            } else {
                let s = snap?;
                // No resolver entry: the TypeInfo was likely created at
                // runtime (e.g. intersect_instance_callable's fake type)
                // after the resolver snapshot was built. Defer to Python
                // so it can access itype.type.name directly.
                s.name.clone()
            };

            if args.is_empty() {
                // has_type_var_tuple_type && len(type_vars) == 1 -> [()]
                if let Some(s) = snap {
                    if s.has_type_var_tuple_type && s.type_vars.len() == 1 {
                        return Some(format!("{base_str}[()]"));
                    }
                }
                return Some(base_str);
            }

            // builtins.tuple special case (messages.py:2734).
            if type_ref == "builtins.tuple" {
                let item_str = format_type_inner(
                    py,
                    &args[0],
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    true,
                    use_star_unpack,
                )?;
                return Some(format!("tuple[{item_str}, ...]"));
            }

            // Generic instance with args.
            let formatted_args = format_list(
                py,
                args,
                verbosity,
                module_names,
                fullnames,
                resolver,
                use_star_unpack,
            )?;
            Some(format!("{base_str}[{formatted_args}]"))
        }

        Type::UnpackType { typ } => {
            // messages.py:2740: options.use_star_unpack()
            let inner = format_type_inner(
                py,
                typ,
                verbosity,
                module_names,
                fullnames,
                resolver,
                true,
                use_star_unpack,
            )?;
            if use_star_unpack {
                Some(format!("*{inner}"))
            } else {
                Some(format!("Unpack[{inner}]"))
            }
        }

        Type::TypeVarType {
            name, namespace, ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            if verbosity >= 2 || fullnames.contains(&fullname) {
                Some(fullname)
            } else {
                Some(name.clone())
            }
        }

        Type::TypeVarTupleType {
            name, namespace, ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            if verbosity >= 2 || fullnames.contains(&fullname) {
                Some(fullname)
            } else {
                Some(name.clone())
            }
        }

        Type::ParamSpecType {
            prefix,
            name,
            namespace,
            flavor,
            ..
        } => {
            // messages.py:2755: name_with_suffix() appends .args/.kwargs
            // based on flavor (0=bare, 1=args, 2=kwargs).
            let suffixed = match flavor {
                1 => format!("{name}.args"),
                2 => format!("{name}.kwargs"),
                _ => name.clone(),
            };
            // scoped_type_var_name uses t.name (bare, without suffix).
            let fullname = scoped_type_var_name(name, namespace);
            let display_name = if verbosity >= 2 || fullnames.contains(&fullname) {
                fullname
            } else {
                suffixed
            };
            if !prefix.arg_types.is_empty() {
                let args = format_callable_args(
                    py,
                    &prefix.arg_types,
                    &prefix.arg_kinds,
                    &prefix.arg_names,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    use_star_unpack,
                )?;
                Some(format!("[{args}, **{display_name}]"))
            } else {
                Some(display_name)
            }
        }

        Type::TupleType {
            partial_fallback,
            items,
            ..
        } => {
            // messages.py:2767: if fallback != builtins.tuple, use fallback.
            let is_tuple = match partial_fallback.as_ref() {
                Type::Instance { type_ref, .. } => type_ref == "builtins.tuple",
                _ => false,
            };
            if !is_tuple {
                return format_type_inner(
                    py,
                    partial_fallback,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    true,
                    use_star_unpack,
                );
            }
            let type_items = format_list(
                py,
                items,
                verbosity,
                module_names,
                fullnames,
                resolver,
                use_star_unpack,
            )?;
            let items_str = if type_items.is_empty() {
                "()".to_string()
            } else {
                type_items
            };
            Some(format!("tuple[{items_str}]"))
        }

        Type::TypedDictType {
            items,
            required_keys,
            readonly_keys,
            fallback,
            ..
        } => {
            // messages.py:2773: if not anonymous, return format(fallback).
            // A TypedDict is anonymous if its fallback fullname is in
            // TPDICT_FB_NAMES (mypy/types.py:126-130).
            let is_anonymous = match fallback.as_ref() {
                Type::Instance { type_ref, .. } => {
                    type_ref == "typing._TypedDict"
                        || type_ref == "typing_extensions._TypedDict"
                        || type_ref == "mypy_extensions._TypedDict"
                }
                _ => true,
            };
            if !is_anonymous {
                return format_type_inner(
                    py,
                    fallback,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    true,
                    use_star_unpack,
                );
            }
            let mut parts = Vec::with_capacity(items.len());
            for (name, item_type) in items {
                let mut modifier = String::new();
                if !required_keys.contains(name) {
                    modifier.push('?');
                }
                if readonly_keys.contains(name) {
                    modifier.push('=');
                }
                let type_str = format_type_inner(
                    py,
                    item_type,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    true,
                    use_star_unpack,
                )?;
                parts.push(format!("{}{modifier}: {type_str}", py_str_repr(name)));
            }
            Some(format!("TypedDict({{{}}})", parts.join(", ")))
        }

        Type::LiteralType { fallback, value } => {
            let val_str = format_literal_value(
                py,
                fallback,
                value,
                verbosity,
                module_names,
                fullnames,
                resolver,
                use_star_unpack,
            )?;
            Some(format!("Literal[{val_str}]"))
        }

        Type::UnionType { items, .. } => format_union_type(
            py,
            items,
            verbosity,
            module_names,
            fullnames,
            resolver,
            use_star_unpack,
        ),

        Type::NoneType => Some("None".to_string()),

        Type::AnyType { .. } => Some("Any".to_string()),

        Type::DeletedType { .. } => Some("<deleted>".to_string()),

        Type::UninhabitedType { .. } => Some("Never".to_string()),

        Type::TypeType { item, is_type_form } => {
            let inner = format_type_inner(
                py,
                item,
                verbosity,
                module_names,
                fullnames,
                resolver,
                true,
                use_star_unpack,
            )?;
            let type_name = if *is_type_form { "TypeForm" } else { "type" };
            Some(format!("{type_name}[{inner}]"))
        }

        Type::CallableType {
            arg_types,
            arg_kinds,
            arg_names,
            ret_type,
            is_ellipsis_args,
            fallback,
            name,
            variables,
            type_guard,
            type_is,
            instance_type,
            ..
        } => {
            // messages.py:2833: FunctionLike dispatch.
            //
            // is_type_obj(): `fallback.type.is_metaclass() and not Uninhabited ret`.
            // is_metaclass: has_base("builtins.type") || fullname == "abc.ABCMeta"
            // || (fallback_to_any and not precise).
            let is_type_obj = match fallback.as_ref() {
                Type::Instance { type_ref, .. } => {
                    let snap = resolver.resolver().get(type_ref);
                    let is_meta = snap
                        .map(|s| {
                            s.has_base("builtins.type")
                                || type_ref == "abc.ABCMeta"
                                || s.fallback_to_any
                        })
                        .unwrap_or(false);
                    let ret_uninhabited = matches!(ret_type.as_ref(), Type::UninhabitedType { .. });
                    is_meta && !ret_uninhabited
                }
                _ => false,
            };
            if is_type_obj {
                // format(TypeType.make_normalized(func.items[0].get_instance_type())).
                // instance_type carries the normalized self-type; defer to
                // Python if absent (we can't reconstruct it reliably).
                if let Some(it) = instance_type {
                    return Some(format!(
                        "type[{}]",
                        format_type_inner(
                            py,
                            it,
                            verbosity,
                            module_names,
                            fullnames,
                            resolver,
                            true,
                            use_star_unpack,
                        )?
                    ));
                }
                return None;
            }

            // Return type with TypeGuard/TypeIs wrapping.
            let return_type = if let Some(tg) = type_guard {
                let tg_str = format_type_inner(
                    py,
                    tg,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    true,
                    use_star_unpack,
                )?;
                format!("TypeGuard[{tg_str}]")
            } else if let Some(ti) = type_is {
                let ti_str = format_type_inner(
                    py,
                    ti,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    true,
                    use_star_unpack,
                )?;
                format!("TypeIs[{ti_str}]")
            } else {
                format_type_inner(
                    py,
                    ret_type,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    true,
                    use_star_unpack,
                )?
            };

            if *is_ellipsis_args {
                return Some(format!("Callable[..., {return_type}]"));
            }

            // ParamSpec detection (messages.py:2846): the two final args
            // must be *args: P.args, **kwargs: P.kwargs.
            let has_param_spec = arg_types.len() >= 2
                && arg_kinds[arg_kinds.len() - 2] == 2
                && arg_kinds[arg_kinds.len() - 1] == 4
                && matches!(arg_types[arg_types.len() - 2], Type::ParamSpecType { .. });
            if has_param_spec {
                // Callable[P, ret]: needs ParamSpec name + prefix. Defer to
                // Python (mypy uses copy_modified with a prefix).
                let _ = (name, variables);
                return None;
            }

            // Use pretty_callable for complex signatures (messages.py:2852).
            // pretty_callable needs FuncDef/definition data not present in
            // the wire format, and renders named/optional/star args with a
            // `def (name: T, ...) -> R` shape. Defer to Python.
            if use_pretty_callable {
                let needs_pretty = arg_kinds
                    .iter()
                    .zip(arg_names.iter())
                    .any(|(k, n)| !should_format_arg_as_type(*k, n.as_deref(), verbosity));
                if needs_pretty {
                    let _ = (name, variables);
                    return None;
                }
            }

            let args = format_callable_args(
                py,
                arg_types,
                arg_kinds,
                arg_names,
                verbosity,
                module_names,
                fullnames,
                resolver,
                use_star_unpack,
            )?;
            Some(format!("Callable[[{args}], {return_type}]"))
        }

        Type::Overloaded { items } => {
            // messages.py:2864: FunctionLike dispatch.
            // is_type_obj() on Overloaded delegates to items[0].
            if let Some(Type::CallableType {
                fallback,
                ret_type,
                instance_type,
                ..
            }) = items.first()
            {
                let is_type_obj = match fallback.as_ref() {
                    Type::Instance { type_ref, .. } => {
                        let snap = resolver.resolver().get(type_ref);
                        let is_meta = snap
                            .map(|s| {
                                s.has_base("builtins.type")
                                    || type_ref == "abc.ABCMeta"
                                    || s.fallback_to_any
                            })
                            .unwrap_or(false);
                        let ret_uninhabited =
                            matches!(ret_type.as_ref(), Type::UninhabitedType { .. });
                        is_meta && !ret_uninhabited
                    }
                    _ => false,
                };
                if is_type_obj {
                    let it = instance_type.as_ref()?;
                    return Some(format!(
                        "type[{}]",
                        format_type_inner(
                            py,
                            it,
                            verbosity,
                            module_names,
                            fullnames,
                            resolver,
                            true,
                            use_star_unpack,
                        )?
                    ));
                }
            }
            // messages.py:2867: "overloaded function"
            Some("overloaded function".to_string())
        }

        Type::UnboundType { .. } => {
            // messages.py:2869: typ.accept(TypeStrVisitor)
            // The wire Display impl handles UnboundType, but format_type_inner
            // delegates to TypeStrVisitor which renders differently from
            // format_type_inner for some cases. For unbound types, the
            // rendering is the same (name + "?"). Defer to Python to be safe.
            None
        }

        Type::Parameters(params) => {
            let args = format_callable_args(
                py,
                &params.arg_types,
                &params.arg_kinds,
                &params.arg_names,
                verbosity,
                module_names,
                fullnames,
                resolver,
                use_star_unpack,
            )?;
            Some(format!("[{args}]"))
        }

        Type::TypeAliasType { .. } => {
            // Already handled above; this is unreachable but the match
            // needs to be exhaustive.
            None
        }
    }
}

/// Format a list of types joined by ", " (messages.py:2672).
fn format_list(
    py: Python<'_>,
    types: &[Type],
    verbosity: i64,
    module_names: bool,
    fullnames: &HashSet<String>,
    resolver: &NativeTypeResolver,
    use_star_unpack: bool,
) -> Option<String> {
    let mut parts = Vec::with_capacity(types.len());
    for t in types {
        parts.push(format_type_inner(
            py,
            t,
            verbosity,
            module_names,
            fullnames,
            resolver,
            true,
            use_star_unpack,
        )?);
    }
    Some(parts.join(", "))
}

/// Format union items with coalescing logic (messages.py:2675-2689, 2788-2818).
fn format_union_type(
    py: Python<'_>,
    items: &[Type],
    verbosity: i64,
    module_names: bool,
    fullnames: &HashSet<String>,
    resolver: &NativeTypeResolver,
    use_star_unpack: bool,
) -> Option<String> {
    // Separate literal items from union items (messages.py:2792).
    let mut literal_items: Vec<&Type> = Vec::new();
    let mut union_items: Vec<&Type> = Vec::new();
    for item in items {
        if matches!(item, Type::LiteralType { .. }) {
            literal_items.push(item);
        } else {
            union_items.push(item);
        }
    }

    // Coalesce multiple Literal[] members (messages.py:2796-2806).
    if literal_items.len() > 1 {
        let literal_strs: Vec<String> = literal_items
            .iter()
            .map(|t| match t {
                Type::LiteralType { fallback, value } => format_literal_value(
                    py,
                    fallback,
                    value,
                    verbosity,
                    module_names,
                    fullnames,
                    resolver,
                    use_star_unpack,
                ),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let literal_str = format!("Literal[{}]", literal_strs.join(", "));

        // Check if union_items is just [None].
        if union_items.len() == 1 && matches!(union_items[0], Type::NoneType) {
            return Some(format!("{literal_str} | None"));
        } else if !union_items.is_empty() {
            let union_str = format_union_items(
                py,
                &union_items,
                verbosity,
                module_names,
                fullnames,
                resolver,
                use_star_unpack,
            )?;
            return Some(format!("{literal_str} | {union_str}"));
        } else {
            return Some(literal_str);
        }
    }

    // When literal_items.len() <= 1, Python uses typ.items (the original
    // full list including literals) for print_as_optional and format_union.
    // messages.py:2838-2849.
    let all_items: Vec<&Type> = items.iter().collect();

    let none_count = all_items
        .iter()
        .filter(|t| matches!(t, Type::NoneType))
        .count();
    let non_none_count = all_items.len() - none_count;
    let print_as_optional = non_none_count == 1;

    if print_as_optional {
        let rest: Vec<&Type> = all_items
            .iter()
            .filter(|t| !matches!(t, Type::NoneType))
            .copied()
            .collect();
        let inner = format_type_inner(
            py,
            rest[0],
            verbosity,
            module_names,
            fullnames,
            resolver,
            true,
            use_star_unpack,
        )?;
        return Some(format!("{inner} | None"));
    }

    let s = format_union_items(
        py,
        &all_items,
        verbosity,
        module_names,
        fullnames,
        resolver,
        use_star_unpack,
    )?;
    Some(s)
}

/// Format union items with the `|` separator, handling MAX_UNION_ITEMS
/// truncation (messages.py:2675-2689).
fn format_union_items(
    py: Python<'_>,
    types: &[&Type],
    verbosity: i64,
    module_names: bool,
    fullnames: &HashSet<String>,
    resolver: &NativeTypeResolver,
    use_star_unpack: bool,
) -> Option<String> {
    let mut formatted: Vec<String> = Vec::new();
    let mut has_none = false;
    for t in types {
        if matches!(t, Type::NoneType) {
            has_none = true;
            continue;
        }
        let s = format_type_inner(
            py,
            t,
            verbosity,
            module_names,
            fullnames,
            resolver,
            true,
            use_star_unpack,
        )?;
        // Deduplicate identical formatted items. Python's get_proper_type
        // simplifies unions like Union[str, str] -> str before formatting.
        // The wire format doesn't run get_proper_type, so we dedup here.
        if !formatted.contains(&s) {
            formatted.push(s);
        }
    }

    let mut more = 0;
    if formatted.len() > MAX_UNION_ITEMS && verbosity == 0 {
        more = formatted.len() - MAX_UNION_ITEMS / 2;
        formatted.truncate(MAX_UNION_ITEMS / 2);
    }

    if more > 0 {
        formatted.push(format!("<{more} more items>"));
    }

    if has_none {
        formatted.push("None".to_string());
    }

    Some(formatted.join(" | "))
}

/// Format a literal value for display (messages.py:2691-2696).
#[allow(clippy::too_many_arguments)]
fn format_literal_value(
    py: Python<'_>,
    fallback: &Type,
    value: &LiteralValue,
    verbosity: i64,
    module_names: bool,
    fullnames: &HashSet<String>,
    resolver: &NativeTypeResolver,
    use_star_unpack: bool,
) -> Option<String> {
    // Check if enum literal.
    let fallback_ref = match fallback {
        Type::Instance { type_ref, .. } => Some(type_ref.as_str()),
        _ => None,
    };
    let snap = fallback_ref.and_then(|r| resolver.resolver().get(r));

    // If the fallback is not in the resolver (e.g. a nested enum like
    // `Wrapper.Color` missed by _collect_type_infos), we can't tell whether
    // this is an enum literal, so defer to Python.
    let s = snap?;
    if s.is_enum {
        let value_name = match value {
            LiteralValue::Str(s) => s.clone(),
            _ => value.to_string(),
        };
        // messages.py:2693: f"{underlying_type}.{typ.value}"
        // underlying_type = format(typ.fallback), which applies the
        // same format_type_inner rules as the enclosing type.
        let underlying = format_type_inner(
            py,
            fallback,
            verbosity,
            module_names,
            fullnames,
            resolver,
            true,
            use_star_unpack,
        )?;
        return Some(format!("{underlying}.{value_name}"));
    }

    // Non-enum: use value_repr().
    if fallback_ref == Some("builtins.bytes") {
        // bytes literal: b + repr(value)
        let raw = value.to_string();
        return Some(format!("b{raw}"));
    }

    // Default: repr(value) via Display.
    Some(value.to_string())
}

/// Determine whether a function argument should be formatted as its Type
/// or with name (messages.py:2618).
///
/// ARG_POS = 0, ARG_OPT = 1, ARG_STAR = 2, ARG_STAR2 = 4,
/// ARG_NAMED = 3, ARG_NAMED_OPT = 5.
fn should_format_arg_as_type(arg_kind: i64, arg_name: Option<&str>, verbosity: i64) -> bool {
    // ArgKind.is_positional(star=False): ARG_POS (0) or ARG_OPT (1),
    // NOT ARG_STAR (2). Matches mypy/nodes.py:2494-2496.
    let is_positional = matches!(arg_kind, 0 | 1);
    (arg_kind == 0 && arg_name.is_none()) || (verbosity == 0 && is_positional)
}

/// Format callable arguments (messages.py:2627).
#[allow(clippy::too_many_arguments)]
fn format_callable_args(
    py: Python<'_>,
    arg_types: &[Type],
    arg_kinds: &[i64],
    arg_names: &[Option<String>],
    verbosity: i64,
    module_names: bool,
    fullnames: &HashSet<String>,
    resolver: &NativeTypeResolver,
    use_star_unpack: bool,
) -> Option<String> {
    let mut arg_strings = Vec::with_capacity(arg_types.len());
    for i in 0..arg_types.len() {
        let arg_name = arg_names.get(i).and_then(|n| n.as_deref());
        let arg_kind = arg_kinds[i];

        if should_format_arg_as_type(arg_kind, arg_name, verbosity) {
            let s = format_type_inner(
                py,
                &arg_types[i],
                verbosity,
                module_names,
                fullnames,
                resolver,
                true,
                use_star_unpack,
            )?;
            arg_strings.push(s);
        } else {
            let constructor = arg_constructor_name(arg_kind);
            let type_str = format_type_inner(
                py,
                &arg_types[i],
                verbosity,
                module_names,
                fullnames,
                resolver,
                true,
                use_star_unpack,
            )?;
            // Python only prints the name when present; starred/None
            // names print without it.
            match (arg_name, is_star(arg_kind)) {
                (Some(name), false) => {
                    arg_strings.push(format!("{constructor}({type_str}, {})", py_str_repr(name)))
                }
                _ => arg_strings.push(format!("{constructor}({type_str})")),
            }
        }
    }
    Some(arg_strings.join(", "))
}

/// ARG_CONSTRUCTOR_NAMES (messages.py:126-133).
fn arg_constructor_name(arg_kind: i64) -> &'static str {
    match arg_kind {
        0 => "Arg",
        1 => "DefaultArg",
        3 => "NamedArg",
        5 => "DefaultNamedArg",
        2 => "VarArg",
        4 => "KwArg",
        _ => "Arg",
    }
}

fn is_star(arg_kind: i64) -> bool {
    matches!(arg_kind, 2 | 4)
}

/// Scoped type variable name (messages.py:2919).
fn scoped_type_var_name(name: &str, namespace: &str) -> String {
    if namespace.is_empty() {
        return name.to_string();
    }
    let suffix = namespace.rsplit('.').next().unwrap_or(namespace);
    format!("{name}@{suffix}")
}

// ---------------------------------------------------------------------------
// find_type_overlaps (messages.py:2927)
// ---------------------------------------------------------------------------

/// Find types that share a short name (messages.py:2927).
/// Returns a set of fullnames that should be printed in full.
fn find_type_overlaps(typ: &Type, _resolver: &NativeTypeResolver) -> HashSet<String> {
    let mut d: std::collections::HashMap<String, HashSet<String>> =
        std::collections::HashMap::new();
    collect_named_types(typ, &mut d);

    // Add typing.X for short names in TYPES_FOR_UNIMPORTED_HINTS.
    let typing_additions: Vec<(String, String)> = d
        .keys()
        .filter_map(|shortname| {
            if TYPES_FOR_UNIMPORTED_HINTS.contains(&format!("typing.{shortname}").as_str()) {
                Some((shortname.clone(), format!("typing.{shortname}")))
            } else {
                None
            }
        })
        .collect();
    for (shortname, fullname) in typing_additions {
        d.get_mut(&shortname).unwrap().insert(fullname);
    }

    let mut overlaps = HashSet::new();
    for fullnames in d.values() {
        if fullnames.len() > 1 {
            for f in fullnames {
                overlaps.insert(f.clone());
            }
        }
    }
    overlaps
}

/// Single-type version for the initial call in format_type_bare.
fn find_type_overlaps_single(typ: &Type, resolver: &NativeTypeResolver) -> HashSet<String> {
    find_type_overlaps(typ, resolver)
}

/// Collect all named types from a type tree (messages.py:2880, 2927).
fn collect_named_types(t: &Type, d: &mut std::collections::HashMap<String, HashSet<String>>) {
    match t {
        Type::Instance { type_ref, args, .. } => {
            let short_name = type_ref.rsplit('.').next().unwrap_or(type_ref);
            d.entry(short_name.to_string())
                .or_default()
                .insert(type_ref.clone());
            for a in args {
                collect_named_types(a, d);
            }
        }
        Type::TypeAliasType { type_ref, args, .. } => {
            let short_name = type_ref.rsplit('.').next().unwrap_or(type_ref);
            d.entry(short_name.to_string())
                .or_default()
                .insert(type_ref.clone());
            for a in args {
                collect_named_types(a, d);
            }
        }
        Type::TypeVarType {
            name,
            namespace,
            upper_bound,
            values,
            default,
            ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            d.entry(name.clone()).or_default().insert(fullname);
            collect_named_types(upper_bound, d);
            for v in values {
                collect_named_types(v, d);
            }
            collect_named_types(default, d);
        }
        Type::TypeVarTupleType {
            name,
            namespace,
            upper_bound,
            default,
            ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            d.entry(name.clone()).or_default().insert(fullname);
            collect_named_types(upper_bound, d);
            collect_named_types(default, d);
        }
        Type::ParamSpecType {
            name,
            namespace,
            prefix,
            upper_bound,
            default,
            ..
        } => {
            let fullname = scoped_type_var_name(name, namespace);
            d.entry(name.clone()).or_default().insert(fullname);
            for a in &prefix.arg_types {
                collect_named_types(a, d);
            }
            collect_named_types(upper_bound, d);
            collect_named_types(default, d);
        }
        Type::CallableType {
            arg_types,
            ret_type,
            variables,
            type_guard,
            type_is,
            ..
        } => {
            for a in arg_types {
                collect_named_types(a, d);
            }
            collect_named_types(ret_type, d);
            for v in variables {
                collect_named_types(v, d);
            }
            if let Some(tg) = type_guard {
                collect_named_types(tg, d);
            }
            if let Some(ti) = type_is {
                collect_named_types(ti, d);
            }
        }
        Type::Overloaded { items } => {
            for i in items {
                collect_named_types(i, d);
            }
        }
        Type::TupleType {
            items,
            partial_fallback,
            ..
        } => {
            for i in items {
                collect_named_types(i, d);
            }
            collect_named_types(partial_fallback, d);
        }
        Type::TypedDictType {
            items, fallback, ..
        } => {
            for (_, t) in items {
                collect_named_types(t, d);
            }
            collect_named_types(fallback, d);
        }
        Type::UnionType { items, .. } => {
            for i in items {
                collect_named_types(i, d);
            }
        }
        Type::TypeType { item, .. } => {
            collect_named_types(item, d);
        }
        Type::UnpackType { typ } => {
            collect_named_types(typ, d);
        }
        Type::LiteralType { fallback, .. } => {
            collect_named_types(fallback, d);
        }
        Type::Parameters(p) => {
            for a in &p.arg_types {
                collect_named_types(a, d);
            }
            for v in &p.variables {
                collect_named_types(v, d);
            }
        }
        _ => {}
    }
}

/// Variant of collect_named_types for cross-type overlap detection
/// (used by format_type_distinctly to merge overlaps across types).
fn collect_named_types_for_overlap(
    t: &Type,
    d: &mut std::collections::HashMap<String, HashSet<String>>,
) {
    collect_named_types(t, d);
}

// ---------------------------------------------------------------------------
// Variance/numbers/union notes (messages.py:3520-3583)
// ---------------------------------------------------------------------------

/// `is_same_type` with `ignore_promotions=True` (subtypes.py:302):
/// mutual `is_proper_subtype` in both directions; the fast path is
/// subsumed by mutual subtyping.
fn is_same(left: &Type, right: &Type, resolver: &TypeResolver) -> Option<bool> {
    let ctx = SubtypeContext::new(false, false, false, true, true, true);
    let lr = is_subtype(left, right, &ctx, resolver)?;
    let rl = is_subtype(right, left, &ctx, resolver)?;
    Some(lr && rl)
}

/// Append variance notes for `list`/`dict` invariance (messages.py:3520).
/// The arg type is only read for the subtype checks (Python reads it too).
/// Returns `None` for any type the Rust path does not handle.
fn append_invariance_notes_inner(
    arg: &Type,
    expected: &Type,
    resolver: &TypeResolver,
) -> Option<Vec<String>> {
    let Type::Instance {
        type_ref: arg_ref,
        args: arg_args,
        ..
    } = arg
    else {
        return None;
    };
    let Type::Instance {
        type_ref: exp_ref,
        args: exp_args,
        ..
    } = expected
    else {
        return None;
    };
    if arg_args.is_empty() || exp_args.is_empty() {
        return None;
    }

    let ctx = SubtypeContext::new(false, false, false, false, false, true);
    let (invariant_type, covariant_suggestion) = if arg_ref == "builtins.list"
        && exp_ref == "builtins.list"
        && is_subtype(&arg_args[0], &exp_args[0], &ctx, resolver)?
    {
        (
            "list",
            "Consider using \"Sequence\" instead, which is covariant",
        )
    } else if arg_ref == "builtins.dict"
        && exp_ref == "builtins.dict"
        && is_same(&arg_args[0], &exp_args[0], resolver)?
        && is_subtype(&arg_args[1], &exp_args[1], &ctx, resolver)?
    {
        (
            "dict",
            "Consider using \"Mapping\" instead, which is covariant in the value type",
        )
    } else {
        return Some(Vec::new());
    };

    Some(vec![
        format!(
            "\"{invariant_type}\" is invariant -- see \
             https://mypy.readthedocs.io/en/stable/common_issues.html#variance"
        ),
        covariant_suggestion.to_string(),
    ])
}

/// Append notes for unsupported types from `numbers` (messages.py:3569).
/// The Python arg_type parameter is unused; only expected_type matters.
fn append_numbers_notes_inner(expected: &Type) -> Option<Vec<String>> {
    let Type::Instance { type_ref, .. } = expected else {
        return None;
    };
    if !UNSUPPORTED_NUMBERS_TYPES.contains(&type_ref.as_str()) {
        return Some(Vec::new());
    }
    Some(vec![
        "Types from \"numbers\" are not supported for static type checking".to_string(),
        "See https://peps.python.org/pep-0484/#the-numeric-tower".to_string(),
        "Consider using a protocol instead, such as typing.SupportsFloat".to_string(),
    ])
}

/// Append a note naming union items not in the second union (messages.py:3552).
fn append_union_note_inner(
    py: Python<'_>,
    arg: &Type,
    expected: &Type,
    resolver: &NativeTypeResolver,
    use_star_unpack: bool,
) -> Option<Vec<String>> {
    let Type::UnionType { items, .. } = arg else {
        return None;
    };
    let items = flatten_nested_unions_inner(items, true, true)?;
    if items.len() < MAX_UNION_ITEMS {
        return Some(Vec::new());
    }
    let ctx = SubtypeContext::new(false, false, false, false, false, true);
    let mut non_matching: Vec<&Type> = Vec::new();
    for item in &items {
        let ok = is_subtype(item, expected, &ctx, resolver.resolver())?;
        if !ok {
            non_matching.push(item);
        }
    }
    if non_matching.is_empty() {
        return Some(Vec::new());
    }
    let mut parts = Vec::with_capacity(non_matching.len());
    for t in &non_matching {
        let fullnames = find_type_overlaps(t, resolver);
        let bare = format_type_inner(py, t, 0, false, &fullnames, resolver, true, use_star_unpack)?;
        parts.push(quote_type_string(&bare));
    }
    let plural = if non_matching.len() == 1 { "" } else { "s" };
    Some(vec![format!(
        "Item{plural} in the first union not in the second: {}",
        parts.join(", ")
    )])
}

// ---------------------------------------------------------------------------
// pretty_callable (messages.py:3111)
// ---------------------------------------------------------------------------

fn arg_kind_is_named(kind: i64) -> bool {
    matches!(kind, 3 | 5)
}

fn arg_kind_is_optional(kind: i64) -> bool {
    matches!(kind, 1 | 5)
}

fn arg_kind_is_positional(kind: i64) -> bool {
    matches!(kind, 0 | 1)
}

/// Render a callable without a FuncDef definition (messages.py:3111).
/// Wire format has no definition; name is the first token of `tp.name`,
/// no leading `self`/`cls`. Defers on non-TypeVarType variables.
fn pretty_callable_inner(
    py: Python<'_>,
    tp: &Type,
    resolver: &NativeTypeResolver,
    reveal_verbose_types: bool,
    use_star_unpack: bool,
) -> Option<String> {
    let Type::CallableType {
        arg_types,
        arg_kinds,
        arg_names,
        ret_type,
        name,
        variables,
        type_guard,
        type_is,
        unpack_kwargs,
        ..
    } = tp
    else {
        return None;
    };

    let mut s = String::new();
    let mut asterisk = false;
    let mut slash = false;
    for i in 0..arg_types.len() {
        if !s.is_empty() {
            s.push_str(", ");
        }
        let kind = *arg_kinds.get(i)?;
        if arg_kind_is_named(kind) && !asterisk {
            s.push_str("*, ");
            asterisk = true;
        }
        if kind == 2 {
            s.push('*');
            asterisk = true;
        }
        if kind == 4 {
            s.push_str("**");
        }
        let mut name = arg_names.get(i).and_then(|n| n.as_deref());
        if name.is_none() && !reveal_verbose_types {
            if kind == 2 && matches!(arg_types[i], Type::UnpackType { .. }) {
                name = Some("args");
            } else if kind == 4 && *unpack_kwargs {
                name = Some("kwargs");
            }
        }
        let mut type_str = format_type_inner(
            py,
            &arg_types[i],
            0,
            false,
            &HashSet::new(),
            resolver,
            true,
            use_star_unpack,
        )?;
        if kind == 4 && *unpack_kwargs {
            if reveal_verbose_types {
                type_str = format!("Unpack[{type_str}]");
            } else {
                type_str = format!("**{type_str}");
            }
        }
        if let Some(n) = name {
            s.push_str(n);
            s.push_str(": ");
        }
        s.push_str(&type_str);
        if arg_kind_is_optional(kind) {
            s.push_str(" = ...");
        }
        if !slash
            && arg_kind_is_positional(kind)
            && name.is_none()
            && (i == arg_types.len() - 1
                || arg_names.get(i + 1).and_then(|n| n.as_deref()).is_some()
                || !arg_kind_is_positional(*arg_kinds.get(i + 1)?))
        {
            s.push_str(", /");
            slash = true;
        }
    }

    // No definition on the wire: `get_func_def(tp) is None`, so the function
    // name is the first whitespace token of `tp.name`, never a `self`/`cls`.
    let func_name = name.as_deref().and_then(|n| n.split_whitespace().next());
    if let Some(fname) = func_name {
        s = format!("{fname}({s})");
    } else {
        s = format!("({s})");
    }

    s.push_str(" -> ");
    if let Some(tg) = type_guard {
        let bare = format_type_inner(
            py,
            tg.as_ref(),
            0,
            false,
            &HashSet::new(),
            resolver,
            true,
            use_star_unpack,
        )?;
        s.push_str(&format!("TypeGuard[{bare}]"));
    } else if let Some(ti) = type_is {
        let bare = format_type_inner(
            py,
            ti.as_ref(),
            0,
            false,
            &HashSet::new(),
            resolver,
            true,
            use_star_unpack,
        )?;
        s.push_str(&format!("TypeIs[{bare}]"));
    } else {
        let bare = format_type_inner(
            py,
            ret_type.as_ref(),
            0,
            false,
            &HashSet::new(),
            resolver,
            true,
            use_star_unpack,
        )?;
        s.push_str(&bare);
    }

    if !variables.is_empty() {
        let mut tvars = Vec::with_capacity(variables.len());
        for tvar in variables {
            let Type::TypeVarType {
                name,
                values,
                upper_bound,
                ..
            } = tvar
            else {
                // Python prints `repr(tvar)` for non-TypeVarType variables,
                // which the wire format cannot reconstruct.
                return None;
            };
            let is_object_upper = matches!(
                upper_bound.as_ref(),
                Type::Instance { type_ref, .. } if type_ref == "builtins.object"
            );
            if !is_object_upper {
                let bare = format_type_inner(
                    py,
                    upper_bound.as_ref(),
                    0,
                    false,
                    &HashSet::new(),
                    resolver,
                    true,
                    use_star_unpack,
                )?;
                tvars.push(format!("{name}: {bare}"));
            } else if !values.is_empty() {
                let mut vals = Vec::with_capacity(values.len());
                for v in values {
                    let bare = format_type_inner(
                        py,
                        v,
                        0,
                        false,
                        &HashSet::new(),
                        resolver,
                        true,
                        use_star_unpack,
                    )?;
                    vals.push(bare);
                }
                tvars.push(format!("{name}: ({})", vals.join(", ")));
            } else {
                tvars.push(name.to_string());
            }
        }
        s = format!("[{}] {s}", tvars.join(", "));
    }
    Some(format!("def {s}"))
}

#[pyfunction]
pub fn rust_append_invariance_notes(
    arg_bytes: &[u8],
    expected_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
) -> Option<Vec<String>> {
    let arg = wire::read_type(&mut ReadBuffer::new(arg_bytes), None).ok()?;
    let expected = wire::read_type(&mut ReadBuffer::new(expected_bytes), None).ok()?;
    append_invariance_notes_inner(&arg, &expected, resolver.resolver())
}

#[pyfunction]
pub fn rust_append_numbers_notes(expected_bytes: &[u8]) -> Option<Vec<String>> {
    let expected = wire::read_type(&mut ReadBuffer::new(expected_bytes), None).ok()?;
    append_numbers_notes_inner(&expected)
}

#[pyfunction]
pub fn rust_append_union_note(
    py: Python<'_>,
    arg_bytes: &[u8],
    expected_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
    use_star_unpack: bool,
) -> Option<Vec<String>> {
    let arg = wire::read_type(&mut ReadBuffer::new(arg_bytes), None).ok()?;
    let expected = wire::read_type(&mut ReadBuffer::new(expected_bytes), None).ok()?;
    append_union_note_inner(py, &arg, &expected, resolver, use_star_unpack)
}

#[pyfunction]
pub fn rust_pretty_callable(
    py: Python<'_>,
    callable_bytes: &[u8],
    resolver: &mut NativeTypeResolver,
    reveal_verbose_types: bool,
    use_star_unpack: bool,
) -> Option<String> {
    let tp = wire::read_type(&mut ReadBuffer::new(callable_bytes), None).ok()?;
    pretty_callable_inner(py, &tp, resolver, reveal_verbose_types, use_star_unpack)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_key_list() {
        assert_eq!(format_key_list(&[], false), "no TypedDict keys");
        assert_eq!(
            format_key_list(&["a".to_string()], false),
            "TypedDict key \"a\""
        );
        assert_eq!(
            format_key_list(&["a".to_string(), "b".to_string()], true),
            "keys (\"a\", \"b\")"
        );
    }

    #[test]
    fn test_quote_type_string() {
        assert_eq!(quote_type_string("int"), "\"int\"");
        assert_eq!(quote_type_string("Module"), "Module");
        assert_eq!(
            quote_type_string("overloaded function"),
            "overloaded function"
        );
        assert_eq!(quote_type_string("<deleted>"), "<deleted>");
        assert_eq!(quote_type_string("Module foo"), "Module foo");
        assert_eq!(quote_type_string("Foo?"), "Foo?");
    }

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize("Hello"), "Hello");
    }

    #[test]
    fn test_pretty_seq() {
        assert_eq!(pretty_seq(&["a".to_string()], "or"), "\"a\"");
        assert_eq!(
            pretty_seq(&["a".to_string(), "b".to_string()], "or"),
            "\"a\" or \"b\""
        );
        assert_eq!(
            pretty_seq(&["a".to_string(), "b".to_string(), "c".to_string()], "or"),
            "\"a\", \"b\", or \"c\""
        );
    }

    #[test]
    fn test_format_string_list() {
        assert_eq!(format_string_list(&["a".to_string()]), "a");
        assert_eq!(
            format_string_list(&["a".to_string(), "b".to_string()]),
            "a and b"
        );
        assert_eq!(
            format_string_list(&["a".to_string(), "b".to_string(), "c".to_string()]),
            "a, b and c"
        );
    }

    #[test]
    fn test_wrong_type_arg_count() {
        assert_eq!(
            wrong_type_arg_count(1, 1, "0", "List"),
            "\"List\" expects 1 type argument, but none given"
        );
        assert_eq!(
            wrong_type_arg_count(2, 2, "1", "Dict"),
            "\"Dict\" expects 2 type arguments, but 1 given"
        );
        assert_eq!(
            wrong_type_arg_count(1, 2, "3", "Foo"),
            "\"Foo\" expects between 1 and 2 type arguments, but 3 given"
        );
    }

    #[test]
    fn test_strip_quotes() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
        assert_eq!(strip_quotes("hello"), "hello");
        assert_eq!(strip_quotes("\"hello"), "hello");
    }

    #[test]
    fn test_extract_type() {
        assert_eq!(extract_type("\"__getitem__\" of list"), "list");
        assert_eq!(extract_type("some name"), "some name");
    }

    #[test]
    fn test_variance_string() {
        assert_eq!(variance_string(1), "covariant");
        assert_eq!(variance_string(2), "contravariant");
        assert_eq!(variance_string(0), "invariant");
    }

    #[test]
    fn test_format_item_name_list() {
        assert_eq!(format_item_name_list(&["a".to_string()]), "(\"a\")");
        assert_eq!(
            format_item_name_list(&[
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "f".to_string()
            ]),
            "(\"a\", \"b\", \"c\", \"d\", \"e\", ...)"
        );
    }

    #[test]
    fn test_should_format_arg_as_type() {
        // ARG_POS (0), no name -> True (both conditions true)
        assert!(should_format_arg_as_type(0, None, 0));
        // ARG_POS (0) with name, verbosity 0 -> True (is_positional)
        assert!(should_format_arg_as_type(0, Some("x"), 0));
        // ARG_POS (0) with name, verbosity 1 -> False
        assert!(!should_format_arg_as_type(0, Some("x"), 1));
        // ARG_OPT (1), no name, verbosity 0 -> True (positional)
        assert!(should_format_arg_as_type(1, None, 0));
        // ARG_OPT (1), no name, verbosity 1 -> False
        assert!(!should_format_arg_as_type(1, None, 1));
        // ARG_NAMED (3) -> False
        assert!(!should_format_arg_as_type(3, Some("x"), 0));
        // ARG_STAR (2), no name, verbosity 0 -> False (not positional per
        // ArgKind.is_positional: only ARG_POS=0, ARG_OPT=1)
        assert!(!should_format_arg_as_type(2, None, 0));
    }

    #[test]
    fn test_scoped_type_var_name() {
        assert_eq!(scoped_type_var_name("T", ""), "T");
        assert_eq!(scoped_type_var_name("T", "foo.bar"), "T@bar");
        assert_eq!(scoped_type_var_name("T", "foo"), "T@foo");
    }

    #[test]
    fn test_arg_constructor_name() {
        assert_eq!(arg_constructor_name(0), "Arg");
        assert_eq!(arg_constructor_name(1), "DefaultArg");
        assert_eq!(arg_constructor_name(2), "VarArg");
        assert_eq!(arg_constructor_name(3), "NamedArg");
        assert_eq!(arg_constructor_name(4), "KwArg");
        assert_eq!(arg_constructor_name(5), "DefaultNamedArg");
    }

    fn make_resolver(snaps: Vec<crate::typeinfo::TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn snap(fullname: &str, name: &str) -> crate::typeinfo::TypeInfoSnapshot {
        let mut s = crate::typeinfo::TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: name.to_string(),
            ..Default::default()
        };
        s.mro.push(fullname.to_string());
        s.has_base.insert(fullname.to_string());
        s
    }

    fn instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    #[test]
    fn test_append_invariance_notes_positive() {
        // list[A] <: list[A] (A has no args, so has_base A→A suffices).
        let r = make_resolver(vec![snap("a.A", "A"), snap("builtins.object", "object")]);
        let arg = instance("builtins.list", vec![instance("a.A", vec![])]);
        let expected = instance("builtins.list", vec![instance("a.A", vec![])]);
        let notes = append_invariance_notes_inner(&arg, &expected, &r).unwrap();
        assert_eq!(
            notes,
            vec![
                "\"list\" is invariant -- see \
                 https://mypy.readthedocs.io/en/stable/common_issues.html#variance"
                    .to_string(),
                "Consider using \"Sequence\" instead, which is covariant".to_string()
            ]
        );
    }

    #[test]
    fn test_append_invariance_notes_negative() {
        // A and B are unrelated, so list[A] <: list[B] is Some(false)
        // (invariance note fires only on a subtype, so no notes).
        let r = make_resolver(vec![
            snap("a.A", "A"),
            snap("a.B", "B"),
            snap("builtins.object", "object"),
        ]);
        let arg = instance("builtins.list", vec![instance("a.A", vec![])]);
        let expected = instance("builtins.list", vec![instance("a.B", vec![])]);
        assert_eq!(
            append_invariance_notes_inner(&arg, &expected, &r),
            Some(vec![])
        );
    }

    #[test]
    fn test_append_invariance_notes_dict_positive() {
        // dict[str, str] <: dict[str, str].
        let r = make_resolver(vec![
            snap("builtins.str", "str"),
            snap("builtins.object", "object"),
        ]);
        let arg = instance(
            "builtins.dict",
            vec![
                instance("builtins.str", vec![]),
                instance("builtins.str", vec![]),
            ],
        );
        assert_eq!(
            append_invariance_notes_inner(&arg, &arg, &r),
            Some(vec![
                "\"dict\" is invariant -- see \
                 https://mypy.readthedocs.io/en/stable/common_issues.html#variance"
                    .to_string(),
                "Consider using \"Mapping\" instead, which is covariant in the value type"
                    .to_string()
            ])
        );
    }

    #[test]
    fn test_append_invariance_notes_non_instance_defers() {
        let r = make_resolver(vec![]);
        let arg = instance("builtins.list", vec![instance("a.A", vec![])]);
        assert_eq!(
            append_invariance_notes_inner(
                &arg,
                &Type::AnyType {
                    type_of_any: 0,
                    source_any: None,
                    missing_import_name: None
                },
                &r
            ),
            None
        );
    }

    #[test]
    fn test_append_numbers_notes_matches() {
        // Numbers types are members of UNSUPPORTED_NUMBERS_TYPES.
        assert_eq!(
            append_numbers_notes_inner(&instance("numbers.Complex", vec![])),
            Some(vec![
                "Types from \"numbers\" are not supported for static type checking".to_string(),
                "See https://peps.python.org/pep-0484/#the-numeric-tower".to_string(),
                "Consider using a protocol instead, such as typing.SupportsFloat".to_string()
            ])
        );
    }

    #[test]
    fn test_append_numbers_notes_non_matching() {
        assert_eq!(
            append_numbers_notes_inner(&instance("builtins.str", vec![])),
            Some(vec![])
        );
        // Non-Instance expected type defers to Python.
        assert_eq!(append_numbers_notes_inner(&Type::NoneType), None);
    }
}
