//! The filesystem-probe trait shared between `fs_cache` (the Rust backing
//! for `mypy.fscache.FileSystemCache`) and `module_resolver` (the native
//! module resolver). Lives in a tiny no-pyo3 crate so the two pyo3 cdylibs
//! can both depend on it without one needing to link the other.

/// Filesystem probe interface used by the native module resolver. The
/// production implementation (`fs_cache::FsCache`) reads the real filesystem
/// through a transactional memoizing cache; the test implementation
/// (`module_resolver::HashMapFs`) is an in-memory store.
pub trait FsProbe {
    fn isfile(&self, path: &str) -> bool;
    fn isdir(&self, path: &str) -> bool;
    fn listdir(&self, path: &str) -> Vec<String>;
    fn isfile_case(&self, path: &str, prefix: &str) -> bool;
    fn exists_case(&self, path: &str, prefix: &str) -> bool;
    fn read(&self, path: &str) -> Vec<u8>;
}
