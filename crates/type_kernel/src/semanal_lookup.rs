//! Issue #491: native `lookup_qualified` dot-chain walk.
//!
//! Mirrors the dot-chain walk in `SemanticAnalyzer.lookup_qualified`
//! (semanal.py:7126-7181). The first part is looked up in Python
//! (`self.lookup(parts[0])`); Rust walks the remaining parts through
//! `TypeInfo.get(part)` (MRO traversal) or a MypyFile's `names` symbol
//! table (via the resolver's module snapshots).
//!
//! Conservative seam: only the `TypeInfo` chain and direct MypyFile
//! name hits are handled. TypeAlias, Var, ParamSpecExpr, PlaceholderNode,
//! and uncertain MypyFile steps (submodule resolution, incomplete
//! namespaces, `__getattr__`, missing modules) defer (return `None`) so
//! Python runs the full logic unchanged. This is the strangler-fig
//! per-call gate.

use pyo3::prelude::*;

use crate::typeinfo::{NativeTypeResolver, TypeInfoSnapshot};
use crate::wire::{self, ReadBuffer, Type};

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
/// (MypyFile, TypeAlias, Any-typed Var, ParamSpecExpr, PlaceholderNode,
/// missing TypeInfo snapshot), it returns `None` so Python falls back.
///
/// Returns `Some((RESULT_KIND, fullname))`:
/// - `(0, fullname)`: resolved to a TypeInfo member; Python calls
///   `TypeInfo.get(last_part)` to get the `SymbolTableNode`.
/// - `(2, "")`: placeholder encountered mid-chain; Python returns
///   the current `sym` unchanged.
/// - `(-1, "")`: member not found in any MRO entry (or a non-Any Var
///   first sym, whose Python else-branch always reports "name not
///   defined"); Python emits the "name not defined" error (unless
///   `suppress_errors`).
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
        // Non-Any Var: Python's else branch sets nextsym = None, reports
        // "name not defined", returns None. Emit RESULT_NOT_FOUND so the
        // shim runs the identical name_not_defined path.
        return Ok(Some((RESULT_NOT_FOUND, String::new())));
    }

    if first_sym_kind == KIND_PARAMSPECEXPR {
        // Python checks part in ("args", "kwargs"); defer.
        return Ok(None);
    }

    if first_sym_kind == KIND_TYPEALIAS {
        // TypeAlias with no_args + Instance target: decode the alias
        // target, extract the Instance type_ref, and continue the
        // TypeInfo chain walk from there (Python: semanal.py:7743-7746).
        let alias = resolver.alias_resolver().get(first_sym_fullname);
        let alias_snap = match alias {
            Some(a) => a,
            None => return Ok(None),
        };
        if !alias_snap.no_args {
            // Python's elif requires no_args=True; no_args=False falls
            // through to the else branch (nextsym = None, error). Defer
            // so Python runs the exact same path.
            return Ok(None);
        }
        let target = match decode_type(&alias_snap.target) {
            Some(t) => t,
            None => return Ok(None),
        };
        let target_ref = match &target {
            Type::Instance { type_ref, .. } => type_ref.clone(),
            _ => {
                // Non-Instance target: Python sets nextsym = None (error).
                // Defer so Python handles it.
                return Ok(None);
            }
        };
        // Continue the TypeInfo chain walk from the alias target.
        let type_resolver = resolver.resolver();
        let mut current_fullname = target_ref;
        for &part in parts.iter().skip(1) {
            let snap = match type_resolver.get(&current_fullname) {
                Some(s) => s,
                None => return Ok(None),
            };
            match find_member_in_mro(type_resolver, snap, part) {
                Some(_found_fullname) => {
                    let next_fullname = format!("{}.{}", current_fullname, part);
                    if type_resolver.get(&next_fullname).is_some() {
                        current_fullname = next_fullname;
                    } else {
                        return Ok(Some((RESULT_TYPEINFO_MEMBER, next_fullname)));
                    }
                }
                None => return Ok(Some((RESULT_NOT_FOUND, String::new()))),
            }
        }
        return Ok(Some((RESULT_TYPEINFO_MEMBER, current_fullname)));
    }

    if first_sym_kind == KIND_MYPYFILE {
        // Walk parts[1..] through module `names` via resolver module
        // snapshots. Only direct non-hidden name hits answer natively;
        // else (submodule, incomplete, `__getattr__`, missing) defers.
        match walk_mypyfile_chain(resolver.resolver(), &parts, first_sym_fullname) {
            WalkOutcome::NotFound => return Ok(Some((RESULT_NOT_FOUND, String::new()))),
            WalkOutcome::Defer => return Ok(None),
            WalkOutcome::Resolved(fullname) => return Ok(Some((RESULT_TYPEINFO_MEMBER, fullname))),
        }
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

/// Outcome of a native MypyFile-chain walk.
enum WalkOutcome {
    /// Positively not found (a `module_hidden` name): Python emits the
    /// name-not-defined error.
    NotFound,
    /// Uncertain: run Python's `get_module_symbol` unchanged.
    Defer,
    /// Resolved: the fullname of the module whose namespace the final
    /// part lives in. Python re-walks to re-materialize the symbol.
    Resolved(String),
}

/// Walk a MypyFile chain (`first_sym_fullname` + remaining parts) through
/// the resolver's module snapshots. Mirrors the MypyFile arm of
/// `SemanticAnalyzer.lookup_qualified`'s loop, which calls
/// `get_module_symbol(node, part)` per step.
///
/// Only positively-provable steps are answered:
/// - module not snapshotted: Defer. Could be the module currently being
///   analyzed (its SCC not yet sealed at snapshot time).
/// - name absent from the module's `names`: Defer. Could be a submodule
///   (is_visible_import), an incomplete namespace
///   (record_incomplete_ref), `__getattr__`, or a missing module.
/// - name present but `module_hidden`: NotFound. Positive proof:
///   `get_module_symbol` returns None, and the loop's
///   `nextsym.module_hidden` check emits the error.
/// - name present, non-hidden, and it is the final part: Resolved with
///   the current module's fullname; Python's re-walk re-materializes
///   the symbol.
/// - name present, non-hidden, further parts remain, and the symbol's
///   node is a snapshotted module: descend into it (by the node's exact
///   fullname, so an aliased name resolves identically to Python).
/// - anything else (non-module member mid-chain, module not snapshotted,
///   name absent from `names`): Defer.
fn walk_mypyfile_chain(
    resolver: &crate::typeinfo::TypeResolver,
    parts: &[&str],
    first_fullname: &str,
) -> WalkOutcome {
    let snap = resolver.get_module(first_fullname);
    let mut current_fullname = first_fullname.to_string();
    let mut snap = match snap {
        Some(s) => s,
        None => return WalkOutcome::Defer,
    };
    let last_index = parts.len() - 1;
    for (i, &part) in parts.iter().skip(1).enumerate() {
        let visible = match snap.visible(part) {
            Some(v) => v,
            None => return WalkOutcome::Defer,
        };
        if !visible {
            // Name exists but is hidden: positively not found.
            return WalkOutcome::NotFound;
        }
        if i == last_index - 1 {
            // Final part: hand the module fullname to Python.
            return WalkOutcome::Resolved(current_fullname.clone());
        }
        // Non-final part: descend into a module, using the symbol's node
        // fullname so an aliased name resolves identically to Python.
        // Non-module members defer: Python fails the MypyFile check.
        let next_fullname = match snap.module_fullname(part) {
            Some(f) => f.to_string(),
            None => return WalkOutcome::Defer,
        };
        let next_snap = match resolver.get_module(&next_fullname) {
            Some(s) => s,
            None => return WalkOutcome::Defer,
        };
        current_fullname = next_fullname;
        snap = next_snap;
    }
    // Unreachable: the loop returns on the final part.
    WalkOutcome::Defer
}

fn decode_type(bytes: &[u8]) -> Option<Type> {
    let mut buf = ReadBuffer::new(bytes);
    wire::read_type(&mut buf, None).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::{ModuleSnapshot, TypeResolver};
    use std::collections::HashMap;

    fn module(fullname: &str, entries: &[(&str, bool, Option<&str>)]) -> (String, ModuleSnapshot) {
        // (name, module_hidden, module_fullname-when-MypyFile)
        let mut symbols = HashMap::new();
        for &(name, hidden, mod_full) in entries {
            let node = mod_full.map(|f| (true, f.to_string()));
            symbols.insert(name.to_string(), (hidden, node));
        }
        (fullname.to_string(), ModuleSnapshot { symbols })
    }

    fn parts(name: &str) -> Vec<&str> {
        name.split('.').collect()
    }

    #[test]
    fn walk_direct_hit_resolves_final_module() {
        let mut r = TypeResolver::new();
        let (m, s) = module("pkg", &[("x", false, None)]);
        r.insert_module(m, s);
        let p = parts("pkg.x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::Resolved(f) if f == "pkg"
        ));
    }

    #[test]
    fn walk_missing_module_defers() {
        let r = TypeResolver::new();
        let p = parts("pkg.x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::Defer
        ));
    }

    #[test]
    fn walk_absent_name_defers() {
        let mut r = TypeResolver::new();
        let (m, s) = module("pkg", &[("y", false, None)]);
        r.insert_module(m, s);
        let p = parts("pkg.x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::Defer
        ));
    }

    #[test]
    fn walk_hidden_name_is_not_found() {
        let mut r = TypeResolver::new();
        let (m, s) = module("pkg", &[("_x", true, None)]);
        r.insert_module(m, s);
        let p = parts("pkg._x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::NotFound
        ));
    }

    #[test]
    fn walk_mid_chain_module_descends_via_exact_fullname() {
        let mut r = TypeResolver::new();
        let (m, s) = module("pkg", &[("aliased", false, Some("other.ns"))]);
        r.insert_module(m, s);
        let (m2, s2) = module("other.ns", &[("x", false, None)]);
        r.insert_module(m2, s2);
        // pkg.aliased.x descends into other.ns (the symbol's real node),
        // not "pkg.aliased" (a name-joined guess that is not a module).
        let p = parts("pkg.aliased.x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::Resolved(f) if f == "other.ns"
        ));
    }

    #[test]
    fn walk_mid_chain_non_module_defers() {
        let mut r = TypeResolver::new();
        let (m, s) = module("pkg", &[("klass", false, None)]);
        r.insert_module(m, s);
        // A non-module member mid-chain: Python would set nextsym to the
        // class node then fail the MypyFile check; defer to Python.
        let p = parts("pkg.klass.x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::Defer
        ));
    }

    #[test]
    fn walk_mid_chain_unresolved_module_defers() {
        let mut r = TypeResolver::new();
        let (m, s) = module("pkg", &[("sub", false, Some("pkg.sub"))]);
        r.insert_module(m, s);
        // The symbol names a module that is not in the snapshot: deferred.
        let p = parts("pkg.sub.x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::Defer
        ));
    }

    #[test]
    fn walk_hidden_mid_chain_is_not_found() {
        let mut r = TypeResolver::new();
        let (m, s) = module(
            "pkg",
            &[("_sub", true, Some("pkg._sub")), ("x", false, None)],
        );
        r.insert_module(m, s);
        let p = parts("pkg._sub.x");
        assert!(matches!(
            walk_mypyfile_chain(&r, &p, "pkg"),
            WalkOutcome::NotFound
        ));
    }
}
