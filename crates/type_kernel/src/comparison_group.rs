//! Native port of `mypy.checker.group_comparison_operands` (checker.py:10262-10359).
//!
//! This is PURE DATA: no `Type` objects, no wire codec, no resolver. The
//! caller (the Python seam in `mypy/checker.py`) maps each distinct
//! `literal_hash` (`Key`) to a stable `i64` id and passes three plain values:
//!
//!   * `ops_and_indices: Vec<(String, i64, i64)>` -- the pairwise comparisons
//!     as (operator, left_operand_index, right_operand_index).
//!   * `literal_hashes: HashMap<i64, i64>` -- operand_index -> stable hash id.
//!   * `operators_to_group: Vec<String>` -- the operators that chain groups.
//!
//! Rust re-implements the union-find `DisjointDict` over the hash ids and the
//! exact chain-grouping algorithm, and returns `Vec<(String, Vec<i64>)>` (the
//! operator with the sorted operand indices). The Python seam is gated on
//! `_native_checker_active` and falls back to the pure-Python body otherwise.
//! The Rust function never defers (no wire / resolver to fail on).

use std::collections::{HashMap, HashSet};

use pyo3::prelude::*;

/// Union-find variant that maps disjoint sets of hash keys to disjoint sets of
/// operand indices (Python `DisjointDict[Key, int]`, checker.py:10167-10259).
struct DisjointDict {
    /// Each hash-key id maps to a unique union-find id.
    key_to_id: HashMap<i64, i64>,
    /// Each id points to its parent id (roots point to themselves).
    id_to_parent_id: HashMap<i64, i64>,
    /// Each root id maps to the set of operand indices in its group.
    root_id_to_values: HashMap<i64, HashSet<i64>>,
}

impl DisjointDict {
    fn new() -> Self {
        Self {
            key_to_id: HashMap::new(),
            id_to_parent_id: HashMap::new(),
            root_id_to_values: HashMap::new(),
        }
    }

    /// Adds a 'Set[hash id] -> Set[index]' mapping, merging with any existing
    /// mapping that shares one or more keys. Empty key sets are a no-op.
    fn add_mapping(&mut self, keys: HashSet<i64>, values: HashSet<i64>) {
        if keys.is_empty() {
            return;
        }
        let subtree_roots: Vec<i64> = keys
            .iter()
            .map(|&k| self.lookup_or_make_root_id(k))
            .collect();
        let new_root = subtree_roots[0];
        {
            let root_values = self
                .root_id_to_values
                .get_mut(&new_root)
                .expect("root exists");
            root_values.extend(values);
        }
        for &subtree_root in &subtree_roots[1..] {
            if subtree_root == new_root || !self.root_id_to_values.contains_key(&subtree_root) {
                continue;
            }
            self.id_to_parent_id.insert(subtree_root, new_root);
            let merged = self.root_id_to_values.remove(&subtree_root).unwrap();
            self.root_id_to_values
                .get_mut(&new_root)
                .unwrap()
                .extend(merged);
        }
    }

    /// Returns all disjoint (hash keys, operand indices) pairs, one per root.
    fn items(&self) -> Vec<(HashSet<i64>, HashSet<i64>)> {
        let mut root_id_to_keys: HashMap<i64, HashSet<i64>> = HashMap::new();
        for key in self.key_to_id.keys() {
            // Pure root lookup (no path compression here; `items` borrows
            // self immutably). The logical root is unchanged by compression.
            let mut i = self.key_to_id[key];
            while i != self.id_to_parent_id[&i] {
                i = self.id_to_parent_id[&i];
            }
            root_id_to_keys.entry(i).or_default().insert(*key);
        }
        let mut output = Vec::new();
        for (root_id, keys) in root_id_to_keys {
            output.push((keys, self.root_id_to_values[&root_id].clone()));
        }
        output
    }

    fn lookup_or_make_root_id(&mut self, key: i64) -> i64 {
        if self.key_to_id.contains_key(&key) {
            self.lookup_root_id(key)
        } else {
            let new_id = self.key_to_id.len() as i64;
            self.key_to_id.insert(key, new_id);
            self.id_to_parent_id.insert(new_id, new_id);
            self.root_id_to_values.insert(new_id, HashSet::new());
            new_id
        }
    }

    fn lookup_root_id(&mut self, key: i64) -> i64 {
        let mut i = self.key_to_id[&key];
        while i != self.id_to_parent_id[&i] {
            // Halving: point at the grandparent to keep tree height bounded.
            let new_parent = self.id_to_parent_id[&self.id_to_parent_id[&i]];
            self.id_to_parent_id.insert(i, new_parent);
            i = new_parent;
        }
        i
    }
}

/// `mypy.checker.group_comparison_operands` (checker.py:10262-10359), Rust port.
#[pyfunction]
pub(crate) fn rust_group_comparison_operands(
    ops_and_indices: Vec<(String, i64, i64)>,
    literal_hashes: HashMap<i64, i64>,
    operators_to_group: Vec<String>,
) -> Vec<(String, Vec<i64>)> {
    let operators_set: HashSet<String> = operators_to_group.into_iter().collect();

    let mut groups: HashMap<String, DisjointDict> = HashMap::new();
    for op in &operators_set {
        groups.insert(op.clone(), DisjointDict::new());
    }

    let mut simplified_operator_list: Vec<(String, Vec<i64>)> = Vec::new();
    let mut last_operator: Option<String> = None;
    let mut current_indices: HashSet<i64> = HashSet::new();
    let mut current_hashes: HashSet<i64> = HashSet::new();

    for (operator, left_index, right_index) in ops_and_indices {
        if last_operator.is_none() {
            last_operator = Some(operator.clone());
        }

        let last = last_operator.as_deref().unwrap();
        let needs_flush =
            !current_indices.is_empty() && (operator != last || !operators_set.contains(&operator));
        if needs_flush {
            let last_op = last.to_string();
            if current_hashes.is_empty() {
                let mut sorted = current_indices.iter().copied().collect::<Vec<i64>>();
                sorted.sort();
                simplified_operator_list.push((last_op, sorted));
            } else {
                groups.get_mut(&last_op).unwrap().add_mapping(
                    std::mem::take(&mut current_hashes),
                    std::mem::take(&mut current_indices),
                );
            }
            last_operator = Some(operator.clone());
            current_indices = HashSet::new();
            current_hashes = HashSet::new();
        }

        current_indices.insert(left_index);
        current_indices.insert(right_index);

        if operators_set.contains(&operator) {
            if let Some(&h) = literal_hashes.get(&left_index) {
                current_hashes.insert(h);
            }
            if let Some(&h) = literal_hashes.get(&right_index) {
                current_hashes.insert(h);
            }
        }
    }

    if let Some(last_op) = &last_operator {
        if current_hashes.is_empty() {
            let mut sorted = current_indices.iter().copied().collect::<Vec<i64>>();
            sorted.sort();
            simplified_operator_list.push((last_op.clone(), sorted));
        } else {
            groups.get_mut(last_op).unwrap().add_mapping(
                std::mem::take(&mut current_hashes),
                std::mem::take(&mut current_indices),
            );
        }
    }

    for operator in groups.keys() {
        for (_keys, indices) in groups.get(operator).unwrap().items() {
            let mut sorted = indices.into_iter().collect::<Vec<i64>>();
            sorted.sort();
            simplified_operator_list.push((operator.clone(), sorted));
        }
    }

    // For stability, reorder by the first operand index (Python's sort is
    // stable and no two entries share a first index, so the order is total).
    simplified_operator_list.sort_by(|a, b| a.1[0].cmp(&b.1[0]));
    simplified_operator_list
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: Vec<(i64, i64)>) -> HashMap<i64, i64> {
        pairs.into_iter().collect()
    }

    #[test]
    fn test_empty() {
        assert_eq!(
            rust_group_comparison_operands(Vec::new(), map(Vec::new()), vec!["==".into()]),
            Vec::<(String, Vec<i64>)>::new()
        );
    }

    #[test]
    fn test_basic_no_group() {
        let ops = vec![
            ("==".into(), 0, 1),
            ("==".into(), 1, 2),
            ("<".into(), 2, 3),
            ("==".into(), 3, 4),
        ];
        assert_eq!(
            rust_group_comparison_operands(ops, map(Vec::new()), vec![]),
            vec![
                ("==".to_string(), vec![0, 1]),
                ("==".to_string(), vec![1, 2]),
                ("<".to_string(), vec![2, 3]),
                ("==".to_string(), vec![3, 4]),
            ]
        );
    }

    #[test]
    fn test_basic_chain() {
        let ops = vec![
            ("==".into(), 0, 1),
            ("==".into(), 1, 2),
            ("<".into(), 2, 3),
            ("==".into(), 3, 4),
        ];
        assert_eq!(
            rust_group_comparison_operands(ops, map(Vec::new()), vec!["==".into()]),
            vec![
                ("==".to_string(), vec![0, 1, 2]),
                ("<".to_string(), vec![2, 3]),
                ("==".to_string(), vec![3, 4]),
            ]
        );
    }

    #[test]
    fn test_doc_example_coalesce() {
        // same == x < y == same : operands 0 and 3 share hash id 0.
        let ops = vec![("==".into(), 0, 1), ("<".into(), 1, 2), ("==".into(), 2, 3)];
        let hashes = map(vec![(0, 0), (3, 0)]);
        assert_eq!(
            rust_group_comparison_operands(ops, hashes, vec!["==".into()]),
            vec![
                ("==".to_string(), vec![0, 1, 2, 3]),
                ("<".to_string(), vec![1, 2])
            ]
        );
    }

    #[test]
    fn test_two_groups_merged() {
        // x0==x1==x2 < x3==x4==x5, operands 0 and 5 share a hash id.
        let ops = vec![
            ("==".into(), 0, 1),
            ("==".into(), 1, 2),
            ("<".into(), 2, 3),
            ("==".into(), 3, 4),
            ("==".into(), 4, 5),
        ];
        let hashes = map(vec![(0, 10), (1, 11), (2, 12), (3, 13), (4, 14), (5, 10)]);
        assert_eq!(
            rust_group_comparison_operands(ops, hashes, vec!["==".into()]),
            vec![
                ("==".to_string(), vec![0, 1, 2, 3, 4, 5]),
                ("<".to_string(), vec![2, 3])
            ]
        );
    }
}
