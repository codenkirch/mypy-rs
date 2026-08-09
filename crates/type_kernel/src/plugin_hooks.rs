//! Stage 12 plugin hook snapshot (plugin_hooks.rs) for Issue #93.
//!
//! Fast registry for plugin hook fullnames plus a Rust-side resolver that
//! iterates the known-plugin list and returns the first non-None hook for a
//! given fullname and hook method name.  When Rust cannot prove presence
//! (unknown fullname, user plugins present, or no hook matched) it returns
//! `None` so the Python caller falls back to the pure-Python
//! `ChainedPlugin._find_hook` chain — the strangler-fig per-call gate.

#![allow(non_local_definitions)]

use pyo3::prelude::*;
use std::collections::HashSet;

#[pyclass]
#[derive(Debug, Clone, Default)]
pub struct PluginHookRegistry {
    hooks: HashSet<String>,
}

#[allow(non_local_definitions)]
#[pymethods]
impl PluginHookRegistry {
    #[new]
    pub fn new(hooks: Vec<String>) -> Self {
        Self {
            hooks: hooks.into_iter().collect(),
        }
    }

    pub fn has_hook(&self, fullname: &str) -> bool {
        self.hooks.contains(fullname)
    }

    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }
}

/// Rust-side plugin hook resolver — ports the lookup phase of
/// `Plugin.get_*_hook` / `ChainedPlugin._find_hook` from Python onto
/// a snapshot of the builtin-only plugin list.
///
/// Returns `Some(PyObject)` when a hook was found for `callable_name` on
/// one of the plugins via the named hook method.  Returns `None` when:
///
/// * the registry does not know about `callable_name` (unknown fullname), or
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
    // 1. Is the fullname in the HashSet registry?
    let has_hook: bool = registry
        .call_method1("has_hook", (callable_name,))?
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

    #[test]
    fn test_plugin_hook_registry() {
        let reg = PluginHookRegistry::new(vec!["mypy.plugins.dataclasses.hook".to_string()]);
        assert!(reg.has_hook("mypy.plugins.dataclasses.hook"));
        assert!(!reg.has_hook("other.hook"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn test_plugin_hook_registry_empty() {
        let reg = PluginHookRegistry::new(vec![]);
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(!reg.has_hook("anything"));
    }
}
