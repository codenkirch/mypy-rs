//! Issue #491: native `lookup_qualified` dot-chain walk.
//!
//! Mirrors the dot-chain walk in `SemanticAnalyzer.lookup_qualified`
//! (semanal.py:7126-7181). The first part is looked up in Python
//! (`self.lookup(parts[0])`); Rust walks the remaining parts through
//! `TypeInfo.get(part)` (MRO traversal) using the resolver snapshot.
//!
//! Conservative seam: only the `TypeInfo` chain is handled. MypyFile,
//! TypeAlias, Var, ParamSpecExpr, and PlaceholderNode cases defer
//! (return `None`) so Python runs the full logic unchanged. This is
//! the strangler-fig per-call gate.

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeInfoSnapshot};

/// Kind codes passed from Python for the first symbol's node.
const KIND_TYPEINFO: i64 = 0;
const KIND_MYPYFILE: i64 = 1;
const KIND_PLACEHOLDER: i64 = 2;
const KIND_TYPEALIAS: i64 = 3;
const KIND_VAR: i64 = 4;
const KIND_PARAMSPECEXPR: i64 = 5;

/// Result kind codes returned to Python.
const RESULT_TYPEINFO_MEMBER: i64 = 0;
const RESULT_PLACEHOLDER: i64 = 2;
const RESULT_NOT_FOUND: i64 = -1;

/// Walk the dot-chain of a qualified name through `TypeInfo.get(part)`
/// using the resolver's TypeInfo snapshots.
///
/// Python calls this after `self.lookup(parts[0])` succeeds. The first
/// sym's kind and fullname are passed in; Rust walks `parts[1:]` through
/// the MRO of each TypeInfo, returning the resolved fullname of the
/// final member. When any step hits a case Rust doesn't handle
/// (MypyFile, TypeAlias, Var, PlaceholderNode, missing TypeInfo
/// snapshot), it returns `None` so Python falls back.
///
/// Returns `Some((RESULT_KIND, fullname))`:
/// - `(0, fullname)`: resolved to a TypeInfo member; Python calls
///   `TypeInfo.get(last_part)` to get the `SymbolTableNode`.
/// - `(2, "")`: placeholder encountered mid-chain; Python returns
///   the current `sym` unchanged.
/// - `(-1, "")`: member not found in any MRO entry; Python emits the
///   "name not defined" error (unless `suppress_errors`).
#[pyfunction]
#[pyo3(signature = (resolver, name, first_sym_kind, first_sym_fullname, first_sym_is_any))]
pub(crate) fn rust_lookup_qualified(
    resolver: &NativeTypeResolver,
    name: &str,
    first_sym_kind: i64,
    first_sym_fullname: &str,
    first_sym_is_any: bool,
) -> PyResult<Option<(i64, String)>> {
    let parts: Vec<&str> = name.split('.').collect();
    // Should have been handled by Python (no dot).
    if parts.len() < 2 {
        return Ok(None);
    }

    // PlaceholderNode: return current sym unchanged.
    if first_sym_kind == KIND_PLACEHOLDER {
        return Ok(Some((RESULT_PLACEHOLDER, String::new())));
    }

    // Var with AnyType: Python handles via implicit_symbol; defer.
    if first_sym_kind == KIND_VAR {
        if first_sym_is_any {
            // Python returns implicit_symbol; we can't build that.
            return Ok(None);
        }
        // Non-Any Var: nextsym = None path; but might be ParamSpecExpr.
        // Defer to Python for the ParamSpecExpr args/kwargs check.
        return Ok(None);
    }

    if first_sym_kind == KIND_PARAMSPECEXPR {
        // Python checks part in ("args", "kwargs"); defer.
        return Ok(None);
    }

    if first_sym_kind == KIND_TYPEALIAS {
        // TypeAlias with no_args + Instance target: would need the alias
        // resolver to decode the target and resolve type_ref. Defer.
        return Ok(None);
    }

    if first_sym_kind == KIND_MYPYFILE {
        // get_module_symbol needs module names + self.modules; the
        // resolver snapshot doesn't carry module symbol tables. Defer.
        return Ok(None);
    }

    // Only TypeInfo chain is handled.
    if first_sym_kind != KIND_TYPEINFO {
        return Ok(None);
    }

    // Walk parts[1..] through the TypeInfo MRO chain.
    let type_resolver = resolver.resolver();
    let mut current_fullname = first_sym_fullname.to_string();

    for &part in parts.iter().skip(1) {
        let snap = match type_resolver.get(&current_fullname) {
            Some(s) => s,
            None => {
                // TypeInfo not in snapshot; defer to Python.
                return Ok(None);
            }
        };
        match find_member_in_mro(type_resolver, snap, part) {
            Some(_found_fullname) => {
                // The found_fullname is the TypeInfo where the member
                // lives. The member itself is accessed by `part`.
                // For the next iteration, we need the member's node

                // type. We can only continue if the member is itself a
                // TypeInfo (nested class). Check via the resolver.
                let next_fullname = format!("{}.{}", current_fullname, part);
                if type_resolver.get(&next_fullname).is_some() {
                    current_fullname = next_fullname;
                } else {
                    // The member is not a TypeInfo in the snapshot —
                    // it's a method, var, etc. Return the resolved
                    // fullname so Python can do the final lookup.
                    return Ok(Some((RESULT_TYPEINFO_MEMBER, next_fullname)));
                }
            }
            None => {
                // Member not found in MRO; Python emits error.
                return Ok(Some((RESULT_NOT_FOUND, String::new())));
            }
        }
    }

    // All parts resolved to TypeInfo members.
    Ok(Some((RESULT_TYPEINFO_MEMBER, current_fullname)))
}

/// Check if `part` exists in the TypeInfo's MRO by walking each MRO
/// entry's `member_info` (name -> (implicit, has_explicit_value)).
/// Returns the fullname of the MRO entry where the member was found,
/// or `None` if not found. Mirrors `TypeInfo.get(part)`.
fn find_member_in_mro(
    resolver: &crate::typeinfo::TypeResolver,
    snap: &TypeInfoSnapshot,
    part: &str,
) -> Option<String> {
    for mro_fullname in &snap.mro {
        if let Some(mro_snap) = resolver.get(mro_fullname) {
            if mro_snap.member_info.contains_key(part) {
                return Some(mro_fullname.clone());
            }
        }
    }
    None
}
