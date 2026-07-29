//! Stage 12 plugin hook snapshot (plugin_hooks.rs) for Issue #93.
//!
//! Fast registry for plugin hook fullnames.

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
}
