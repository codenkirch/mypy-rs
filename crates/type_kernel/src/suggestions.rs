//! Stage 18 did-you-mean suggestions (suggestions.rs) for Issue #305.
//!
//! Ports the fuzzy-matching suggestion logic from `mypy/messages.py`:
//! - `_real_quick_ratio` (cheap upper bound on the similarity ratio)
//! - `best_matches` (ranked did-you-mean suggestions)
//! - `pretty_seq` (formatted output of matched names)
//!
//! The Python path uses `difflib.SequenceMatcher.ratio()`, which implements
//! the Ratcliff-Obershelp algorithm: find the longest matching block,
//! recurse on both sides, then ratio = 2*M / (len(a) + len(b)) where M is
//! the total matched character count. We port that algorithm in pure Rust
//! so the behavior matches byte-for-byte.

use pyo3::prelude::*;
use std::cmp::Ordering;

/// Upper bound on `difflib.SequenceMatcher.ratio()`, matching
/// `_real_quick_ratio` in `mypy/messages.py`.
fn real_quick_ratio(a: &str, b: &str) -> f64 {
    let al = a.chars().count();
    let bl = b.chars().count();
    if al + bl == 0 {
        return 1.0;
    }
    2.0 * (al.min(bl) as f64) / (al + bl) as f64
}

/// Find the longest matching block between `a[alo..ahi]` and `b[blo..bhi]`.
///
/// Matches Python's `difflib.SequenceMatcher.find_longest_match`:
/// returns `(i, j, k)` where `a[i..i+k] == b[j..j+k]` is the longest
/// common substring, ties broken by earliest in `a` then earliest in `b`,
/// and empty match returns `(alo, blo, 0)`.
fn find_longest_match(
    a: &[char],
    alo: usize,
    ahi: usize,
    b: &[char],
    blo: usize,
    bhi: usize,
) -> (usize, usize, usize) {
    let mut besti = alo;
    let mut bestj = blo;
    let mut bestsize: usize = 0;

    // For each j in b, track the longest match ending at that position:
    // iterate over a, updating the b positions as CPython's difflib.py does.

    let b_len = bhi - blo;
    let mut j2len = vec![0usize; b_len + 1];

    for (i, &ai) in a.iter().enumerate().take(ahi).skip(alo) {
        let mut j2len_new = vec![0usize; b_len + 1];
        for j in blo..bhi {
            if ai == b[j] {
                let k = j2len[j - blo];
                j2len_new[j + 1 - blo] = k + 1;
                let size = k + 1;
                if size > bestsize {
                    bestsize = size;
                    besti = i + 1 - size;
                    bestj = j + 1 - size;
                }
            }
        }
        j2len = j2len_new;
    }

    (besti, bestj, bestsize)
}

/// Compute the Ratcliff-Obershelp similarity ratio, matching
/// `difflib.SequenceMatcher(None, a, b).ratio()`.
///
/// Recursively finds the longest matching block, then recurses on both
/// sides. M = total matched chars, ratio = 2*M / (len(a) + len(b)).
pub fn sequence_matcher_ratio(a: &str, b: &str) -> f64 {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let la = a_chars.len();
    let lb = b_chars.len();

    if la == 0 && lb == 0 {
        return 1.0;
    }
    if la == 0 || lb == 0 {
        return 0.0;
    }

    let matches = count_matches(&a_chars, 0, la, &b_chars, 0, lb);
    2.0 * (matches as f64) / (la + lb) as f64
}

/// Recursively count matched characters (the M in the ratio formula).
fn count_matches(a: &[char], alo: usize, ahi: usize, b: &[char], blo: usize, bhi: usize) -> usize {
    let (i, j, k) = find_longest_match(a, alo, ahi, b, blo, bhi);
    if k == 0 {
        return 0;
    }
    let mut total = k;
    if i > alo && j > blo {
        total += count_matches(a, alo, i, b, blo, j);
    }
    if i + k < ahi && j + k < bhi {
        total += count_matches(a, i + k, ahi, b, j + k, bhi);
    }
    total
}

/// Rank options by similarity to `current`, returning the top `n`.
///
/// Mirrors `best_matches` in `mypy/messages.py`:
/// 1. Filter by `_real_quick_ratio > 0.75`.
/// 2. If 50+ candidates remain, further filter by `abs(len-o - len-current) <= 1`.
/// 3. Compute `SequenceMatcher.ratio()` for each remaining candidate.
/// 4. Keep only ratio > 0.75, sort by (-ratio, name), take top `n`.
#[pyfunction]
pub fn rust_best_matches(current: &str, options: Vec<String>, n: usize) -> Vec<String> {
    if current.is_empty() {
        return vec![];
    }

    let mut filtered: Vec<String> = options
        .into_iter()
        .filter(|o| real_quick_ratio(current, o) > 0.75)
        .collect();

    if filtered.len() >= 50 {
        let cur_len = current.chars().count();
        filtered.retain(|o| (o.chars().count() as isize - cur_len as isize).abs() <= 1);
    }

    let mut scored: Vec<(String, f64)> = filtered
        .into_iter()
        .map(|o| {
            let ratio = sequence_matcher_ratio(current, &o);
            (o, ratio)
        })
        .filter(|(_, ratio)| *ratio > 0.75)
        .collect();

    scored.sort_by(|a, b| match b.1.partial_cmp(&a.1) {
        Some(Ordering::Equal) | None => a.0.cmp(&b.0),
        Some(ord) => ord,
    });

    scored.into_iter().take(n).map(|(s, _)| s).collect()
}

/// Format a sequence of names for did-you-mean messages.
///
/// Mirrors `pretty_seq` in `mypy/messages.py`:
/// - 1 item: `"a"`
/// - 2 items: `"a" or "b"`
/// - 3+ items: `"a", "b", or "c"`
#[pyfunction]
pub fn rust_pretty_seq(args: Vec<String>, conjunction: &str) -> String {
    let quoted: Vec<String> = args.iter().map(|a| format!("\"{a}\"")).collect();
    if quoted.is_empty() {
        return String::new();
    }
    if quoted.len() == 1 {
        return quoted[0].clone();
    }
    if quoted.len() == 2 {
        return format!("{} {} {}", quoted[0], conjunction, quoted[1]);
    }
    let last_sep = format!(", {conjunction} ");
    let (init, last) = quoted.split_at(quoted.len() - 1);
    format!("{}{}{}", init.join(", "), last_sep, last[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_real_quick_ratio() {
        assert_eq!(real_quick_ratio("foo", "foo"), 1.0);
        assert!((real_quick_ratio("foo", "foobar") - 2.0 * 3.0 / 9.0).abs() < 1e-9);
    }

    #[test]
    fn test_sequence_matcher_ratio_identical() {
        assert!((sequence_matcher_ratio("foo", "foo") - 1.0).abs() < 1e-9);
        assert!((sequence_matcher_ratio("abracadabra", "abracadabra") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_sequence_matcher_ratio_disjoint() {
        assert!((sequence_matcher_ratio("foo", "bar") - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_sequence_matcher_ratio_partial() {
        // helo / hello: match = 4 (helo), total = 9 -> 8/9
        let r = sequence_matcher_ratio("helo", "hello");
        assert!((r - 8.0 / 9.0).abs() < 1e-9, "got {r}");

        // append / apend: "ap" matches (len 2); right of that, "pend" vs "end"
        // gives "end" (len 3); ratio = 2*5/11 = 10/11 = 0.909...
        let r2 = sequence_matcher_ratio("append", "apend");
        assert!((r2 - 10.0 / 11.0).abs() < 1e-9, "got {r2}");
    }

    #[test]
    fn test_sequence_matcher_ratio_empty() {
        assert!((sequence_matcher_ratio("", "") - 1.0).abs() < 1e-9);
        assert!((sequence_matcher_ratio("x", "") - 0.0).abs() < 1e-9);
        assert!((sequence_matcher_ratio("", "x") - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_best_matches_basic() {
        let result = rust_best_matches(
            "helo",
            vec![
                "hello".to_string(),
                "help".to_string(),
                "hero".to_string(),
                "hola".to_string(),
                "foo".to_string(),
                "".to_string(),
            ],
            3,
        );
        assert_eq!(result, vec!["hello".to_string()]);
    }

    #[test]
    fn test_best_matches_empty_current() {
        let result = rust_best_matches("", vec!["a".to_string(), "b".to_string()], 3);
        assert!(result.is_empty());
    }

    #[test]
    fn test_best_matches_many() {
        let result = rust_best_matches(
            "test",
            vec![
                "text".to_string(),
                "tent".to_string(),
                "test".to_string(),
                "toast".to_string(),
                "foo".to_string(),
                "best".to_string(),
                "rest".to_string(),
                "pest".to_string(),
                "lest".to_string(),
            ],
            3,
        );
        assert_eq!(result, vec!["test".to_string()]);
    }

    #[test]
    fn test_pretty_seq() {
        assert_eq!(rust_pretty_seq(vec!["a".to_string()], "or"), "\"a\"");
        assert_eq!(
            rust_pretty_seq(vec!["a".to_string(), "b".to_string()], "or"),
            "\"a\" or \"b\""
        );
        assert_eq!(
            rust_pretty_seq(
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                "or"
            ),
            "\"a\", \"b\", or \"c\""
        );
        assert_eq!(
            rust_pretty_seq(
                vec![
                    "a".to_string(),
                    "b".to_string(),
                    "c".to_string(),
                    "d".to_string()
                ],
                "and"
            ),
            "\"a\", \"b\", \"c\", and \"d\""
        );
    }
}
