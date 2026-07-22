//! Stage 5 seam: C3 method-resolution-order linearization.
//!
//! Ports `mypy.mro` (mro.py:10-63) to Rust. The Python `linearize_hierarchy`
//! (mro.py:27-43) recurses over `TypeInfo.direct_base_classes()` and merges
//! the per-base linearizations with the C3 `merge` (mro.py:46-63), raising
//! `MroError` on an inconsistent hierarchy. `calculate_mro` (mro.py:10) is the
//! public entry that sets `info.mro`, `info.fallback_to_any`, and resets the
//! subtype caches; that side-effecting half stays Python-side.
//!
//! The Rust path works on fullnames instead of live `TypeInfo` objects: the
//! snapshot's `bases` field is a list of wire-format `Instance` blobs whose
//! `type_ref` is each base's fullname (mirrors `direct_base_classes`, which
//! is `[base.type for base in self.bases]` per nodes.py:4151-4156). The
//! Python shim converts the returned fullname list back to `TypeInfo`
//! objects via the fullname map before assigning `info.mro`.
//!
//! The strangler-fig contract mirrors `erase::erase_type` (Stage 1) and
//! `subtypes::rust_is_subtype` (Stage 3c): return `None` (Python `None`)
//! for any case Rust does not handle, and the Python caller re-runs the
//! full path. Deferred cases:
//!   * Cycles (MroError, or a base revisited mid-recursion).
//!   * A base missing from the snapshot (stale mid-build graph).
//!   * The `obj_type` callback edge (mro.py:34): a class with no bases that
//!     is not `builtins.object` needs the `obj_type` callback to synthesize
//!     a dummy `object` base. The Rust signature carries no callback, so
//!     such a class must fall through.
//!   * A cached `info.mro`: the Python shim short-circuits on `info.mro`
//!     before calling Rust, mirroring mro.py:31, so Rust is never asked to
//!     re-linearize a class whose MRO is already set.

use std::collections::HashSet;

use pyo3::prelude::*;

use crate::typeinfo::NativeTypeResolver;
use crate::wire::{self, ReadBuffer, Type};

/// Decode a wire-format `Instance` blob to its base fullname (`type_ref`).
/// Returns `None` on any read failure or non-Instance base (mirrors the
/// `direct_base_classes` assumption that every base is an Instance).
fn base_fullname(bytes: &[u8]) -> Option<String> {
    let mut buf = ReadBuffer::new(bytes);
    match wire::read_type(&mut buf, None).ok()? {
        Type::Instance { type_ref, .. } => Some(type_ref),
        _ => None,
    }
}

/// C3 merge (mro.py:46-63). Returns `None` in place of `MroError` so the
/// Python shim falls through and raises the real `MroError`.
///
/// Mirrors the algorithm verbatim: at each step, drop empty seqs, find a
/// seq head that is not in the tail of any other seq, append it to the
/// result, and remove it from every seq. If no such head exists the
/// hierarchy is inconsistent.
fn merge(seqs: Vec<Vec<String>>) -> Option<Vec<String>> {
    let mut seqs: Vec<Vec<String>> = seqs.into_iter().filter(|s| !s.is_empty()).collect();
    let mut result: Vec<String> = Vec::new();
    loop {
        seqs.retain(|s| !s.is_empty());
        if seqs.is_empty() {
            return Some(result);
        }
        // Find the first seq whose head is not in the tail of any seq
        // (mro.py:53-56). The `for...else` raises MroError if none qualify.
        let mut chosen: Option<(usize, String)> = None;
        for (i, seq) in seqs.iter().enumerate() {
            let head = &seq[0];
            let in_tail = seqs.iter().any(|s| s.len() > 1 && s[1..].contains(head));
            if !in_tail {
                chosen = Some((i, head.clone()));
                break;
            }
        }
        let (idx, head) = chosen?;
        result.push(head.clone());
        // Remove the head from every seq that starts with it (mro.py:60-62).
        for seq in seqs.iter_mut() {
            if !seq.is_empty() && seq[0] == head {
                seq.remove(0);
            }
        }
        // Drop the chosen seq if it is now empty; the loop's retain handles
        // the general case, but removing here keeps the indices stable for
        // the next iteration's `i` scan (Python's `[s for s in seqs if s]`).
        if seqs[idx].is_empty() {
            seqs.remove(idx);
        }
    }
}

/// Recursive C3 linearization (mro.py:27-43).
///
/// `visited` is the recursion stack: a class being linearized whose own
/// fullname is already on the stack is a cycle, so we return `None` and let
/// Python raise the real `MroError`. (mypy pre-filters raw inheritance
/// cycles in `semanal.verify_base_classes`, so a cycle reaching here means
/// the snapshot is inconsistent mid-build.)
fn linearize(
    fullname: &str,
    resolver: &crate::typeinfo::TypeResolver,
    visited: &mut HashSet<String>,
) -> Option<Vec<String>> {
    let snap = resolver.get(fullname)?;
    // Mirror mro.py:31: a cached mro short-circuits. The Python shim guards
    // this before calling Rust, so reaching here with a non-empty mro is a
    // resolver built mid-build with partial state; treat it as the answer.
    if !snap.mro.is_empty() {
        return Some(snap.mro.clone());
    }
    // The `obj_type` callback edge (mro.py:34): a class with no bases that
    // is not `builtins.object` needs `obj_type` to synthesize a dummy
    // `object` base. Rust has no callback, so defer to Python.
    if snap.bases.is_empty() {
        if fullname == "builtins.object" {
            return Some(vec![fullname.to_string()]);
        }
        return None;
    }
    // Cycle guard: if this class is already on the recursion stack, defer.
    if !visited.insert(fullname.to_string()) {
        return None;
    }
    let mut base_fullnames: Vec<String> = Vec::with_capacity(snap.bases.len());
    for base_blob in &snap.bases {
        base_fullnames.push(base_fullname(base_blob)?);
    }
    // Recurse into each base, building the per-base linearizations.
    let mut lin_bases: Vec<Vec<String>> = Vec::with_capacity(base_fullnames.len());
    for bn in &base_fullnames {
        lin_bases.push(linearize(bn, resolver, visited)?);
    }
    // mro.py:42 appends the raw bases list as the final merge input.
    lin_bases.push(base_fullnames);
    visited.remove(fullname);
    let mut result = vec![fullname.to_string()];
    let merged = merge(lin_bases)?;
    result.extend(merged);
    Some(result)
}

/// Stage 5 `#[pyfunction]` entry: linearize the hierarchy of the TypeInfo
/// named `info_fullname` via the snapshot held by `resolver`.
///
/// Returns the MRO as a list of fullnames, or `None` (Python `None`) when
/// Rust declines (cycles, missing base, the `obj_type` callback edge, or an
/// inconsistent merge). The Python shim in `mypy/mro.py` converts a
/// returned list back to `TypeInfo` objects, sets `info.mro` and
/// `info.fallback_to_any`, and resets the subtype caches; on `None` it
/// re-runs the full Python path (which raises `MroError` on inconsistency).
///
/// The shim never calls this for a class whose `info.mro` is already set
/// (it short-circuits first, mirroring mro.py:31), so this entry does not
/// need to read the live `info.mro`.
#[pyfunction]
pub fn rust_linearize_hierarchy(
    resolver: &NativeTypeResolver,
    info_fullname: String,
) -> Option<Vec<String>> {
    let mut visited: HashSet<String> = HashSet::new();
    linearize(&info_fullname, resolver.resolver(), &mut visited)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typeinfo::{TypeInfoSnapshot, TypeResolver};

    /// Snapshot with the given direct bases, encoded as INSTANCE_SIMPLE wire
    /// blobs via the shared test helper (mirrors `setops::snap_with_bases`).
    fn snap_bases(fullname: &str, base_refs: &[&str]) -> TypeInfoSnapshot {
        let bases: Vec<Vec<u8>> = base_refs
            .iter()
            .map(|r| crate::wire::encode_instance_simple_for_test(r))
            .collect();
        TypeInfoSnapshot {
            fullname: fullname.to_string(),
            name: fullname.rsplit('.').next().unwrap_or(fullname).to_string(),
            bases,
            ..Default::default()
        }
    }

    fn resolver_with(snaps: Vec<TypeInfoSnapshot>) -> TypeResolver {
        let mut r = TypeResolver::new();
        for s in snaps {
            r.insert(s.fullname.clone(), s);
        }
        r
    }

    fn linearize_fullname(resolver: &TypeResolver, fullname: &str) -> Option<Vec<String>> {
        let mut visited = HashSet::new();
        linearize(fullname, resolver, &mut visited)
    }

    // --- Object root ---

    #[test]
    fn object_with_no_bases_linearizes_to_itself() {
        let r = resolver_with(vec![snap_bases("builtins.object", &[])]);
        let mro = linearize_fullname(&r, "builtins.object");
        assert_eq!(mro, Some(vec!["builtins.object".to_string()]));
    }

    #[test]
    fn class_with_no_bases_not_object_defers() {
        // mro.py:34: a baseless non-object class needs the obj_type callback.
        let r = resolver_with(vec![snap_bases("mymod.Standalone", &[])]);
        assert_eq!(linearize_fullname(&r, "mymod.Standalone"), None);
    }

    // --- Direct base ---

    #[test]
    fn direct_base_appends_object() {
        // B : object  ->  B, object
        let b = snap_bases("mymod.B", &["builtins.object"]);
        let obj_snap = snap_bases("builtins.object", &[]);
        let r = resolver_with(vec![b, obj_snap]);
        let mro = linearize_fullname(&r, "mymod.B");
        assert_eq!(
            mro,
            Some(vec!["mymod.B".to_string(), "builtins.object".to_string()])
        );
    }

    // --- Diamond inheritance ---

    #[test]
    fn diamond_inheritance_c3_order() {
        // D : object
        // B : D
        // C : D
        // A : B, C  ->  A, B, C, D, object
        let a = snap_bases("mymod.A", &["mymod.B", "mymod.C"]);
        let b = snap_bases("mymod.B", &["mymod.D"]);
        let c = snap_bases("mymod.C", &["mymod.D"]);
        let d = snap_bases("mymod.D", &["builtins.object"]);
        let obj_snap = snap_bases("builtins.object", &[]);
        let r = resolver_with(vec![a, b, c, d, obj_snap]);
        let mro = linearize_fullname(&r, "mymod.A");
        assert_eq!(
            mro,
            Some(vec![
                "mymod.A".to_string(),
                "mymod.B".to_string(),
                "mymod.C".to_string(),
                "mymod.D".to_string(),
                "builtins.object".to_string(),
            ])
        );
    }

    // --- Cycle detection ---

    #[test]
    fn cycle_defers_to_python() {
        // A : B, B : A  ->  cycle, Rust returns None.
        let a = snap_bases("mymod.A", &["mymod.B"]);
        let b = snap_bases("mymod.B", &["mymod.A"]);
        let r = resolver_with(vec![a, b]);
        assert_eq!(linearize_fullname(&r, "mymod.A"), None);
    }

    #[test]
    fn self_cycle_defers_to_python() {
        // A : A  ->  cycle on the very first recursion.
        let a = snap_bases("mymod.A", &["mymod.A"]);
        let r = resolver_with(vec![a]);
        assert_eq!(linearize_fullname(&r, "mymod.A"), None);
    }

    // --- Missing base ---

    #[test]
    fn missing_base_defers_to_python() {
        // A : B, but B is absent from the snapshot.
        let a = snap_bases("mymod.A", &["mymod.B"]);
        let r = resolver_with(vec![a]);
        assert_eq!(linearize_fullname(&r, "mymod.A"), None);
    }

    // --- Inconsistent merge (C3 invariant) ---

    #[test]
    fn inconsistent_merge_defers_to_python() {
        // X : A, B  and  Y : B, A  with Z : X, Y  ->  merge fails (A before B
        // in X, B before A in Y); no consistent head exists. Rust returns
        // None so Python raises MroError.
        // A : object, B : object (so A and B each linearize trivially).
        let a = snap_bases("mymod.A", &["builtins.object"]);
        let b = snap_bases("mymod.B", &["builtins.object"]);
        // X : A, B  ->  X, A, B, object
        let x = snap_bases("mymod.X", &["mymod.A", "mymod.B"]);
        // Y : B, A  ->  Y, B, A, object
        let y = snap_bases("mymod.Y", &["mymod.B", "mymod.A"]);
        // Z : X, Y  ->  merge of [X,A,B,obj], [Y,B,A,obj], [X,Y] fails.
        let z = snap_bases("mymod.Z", &["mymod.X", "mymod.Y"]);
        let obj_snap = snap_bases("builtins.object", &[]);
        let r = resolver_with(vec![z, x, y, a, b, obj_snap]);
        assert_eq!(linearize_fullname(&r, "mymod.Z"), None);
    }

    #[test]
    fn consistent_merge_succeeds() {
        // A : object, B : A, object  ->  B, A, object (consistent).
        let a = snap_bases("mymod.A", &["builtins.object"]);
        let b = snap_bases("mymod.B", &["mymod.A", "builtins.object"]);
        let obj_snap = snap_bases("builtins.object", &[]);
        let r = resolver_with(vec![b, a, obj_snap]);
        let mro = linearize_fullname(&r, "mymod.B");
        assert_eq!(
            mro,
            Some(vec![
                "mymod.B".to_string(),
                "mymod.A".to_string(),
                "builtins.object".to_string(),
            ])
        );
    }

    // --- Cached mro short-circuit ---

    #[test]
    fn cached_mro_short_circuits() {
        // A snapshot with a non-empty mro returns it verbatim.
        let mut a = snap_bases("mymod.A", &[]);
        a.mro = vec!["mymod.A".to_string(), "builtins.object".to_string()];
        let r = resolver_with(vec![a]);
        let mro = linearize_fullname(&r, "mymod.A");
        assert_eq!(
            mro,
            Some(vec!["mymod.A".to_string(), "builtins.object".to_string(),])
        );
    }

    // --- merge unit tests ---

    #[test]
    fn merge_empty_returns_empty() {
        assert_eq!(merge(vec![]), Some(vec![]));
    }

    #[test]
    fn merge_single_seq_returns_it() {
        let seq = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(merge(vec![seq.clone()]), Some(seq));
    }

    #[test]
    fn merge_drains_first_seq_when_heads_unconstrained() {
        // [A,B,C], [D,E,F]: no element appears in another seq's tail, so the
        // first seq's heads are picked greedily before the second's. C3 does
        // NOT interleave unconstrained seqs.
        let s1 = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let s2 = vec!["D".to_string(), "E".to_string(), "F".to_string()];
        let result = merge(vec![s1, s2]).unwrap();
        assert_eq!(
            result,
            vec!["A", "B", "C", "D", "E", "F"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn merge_interleaves_when_shared_tail_constraints_heads() {
        // [A,B,X], [C,D,X]: X is the shared tail of both. After A and C are
        // picked, B is head of [B,X] and not in tail of [D,X]=[D] (X is tail
        // but B is the head), so B is picked; then D; then X.
        let s1 = vec!["A".to_string(), "B".to_string(), "X".to_string()];
        let s2 = vec!["C".to_string(), "D".to_string(), "X".to_string()];
        let result = merge(vec![s1, s2]).unwrap();
        assert_eq!(
            result,
            vec!["A", "B", "C", "D", "X"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn merge_inconsistent_returns_none() {
        // [A,B] and [B,A]: A is in [B,A]'s tail [A], B is in [A,B]'s tail [B].
        // No head qualifies -> None (MroError).
        let s1 = vec!["A".to_string(), "B".to_string()];
        let s2 = vec!["B".to_string(), "A".to_string()];
        assert_eq!(merge(vec![s1, s2]), None);
    }

    #[test]
    fn merge_drops_empty_seqs() {
        let s1 = vec!["A".to_string()];
        let s2: Vec<String> = vec![];
        let s3 = vec!["B".to_string()];
        assert_eq!(
            merge(vec![s1, s2, s3]),
            Some(vec!["A".to_string(), "B".to_string()])
        );
    }
}
