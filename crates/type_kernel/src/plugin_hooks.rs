//! Stage 4/12 plugin hook snapshot (plugin_hooks.rs) for Issue #93.
//!
//! Fast registry for plugin hook fullnames plus a Rust-side resolver that
//! iterates the known-plugin list and returns the first non-None hook for a
//! given fullname and hook method name.  When Rust cannot prove presence
//! (unknown fullname, user plugins present, or no hook matched) it returns
//! `None` so the Python caller falls back to the pure-Python
//! `ChainedPlugin._find_hook` chain — the strangler-fig per-call gate.
//!
//! The registry is kind-keyed (Phase C3, issue #610): it stores per-hook-method
//! fullname sets so each dispatch site can prove absence for *its own* hook
//! kind without a default-plugin name from another kind (e.g. a class-decorator
//! name like `dataclasses.dataclass`) polluting the call-hook union and
//! forcing the Python chain on every unrelated call.

#![allow(non_local_definitions)]

use pyo3::prelude::*;
use std::collections::{HashMap, HashSet};

/// Which hook-method kinds require a name set.
///
/// `get_metaclass_hook` and `get_base_class_hook` are NOT overridden by
/// DefaultPlugin (they inherit the base no-op), so they contribute no names
/// and are not represented in the registry.  `get_attribute_hook` gates the
/// hot per-attribute-access path in checkmember.
const HOOK_KINDS: &[&str] = &[
    "get_function_hook",
    "get_function_signature_hook",
    "get_method_signature_hook",
    "get_method_hook",
    "get_attribute_hook",
    "get_class_decorator_hook",
    "get_class_decorator_hook_2",
];

#[pyclass]
#[derive(Debug, Clone, Default)]
pub struct PluginHookRegistry {
    /// fullname sets keyed by hook-method name ("get_function_hook", ...).
    hooks: HashMap<String, HashSet<String>>,
}

#[allow(non_local_definitions)]
#[pymethods]
impl PluginHookRegistry {
    /// Build a kind-keyed registry from `{hook_method_name: [fullnames...]}`.
    ///
    /// Unknown hook-method keys are ignored, so callers may pass the full
    /// DefaultPlugin surface (e.g. `DEFAULT_HOOK_FULLNAMES_BY_KIND` per
    /// kind) without tripping over keys the kernel does not classify.
    #[new]
    pub fn new(hooks: HashMap<String, Vec<String>>) -> Self {
        Self {
            hooks: hooks
                .into_iter()
                .filter(|(kind, _)| HOOK_KINDS.contains(&kind.as_str()))
                .map(|(kind, names)| (kind, names.into_iter().collect()))
                .collect(),
        }
    }

    /// True when `fullname` appears under any of the four call-hook kinds.
    ///
    /// This is the Stage 4 call-union membership test.  Kept out of
    /// `has_hook_for` so names owned by non-call kinds (attribute,
    /// class-decorator, ...) never widen the call union.
    pub fn has_call_hook(&self, fullname: &str) -> bool {
        HOOK_KINDS.iter().take(4).any(|kind| {
            self.hooks
                .get(*kind)
                .is_some_and(|names| names.contains(fullname))
        })
    }

    /// True when `fullname` appears under the named hook kind.
    pub fn has_hook_for(&self, hook_method_name: &str, fullname: &str) -> bool {
        self.hooks
            .get(hook_method_name)
            .is_some_and(|names| names.contains(fullname))
    }

    /// Backwards-compatible union membership over every known hook kind:
    /// did the DefaultPlugin declare *any* hook for `fullname`?
    pub fn has_hook(&self, fullname: &str) -> bool {
        self.hooks.values().any(|names| names.contains(fullname))
    }

    pub fn len(&self) -> usize {
        self.hooks.values().map(HashSet::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.hooks.values().all(HashSet::is_empty)
    }
}

/// Rust-side plugin hook resolver — ports the lookup phase of
/// `Plugin.get_*_hook` / `ChainedPlugin._find_hook` from Python onto
/// a snapshot of the builtin-only plugin list.
///
/// Returns `Some(PyObject)` when a hook was found for `callable_name` on
/// one of the plugins via the named hook method.  Returns `None` when:
///
/// * the registry does not know about `callable_name` for that hook kind
///   (unknown fullname), or
/// * user plugins are present (their hooks are not enumerable), or
/// * no plugin returned a non-None hook.
///
/// The Python caller treats `None` as "use the full Python chain".
#[pyfunction]
pub fn rust_resolve_plugin_hook(
    _py: Python<'_>,
    registry: &PyAny,
    callable_name: &str,
    plugin_list: &PyAny,
    hook_method_name: &str,
) -> PyResult<Option<PyObject>> {
    // 1. Is the fullname registered under this hook kind?
    let has_hook: bool = registry
        .call_method1("has_hook_for", (hook_method_name, callable_name))?
        .extract()?;
    if !has_hook {
        return Ok(None);
    }

    // 2. User plugins present — Rust cannot prove correctness, defer to Python.
    let len: usize = plugin_list.len()?;
    if len == 0 {
        return Ok(None);
    }

    // 3. Iterate the plugin list, call the named hook method on each plugin,
    //    return the first non-None result (mirrors ChainedPlugin dispatch).
    for item in plugin_list.iter()? {
        let plugin: &PyAny = item?;
        let hook_fn = plugin.getattr(hook_method_name)?;
        let result = hook_fn.call1((callable_name,))?;
        if !result.is_none() {
            return Ok(Some(result.into()));
        }
    }

    // 4. None of the plugins returned a hook.  Defer to Python for the
    //    full chain (which may include plugin overrides, subclass logic, etc.).
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn map(pairs: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        pairs
            .iter()
            .map(|(kind, names)| {
                (
                    (*kind).to_string(),
                    names.iter().map(|n| (*n).to_string()).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn test_plugin_hook_registry() {
        let reg = PluginHookRegistry::new(map(&[(
            "get_function_hook",
            &["mypy.plugins.dataclasses.hook"],
        )]));
        assert!(reg.has_hook("mypy.plugins.dataclasses.hook"));
        assert!(!reg.has_hook("other.hook"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_plugin_hook_registry_empty() {
        let reg = PluginHookRegistry::new(map(&[]));
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(!reg.has_hook("anything"));
        assert!(!reg.has_call_hook("anything"));
        assert!(!reg.has_hook_for("get_function_hook", "anything"));
    }

    #[test]
    fn test_call_hook_union_excludes_non_call_kinds() {
        // A class-decorator name must not widen the call-hook union (C3
        // regression guard: `plugin_call_hook_known_absent` reads the
        // call union only).
        let reg = PluginHookRegistry::new(map(&[
            ("get_function_hook", &["builtins.len"]),
            ("get_class_decorator_hook", &["dataclasses.dataclass"]),
        ]));
        assert!(reg.has_call_hook("builtins.len"));
        assert!(!reg.has_call_hook("dataclasses.dataclass"));
        assert!(reg.has_hook_for("get_class_decorator_hook", "dataclasses.dataclass"));
        // union predicate still sees it
        assert!(reg.has_hook("dataclasses.dataclass"));
    }

    #[test]
    fn test_unknown_hook_kind_ignored() {
        let reg = PluginHookRegistry::new(map(&[
            ("get_function_hook", &["builtins.len"]),
            ("get_something_else", &["should.not.count"]),
        ]));
        assert_eq!(reg.len(), 1);
        assert!(reg.has_call_hook("builtins.len"));
        assert!(!reg.has_hook_for("get_something_else", "should.not.count"));
    }
}
