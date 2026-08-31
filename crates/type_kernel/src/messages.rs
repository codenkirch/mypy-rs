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
use pyo3::types::PyType;

use crate::subtypes::{is_subtype, SubtypeContext};
use crate::suggestions::rust_best_matches;
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

/// Decide the `min_verbosity` for `format_type_distinctly` (messages.py:3067-3083).
///
/// When the pair is two `CallableType`s and `right` has any named args, Python
/// bumps verbosity to 1 if `is_subtype(left, right, ignore_pos_arg_names=True)`.
/// Everything else keeps verbosity 0. Returns `None` when the native subtype
/// solver cannot decide the Callable pair (Python falls back). Previously the
/// calling seam deferred the whole branch whenever `right` had named args,
/// even when `is_subtype` was False (the common incompatible-args case).
fn callable_pair_min_verbosity(types: &[Type], resolver: &TypeResolver) -> Option<i64> {
    if types.len() != 2 {
        return Some(0);
    }
    let (left, right) = (&types[0], &types[1]);
    if !(matches!(left, Type::CallableType { .. }) && matches!(right, Type::CallableType { .. })) {
        return Some(0);
    }
    let Type::CallableType { arg_names, .. } = right else {
        return Some(0);
    };
    if !arg_names.iter().any(|n| n.is_some()) {
        return Some(0);
    }
    // is_subtype(left, right, ignore_pos_arg_names=True), non-proper, with the
    // default options (strict_optional=True) Python uses when options=None.
    let ctx = SubtypeContext::with_callable_flags(
        false, // ignore_type_params
        false, // ignore_declared_variance
        false, // always_covariant
        false, // ignore_promotions
        false, // proper_subtype
        true,  // strict_optional (Python default when options=None)
        true,  // ignore_pos_arg_names
        false, // strict_concatenate
    );
    let sub = is_subtype(left, right, &ctx, resolver)?;
    Some(if sub { 1 } else { 0 })
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
    // Defer only when the native subtype solver cannot decide the pair.
    let min_verbosity = callable_pair_min_verbosity(&types, resolver.resolver())?;

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

        // ErasedType has no dedicated branch in messages.py format_type_inner;
        // it falls through to the `else` default of "object".
        Type::ErasedType => Some("object".to_string()),

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
    // Python only indexes args[0]/args[1] inside the list/list and
    // dict/dict arms (its `and` chains short-circuit on the fullnames), so
    // any other Instance pair contributes no notes even with empty args.
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

    let ctx = SubtypeContext::new(false, false, false, false, false, true);
    let (invariant_type, covariant_suggestion) =
        if arg_ref == "builtins.list" && exp_ref == "builtins.list" {
            if arg_args.is_empty() || exp_args.is_empty() {
                // Python would index args[0]; defer so the fallback body
                // reproduces the same behavior.
                return None;
            }
            match is_subtype(&arg_args[0], &exp_args[0], &ctx, resolver) {
                Some(true) => (
                    "list",
                    "Consider using \"Sequence\" instead, which is covariant",
                ),
                Some(false) => return Some(Vec::new()),
                None => return None,
            }
        } else if arg_ref == "builtins.dict" && exp_ref == "builtins.dict" {
            if arg_args.is_empty() || exp_args.is_empty() {
                // Python would index args[0]/args[1].
                return None;
            }
            match is_same(&arg_args[0], &exp_args[0], resolver) {
                Some(true) => {}
                Some(false) => return Some(Vec::new()),
                None => return None,
            }
            match is_subtype(&arg_args[1], &exp_args[1], &ctx, resolver) {
                Some(true) => (
                    "dict",
                    "Consider using \"Mapping\" instead, which is covariant in the value type",
                ),
                Some(false) => return Some(Vec::new()),
                None => return None,
            }
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
// make_inferred_type_note decision (Issue #982, messages.py:3770-3800)
// ---------------------------------------------------------------------------

/// Pure wire decision core of `make_inferred_type_note`. Both types must be
/// same-fullname Instances with non-empty args, and every per-arg subtype
/// result must hold. `arg_results` is the zip'd list the Python shim computed
/// with the already-native `is_subtype`, mirroring
/// `zip(subtype.args, supertype.args)` (arg-count mismatches only require the
/// min-length prefix to pass, exactly like the Python loop).
fn inferred_note_wire_decision(subtype: &Type, supertype: &Type, arg_results: &[bool]) -> bool {
    match (subtype, supertype) {
        (
            Type::Instance {
                type_ref: sub_ref,
                args: sub_args,
                ..
            },
            Type::Instance {
                type_ref: sup_ref,
                args: sup_args,
                ..
            },
        ) => {
            sub_ref == sup_ref
                && !sub_args.is_empty()
                && !sup_args.is_empty()
                && arg_results.iter().all(|&ok| ok)
        }
        _ => false,
    }
}

/// Context classifier for `make_inferred_type_note` (messages.py:3788-3791):
/// the note only fires for a `return` statement returning an inferred local
/// variable (`ReturnStmt` -> `NameExpr` -> `Var.is_inferred`). Mirrors
/// `rust_is_magic_base` (pure PyO3 node predicate, never defers): an
/// unreadable node fact decides `false`.
fn inferred_note_context_fires(py: Python<'_>, context: &PyAny) -> PyResult<bool> {
    let nodes = py.import("mypy.nodes")?;
    let return_stmt: &PyType = nodes.getattr("ReturnStmt")?.downcast()?;
    let name_expr: &PyType = nodes.getattr("NameExpr")?.downcast()?;
    let var_cls: &PyType = nodes.getattr("Var")?.downcast()?;

    if !context.is_instance(return_stmt)? {
        return Ok(false);
    }
    let expr = match context.getattr("expr") {
        Ok(expr) => expr,
        Err(_) => return Ok(false),
    };
    if !expr.is_instance(name_expr)? {
        return Ok(false);
    }
    let node = match expr.getattr("node") {
        Ok(node) => node,
        Err(_) => return Ok(false),
    };
    if !node.is_instance(var_cls)? {
        return Ok(false);
    }
    let is_inferred = match node
        .getattr("is_inferred")
        .and_then(|v| v.extract::<bool>())
    {
        Ok(b) => b,
        Err(_) => return Ok(false),
    };
    if !is_inferred {
        return Ok(false);
    }
    // node.name is part of the fact chain (the Python shim formats the note
    // from `context.expr.name`); an unreadable name decides false.
    if node.getattr("name").is_err() {
        return Ok(false);
    }
    Ok(true)
}

/// Decision of `make_inferred_type_note` (messages.py:3770-3800, issue #982),
/// called from checker.py:9046 on inferred-return mismatches.
///
/// Rust owns the pure bool decision: same-fullname generic Instances whose
/// every per-arg subtype result (computed by the shim with the already-native
/// `is_subtype`) is true, in a `return` statement returning an inferred local
/// variable. Python keeps note emission (formats the message from
/// `context.expr.name` + the supertype string) and runs the pure-Python body
/// as the fallback on the gate-off path.
///
/// Never defers: undecodable wire bytes or an unreadable node fact decide
/// `false` (note absent), matching the total-predicate shape of
/// `rust_is_magic_base`.
#[pyfunction]
pub fn rust_make_inferred_type_note(
    py: Python<'_>,
    subtype_bytes: &[u8],
    supertype_bytes: &[u8],
    arg_results: Vec<bool>,
    context: &PyAny,
) -> PyResult<bool> {
    let subtype = match wire::read_type(&mut ReadBuffer::new(subtype_bytes), None) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let supertype = match wire::read_type(&mut ReadBuffer::new(supertype_bytes), None) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    if !inferred_note_wire_decision(&subtype, &supertype, &arg_results) {
        return Ok(false);
    }
    inferred_note_context_fires(py, context)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Callable name helpers (messages.py:3427-3438)
// ---------------------------------------------------------------------------

/// `mypy/messages.py:3427` — return quoted callable name for messages.
///
/// If `name` starts with `<` or is `None`/empty, returns `None` so the Python
/// caller falls back to the native path.
#[pyfunction]
pub fn rust_callable_name(name: String) -> Option<String> {
    if name.is_empty() || name.starts_with('<') {
        return None;
    }
    let quoted = format!("\"{}\"", name);
    Some(quoted.replace(" of ", "\" of \""))
}

/// `mypy/messages.py:3434` — return `" for {name}"` or `""`.
///
/// Thin wrapper around `callable_name`: if the name is present, returns
/// `" for <quoted_name>"`; otherwise `None` to let Python return `""`.
#[pyfunction]
pub fn rust_for_function(name: String) -> Option<String> {
    rust_callable_name(name).map(|n| format!(" for {n}"))
}

// ── dmypy server helper (Issue #358)
// ───────────────

/// `mypy/util.py:count_stats` — count errors, notes and error_files in a
/// message list.
///
/// Pure computation over a `list[str]`. Called from `dmypy_server.py` during
/// `initialize_fine_grained` and `increment_output` to compute the daemon status
/// code from the formatted message list. Returns `(n_errors, n_notes, n_error_files)`.
///
/// Issue #358: port of dmypy server pure helpers behind the `native_type_kernel`
/// gate. All server orchestration (check, recheck, run, status commands) stays
/// in Python; only the separable pure helpers are ported.
#[pyfunction]
pub fn rust_count_stats(messages: Vec<String>) -> (i64, i64, i64) {
    let errors: i64 = messages.iter().filter(|e| e.contains(": error:")).count() as i64;
    let notes: i64 = messages.iter().filter(|e| e.contains(": note:")).count() as i64;
    let error_files: i64 = messages
        .iter()
        .filter(|e| e.contains(": error:"))
        .map(|e| e.split(':').next().unwrap_or("").to_string())
        .collect::<HashSet<String>>()
        .len() as i64;
    (errors, notes, error_files)
}

// ---------------------------------------------------------------------------
// Pure string-message generators (Issue #438)
// ---------------------------------------------------------------------------

// These mirror the message-body construction in mypy/messages.py for the
// functions that take only pre-resolved strings/ints (no live Type). The
// Python wrapper extracts the needed data from CallableType / context and

// passes it here; if we return None, Python falls back to its own body.

/// `mypy/messages.py:947` — too_few_arguments message body.
///
/// `argument_names` carries the call-site argument names (None = positional).
/// `callee_arg_names` / `callee_min_args` come from the CallableType.
/// `callee_name` is `callable_name(callee)` (already resolved, may be None).
/// `for_func` is `for_function(callee)` (already resolved).
/// Returns None when the `prefer_simple_messages` + `argument_names` branch
/// produces a message that requires the `callee_name is not None and diff and
/// all(d is not None for d in diff)` condition but `callee_name` is None — in
/// that case Python's fallback uses `for_function(callee)` which we cannot
/// reconstruct, so we return None to let Python handle it.
#[pyfunction]
#[pyo3(signature = (prefer_simple, argument_names, callee_arg_names, callee_min_args, callee_name, for_func))]
#[allow(clippy::too_many_arguments)]
pub fn rust_too_few_arguments(
    prefer_simple: bool,
    argument_names: Option<Vec<Option<String>>>,
    callee_arg_names: Vec<Option<String>>,
    callee_min_args: i64,
    callee_name: Option<String>,
    for_func: String,
) -> Option<String> {
    if prefer_simple {
        return Some("Too few arguments".to_string());
    }
    if let Some(arg_names) = &argument_names {
        let num_positional_args = arg_names.iter().filter(|k| k.is_none()).count();
        // Python slice: callee.arg_names[num_positional_args : callee.min_args].
        // Python returns empty when start > end or start > len; clamp to match.
        let start = num_positional_args.min(callee_arg_names.len());
        let end = (callee_min_args as usize).min(callee_arg_names.len());
        let arguments_left: Vec<Option<String>> = if start >= end {
            Vec::new()
        } else {
            callee_arg_names[start..end].to_vec()
        };
        let diff: Vec<Option<String>> = arguments_left
            .iter()
            .filter(|k| !arg_names.contains(k))
            .cloned()
            .collect();
        let mut msg = if diff.len() == 1 {
            "Missing positional argument".to_string()
        } else {
            "Missing positional arguments".to_string()
        };
        if let Some(cn) = &callee_name {
            if !diff.is_empty() && diff.iter().all(|d| d.is_some()) {
                let names: Vec<String> = diff.iter().map(|d| d.clone().unwrap()).collect();
                let args = names.join("\", \"");
                msg += format!(" \"{args}\" in call to {cn}").as_str();
                return Some(msg);
            }
        }
        // Fallback: "Too few arguments" + for_function(callee).
        // If callee_name is None, Python's for_function returns "" so the
        // message is just "Too few arguments". We can produce that here.
        let _ = callee_name;
        return Some(format!("Too few arguments{for_func}"));
    }
    Some(format!("Too few arguments{for_func}"))
}

/// `mypy/messages.py:976` — too_many_arguments message body.
#[pyfunction]
pub fn rust_too_many_arguments(prefer_simple: bool, for_func: String) -> String {
    if prefer_simple {
        "Too many arguments".to_string()
    } else {
        format!("Too many arguments{for_func}")
    }
}

/// `mypy/messages.py:997` — too_many_positional_arguments message body.
#[pyfunction]
pub fn rust_too_many_positional_arguments(prefer_simple: bool, for_func: String) -> String {
    if prefer_simple {
        "Too many positional arguments".to_string()
    } else {
        format!("Too many positional arguments{for_func}")
    }
}

/// `mypy/messages.py:971` — missing_named_argument message body.
#[pyfunction]
pub fn rust_missing_named_argument(name: String, for_func: String) -> String {
    format!("Missing named argument \"{name}\"{for_func}")
}

/// `mypy/messages.py:1019` — unexpected_keyword_argument_for_function body.
#[pyfunction]
pub fn rust_unexpected_keyword_argument_for_function(
    for_func: String,
    name: String,
    matches: Option<Vec<String>>,
) -> String {
    let mut msg = format!("Unexpected keyword argument \"{name}\"{for_func}");
    if let Some(m) = &matches {
        if !m.is_empty() {
            msg.push_str(&format!("; did you mean {}?", pretty_seq(m, "or")));
        }
    }
    msg
}

/// `mypy/messages.py:916` — invalid_index_type message body.
///
/// `index_str` and `expected_str` are already resolved via
/// `format_type_distinctly` (Python side or Rust). `base_str` is the
/// pre-resolved base type string.
#[pyfunction]
pub fn rust_invalid_index_type(
    index_str: String,
    expected_str: String,
    base_str: String,
) -> String {
    format!("Invalid index type {index_str} for {base_str}; expected type {expected_str}")
}

/// `mypy/messages.py:1232` — wrong_number_values_to_unpack message body.
#[pyfunction]
pub fn rust_wrong_number_values_to_unpack(provided: i64, expected: i64) -> String {
    if provided < expected {
        if provided == 1 {
            format!("Need more than 1 value to unpack ({expected} expected)")
        } else {
            format!("Need more than {provided} values to unpack ({expected} expected)")
        }
    } else if provided > expected {
        format!("Too many values to unpack ({expected} expected, {provided} provided)")
    } else {
        // provided == expected: no error in Python. Return empty to signal
        // no-op; Python wrapper checks for this.
        String::new()
    }
}

/// `mypy/messages.py:1500` — undefined_in_superclass message body.
#[pyfunction]
pub fn rust_undefined_in_superclass(member: String) -> String {
    format!("\"{member}\" undefined in superclass")
}

/// `mypy/messages.py:1833` — signatures_incompatible message body.
#[pyfunction]
pub fn rust_signatures_incompatible(method: String, other_method: String) -> String {
    format!("Signatures of \"{method}\" and \"{other_method}\" are incompatible")
}

/// `mypy/messages.py:1284` — signature_incompatible_with_supertype error body.
///
/// Only the `fail` line; the notes (format_type_distinctly, pretty_callable)
/// stay on Python because they need live Type objects.
#[pyfunction]
pub fn rust_signature_incompatible_with_supertype(name: String, target: String) -> String {
    format!("Signature of \"{name}\" incompatible with {target}")
}

/// `op_methods` from `mypy/operators.py` — binary operator id to dunder.
/// Order matters: `has_no_attr` scans it to name the operator for tag 2.
const OP_METHODS: &[(&str, &str)] = &[
    ("+", "__add__"),
    ("-", "__sub__"),
    ("*", "__mul__"),
    ("/", "__truediv__"),
    ("%", "__mod__"),
    ("divmod", "__divmod__"),
    ("//", "__floordiv__"),
    ("**", "__pow__"),
    ("@", "__matmul__"),
    ("&", "__and__"),
    ("|", "__or__"),
    ("^", "__xor__"),
    ("<<", "__lshift__"),
    (">>", "__rshift__"),
    ("==", "__eq__"),
    ("!=", "__ne__"),
    ("<", "__lt__"),
    (">=", "__ge__"),
    (">", "__gt__"),
    ("<=", "__le__"),
    ("in", "__contains__"),
];

/// `COMMON_MISTAKES` from `mypy/messages.py:3608`.
const COMMON_MISTAKES: &[(&str, &[&str])] = &[("add", &["append", "extend"])];

/// `mypy/messages.py:has_no_attr` — member-access message arbitration.
///
/// Python keeps all side effects (fail/note emission,
/// `unsupported_left_operand`, formatting); Rust only picks the branch.
/// Returns `(tag, op, matches)`: branch tag, the operator id for tag 2, and
/// the did-you-mean matches for tag 12. Never defers: the scalar facts
/// cover every reachable branch.
#[allow(clippy::too_many_arguments)]
#[pyfunction]
pub fn rust_classify_has_no_attr(
    member: String,
    is_instance: bool,
    is_function_like: bool,
    is_type_obj: bool,
    is_union: bool,
    is_typevar: bool,
    typevar_bound_is_union: bool,
    has_readable_member: bool,
    instance_fullname: String,
    are_type_names_disabled: bool,
    instance_has_names: bool,
    module_private: bool,
    instance_names: Vec<String>,
    module_public_names: Vec<String>,
) -> (i64, String, Vec<String>) {
    // Tags 0-10: the special-case front of has_no_attr (messages.py:391-471).
    if is_instance && has_readable_member {
        return (0, String::new(), vec![]);
    }
    if member == "__contains__" {
        return (1, String::new(), vec![]);
    }
    if let Some((op, _)) = OP_METHODS.iter().find(|(_, m)| *m == member) {
        return (2, (*op).to_string(), vec![]);
    }
    match member.as_str() {
        "__neg__" => return (3, String::new(), vec![]),
        "__pos__" => return (4, String::new(), vec![]),
        "__invert__" => return (5, String::new(), vec![]),
        "__getitem__" => {
            // messages.py:429-446: type objects report without a code.
            if is_function_like && is_type_obj {
                return (6, String::new(), vec![]);
            }
            return (7, String::new(), vec![]);
        }
        "__setitem__" => return (8, String::new(), vec![]),
        "__call__" => {
            // messages.py:457-471: 'builtins.function' gets a clearer message.
            if is_instance && instance_fullname == "builtins.function" {
                return (9, String::new(), vec![]);
            }
            return (10, String::new(), vec![]);
        }
        _ => {}
    }
    // The non-special tail (messages.py:504-601). The Union/TypeVar/else
    // chain hangs off `if not are_type_names_disabled()`; with type names
    // enabled everything lands in the plain or Instance sub-block.
    if !are_type_names_disabled {
        if is_instance && instance_has_names {
            if module_private {
                return (11, String::new(), vec![]);
            }
            let mut alternatives: HashSet<String> = instance_names.iter().cloned().collect();
            alternatives.extend(module_public_names.iter().cloned());
            alternatives.remove(&member);
            let common: &[&str] = COMMON_MISTAKES
                .iter()
                .find(|(m, _)| **m == member)
                .map(|(_, fixes)| *fixes)
                .unwrap_or(&[]);
            let mut matches: Vec<String> = common
                .iter()
                .filter(|f| alternatives.contains(**f))
                .map(|f| f.to_string())
                .collect();
            matches.extend(rust_best_matches(
                &member,
                alternatives.into_iter().collect(),
                3,
            ));
            if member == "__aiter__" && matches == ["__iter__".to_string()] {
                matches.clear(); // Avoid misleading suggestion
            }
            if !matches.is_empty() {
                return (12, String::new(), matches);
            }
            return (13, String::new(), vec![]);
        }
        return (16, String::new(), vec![]);
    }
    if is_union {
        return (14, String::new(), vec![]);
    }
    if is_typevar {
        // A TypeVarType whose upper bound is not a union produces no
        // message at all (messages.py:578-591 falls out of the whole
        // block); that is the only silent tail.
        if typevar_bound_is_union {
            return (15, String::new(), vec![]);
        }
        return (17, String::new(), vec![]);
    }
    (16, String::new(), vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_has_no_attr_not_assignable() {
        let (tag, op, matches) = rust_classify_has_no_attr(
            "x".to_string(),
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            "mod.C".to_string(),
            false,
            true,
            false,
            vec!["x".to_string()],
            vec![],
        );
        assert_eq!((tag, op, matches), (0, String::new(), vec![]));
    }

    #[test]
    fn test_classify_has_no_attr_operators() {
        let mk = |member: &str| {
            rust_classify_has_no_attr(
                member.to_string(),
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                String::new(),
                false,
                false,
                false,
                vec![],
                vec![],
            )
        };
        assert_eq!(mk("__contains__").0, 1);
        let (tag, op, _) = mk("__add__");
        assert_eq!((tag, op.as_str()), (2, "+"));
        let (tag, op, _) = mk("__mod__");
        assert_eq!((tag, op.as_str()), (2, "%"));
        assert_eq!(mk("__neg__").0, 3);
        assert_eq!(mk("__pos__").0, 4);
        assert_eq!(mk("__invert__").0, 5);
    }

    #[test]
    fn test_classify_has_no_attr_indexing_and_call() {
        let mk = |member: &str, is_fn: bool, is_type_obj: bool, fullname: &str| {
            rust_classify_has_no_attr(
                member.to_string(),
                fullname == "builtins.function",
                is_fn,
                is_type_obj,
                false,
                false,
                false,
                false,
                fullname.to_string(),
                false,
                false,
                false,
                vec![],
                vec![],
            )
        };
        assert_eq!(mk("__getitem__", true, true, "type[T]").0, 6);
        assert_eq!(mk("__getitem__", true, false, "f").0, 7);
        assert_eq!(mk("__getitem__", false, false, "i").0, 7);
        assert_eq!(mk("__setitem__", false, false, "i").0, 8);
        assert_eq!(mk("__call__", false, false, "builtins.function").0, 9);
        assert_eq!(mk("__call__", false, false, "i").0, 10);
    }

    #[test]
    fn test_classify_has_no_attr_ordinary_tail() {
        let mk = |member: &str,
                  is_instance: bool,
                  has_names: bool,
                  disabled: bool,
                  private: bool,
                  is_union: bool,
                  is_typevar: bool,
                  tv_union_bound: bool,
                  names: Vec<String>,
                  mod_names: Vec<String>| {
            rust_classify_has_no_attr(
                member.to_string(),
                is_instance,
                false,
                false,
                is_union,
                is_typevar,
                tv_union_bound,
                false,
                String::new(),
                disabled,
                has_names,
                private,
                names,
                mod_names,
            )
        };
        // Module-private member: tag 11.
        let (tag, _, _) = mk(
            "x",
            true,
            true,
            false,
            true,
            false,
            false,
            false,
            vec!["a".to_string()],
            vec!["x".to_string()],
        );
        assert_eq!(tag, 11);
        // Suggestion: member "add" with "append" among the names.
        let (tag, _, matches) = mk(
            "add",
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            vec!["append".to_string()],
            vec![],
        );
        assert_eq!(tag, 12);
        assert_eq!(matches, vec!["append".to_string()]);
        // The __aiter__ -> __iter__ suggestion is suppressed.
        let (tag, _, matches) = mk(
            "__aiter__",
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            vec!["__iter__".to_string()],
            vec![],
        );
        assert_eq!((tag, matches), (13, Vec::<String>::new()));
        // Plain Instance attribute miss: tag 13.
        let (tag, _, _) = mk(
            "zz",
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            vec!["append".to_string()],
            vec![],
        );
        assert_eq!(tag, 13);
        // Instance with empty names falls to the plain else-branch: tag 16.
        assert_eq!(
            mk(
                "zz",
                true,
                false,
                false,
                false,
                false,
                false,
                false,
                vec![],
                vec![]
            )
            .0,
            16
        );
        // With type names enabled, union and typevar shapes land in the
        // plain else-branch: tag 16.
        assert_eq!(
            mk(
                "x",
                false,
                false,
                false,
                false,
                true,
                false,
                false,
                vec![],
                vec![]
            )
            .0,
            16
        );
        assert_eq!(
            mk(
                "x",
                false,
                false,
                false,
                false,
                false,
                true,
                true,
                vec![],
                vec![]
            )
            .0,
            16
        );
        assert_eq!(
            mk(
                "x",
                false,
                false,
                false,
                false,
                false,
                true,
                false,
                vec![],
                vec![]
            )
            .0,
            16
        );
        // Type names disabled: the Instance suggestion sub-block is
        // suppressed, but the tail still dispatches to the plain message.
        assert_eq!(
            mk(
                "x",
                true,
                true,
                true,
                false,
                false,
                false,
                false,
                vec!["append".to_string()],
                vec![]
            )
            .0,
            16
        );
        // Disabled: union and typevar arms get their own tags.
        assert_eq!(
            mk(
                "x",
                false,
                false,
                true,
                false,
                true,
                false,
                false,
                vec![],
                vec![]
            )
            .0,
            14
        );
        assert_eq!(
            mk(
                "x",
                false,
                false,
                true,
                false,
                false,
                true,
                true,
                vec![],
                vec![]
            )
            .0,
            15
        );
        // Disabled typevar with a non-union bound is silent: tag 17.
        assert_eq!(
            mk(
                "x",
                false,
                false,
                true,
                false,
                false,
                true,
                false,
                vec![],
                vec![]
            )
            .0,
            17
        );
    }

    #[test]
    fn test_classify_has_no_attr_matches_exclude_member_and_private_filter() {
        // Public module names join the alternatives; the member itself is
        // always removed from the candidate set.
        let (tag, _, matches) = rust_classify_has_no_attr(
            "appnd".to_string(),
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            String::new(),
            false,
            true,
            false,
            vec!["apend".to_string()],
            vec!["append".to_string()],
        );
        assert_eq!(tag, 12);
        assert!(matches.contains(&"append".to_string()));
        assert!(matches.contains(&"apend".to_string()));
        assert!(!matches.contains(&"appnd".to_string()));
    }

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

    fn callable_(
        arg_types: Vec<Type>,
        arg_kinds: Vec<i64>,
        arg_names: Vec<Option<String>>,
        ret_type: Type,
        name: Option<String>,
    ) -> Type {
        Type::CallableType {
            fallback: Box::new(instance("builtins.function", vec![])),
            instance_type: None,
            is_ellipsis_args: false,
            implicit: false,
            is_bound: false,
            from_concatenate: false,
            imprecise_arg_kinds: false,
            unpack_kwargs: false,
            from_type_type: false,
            arg_types,
            arg_kinds,
            arg_names,
            ret_type: Box::new(ret_type),
            name,
            variables: vec![],
            type_guard: None,
            type_is: None,
        }
    }

    #[test]
    fn test_callable_pair_min_verbosity_wrong_arity() {
        // Non-pair inputs always resolve to verbosity 0 (no defer).
        let r = TypeResolver::new();
        let t = instance("builtins.object", vec![]);
        assert_eq!(
            callable_pair_min_verbosity(std::slice::from_ref(&t), &r),
            Some(0)
        );
        assert_eq!(
            callable_pair_min_verbosity(&[t.clone(), t.clone(), t.clone()], &r),
            Some(0)
        );
    }

    #[test]
    fn test_callable_pair_min_verbosity_non_callable() {
        // Two non-callable types: verbosity 0, never defers.
        let r = TypeResolver::new();
        assert_eq!(
            callable_pair_min_verbosity(&[instance("a.A", vec![]), instance("b.B", vec![])], &r),
            Some(0)
        );
    }

    #[test]
    fn test_callable_pair_min_verbosity_no_named_args() {
        // Callables without named args skip the bump (verbosity 0).
        let r = make_resolver(vec![
            snap("builtins.object", "object"),
            snap("builtins.function", "function"),
        ]);
        let left = callable_(
            vec![instance("builtins.object", vec![])],
            vec![0],
            vec![None],
            instance("builtins.object", vec![]),
            None,
        );
        let right = callable_(
            vec![instance("builtins.object", vec![])],
            vec![0],
            vec![None],
            instance("builtins.object", vec![]),
            None,
        );
        assert_eq!(callable_pair_min_verbosity(&[left, right], &r), Some(0));
    }

    #[test]
    fn test_callable_pair_min_verbosity_subtype_bump() {
        // Identical named-arg callables: is_subtype(left, right,
        // ignore_pos_arg_names=True) is True, so verbosity bumps to 1.
        let r = make_resolver(vec![
            snap("builtins.object", "object"),
            snap("builtins.function", "function"),
        ]);
        let left = callable_(
            vec![instance("builtins.object", vec![])],
            vec![3],
            vec![Some("x".to_string())],
            instance("builtins.object", vec![]),
            Some("f".to_string()),
        );
        let right = callable_(
            vec![instance("builtins.object", vec![])],
            vec![3],
            vec![Some("x".to_string())],
            instance("builtins.object", vec![]),
            Some("f".to_string()),
        );
        assert_eq!(callable_pair_min_verbosity(&[left, right], &r), Some(1));
    }

    #[test]
    fn test_callable_pair_min_verbosity_non_subtype_no_bump() {
        // Left has arg A, right has arg B, A and B unrelated: not a subtype,
        // so verbosity stays 0. This is the case previously deferred.
        let r = make_resolver(vec![
            snap("a.A", "A"),
            snap("b.B", "B"),
            snap("builtins.object", "object"),
            snap("builtins.function", "function"),
        ]);
        let left = callable_(
            vec![instance("a.A", vec![])],
            vec![3],
            vec![Some("x".to_string())],
            instance("builtins.object", vec![]),
            None,
        );
        let right = callable_(
            vec![instance("b.B", vec![])],
            vec![3],
            vec![Some("y".to_string())],
            instance("builtins.object", vec![]),
            None,
        );
        assert_eq!(callable_pair_min_verbosity(&[left, right], &r), Some(0));
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
    fn test_append_invariance_notes_non_list_dict_empty_args_native() {
        // Python indexes args[0]/args[1] only inside the list/list and
        // dict/dict arms; any other pair contributes no notes even with
        // empty args, so Rust answers Some(vec![]) instead of deferring.
        let r = make_resolver(vec![]);
        let arg = instance("a.X", vec![]);
        let exp = instance("a.Y", vec![]);
        assert_eq!(append_invariance_notes_inner(&arg, &exp, &r), Some(vec![]));
        // Instance vs the same fullname with no args: still no notes.
        assert_eq!(append_invariance_notes_inner(&arg, &arg, &r), Some(vec![]));
    }

    #[test]
    fn test_append_invariance_notes_list_empty_args_defers() {
        // list/list with empty args would index args[0] in Python; defer so
        // the Python body reproduces its own behavior.
        let r = make_resolver(vec![]);
        let arg = instance("builtins.list", vec![]);
        assert_eq!(append_invariance_notes_inner(&arg, &arg, &r), None);
    }

    #[test]
    fn test_append_invariance_notes_dict_empty_args_defers() {
        let r = make_resolver(vec![]);
        let arg = instance("builtins.dict", vec![]);
        assert_eq!(append_invariance_notes_inner(&arg, &arg, &r), None);
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

    // ── Issue #438: pure string-message generators
    // ──────────────────────────

    #[test]
    fn test_too_few_arguments_simple() {
        assert_eq!(
            rust_too_few_arguments(true, None, vec![], 0, None, String::new()),
            Some("Too few arguments".to_string())
        );
    }

    #[test]
    fn test_too_few_arguments_no_arg_names() {
        assert_eq!(
            rust_too_few_arguments(false, None, vec![], 0, None, " for \"f\"".to_string()),
            Some("Too few arguments for \"f\"".to_string())
        );
    }

    #[test]
    fn test_too_few_arguments_with_names_single_diff() {
        // callee arg_names: [None, "y"], min_args=2, call arg_names: ["x", None]
        // num_positional=1, arguments_left = arg_names[1:2] = ["y"]
        // diff = ["y"] (not in ["x", None]) -> single missing
        let result = rust_too_few_arguments(
            false,
            Some(vec![Some("x".to_string()), None]),
            vec![None, Some("y".to_string())],
            2,
            Some("\"f\"".to_string()),
            " for \"f\"".to_string(),
        );
        assert_eq!(
            result,
            Some("Missing positional argument \"y\" in call to \"f\"".to_string())
        );
    }

    #[test]
    fn test_too_few_arguments_with_names_multi_diff() {
        // callee arg_names: [None, "y", "z"], min_args=3, call: [None, None, None]
        // num_positional=3, arguments_left = arg_names[3:3] = [] -> no diff
        // falls through to for_function path
        let result = rust_too_few_arguments(
            false,
            Some(vec![None, None, None]),
            vec![None, Some("y".to_string()), Some("z".to_string())],
            3,
            Some("\"f\"".to_string()),
            " for \"f\"".to_string(),
        );
        // diff is empty, so callee_name condition fails -> fallback
        assert_eq!(result, Some("Too few arguments for \"f\"".to_string()));
    }

    #[test]
    fn test_too_many_arguments() {
        assert_eq!(
            rust_too_many_arguments(true, String::new()),
            "Too many arguments"
        );
        assert_eq!(
            rust_too_many_arguments(false, " for \"f\"".to_string()),
            "Too many arguments for \"f\""
        );
    }

    #[test]
    fn test_too_many_positional_arguments() {
        assert_eq!(
            rust_too_many_positional_arguments(true, String::new()),
            "Too many positional arguments"
        );
        assert_eq!(
            rust_too_many_positional_arguments(false, " for \"f\"".to_string()),
            "Too many positional arguments for \"f\""
        );
    }

    #[test]
    fn test_missing_named_argument() {
        assert_eq!(
            rust_missing_named_argument("x".to_string(), " for \"f\"".to_string()),
            "Missing named argument \"x\" for \"f\""
        );
        assert_eq!(
            rust_missing_named_argument("x".to_string(), String::new()),
            "Missing named argument \"x\""
        );
    }

    #[test]
    fn test_unexpected_keyword_argument_for_function() {
        assert_eq!(
            rust_unexpected_keyword_argument_for_function(
                " for \"f\"".to_string(),
                "x".to_string(),
                None
            ),
            "Unexpected keyword argument \"x\" for \"f\""
        );
        assert_eq!(
            rust_unexpected_keyword_argument_for_function(
                String::new(),
                "x".to_string(),
                Some(vec!["y".to_string()])
            ),
            "Unexpected keyword argument \"x\"; did you mean \"y\"?"
        );
        assert_eq!(
            rust_unexpected_keyword_argument_for_function(
                String::new(),
                "x".to_string(),
                Some(vec!["y".to_string(), "z".to_string()])
            ),
            "Unexpected keyword argument \"x\"; did you mean \"y\" or \"z\"?"
        );
        // Empty matches -> no suggestion.
        assert_eq!(
            rust_unexpected_keyword_argument_for_function(
                String::new(),
                "x".to_string(),
                Some(vec![])
            ),
            "Unexpected keyword argument \"x\""
        );
    }

    #[test]
    fn test_invalid_index_type() {
        assert_eq!(
            rust_invalid_index_type(
                "str".to_string(),
                "int".to_string(),
                "list[str]".to_string()
            ),
            "Invalid index type str for list[str]; expected type int"
        );
    }

    #[test]
    fn test_wrong_number_values_to_unpack() {
        assert_eq!(
            rust_wrong_number_values_to_unpack(1, 3),
            "Need more than 1 value to unpack (3 expected)"
        );
        assert_eq!(
            rust_wrong_number_values_to_unpack(2, 3),
            "Need more than 2 values to unpack (3 expected)"
        );
        assert_eq!(
            rust_wrong_number_values_to_unpack(5, 3),
            "Too many values to unpack (3 expected, 5 provided)"
        );
        assert_eq!(rust_wrong_number_values_to_unpack(3, 3), "");
    }

    #[test]
    fn test_undefined_in_superclass() {
        assert_eq!(
            rust_undefined_in_superclass("foo".to_string()),
            "\"foo\" undefined in superclass"
        );
    }

    #[test]
    fn test_signatures_incompatible() {
        assert_eq!(
            rust_signatures_incompatible("f".to_string(), "g".to_string()),
            "Signatures of \"f\" and \"g\" are incompatible"
        );
    }

    #[test]
    fn test_signature_incompatible_with_supertype() {
        assert_eq!(
            rust_signature_incompatible_with_supertype(
                "f".to_string(),
                "supertype \"A\"".to_string()
            ),
            "Signature of \"f\" incompatible with supertype \"A\""
        );
    }

    fn wire_instance(type_ref: &str, args: Vec<Type>) -> Type {
        Type::Instance {
            type_ref: type_ref.to_string(),
            args,
            last_known_value: None,
            extra_attrs: None,
        }
    }

    fn wire_any() -> Type {
        Type::AnyType {
            type_of_any: 0,
            source_any: None,
            missing_import_name: None,
        }
    }

    #[test]
    fn test_inferred_note_wire_decision() {
        let lst_int = wire_instance("builtins.list", vec![wire_instance("builtins.int", vec![])]);
        let lst_str = wire_instance("builtins.list", vec![wire_instance("builtins.str", vec![])]);
        let set_int = wire_instance("builtins.set", vec![wire_instance("builtins.int", vec![])]);
        let bare_list = wire_instance("builtins.list", vec![]);

        // Same fullname, non-empty args, all per-arg results true.
        assert!(inferred_note_wire_decision(&lst_int, &lst_int, &[true]));
        // A false per-arg result suppresses the note.
        assert!(!inferred_note_wire_decision(&lst_int, &lst_str, &[false]));
        // Different fullnames.
        assert!(!inferred_note_wire_decision(&lst_int, &set_int, &[true]));
        // Non-Instance subtype or supertype.
        assert!(!inferred_note_wire_decision(&wire_any(), &lst_int, &[true]));
        assert!(!inferred_note_wire_decision(&lst_int, &wire_any(), &[true]));
        // Empty args on either side.
        assert!(!inferred_note_wire_decision(&bare_list, &bare_list, &[]));
        assert!(!inferred_note_wire_decision(&lst_int, &bare_list, &[true]));
        // Arg-count mismatch: only the zip'd prefix must pass.
        let lst_int_int = wire_instance(
            "builtins.list",
            vec![
                wire_instance("builtins.int", vec![]),
                wire_instance("builtins.int", vec![]),
            ],
        );
        assert!(inferred_note_wire_decision(&lst_int, &lst_int_int, &[true]));
        assert!(!inferred_note_wire_decision(
            &lst_int,
            &lst_int_int,
            &[true, false]
        ));
    }
}
