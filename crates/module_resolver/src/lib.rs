//! Native module-resolution core for mypy.
//!
//! This is the Rust port of the pure path-resolution core of
//! `mypy.modulefinder.FindModuleCache._find_module` plus its helpers
//! (`_find_module_non_stub_helper`, `_update_ns_ancestors`,
//! `find_lib_path_dirs`, `get_toplevel_possibilities`, `verify_module`,
//! `highest_init_level`). Policy, diagnostics, result caching, and the
//! WRONG_WORKING_DIRECTORY decoration stay in Python; this crate only
//! resolves a module id to a path or a `ModuleNotFoundReason`.
//!
//! Filesystem access goes through the `FsProbe` trait, which has two
//! implementations:
//!   * `StdFs` — reads the real filesystem via `std::fs` directly, with no
//!     Python callbacks across the PyO3 boundary. This is the production
//!     path: the native resolver owns the filesystem for cold runs, which is
//!     the direction the strangler-fig migration is heading (pure Rust,
//!     no Python runtime). The dispatch gate in `FindModuleCache._resolve`
//!     keeps daemon (`fine_grained_incremental`) and Bazel runs on the
//!     Python `_find_module` path, so Rust only ever sees real files.
//!     Case-sensitive matching on macOS/Windows is replicated via
//!     `read_dir` listing checks, mirroring `FileSystemCache.isfile_case`
//!     and `exists_case`.
//!   * `HashMapFs` — an in-memory store used by the Rust unit tests.

mod fs_cache;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use fs_probe::FsProbe;
use pyo3::prelude::*;

pub use fs_cache::FsCache;

// ---------------------------------------------------------------------------
// Result encoding
// ---------------------------------------------------------------------------

// Mirrors mypy.modulefinder.ModuleNotFoundReason values.
const REASON_NOT_FOUND: u8 = 0;
const REASON_FOUND_WITHOUT_TYPE_HINTS: u8 = 1;
/// Unused at the Rust layer: Python decorates NOT_FOUND -> WRONG_WORKING_DIRECTORY.
#[allow(dead_code)]
const REASON_WRONG_WORKING_DIRECTORY: u8 = 2;
const REASON_APPROVED_STUBS_NOT_INSTALLED: u8 = 3;
// Sentinel meaning "found, path is in the Option<String>".
const FOUND: u8 = 255;

const PYTHON_EXTENSIONS: &[&str] = &[".pyi", ".py"];

// ---------------------------------------------------------------------------
// Dependency-record extraction (mirrors mypy.build.all_imported_modules_in_file)
// ---------------------------------------------------------------------------

// Import-priority constants. Mirrors mypy.build.PRI_*.
const PRI_HIGH: i32 = 5; // top-level "from X import blah"
const PRI_MED: i32 = 10; // top-level "import X"
const PRI_LOW: i32 = 20; // either form inside a function
const PRI_MYPY: i32 = 25; // inside "if MYPY" or "if TYPE_CHECKING"

// ImportRecord kinds matching the Python ImportBase subclasses.
const IMP_IMPORT: u8 = 0;
const IMP_IMPORTFROM: u8 = 1;
const IMP_IMPORTALL: u8 = 2;

/// A single import statement, in the plain-record shape Python passes across
/// the PyO3 boundary. Mirrors the `Import`/`ImportFrom`/`ImportAll` nodes'
/// dependency-relevant fields (see `mypy/nodes.py:621-708`).
///
/// PyO3's `FromPyObject` for a tuple struct extracts positionally from a
/// Python sequence (tuple), which matches how `_import_to_record` builds the
/// record on the Python side.
#[derive(FromPyObject)]
struct ImportRecord(
    u8,                            // kind: IMP_IMPORT | IMP_IMPORTFROM | IMP_IMPORTALL
    String,                        // module
    i32,                           // relative
    Vec<(String, Option<String>)>, // ids / names
    i32,                           // line
    bool,                          // is_top_level
    bool,                          // is_unreachable
    bool,                          // is_unreachable_dependency
    bool,                          // is_mypy_only
);

/// `import_priority` from `mypy/build.py:623-632`.
fn import_priority(is_top_level: bool, is_mypy_only: bool, toplevel_priority: i32) -> i32 {
    if !is_top_level {
        PRI_LOW
    } else if is_mypy_only {
        std::cmp::max(PRI_MYPY, toplevel_priority)
    } else {
        toplevel_priority
    }
}

/// `correct_rel_imp` from `mypy/build.py:1182-1200`. Returns `None` when the
/// relative import resolves to an empty id (a blocking error Python reports).
fn correct_rel_imp(file_id: &str, file_path: &str, module: &str, relative: i32) -> Option<String> {
    if relative == 0 {
        return Some(module.to_string());
    }
    let mut rel = relative;
    if Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with("__init__."))
        .unwrap_or(false)
    {
        rel -= 1;
    }
    let mut new_id = file_id.to_string();
    if rel != 0 {
        let parts: Vec<&str> = new_id.split('.').collect();
        let take = parts.len().saturating_sub(rel as usize);
        new_id = parts[..take].join(".");
    }
    if !module.is_empty() {
        if new_id.is_empty() {
            new_id = module.to_string();
        } else {
            new_id = format!("{}.{}", new_id, module);
        }
    }
    if new_id.is_empty() {
        None
    } else {
        Some(new_id)
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Mirror mypy.util.os_path_join on linux/darwin: str-only, 2 args, `/`-based.
/// All resolver inputs are already absolute (SearchPaths.__init__ runs
/// os.path.abspath), so this is sufficient and matches byte-for-byte.
fn join_path(path: &str, b: &str) -> String {
    if b.is_empty() {
        return path.to_string();
    }
    if b.starts_with('/') {
        return b.to_string();
    }
    if path.is_empty() {
        return b.to_string();
    }
    if path.ends_with('/') {
        format!("{}{}", path, b)
    } else {
        format!("{}/{}", path, b)
    }
}

/// Mirror os.path.join(a, b, c, ...) over a slice of components.
fn join_path_many(base: &str, components: &[String]) -> String {
    let mut acc = base.to_string();
    for c in components {
        acc = join_path(&acc, c);
    }
    acc
}

/// Mirror os.path.dirname: strip the last path component. Works on `/`-joined
/// strings; a trailing slash is collapsed first.
fn dirname(path: &str) -> String {
    // Mirror CPython os.path.dirname semantics on POSIX.
    let bytes = path.as_bytes();
    // Strip trailing slashes (except a single leading slash).
    let mut end = bytes.len();
    while end > 1 && bytes[end - 1] == b'/' {
        end -= 1;
    }
    if end == 0 {
        return String::new();
    }
    if let Some(idx) = bytes[..end].iter().rposition(|&b| b == b'/') {
        if idx == 0 {
            // root
            "/".to_string()
        } else {
            String::from_utf8_lossy(&bytes[..idx]).to_string()
        }
    } else {
        // no slash → empty dirname
        String::new()
    }
}

/// Mirror os.path.basename: strip directory prefix.
fn basename(path: &str) -> String {
    let bytes = path.as_bytes();
    let mut end = bytes.len();
    while end > 1 && bytes[end - 1] == b'/' {
        end -= 1;
    }
    if end == 0 {
        return String::new();
    }
    if let Some(idx) = bytes[..end].iter().rposition(|&b| b == b'/') {
        String::from_utf8_lossy(&bytes[idx + 1..end]).to_string()
    } else {
        String::from_utf8_lossy(&bytes[..end]).to_string()
    }
}

fn is_init_file(path: &str) -> bool {
    matches!(basename(path).as_str(), "__init__.py" | "__init__.pyi")
}

fn split_dot(id: &str) -> Vec<String> {
    id.split('.').map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// In-memory FsProbe for unit tests
// ---------------------------------------------------------------------------
// In-memory FsProbe for unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
struct HashMapFs {
    files: HashMap<String, Vec<u8>>,
    dirs: BTreeSet<String>,
}

#[cfg(test)]
impl HashMapFs {
    fn new() -> Self {
        HashMapFs {
            files: HashMap::new(),
            dirs: BTreeSet::new(),
        }
    }
    fn file(mut self, path: &str, content: &str) -> Self {
        self.files
            .insert(path.to_string(), content.as_bytes().to_vec());
        // Auto-register parent dirs.
        let mut acc = String::new();
        for component in path.split('/').skip(1) {
            if acc.is_empty() {
                acc = format!("/{}", component);
            } else {
                acc = format!("{}/{}", acc, component);
            }
            if acc != path {
                self.dirs.insert(acc.clone());
            }
        }
        self
    }
    fn dir(mut self, path: &str) -> Self {
        self.dirs.insert(path.to_string());
        self
    }
}

#[cfg(test)]
impl FsProbe for HashMapFs {
    fn isfile(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }
    fn isdir(&self, path: &str) -> bool {
        self.dirs.contains(path)
    }
    fn listdir(&self, path: &str) -> Vec<String> {
        let prefix = if path.ends_with('/') {
            path.to_string()
        } else {
            format!("{}/", path)
        };
        let mut out: Vec<String> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for f in self.files.keys() {
            if let Some(rest) = f.strip_prefix(&prefix) {
                if let Some(name) = rest.split('/').next() {
                    if !seen.contains(name) {
                        seen.insert(name.to_string());
                        out.push(name.to_string());
                    }
                }
            }
        }
        for d in &self.dirs {
            if let Some(rest) = d.strip_prefix(&prefix) {
                if let Some(name) = rest.split('/').next() {
                    if !seen.contains(name) {
                        seen.insert(name.to_string());
                        out.push(name.to_string());
                    }
                }
            }
        }
        out
    }
    // Tests use case-sensitive paths, so the case variants just delegate.
    fn isfile_case(&self, path: &str, _prefix: &str) -> bool {
        self.isfile(path)
    }
    fn exists_case(&self, path: &str, _prefix: &str) -> bool {
        self.isfile(path) || self.isdir(path)
    }
    fn read(&self, path: &str) -> Vec<u8> {
        self.files.get(path).cloned().unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// NativeResolver — production resolver backed by a shared FsCache
// ---------------------------------------------------------------------------

/// Native module resolver: owns the stubinfo tables, search paths, resolver
/// config, and the cross-call resolution caches for the lifetime of a
/// `FindModuleCache`.
///
/// Exposed to Python as `NativeResolver`. Constructed once when the dispatch
/// gate in `FindModuleCache._resolve` first routes to Rust, then reused for
/// every subsequent `find_module` call on that cache.
///
/// Filesystem access is **not** owned here: the resolver reads through a
/// shared `FsCache` (`Py<FsCache>`) that is owned by the same
/// `FileSystemCache` Python delegate the rest of mypy uses. This eliminates
/// the dual-cache hazard that previously forced `_native_gate_active` to
/// exclude daemon (`fine_grained_incremental`) and Bazel modes: there is now
/// exactly one FS cache per transaction, so the resolver and every other
/// fscache caller see the same snapshot. `fscache.flush()` clears the FS
/// caches; `NativeResolver::flush()` clears the resolution caches
/// (`initial_components`, `ns_ancestors`) that are derived from `listdir`
/// and must not outlive a transaction.
#[pyclass(name = "NativeResolver")]
struct NativeResolver {
    /// Shared transactional FS cache. Borrowed per call via `as_ref(py)` so
    /// the resolver reads through the same snapshot as the rest of mypy.
    fs_cache: Py<FsCache>,
    // --- Resolver config (stable for the lifetime of the FindModuleCache) ---
    namespace_packages: bool,
    use_builtins_fixtures: bool,
    python_path: Vec<String>,
    mypy_path: Vec<String>,
    package_path: Vec<String>,
    typeshed_path: Vec<String>,
    /// (module_name, min_version, max_version) for stdlib version gating.
    /// Used by `use_typeshed_for` in the dependency-record walk so Rust's
    /// `is_module` mirrors Python's `find_module`/`_typeshed_has_version`:
    /// a stdlib module outside the target version range is NOT looked up in
    /// typeshed, so it resolves as NOT_FOUND (matching Python).
    stdlib_versions: HashMap<String, ((u8, u8), Option<(u8, u8)>)>,
    /// Clamped Python target version (`max(python_version, (3, 10))`),
    /// mirroring `typeshed_py_version`. Used for stdlib version gating in
    /// `use_typeshed_for`.
    python_version: (u8, u8),
    /// Top-level module names with a flat stub-distribution lookup.
    stub_flat: BTreeSet<String>,
    /// Namespace-lookup stub distributions: module_name -> dist_name.
    stub_namespace: BTreeMap<String, String>,
    // --- Cross-call resolution caches (mirror FindModuleCache fields) ---
    /// Cache for get_toplevel_possibilities: lib_path tuple key ->
    /// toplevel component -> list of dirs. Persists across find_module calls,
    /// mirroring FindModuleCache.initial_components.
    initial_components: RefCell<HashMap<Vec<String>, HashMap<String, Vec<String>>>>,
    /// namespace-package ancestor paths (pkg_id -> path). Persists across
    /// find_module calls, mirroring FindModuleCache.ns_ancestors.
    ns_ancestors: RefCell<HashMap<String, String>>,
}

#[pymethods]
impl NativeResolver {
    /// Construct a `NativeResolver` with all stable config. Called once by
    /// `FindModuleCache._resolve` on first native dispatch; the returned
    /// object is reused for all subsequent `find_module` calls on that cache.
    ///
    /// `fs_cache` is the shared `FsCache` pyclass owned by the Python
    /// `FileSystemCache` delegate (`fscache._rust`). The resolver borrows it
    /// per call so it reads through the same transactional snapshot as the
    /// rest of mypy — eliminating the dual-cache hazard.
    #[new]
    #[pyo3(signature = (
        fs_cache,
        namespace_packages,
        use_builtins_fixtures,
        python_path,
        mypy_path,
        package_path,
        typeshed_path,
        python_version,
        stdlib_versions,
        stub_flat,
        stub_namespace,
    ))]
    fn new(
        fs_cache: Py<FsCache>,
        namespace_packages: bool,
        use_builtins_fixtures: bool,
        python_path: Vec<String>,
        mypy_path: Vec<String>,
        package_path: Vec<String>,
        typeshed_path: Vec<String>,
        python_version: (u8, u8),
        stdlib_versions: Vec<(String, (u8, u8), Option<(u8, u8)>)>,
        stub_flat: Vec<String>,
        stub_namespace: Vec<(String, String)>,
    ) -> Self {
        // Clamp to (3, 10) minimum, mirroring typeshed_py_version.
        let python_version = std::cmp::max(python_version, (3, 10));
        NativeResolver {
            fs_cache,
            namespace_packages,
            use_builtins_fixtures,
            python_path,
            mypy_path,
            package_path,
            typeshed_path,
            stdlib_versions: stdlib_versions
                .into_iter()
                .map(|(name, lo, hi)| (name, (lo, hi)))
                .collect(),
            python_version,
            stub_flat: stub_flat.into_iter().collect(),
            stub_namespace: stub_namespace.into_iter().collect(),
            initial_components: RefCell::new(HashMap::new()),
            ns_ancestors: RefCell::new(HashMap::new()),
        }
    }

    /// Clear the cross-call resolution caches (`initial_components`,
    /// `ns_ancestors`). These are derived from `listdir` and must not outlive
    /// an FS transaction; `FindModuleCache.clear()` calls this on every
    /// fine-grained increment so the resolver doesn't read stale toplevel
    /// listings after `fscache.flush()`.
    ///
    /// The shared `FsCache`'s own caches are flushed separately by
    /// `FileSystemCache.flush()`; this method only touches the
    /// resolver-private caches.
    fn flush(&self) {
        self.initial_components.borrow_mut().clear();
        self.ns_ancestors.borrow_mut().clear();
    }

    /// Resolve a module id to a path or a `ModuleNotFoundReason`.
    ///
    /// Returns `(kind, path, can_cache)`: `kind == 255` means found (path is
    /// `Some`); otherwise `kind` is a `ModuleNotFoundReason` value and path
    /// is `None`.
    ///
    /// Only the per-call varying args cross the PyO3 boundary here: `id`,
    /// `use_typeshed`, and `follow_untyped_imports`. All stable config (search
    /// paths, stub tables, resolver flags) was set at construction time.
    fn resolve<'py>(
        &self,
        py: Python<'py>,
        id: &str,
        use_typeshed: bool,
        follow_untyped_imports: bool,
    ) -> PyResult<PyObject> {
        let fs = self.fs_cache.borrow(py);
        let inputs = ResolveInputs {
            id,
            use_typeshed,
            namespace_packages: self.namespace_packages,
            use_builtins_fixtures: self.use_builtins_fixtures,
            follow_untyped_imports,
            python_path: &self.python_path,
            mypy_path: &self.mypy_path,
            package_path: &self.package_path,
            typeshed_path: &self.typeshed_path,
            stdlib_versions: &self.stdlib_versions,
            stub_flat: &self.stub_flat,
            stub_namespace: &self.stub_namespace,
            fs: &*fs,
        };
        let mut initial_components = self.initial_components.borrow_mut();
        let mut ns_ancestors = self.ns_ancestors.borrow_mut();
        let mut resolver = Resolver::new(&inputs, &mut initial_components, &mut ns_ancestors);
        let (kind, path, can_cache) = resolver.find_module(id, use_typeshed);
        let path_obj: PyObject = match path {
            Some(s) => s.into_py(py),
            None => py.None(),
        };
        Ok((kind, path_obj, can_cache).into_py(py))
    }

    /// Compute dependency records for a module's imports.
    ///
    /// Mirrors `BuildManager.all_imported_modules_in_file`
    /// (`mypy/build.py:1202-1262`): walks the import list, computes
    /// priorities (`import_priority`), corrects relative imports
    /// (`correct_rel_imp`), expands ancestor packages for `Import`,
    /// discriminates submodule-vs-name for `ImportFrom`, and checks module
    /// existence via `is_module` (the `known_modules` set first, then this
    /// resolver's `resolve()`).
    ///
    /// Returns `(records, error)` where `records` is a list of
    /// `(priority, module_id, line)` tuples and `error` is an optional
    /// `(line, message)` for the blocking "No parent module" relative-import
    /// error that Python reports via `Errors`.
    ///
    /// Only the per-call varying args cross the boundary here: the import
    /// list (already deserialized by Python) and the known-modules set
    /// (rebuilt per call from `manager.modules` + `source_set.source_modules`).
    fn compute_dep_records(
        &self,
        py: Python<'_>,
        file_id: &str,
        file_path: &str,
        imports: Vec<ImportRecord>,
        known_modules: HashSet<String>,
    ) -> PyResult<(PyObject, PyObject)> {
        let fs = self.fs_cache.borrow(py);
        let (res, error) = dep_records_with(
            &*fs,
            file_id,
            file_path,
            &imports,
            &known_modules,
            &self.initial_components,
            &self.ns_ancestors,
            &self.python_path,
            &self.mypy_path,
            &self.package_path,
            &self.typeshed_path,
            &self.stdlib_versions,
            self.python_version,
            &self.stub_flat,
            &self.stub_namespace,
            self.namespace_packages,
            self.use_builtins_fixtures,
        )?;
        Ok((res.into_py(py), error.into_py(py)))
    }

    /// Resolve a batch of module ids in one PyO3 call.
    ///
    /// Mirrors `resolve` per id but amortizes the boundary cost: one crossing
    /// resolves N ids, returning one `Vec<(kind, path, can_cache)>`. `kind ==
    /// 255` (FOUND) means `path` is `Some`; otherwise it's `None` and `kind`
    /// is a `ModuleNotFoundReason` value.
    ///
    /// `use_typeshed` is computed inside Rust via `use_typeshed_for` (matching
    /// the dep-records walk, not the per-call `resolve` path), so each id is a
    /// bare `(module_id, follow_untyped_imports)` pair — no Python-side
    /// per-id work, which is the whole point of batching.
    fn resolve_many(
        &self,
        py: Python<'_>,
        ids_with_follow: Vec<(String, bool)>,
    ) -> PyResult<Vec<(u8, Option<String>, bool)>> {
        let fs = self.fs_cache.borrow(py);
        let res = resolve_many_with(
            &*fs,
            &ids_with_follow,
            &self.initial_components,
            &self.ns_ancestors,
            &self.python_path,
            &self.mypy_path,
            &self.package_path,
            &self.typeshed_path,
            &self.stdlib_versions,
            self.python_version,
            &self.stub_flat,
            &self.stub_namespace,
            self.namespace_packages,
            self.use_builtins_fixtures,
        )?;
        Ok(res)
    }
}

/// Batched resolution core, generic over the `FsProbe` implementation so it can
/// be unit-tested with `HashMapFs` (mirroring `dep_records_with`). The resolver
/// config is passed in so each id resolves against the same config as a real
/// `find_module` call. `initial_components` and `ns_ancestors` are `RefCell`s
/// so the caller controls cross-call cache lifetime — a single
/// `resolve_many_with` call shares one cache borrow across all ids, exactly as
/// `dep_records_with` does across one file's import list.
#[allow(clippy::too_many_arguments)]
fn resolve_many_with<F: FsProbe>(
    fs: &F,
    ids_with_follow: &[(String, bool)],
    initial_components: &RefCell<HashMap<Vec<String>, HashMap<String, Vec<String>>>>,
    ns_ancestors: &RefCell<HashMap<String, String>>,
    python_path: &[String],
    mypy_path: &[String],
    package_path: &[String],
    typeshed_path: &[String],
    stdlib_versions: &HashMap<String, ((u8, u8), Option<(u8, u8)>)>,
    python_version: (u8, u8),
    stub_flat: &BTreeSet<String>,
    stub_namespace: &BTreeMap<String, String>,
    namespace_packages: bool,
    use_builtins_fixtures: bool,
) -> PyResult<Vec<(u8, Option<String>, bool)>> {
    let mut out: Vec<(u8, Option<String>, bool)> = Vec::with_capacity(ids_with_follow.len());
    let mut initial_components = initial_components.borrow_mut();
    let mut ns_ancestors = ns_ancestors.borrow_mut();

    for (id, follow_untyped_imports) in ids_with_follow {
        let use_typeshed = use_typeshed_for(id, python_version, stdlib_versions);
        let inputs = ResolveInputs {
            id,
            use_typeshed,
            namespace_packages,
            use_builtins_fixtures,
            follow_untyped_imports: *follow_untyped_imports,
            python_path,
            mypy_path,
            package_path,
            typeshed_path,
            stdlib_versions,
            stub_flat,
            stub_namespace,
            fs: &*fs,
        };
        let mut resolver = Resolver::new(&inputs, &mut initial_components, &mut ns_ancestors);
        let (kind, path, can_cache) = resolver.find_module(id, use_typeshed);
        out.push((kind, path, can_cache));
    }

    Ok(out)
}
/// it can be unit-tested with `HashMapFs`. The resolver config (search paths,
/// stub tables, flags) is passed in so `is_module` lookups use the same config
/// as a real `find_module` call. `initial_components` and `ns_ancestors` are
/// `RefCell`s so the caller controls cross-call cache lifetime.
fn dep_records_with<F: FsProbe>(
    fs: &F,
    file_id: &str,
    file_path: &str,
    imports: &[ImportRecord],
    known_modules: &HashSet<String>,
    initial_components: &RefCell<HashMap<Vec<String>, HashMap<String, Vec<String>>>>,
    ns_ancestors: &RefCell<HashMap<String, String>>,
    python_path: &[String],
    mypy_path: &[String],
    package_path: &[String],
    typeshed_path: &[String],
    stdlib_versions: &HashMap<String, ((u8, u8), Option<(u8, u8)>)>,
    python_version: (u8, u8),
    stub_flat: &BTreeSet<String>,
    stub_namespace: &BTreeMap<String, String>,
    namespace_packages: bool,
    use_builtins_fixtures: bool,
) -> PyResult<(Vec<(i32, String, i32)>, Option<(i32, String)>)> {
    let mut res: Vec<(i32, String, i32)> = Vec::new();
    let mut error: Option<(i32, String)> = None;

    for imp in imports {
        // Destructure the tuple struct into named fields for readability.
        let (
            kind,
            module,
            relative,
            ids,
            line,
            is_top_level,
            is_unreachable,
            is_unreachable_dependency,
            is_mypy_only,
        ) = (
            imp.0, &imp.1, imp.2, &imp.3, imp.4, imp.5, imp.6, imp.7, imp.8,
        );

        if is_unreachable && !is_unreachable_dependency {
            continue;
        }
        let include_only_if_resolvable = is_unreachable_dependency;

        match kind {
            IMP_IMPORT => {
                let pri = import_priority(is_top_level, is_mypy_only, PRI_MED);
                let ancestor_pri = import_priority(is_top_level, is_mypy_only, PRI_LOW);
                for (id, _asname) in ids {
                    if include_only_if_resolvable
                        && !is_module_inline(
                            fs,
                            id,
                            known_modules,
                            initial_components,
                            ns_ancestors,
                            python_path,
                            mypy_path,
                            package_path,
                            typeshed_path,
                            stdlib_versions,
                            python_version,
                            stub_flat,
                            stub_namespace,
                            namespace_packages,
                            use_builtins_fixtures,
                        )?
                    {
                        continue;
                    }
                    res.push((pri, id.clone(), line));
                    // Expand ancestor packages (mirrors build.py:1222-1226):
                    // `id.split(".")[:-1]` — all components except the last.
                    let parts: Vec<&str> = id.split('.').collect();
                    let mut ancestors: Vec<String> = Vec::new();
                    for part in &parts[..parts.len().saturating_sub(1)] {
                        ancestors.push(part.to_string());
                        res.push((ancestor_pri, ancestors.join("."), line));
                    }
                }
            }
            IMP_IMPORTFROM => {
                let cur_id = match correct_rel_imp(file_id, file_path, module, relative) {
                    Some(id) => id,
                    None => {
                        error = Some((
                            line,
                            "No parent module -- cannot perform relative import".to_string(),
                        ));
                        return Ok((res, error));
                    }
                };
                if include_only_if_resolvable
                    && !is_module_inline(
                        fs,
                        &cur_id,
                        known_modules,
                        initial_components,
                        ns_ancestors,
                        python_path,
                        mypy_path,
                        package_path,
                        typeshed_path,
                        stdlib_versions,
                        python_version,
                        stub_flat,
                        stub_namespace,
                        namespace_packages,
                        use_builtins_fixtures,
                    )?
                {
                    continue;
                }
                let mut all_are_submodules = true;
                let pri_sub = import_priority(is_top_level, is_mypy_only, PRI_MED);
                for (name, _asname) in ids {
                    let sub_id = format!("{}.{}", cur_id, name);
                    if is_module_inline(
                        fs,
                        &sub_id,
                        known_modules,
                        initial_components,
                        ns_ancestors,
                        python_path,
                        mypy_path,
                        package_path,
                        typeshed_path,
                        stdlib_versions,
                        python_version,
                        stub_flat,
                        stub_namespace,
                        namespace_packages,
                        use_builtins_fixtures,
                    )? {
                        res.push((pri_sub, sub_id, line));
                    } else {
                        all_are_submodules = false;
                    }
                }
                // Workaround for cycle-handling bugs (#4498): if all imported
                // names are submodules, import the parent at a lower priority
                // (mirrors build.py:1246).
                let pri = import_priority(
                    is_top_level,
                    is_mypy_only,
                    if all_are_submodules {
                        PRI_LOW
                    } else {
                        PRI_HIGH
                    },
                );
                res.push((pri, cur_id, line));
            }
            IMP_IMPORTALL => {
                let cur_id = match correct_rel_imp(file_id, file_path, module, relative) {
                    Some(id) => id,
                    None => {
                        error = Some((
                            line,
                            "No parent module -- cannot perform relative import".to_string(),
                        ));
                        return Ok((res, error));
                    }
                };
                if include_only_if_resolvable
                    && !is_module_inline(
                        fs,
                        &cur_id,
                        known_modules,
                        initial_components,
                        ns_ancestors,
                        python_path,
                        mypy_path,
                        package_path,
                        typeshed_path,
                        stdlib_versions,
                        python_version,
                        stub_flat,
                        stub_namespace,
                        namespace_packages,
                        use_builtins_fixtures,
                    )?
                {
                    continue;
                }
                let pri = import_priority(is_top_level, is_mypy_only, PRI_HIGH);
                res.push((pri, cur_id, line));
            }
            _ => {}
        }
    }

    // Sort by descending dot count so modules come before their ancestors
    // (mirrors build.py:1261). This primes FindModuleCache.ns_ancestors.
    res.sort_by(|a, b| b.1.matches('.').count().cmp(&a.1.matches('.').count()));

    Ok((res, error))
}

/// `is_module` from `mypy/build.py:1264-1278`, generic over `FsProbe`. Checks
/// the known-modules set first (the fast path), then resolves on disk via the
/// shared `Resolver` caches.
///
/// `use_typeshed` is computed here (not Python-side) because `is_module` is
/// called from inside the Rust dependency walk — computing it Python-side would
/// require a per-id callback across the PyO3 boundary, defeating the purpose of
/// running the walk in Rust. The computation mirrors
/// `FindModuleCache.find_module` + `_typeshed_has_version` exactly.
#[allow(clippy::too_many_arguments)]
fn is_module_inline<F: FsProbe>(
    fs: &F,
    id: &str,
    known_modules: &HashSet<String>,
    initial_components: &RefCell<HashMap<Vec<String>, HashMap<String, Vec<String>>>>,
    ns_ancestors: &RefCell<HashMap<String, String>>,
    python_path: &[String],
    mypy_path: &[String],
    package_path: &[String],
    typeshed_path: &[String],
    stdlib_versions: &HashMap<String, ((u8, u8), Option<(u8, u8)>)>,
    python_version: (u8, u8),
    stub_flat: &BTreeSet<String>,
    stub_namespace: &BTreeMap<String, String>,
    namespace_packages: bool,
    use_builtins_fixtures: bool,
) -> PyResult<bool> {
    if known_modules.contains(id) {
        return Ok(true);
    }
    let use_typeshed = use_typeshed_for(id, python_version, stdlib_versions);
    let inputs = ResolveInputs {
        id,
        use_typeshed,
        namespace_packages,
        use_builtins_fixtures,
        follow_untyped_imports: false,
        python_path,
        mypy_path,
        package_path,
        typeshed_path,
        stdlib_versions,
        stub_flat,
        stub_namespace,
        fs,
    };
    let mut ic = initial_components.borrow_mut();
    let mut ns = ns_ancestors.borrow_mut();
    let mut resolver = Resolver::new(&inputs, &mut ic, &mut ns);
    let (kind, _path, _can_cache) = resolver.find_module(id, use_typeshed);
    Ok(kind == FOUND)
}

/// Mirror `FindModuleCache.find_module`'s `use_typeshed` computation
/// (`mypy/modulefinder.py:343-349`) + `_typeshed_has_version`
/// (`mypy/modulefinder.py:426-431`).
///
/// A stdlib module outside the target Python version range is NOT looked up in
/// typeshed. This is what makes `import tomllib` (added in 3.11) resolve as
/// NOT_FOUND when targeting 3.10, so the dependency walk skips it via
/// `include_only_if_resolvable`.
fn use_typeshed_for(
    id: &str,
    python_version: (u8, u8),
    stdlib_versions: &HashMap<String, ((u8, u8), Option<(u8, u8)>)>,
) -> bool {
    let top_level = id.split('.').next().unwrap_or(id);
    let key = if stdlib_versions.contains_key(id) {
        id
    } else if stdlib_versions.contains_key(top_level) {
        top_level
    } else {
        // Not a known stdlib module → search typeshed.
        return true;
    };
    let (min_version, max_version) = &stdlib_versions[key];
    // python_version is already clamped to (3, 10) at construction time.
    python_version >= *min_version && max_version.map_or(true, |max| python_version <= max)
}

// ---------------------------------------------------------------------------
// Resolution core
// ---------------------------------------------------------------------------

/// Inputs to a single resolution call. Built once per `find_module` invocation
/// from the `FindModuleCache` state; all the policy decisions (use_typeshed,
/// follow_untyped_imports) are computed Python-side and passed in as plain
/// values so Rust never touches `Options`.
struct ResolveInputs<'a, F: FsProbe> {
    /// Module id being resolved. (Stored for debugging; the resolver passes
    /// `id` directly to `find_module`.)
    #[allow(dead_code)]
    id: &'a str,
    /// Whether to search typeshed. (Currently unused at the Rust layer
    /// because `use_typeshed` is passed through to `find_module` directly;
    /// kept in the struct for future fscache-snapshot strategies.)
    #[allow(dead_code)]
    use_typeshed: bool,
    namespace_packages: bool,
    use_builtins_fixtures: bool,
    follow_untyped_imports: bool,
    python_path: &'a [String],
    mypy_path: &'a [String],
    package_path: &'a [String],
    typeshed_path: &'a [String],
    /// (module_name, (min_version, max_version)) for stdlib version gating.
    /// Unused by `Resolver::find_module` itself (which receives `use_typeshed`
    /// pre-computed), but referenced by `is_module_inline` via
    /// `use_typeshed_for`. Kept in the struct so `resolve()` and the
    /// dependency walk share one input bundle.
    #[allow(dead_code)]
    stdlib_versions: &'a HashMap<String, ((u8, u8), Option<(u8, u8)>)>,
    /// Top-level module names with a flat stub-distribution lookup
    /// (mirrors mypy.stubinfo.non_bundled_packages_flat keys + legacy bundled).
    stub_flat: &'a BTreeSet<String>,
    /// Namespace-lookup stub distributions: (module_name, dist_name).
    /// Mirrors mypy.stubinfo.non_bundled_packages_namespace flattened.
    stub_namespace: &'a BTreeMap<String, String>,
    fs: &'a F,
}

/// Mutable resolution state for a single `find_module` call. The
/// `initial_components` and `ns_ancestors` caches are borrowed from the
/// long-lived `NativeResolver` (or a fresh HashMap for tests) so the
/// cross-call memoization persists, mirroring `FindModuleCache`'s
/// `initial_components` and `ns_ancestors` fields.
struct Resolver<'a, F: FsProbe> {
    inp: &'a ResolveInputs<'a, F>,
    /// Cache for get_toplevel_possibilities: lib_path tuple key ->
    /// toplevel component -> list of dirs. Borrowed from the owner
    /// (NativeResolver for production, local HashMap for tests).
    initial_components: &'a mut HashMap<Vec<String>, HashMap<String, Vec<String>>>,
    /// namespace-package ancestor paths (pkg_id -> path). Borrowed from
    /// the owner.
    ns_ancestors: &'a mut HashMap<String, String>,
}

/// Result of a resolution: (kind, optional path, can_cache).
/// `kind == FOUND` means "found, path is Some".
type ResolveResult = (u8, Option<String>, bool);

impl<'a, F: FsProbe> Resolver<'a, F> {
    fn new(
        inp: &'a ResolveInputs<'a, F>,
        initial_components: &'a mut HashMap<Vec<String>, HashMap<String, Vec<String>>>,
        ns_ancestors: &'a mut HashMap<String, String>,
    ) -> Self {
        Resolver {
            inp,
            initial_components,
            ns_ancestors,
        }
    }

    // Mirrors FindModuleCache.find_lib_path_dirs.
    fn find_lib_path_dirs(&mut self, id: &str, lib_path: &[String]) -> Vec<(String, bool)> {
        let components = split_dot(id);
        let dir_chain: String = components[..components.len() - 1]
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("/");
        let toplevel = self.get_toplevel_possibilities(lib_path, &components[0]);
        let mut dirs = Vec::new();
        for pathitem in toplevel {
            let dir = if dir_chain.is_empty() {
                pathitem.clone()
            } else {
                join_path(&pathitem, &dir_chain)
            };
            if self.inp.fs.isdir(&dir) {
                dirs.push((dir, true));
            }
        }
        dirs
    }

    // Mirrors FindModuleCache.get_toplevel_possibilities.
    fn get_toplevel_possibilities(&mut self, lib_path: &[String], id: &str) -> Vec<String> {
        let key: Vec<String> = lib_path.to_vec();
        if !self.initial_components.contains_key(&key) {
            let mut components: HashMap<String, Vec<String>> = HashMap::new();
            for dir in lib_path {
                let contents = self.inp.fs.listdir(dir);
                for name in contents {
                    let stem = match name.rfind('.') {
                        Some(idx) => &name[..idx],
                        None => &name[..],
                    };
                    components
                        .entry(stem.to_string())
                        .or_default()
                        .push(dir.clone());
                }
            }
            self.initial_components.insert(key.clone(), components);
        }
        self.initial_components
            .get(&key)
            .and_then(|m| m.get(id))
            .cloned()
            .unwrap_or_default()
    }

    // Mirrors FindModuleCache._find_module_non_stub_helper.
    fn find_module_non_stub_helper(&self, id: &str, pkg_dir: &str) -> Result<(String, bool), u8> {
        let mut plausible_match = false;
        let mut dir_path = pkg_dir.to_string();
        let components = split_dot(id);
        let last_idx = components.len() - 1;
        for (index, component) in components.iter().enumerate() {
            dir_path = join_path(&dir_path, component);
            let typed_marker = join_path(&dir_path, "py.typed");
            if self.inp.fs.isfile(&typed_marker) {
                let parent_components = &components[..components.len() - 1];
                let path = join_path_many(pkg_dir, parent_components);
                return Ok((path, index == 0));
            }
            let as_py = format!("{}.py", dir_path);
            if !plausible_match && (self.inp.fs.isdir(&dir_path) || self.inp.fs.isfile(&as_py)) {
                plausible_match = true;
            }
            if !self.inp.fs.isdir(&dir_path) {
                break;
            }
            let _ = last_idx; // silence unused; index is used in the return above
        }
        if plausible_match {
            if self.inp.follow_untyped_imports {
                let parent_components = &components[..components.len() - 1];
                let path = join_path_many(pkg_dir, parent_components);
                return Ok((path, false));
            }
            Err(REASON_FOUND_WITHOUT_TYPE_HINTS)
        } else {
            Err(REASON_NOT_FOUND)
        }
    }

    // Mirrors FindModuleCache._update_ns_ancestors.
    fn update_ns_ancestors(&mut self, components: &[String], match_path: &str, _verify: bool) {
        let mut path = match_path.to_string();
        for i in 1..components.len() {
            let pkg_id = components[..components.len() - i].join(".");
            if !self.ns_ancestors.contains_key(&pkg_id) && self.inp.fs.isdir(&path) {
                self.ns_ancestors.insert(pkg_id, path.clone());
            }
            path = dirname(&path);
        }
    }

    // Mirrors verify_module (module-level free function).
    fn verify_module(&self, id: &str, path: &str, prefix: &str) -> bool {
        let mut p = if is_init_file(path) {
            dirname(path)
        } else {
            path.to_string()
        };
        let dot_count = id.matches('.').count();
        for _ in 0..dot_count {
            p = dirname(&p);
            let found = PYTHON_EXTENSIONS.iter().any(|ext| {
                let init = join_path(&p, &format!("__init__{}", ext));
                self.inp.fs.isfile_case(&init, prefix)
            });
            if !found {
                return false;
            }
        }
        true
    }

    // Mirrors highest_init_level (module-level free function).
    fn highest_init_level(&self, id: &str, path: &str, prefix: &str) -> usize {
        let mut p = if is_init_file(path) {
            dirname(path)
        } else {
            path.to_string()
        };
        let mut level = 0;
        let dot_count = id.matches('.').count();
        for i in 0..dot_count {
            p = dirname(&p);
            let found = PYTHON_EXTENSIONS.iter().any(|ext| {
                let init = join_path(&p, &format!("__init__{}", ext));
                self.inp.fs.isfile_case(&init, prefix)
            });
            if found {
                level = i + 1;
            }
        }
        level
    }

    // Mirrors mypy.stubinfo.stub_distribution_name. Returns an owned String
    // to avoid lifetime entanglement between the two source tables.
    fn stub_distribution_name(&self, module: &str) -> Option<String> {
        let top_level = match module.split('.').next() {
            Some(t) => t,
            None => return None,
        };
        // Flat lookup is keyed by top-level; the Python function returns the
        // dist for the top-level name. We don't have the dist string here
        // (only the name set), but the approved-stubs branch only needs to
        // know *whether* a dist exists, not its name. So a flat match means
        // "approved, dist name is the top-level's entry".
        if self.inp.stub_flat.contains(top_level) {
            return Some(top_level.to_string());
        }
        // Namespace lookup: the Python function checks whether the top-level
        // is a key in non_bundled_packages_namespace (a nested dict). Our
        // flattened stub_namespace map contains the FULL dotted module names
        // as keys (e.g. "google.cloud.ndb"), not the top-level. So we check
        // whether ANY key starts with the top-level prefix, then walk
        // components longest-first to find the most specific match.
        let has_namespace = self
            .inp
            .stub_namespace
            .keys()
            .any(|k| k == top_level || k.starts_with(&format!("{}.", top_level)));
        if has_namespace {
            let components: Vec<&str> = module.split('.').collect();
            for i in (1..=components.len()).rev() {
                let candidate = components[..i].join(".");
                if let Some(dist) = self.inp.stub_namespace.get(&candidate) {
                    return Some(dist.clone());
                }
            }
        }
        None
    }

    // Mirrors FindModuleCache._find_module.
    fn find_module(&mut self, id: &str, use_typeshed: bool) -> ResolveResult {
        // Fast path for the source set is handled in Python before calling
        // Rust, so we skip find_module_via_source_set here.

        let components = split_dot(id);
        let dir_chain: String = components[..components.len() - 1]
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("/");

        // Third-party stub/typed package collection.
        let mut third_party_inline_dirs: Vec<(String, bool)> = Vec::new();
        let mut third_party_stubs_dirs: Vec<(String, bool)> = Vec::new();
        let mut found_possible_third_party_missing_type_hints = false;

        let top = &components[0];
        let stub_name = format!("{}-stubs", top);
        let mut candidate_package_dirs: BTreeSet<String> = BTreeSet::new();
        for component in [top.clone(), stub_name.clone()] {
            for (d, _v) in self.find_lib_path_dirs(&component, self.inp.package_path) {
                candidate_package_dirs.insert(d);
            }
        }

        let mut can_cache_any_result = true;
        for pkg_dir in self.inp.package_path.iter() {
            if !candidate_package_dirs.contains(pkg_dir) {
                continue;
            }
            let stub_dir = join_path(pkg_dir, &stub_name);
            if self.inp.fs.isdir(&stub_dir) {
                let stub_typed_file = join_path(&stub_dir, "py.typed");
                let mut stub_components = vec![stub_name.clone()];
                stub_components.extend_from_slice(&components[1..]);
                let path = join_path_many(pkg_dir, &stub_components[..stub_components.len() - 1]);
                if self.inp.fs.isdir(&path) {
                    if self.inp.fs.isfile(&stub_typed_file) {
                        // Partial stub packages declare themselves via py.typed containing "partial".
                        let content = self.inp.fs.read(&stub_typed_file);
                        let text = String::from_utf8_lossy(&content);
                        if text.trim() == "partial" {
                            let runtime_path = join_path(pkg_dir, &dir_chain);
                            third_party_inline_dirs.push((runtime_path, true));
                            // Partial stub packages may lack __init__.pyi.
                            third_party_stubs_dirs.push((path, false));
                        } else {
                            third_party_stubs_dirs.push((path, true));
                        }
                    } else {
                        third_party_stubs_dirs.push((path, true));
                    }
                }
            }
            match self.find_module_non_stub_helper(id, pkg_dir) {
                Err(reason) => {
                    if reason == REASON_FOUND_WITHOUT_TYPE_HINTS {
                        found_possible_third_party_missing_type_hints = true;
                        can_cache_any_result = false;
                    }
                }
                Ok(dir_match) => {
                    third_party_inline_dirs.push(dir_match.clone());
                    self.update_ns_ancestors(&components, &dir_match.0, dir_match.1);
                }
            }
        }

        if self.inp.use_builtins_fixtures {
            third_party_inline_dirs.clear();
            third_party_stubs_dirs.clear();
            found_possible_third_party_missing_type_hints = false;
        }

        let mut python_mypy_path: Vec<String> = Vec::new();
        python_mypy_path.extend_from_slice(self.inp.mypy_path);
        python_mypy_path.extend_from_slice(self.inp.python_path);
        let mut candidate_base_dirs = self.find_lib_path_dirs(id, &python_mypy_path);
        if use_typeshed {
            let mut t = self.find_lib_path_dirs(id, self.inp.typeshed_path);
            candidate_base_dirs.append(&mut t);
        }
        candidate_base_dirs.extend_from_slice(&third_party_stubs_dirs);
        candidate_base_dirs.extend_from_slice(&third_party_inline_dirs);

        let seplast = format!("/{}", components[components.len() - 1]);
        let sepinit = "/__init__".to_string();
        let mut near_misses: Vec<(String, String)> = Vec::new();

        for (base_dir, verify) in candidate_base_dirs.clone() {
            let base_path = format!("{}{}", base_dir, seplast);
            let mut has_init = false;
            let mut dir_prefix = base_dir.clone();
            for _ in 0..components.len() - 1 {
                dir_prefix = dirname(&dir_prefix);
            }

            // Stubs-only packages always take precedence over py.typed packages.
            let path_stubs = format!("{}-stubs{}.pyi", base_path, sepinit);
            if self.inp.fs.isfile_case(&path_stubs, &dir_prefix) {
                if verify && !self.verify_module(id, &path_stubs, &dir_prefix) {
                    near_misses.push((path_stubs, dir_prefix.clone()));
                } else {
                    return (FOUND, Some(path_stubs), true);
                }
            }

            // Prefer package over module: baz/__init__.py* over baz.py*.
            for ext in PYTHON_EXTENSIONS {
                let path = format!("{}{}{}", base_path, sepinit, ext);
                if self.inp.fs.isfile_case(&path, &dir_prefix) {
                    has_init = true;
                    if verify && !self.verify_module(id, &path, &dir_prefix) {
                        near_misses.push((path.clone(), dir_prefix.clone()));
                        continue;
                    }
                    return (FOUND, Some(path), true);
                }
            }

            // Namespace-mode near-miss registration.
            if self.inp.namespace_packages
                && !has_init
                && self.inp.fs.exists_case(&base_path, &dir_prefix)
                && !self.inp.fs.isfile_case(&base_path, &dir_prefix)
            {
                near_misses.push((base_path.clone(), dir_prefix.clone()));
            }

            // No package, look for module.
            for ext in PYTHON_EXTENSIONS {
                let path = format!("{}{}", base_path, ext);
                if self.inp.fs.isfile_case(&path, &dir_prefix) {
                    if verify && !self.verify_module(id, &path, &dir_prefix) {
                        near_misses.push((path.clone(), dir_prefix.clone()));
                        continue;
                    }
                    return (FOUND, Some(path), true);
                }
            }
        }

        // Namespace-mode disambiguation: highest __init__ level wins.
        if self.inp.namespace_packages && !near_misses.is_empty() {
            let mut levels: Vec<usize> = Vec::new();
            for (path, dir_prefix) in &near_misses {
                levels.push(self.highest_init_level(id, path, dir_prefix));
            }
            let max = *levels.iter().max().unwrap_or(&0);
            if let Some(idx) = levels.iter().position(|&l| l == max) {
                let (path, _) = near_misses[idx].clone();
                return (FOUND, Some(path), true);
            }
        }

        // ns_ancestors fallback for subpackages of typed namespace packages.
        if let Some(ancestor) = self.ns_ancestors.get(id).cloned() {
            return (FOUND, Some(ancestor), true);
        }

        // Approved-stubs branch. Mirrors mypy.modulefinder._find_module
        // lines 639-653. The control flow is subtle:
        //   * If no parent shares our dist name (the `for...else` falls through),
        //     this id IS the approved-stubs root → return APPROVED.
        //   * If a parent shares our dist name (the loop `break`s), recurse on
        //     that parent: if it's APPROVED, we're APPROVED too; otherwise NOT_FOUND.
        let approved_dist = self.stub_distribution_name(id);
        if let Some(dist_str) = approved_dist {
            if components.len() == 1 {
                return (REASON_APPROVED_STUBS_NOT_INSTALLED, None, true);
            }
            let mut matching_parent: Option<String> = None;
            for i in 1..components.len() {
                let parent_id = components[..i].join(".");
                let parent_dist = self.stub_distribution_name(&parent_id);
                if parent_dist.as_deref() == Some(dist_str.as_str()) {
                    matching_parent = Some(parent_id);
                    break;
                }
            }
            match matching_parent {
                None => {
                    // No parent shares the dist → this is the approved root.
                    return (REASON_APPROVED_STUBS_NOT_INSTALLED, None, true);
                }
                Some(parent_id) => {
                    // Recurse on the parent (mirrors self.find_module(parent_id)).
                    // Shares our initial_components and ns_ancestors caches,
                    // matching Python's FindModuleCache.find_module recursion.
                    let parent_result = self.find_module(&parent_id, use_typeshed);
                    if parent_result.0 == REASON_APPROVED_STUBS_NOT_INSTALLED {
                        return (REASON_APPROVED_STUBS_NOT_INSTALLED, None, true);
                    }
                    return (REASON_NOT_FOUND, None, true);
                }
            }
        }

        if found_possible_third_party_missing_type_hints {
            return (REASON_FOUND_WITHOUT_TYPE_HINTS, None, can_cache_any_result);
        }
        (REASON_NOT_FOUND, None, true)
    }
}

// ---------------------------------------------------------------------------
// PyO3 module init
// ---------------------------------------------------------------------------

#[pymodule]
fn module_resolver(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add_class::<FsCache>()?;
    module.add_class::<NativeResolver>()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(
        fs: &HashMapFs,
        id: &str,
        search: &[&str],
        ns: bool,
        follow_untyped: bool,
        approved: &[&str],
    ) -> (u8, Option<String>, bool) {
        resolve_with(fs, id, &[], search, ns, follow_untyped, approved)
    }

    fn resolve_with(
        fs: &HashMapFs,
        id: &str,
        package_path: &[&str],
        mypy_path: &[&str],
        ns: bool,
        follow_untyped: bool,
        approved: &[&str],
    ) -> (u8, Option<String>, bool) {
        let pkg: Vec<String> = package_path.iter().map(|s| s.to_string()).collect();
        let myp: Vec<String> = mypy_path.iter().map(|s| s.to_string()).collect();
        let flat_set: BTreeSet<String> = approved.iter().map(|s| s.to_string()).collect();
        let ns_map: BTreeMap<String, String> = BTreeMap::new();
        let stdlib_map: HashMap<String, ((u8, u8), Option<(u8, u8)>)> = HashMap::new();
        let inputs = ResolveInputs {
            id,
            use_typeshed: false,
            namespace_packages: ns,
            use_builtins_fixtures: false,
            follow_untyped_imports: follow_untyped,
            python_path: &[],
            mypy_path: &myp,
            package_path: &pkg,
            typeshed_path: &[],
            stdlib_versions: &stdlib_map,
            stub_flat: &flat_set,
            stub_namespace: &ns_map,
            fs: &*fs,
        };
        let mut ic: HashMap<Vec<String>, HashMap<String, Vec<String>>> = HashMap::new();
        let mut ns: HashMap<String, String> = HashMap::new();
        let mut r = Resolver::new(&inputs, &mut ic, &mut ns);
        r.find_module(id, false)
    }

    fn fs() -> HashMapFs {
        HashMapFs::new()
    }

    #[test]
    fn finds_top_level_module_py() {
        let f = fs().file("/lib/pkg1/a.py", "");
        let (kind, path, _) = resolve(&f, "a", &["/lib/pkg1"], false, false, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/pkg1/a.py"));
    }

    #[test]
    fn prefers_pyi_over_py() {
        let f = fs().file("/lib/pkg1/c.pyi", "").file("/lib/pkg1/c.py", "");
        let (kind, path, _) = resolve(&f, "c", &["/lib/pkg1"], false, false, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/pkg1/c.pyi"));
    }

    #[test]
    fn prefers_package_over_module() {
        let f = fs()
            .file("/lib/pkg1/b/__init__.py", "")
            .file("/lib/pkg1/b.pyi", "");
        let (kind, path, _) = resolve(&f, "b", &["/lib/pkg1"], false, false, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/pkg1/b/__init__.py"));
    }

    #[test]
    fn prefers_pyi_package_over_py_module() {
        let f = fs()
            .file("/lib/pkg1/a/__init__.pyi", "")
            .file("/lib/pkg1/a/__init__.py", "");
        let (kind, path, _) = resolve(&f, "a", &["/lib/pkg1"], false, false, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/pkg1/a/__init__.pyi"));
    }

    #[test]
    fn not_found_returns_not_found_reason() {
        let f = fs();
        let (kind, _, _) = resolve(&f, "does_not_exist", &["/lib/pkg1"], false, false, &[]);
        assert_eq!(kind, REASON_NOT_FOUND);
    }

    #[test]
    fn untyped_without_follow_returns_found_without_type_hints() {
        // _find_module_non_stub_helper only runs against package_path, so the
        // untyped package must live there (not mypy_path) to trigger the reason.
        // The package has __init__.py but no py.typed marker.
        let f = fs().file("/lib/pkg1/untyped/__init__.py", "");
        let (kind, _, _) = resolve_with(&f, "untyped", &["/lib/pkg1"], &[], false, false, &[]);
        assert_eq!(kind, REASON_FOUND_WITHOUT_TYPE_HINTS);
    }

    #[test]
    fn untyped_with_follow_returns_found() {
        // With follow_untyped_imports, an untyped but real package resolves
        // to its __init__ file rather than reporting FOUND_WITHOUT_TYPE_HINTS.
        let f = fs().file("/lib/pkg1/untyped/__init__.py", "");
        let (kind, path, _) = resolve_with(&f, "untyped", &["/lib/pkg1"], &[], false, true, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/pkg1/untyped/__init__.py"));
    }

    #[test]
    fn typed_package_resolves() {
        let f = fs()
            .file("/lib/pkg1/typed/py.typed", "")
            .file("/lib/pkg1/typed/__init__.py", "")
            .file("/lib/pkg1/typed/a.py", "");
        let (kind, path, _) = resolve(&f, "typed.a", &["/lib/pkg1"], false, false, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/pkg1/typed/a.py"));
    }

    #[test]
    fn namespace_mode_finds_namespace_pkg() {
        // nsx has no __init__ in any dir; namespace mode returns the first dir.
        let f = fs()
            .dir("/lib/nsx-pkg1/nsx")
            .file("/lib/nsx-pkg1/nsx/a.py", "");
        let (kind, path, _) = resolve(&f, "nsx", &["/lib/nsx-pkg1"], true, false, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/nsx-pkg1/nsx"));
    }

    #[test]
    fn non_namespace_mode_rejects_namespace_pkg() {
        let f = fs()
            .dir("/lib/nsx-pkg1/nsx")
            .file("/lib/nsx-pkg1/nsx/a.py", "");
        let (kind, _, _) = resolve(&f, "nsx", &["/lib/nsx-pkg1"], false, false, &[]);
        assert_eq!(kind, REASON_NOT_FOUND);
    }

    #[test]
    fn stub_package_takes_precedence() {
        let f = fs()
            .file("/lib/spkg-stubs/__init__.pyi", "")
            .file("/lib/spkg/__init__.py", "");
        let (kind, path, _) = resolve(&f, "spkg", &["/lib"], false, false, &[]);
        assert_eq!(kind, FOUND);
        assert_eq!(path.as_deref(), Some("/lib/spkg-stubs/__init__.pyi"));
    }

    #[test]
    fn approved_stubs_returns_reason() {
        let f = fs();
        let (kind, _, _) = resolve(
            &f,
            "someapproved",
            &["/lib"],
            false,
            false,
            &["someapproved"],
        );
        assert_eq!(kind, REASON_APPROVED_STUBS_NOT_INSTALLED);
    }

    #[test]
    fn join_path_handles_trailing_slash() {
        assert_eq!(join_path("/a/", "b"), "/a/b");
        assert_eq!(join_path("/a", "b"), "/a/b");
        assert_eq!(join_path("", "b"), "b");
        assert_eq!(join_path("/a", "/b"), "/b");
    }

    #[test]
    fn dirname_mirrors_posix() {
        assert_eq!(dirname("/a/b/c.py"), "/a/b");
        assert_eq!(dirname("/a/b/"), "/a");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(dirname("/"), "/");
        assert_eq!(dirname("nopath"), "");
    }

    #[test]
    fn basename_mirrors_posix() {
        assert_eq!(basename("/a/b/c.py"), "c.py");
        assert_eq!(basename("/a/b/__init__.pyi"), "__init__.pyi");
        assert_eq!(basename("/"), "");
    }

    // --- Dependency-record extraction tests ---
    // These exercise `dep_records_with` (the core of
    // `NativeResolver::compute_dep_records`) using `HashMapFs` as the
    // filesystem. They mirror the cases in `mypy/build.py:all_imported_modules_in_file`.

    use std::cell::RefCell as TestRefCell;

    /// Build the import-record inputs and call `dep_records_with` against a
    /// `HashMapFs` with the given search paths. The search path is passed as
    /// `mypy_path` (matching `resolve_with`) since test modules are plain
    /// source files, not typed third-party packages.
    fn dep_records(
        fs: &HashMapFs,
        file_id: &str,
        file_path: &str,
        imports: &[ImportRecord],
        known: &[&str],
        search_path: &[&str],
    ) -> (Vec<(i32, String, i32)>, Option<(i32, String)>) {
        let myp: Vec<String> = search_path.iter().map(|s| s.to_string()).collect();
        let flat = BTreeSet::<String>::new();
        let ns_map = BTreeMap::<String, String>::new();
        let stdlib_map = HashMap::<String, ((u8, u8), Option<(u8, u8)>)>::new();
        let known_set: HashSet<String> = known.iter().map(|s| s.to_string()).collect();
        let ic = TestRefCell::new(HashMap::new());
        let ns = TestRefCell::new(HashMap::new());
        dep_records_with(
            &*fs,
            file_id,
            file_path,
            imports,
            &known_set,
            &ic,
            &ns,
            &[],
            &myp,
            &[],
            &[],
            &stdlib_map,
            (3, 10),
            &flat,
            &ns_map,
            false,
            false,
        )
        .unwrap()
    }

    fn imp(id: &str, line: i32) -> ImportRecord {
        ImportRecord(
            IMP_IMPORT,
            id.to_string(),
            0,
            vec![(id.to_string(), None)],
            line,
            true,
            false,
            false,
            false,
        )
    }

    fn imp_from(module: &str, names: &[&str], line: i32) -> ImportRecord {
        ImportRecord(
            IMP_IMPORTFROM,
            module.to_string(),
            0,
            names.iter().map(|n| (n.to_string(), None)).collect(),
            line,
            true,
            false,
            false,
            false,
        )
    }

    fn imp_all(module: &str, line: i32) -> ImportRecord {
        ImportRecord(
            IMP_IMPORTALL,
            module.to_string(),
            0,
            vec![],
            line,
            true,
            false,
            false,
            false,
        )
    }

    #[test]
    fn import_emits_module_and_ancestors() {
        let f = fs().file("/lib/pkg/a/b/c.py", "");
        let (recs, err) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/__init__.py",
            &[imp("a.b.c", 1)],
            &[],
            &["/lib/pkg"],
        );
        assert!(err.is_none());
        // Module itself at PRI_MED, ancestors at PRI_LOW. Sorted by
        // descending dot count: a.b.c (2 dots) before a.b (1) before a (0).
        assert_eq!(
            recs,
            vec![
                (PRI_MED, "a.b.c".to_string(), 1),
                (PRI_LOW, "a.b".to_string(), 1),
                (PRI_LOW, "a".to_string(), 1),
            ]
        );
    }

    #[test]
    fn import_from_with_names_uses_pri_high() {
        // `from m import x` where x is NOT a submodule → PRI_HIGH for m.
        let f = fs().file("/lib/pkg/m.py", "");
        let (recs, _) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/__init__.py",
            &[imp_from("m", &["x"], 1)],
            &[],
            &["/lib/pkg"],
        );
        assert_eq!(recs, vec![(PRI_HIGH, "m".to_string(), 1)]);
    }

    #[test]
    fn import_from_all_submodules_uses_pri_low() {
        // `from m import sub` where sub IS a submodule → PRI_LOW for m
        // (the #4498 cycle workaround).
        let f = fs()
            .file("/lib/pkg/m/__init__.py", "")
            .file("/lib/pkg/m/sub.py", "");
        let (recs, _) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/__init__.py",
            &[imp_from("m", &["sub"], 1)],
            &[],
            &["/lib/pkg"],
        );
        // sub at PRI_MED, m at PRI_LOW. sub has 1 dot, m has 0 → sub first.
        assert_eq!(
            recs,
            vec![
                (PRI_MED, "m.sub".to_string(), 1),
                (PRI_LOW, "m".to_string(), 1),
            ]
        );
    }

    #[test]
    fn import_from_mixed_submodule_and_name_uses_pri_high() {
        // `from m import sub, x` where sub IS a submodule but x is NOT →
        // not all_are_submodules → PRI_HIGH for m.
        let f = fs()
            .file("/lib/pkg/m/__init__.py", "")
            .file("/lib/pkg/m/sub.py", "");
        let (recs, _) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/__init__.py",
            &[imp_from("m", &["sub", "x"], 1)],
            &[],
            &["/lib/pkg"],
        );
        // m.sub at PRI_MED, m at PRI_HIGH.
        assert_eq!(
            recs,
            vec![
                (PRI_MED, "m.sub".to_string(), 1),
                (PRI_HIGH, "m".to_string(), 1),
            ]
        );
    }

    #[test]
    fn import_all_uses_pri_high() {
        let f = fs().file("/lib/pkg/m.py", "");
        let (recs, _) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/__init__.py",
            &[imp_all("m", 1)],
            &[],
            &["/lib/pkg"],
        );
        assert_eq!(recs, vec![(PRI_HIGH, "m".to_string(), 1)]);
    }

    #[test]
    fn relative_import_corrected() {
        // `from . import x` inside pkg.mod → cur_id = "pkg".
        // x IS a submodule (pkg.x = /lib/pkg/pkg/x.py exists), so it gets
        // PRI_MED and the parent pkg gets PRI_LOW (all_are_submodules).
        let f = fs()
            .file("/lib/pkg/pkg/__init__.py", "")
            .file("/lib/pkg/pkg/x.py", "");
        let mut r = imp_from("", &["x"], 1);
        r.2 = 1;
        let (recs, _) = dep_records(
            &f,
            "pkg.mod",
            "/lib/pkg/pkg/mod.py",
            &[r],
            &[],
            &["/lib/pkg"],
        );
        assert_eq!(
            recs,
            vec![
                (PRI_MED, "pkg.x".to_string(), 1),
                (PRI_LOW, "pkg".to_string(), 1),
            ]
        );
    }

    #[test]
    fn relative_import_in_init_file_adjusts() {
        // `from . import x` inside pkg/__init__.py → relative 1 becomes 0,
        // so cur_id = "pkg" + "" = "pkg", then sub_id = "pkg.x".
        let f = fs()
            .file("/lib/pkg/pkg/__init__.py", "")
            .file("/lib/pkg/pkg/x.py", "");
        let mut r = imp_from("", &["x"], 1);
        r.2 = 1;
        let (recs, _) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/pkg/__init__.py",
            &[r],
            &[],
            &["/lib/pkg"],
        );
        // x is a submodule of pkg → PRI_MED for pkg.x, PRI_LOW for pkg.
        assert_eq!(
            recs,
            vec![
                (PRI_MED, "pkg.x".to_string(), 1),
                (PRI_LOW, "pkg".to_string(), 1),
            ]
        );
    }

    #[test]
    fn relative_import_no_parent_emits_error() {
        // `from .. import x` at top level → empty new_id → blocking error.
        let f = fs();
        let mut r = imp_from("", &["x"], 3);
        r.2 = 1;
        let (_, err) = dep_records(&f, "", "/lib/mod.py", &[r], &[], &["/lib"]);
        assert!(err.is_some());
        let (line, msg) = err.unwrap();
        assert_eq!(line, 3);
        assert!(msg.contains("No parent module"));
    }

    #[test]
    fn unreachable_import_skipped() {
        // is_unreachable and not is_unreachable_dependency → skipped.
        let f = fs().file("/lib/pkg/m.py", "");
        let mut r = imp("m", 1);
        r.6 = true;
        r.7 = false;
        let (recs, _) = dep_records(&f, "pkg", "/lib/pkg/__init__.py", &[r], &[], &["/lib/pkg"]);
        assert!(recs.is_empty());
    }

    #[test]
    fn unreachable_dependency_included_if_resolvable() {
        // is_unreachable_dependency → included only if is_module returns true.
        let f = fs().file("/lib/pkg/m.py", "");
        let mut r = imp("m", 1);
        r.6 = true;
        r.7 = true;
        let (recs, _) = dep_records(&f, "pkg", "/lib/pkg/__init__.py", &[r], &[], &["/lib/pkg"]);
        // m.py exists → included. Ancestors: none (m has no dots).
        assert_eq!(recs, vec![(PRI_MED, "m".to_string(), 1)]);
    }

    #[test]
    fn unreachable_dependency_excluded_if_not_resolvable() {
        // is_unreachable_dependency but module doesn't exist → excluded.
        let f = fs();
        let mut r = imp("nonexistent", 1);
        r.6 = true;
        r.7 = true;
        let (recs, _) = dep_records(&f, "pkg", "/lib/pkg/__init__.py", &[r], &[], &["/lib/pkg"]);
        assert!(recs.is_empty());
    }

    #[test]
    fn known_modules_short_circuits_is_module() {
        // If a module id is in `known_modules`, it's considered a module even
        // if it doesn't exist on disk. This mirrors `BuildManager.is_module`'s
        // `self.modules` / `source_set.source_modules` fast paths.
        let f = fs(); // no files on disk
        let (recs, _) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/__init__.py",
            &[imp_from("known_mod", &["x"], 1)],
            &["known_mod"],
            &["/lib/pkg"],
        );
        // known_mod is in known_modules → is_module returns true → x is NOT a
        // submodule (no "known_mod.x" on disk or in known set) → PRI_HIGH.
        assert_eq!(recs, vec![(PRI_HIGH, "known_mod".to_string(), 1)]);
    }

    #[test]
    fn import_priority_inside_function_is_pri_low() {
        let f = fs().file("/lib/pkg/m.py", "");
        let mut r = imp("m", 1);
        r.5 = false;
        let (recs, _) = dep_records(&f, "pkg", "/lib/pkg/__init__.py", &[r], &[], &["/lib/pkg"]);
        assert_eq!(recs, vec![(PRI_LOW, "m".to_string(), 1)]);
    }

    #[test]
    fn import_priority_mypy_only_is_pri_mypy() {
        let f = fs().file("/lib/pkg/m.py", "");
        let mut r = imp("m", 1);
        r.8 = true;
        let (recs, _) = dep_records(&f, "pkg", "/lib/pkg/__init__.py", &[r], &[], &["/lib/pkg"]);
        assert_eq!(recs, vec![(PRI_MYPY, "m".to_string(), 1)]);
    }

    #[test]
    fn records_sorted_by_descending_dot_count() {
        // Ensure the sort puts deeper modules before their ancestors so
        // FindModuleCache.ns_ancestors gets primed correctly.
        let f = fs()
            .file("/lib/pkg/a/__init__.py", "")
            .file("/lib/pkg/a/b.py", "")
            .file("/lib/pkg/c.py", "");
        let imports = vec![imp("c", 1), imp("a.b", 2)];
        let (recs, _) = dep_records(
            &f,
            "pkg",
            "/lib/pkg/__init__.py",
            &imports,
            &[],
            &["/lib/pkg"],
        );
        // a.b (1 dot) before c (0 dots), then a (0 dots) from ancestor expansion.
        let ids: Vec<&str> = recs.iter().map(|(_, id, _)| id.as_str()).collect();
        assert_eq!(ids, vec!["a.b", "c", "a"]);
    }

    // --- stdlib version-gating regression tests ---
    // `is_module_inline` must replicate `FindModuleCache.find_module`'s
    // `use_typeshed` computation (`_typeshed_has_version`): a stdlib module
    // outside the target Python version range must NOT be looked up in
    // typeshed. This is what makes `import tomllib` (added in 3.11) resolve
    // as NOT_FOUND when targeting 3.10, so the dependency walk skips it via
    // `include_only_if_resolvable` instead of including it as a phantom dep.

    fn dep_records_versioned(
        fs: &HashMapFs,
        file_id: &str,
        file_path: &str,
        imports: &[ImportRecord],
        known: &[&str],
        mypy_path: &[&str],
        typeshed_path: &[&str],
        python_version: (u8, u8),
        stdlib_versions: &[(&str, (u8, u8), Option<(u8, u8)>)],
    ) -> Vec<(i32, String, i32)> {
        let myp: Vec<String> = mypy_path.iter().map(|s| s.to_string()).collect();
        let tsp: Vec<String> = typeshed_path.iter().map(|s| s.to_string()).collect();
        let flat = BTreeSet::<String>::new();
        let ns_map = BTreeMap::<String, String>::new();
        let stdlib_map: HashMap<String, ((u8, u8), Option<(u8, u8)>)> = stdlib_versions
            .iter()
            .map(|(n, lo, hi)| (n.to_string(), (*lo, *hi)))
            .collect();
        let known_set: HashSet<String> = known.iter().map(|s| s.to_string()).collect();
        let ic = TestRefCell::new(HashMap::new());
        let ns = TestRefCell::new(HashMap::new());
        dep_records_with(
            &*fs,
            file_id,
            file_path,
            imports,
            &known_set,
            &ic,
            &ns,
            &[],
            &myp,
            &[],
            &tsp,
            &stdlib_map,
            python_version,
            &flat,
            &ns_map,
            false,
            false,
        )
        .unwrap()
        .0
    }

    #[test]
    fn unreachable_dependency_skipped_when_typeshed_version_too_low() {
        // `tomllib` is registered in typeshed with min version (3, 11). When
        // targeting 3.10, `is_module` must NOT look it up in typeshed, so it
        // resolves as NOT_FOUND and the import (an unreachable dependency) is
        // skipped — mirroring Python's `find_module` + `_typeshed_has_version`.
        let f = fs().file("/typeshed/stdlib/tomllib/__init__.pyi", "");
        let mut r = imp("tomllib", 1);
        r.6 = true; // is_unreachable
        r.7 = true; // is_unreachable_dependency
        let recs = dep_records_versioned(
            &f,
            "config_parser",
            "/src/config_parser.py",
            &[r],
            &[],
            &[],
            &["/typeshed/stdlib"],
            (3, 10),
            &[("tomllib", (3, 11), None)],
        );
        // Skipped: tomllib is outside the target version range → NOT_FOUND →
        // include_only_if_resolvable drops it.
        assert!(recs.is_empty(), "expected no records, got {:?}", recs);
    }

    #[test]
    fn unreachable_dependency_included_when_typeshed_version_in_range() {
        // Same setup as above but targeting 3.11: tomllib is now in range,
        // typeshed resolves it, and the import is included.
        let f = fs().file("/typeshed/stdlib/tomllib/__init__.pyi", "");
        let mut r = imp("tomllib", 1);
        r.6 = true; // is_unreachable
        r.7 = true; // is_unreachable_dependency
        let recs = dep_records_versioned(
            &f,
            "config_parser",
            "/src/config_parser.py",
            &[r],
            &[],
            &[],
            &["/typeshed/stdlib"],
            (3, 11),
            &[("tomllib", (3, 11), None)],
        );
        assert_eq!(recs, vec![(PRI_MED, "tomllib".to_string(), 1)]);
    }

    // --- Batched resolution tests ---
    // Exercise `resolve_many_with` (the core of `NativeResolver::resolve_many`)
    // using `HashMapFs`, mirroring how `dep_records_with` is tested above.

    /// Resolve a batch of ids against a `HashMapFs` with the given search paths.
    /// Mirrors the `resolve` test helper: `search` is passed as `mypy_path`
    /// (matching how `resolve_with(fs, id, &[], search, ...)` wires it), so
    /// plain modules resolve as FOUND rather than FOUND_WITHOUT_TYPE_HINTS.
    fn resolve_many(
        fs: &HashMapFs,
        ids_with_follow: &[(&str, bool)],
        search: &[&str],
        ns: bool,
    ) -> Vec<(u8, Option<String>, bool)> {
        let myp: Vec<String> = search.iter().map(|s| s.to_string()).collect();
        let flat = BTreeSet::<String>::new();
        let ns_map = BTreeMap::<String, String>::new();
        let stdlib_map = HashMap::<String, ((u8, u8), Option<(u8, u8)>)>::new();
        let input: Vec<(String, bool)> = ids_with_follow
            .iter()
            .map(|(id, fu)| (id.to_string(), *fu))
            .collect();
        let ic = TestRefCell::new(HashMap::new());
        let ns_anc = TestRefCell::new(HashMap::new());
        resolve_many_with(
            &*fs,
            &input,
            &ic,
            &ns_anc,
            &[],
            &myp,
            &[],
            &[],
            &stdlib_map,
            (3, 10),
            &flat,
            &ns_map,
            ns,
            false,
        )
        .unwrap()
    }

    #[test]
    fn resolve_many_finds_mixed_batch() {
        // One found module, one not-found, one found under a different name:
        // the batched call must return one result per id in input order,
        // matching what `resolve` would return for each id individually.
        let f = fs()
            .file("/lib/pkg1/a.py", "")
            .file("/lib/pkg1/c.py", "");
        let res = resolve_many(
            &f,
            &[("a", false), ("missing", false), ("c", false)],
            &["/lib/pkg1"],
            false,
        );
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].0, FOUND);
        assert_eq!(res[0].1.as_deref(), Some("/lib/pkg1/a.py"));
        assert_eq!(res[1].0, REASON_NOT_FOUND);
        assert_eq!(res[1].1, None);
        assert_eq!(res[2].0, FOUND);
        assert_eq!(res[2].1.as_deref(), Some("/lib/pkg1/c.py"));
    }

    #[test]
    fn resolve_many_shares_caches_across_ids() {
        // Resolving `pkg.a` then `pkg.b` against the same package exercises the
        // shared `initial_components` cache (the toplevel-components lookup
        // for `/lib/pkg` is computed once for `pkg.a` and reused for `pkg.b`).
        // The test passes as long as both ids resolve correctly; the cache
        // sharing is the mechanism under test.
        let f = fs()
            .file("/lib/pkg/py.typed", "")
            .file("/lib/pkg/__init__.py", "")
            .file("/lib/pkg/a.py", "")
            .file("/lib/pkg/b.py", "");
        let res = resolve_many(
            &f,
            &[("pkg.a", false), ("pkg.b", false)],
            &["/lib"],
            false,
        );
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].0, FOUND);
        assert_eq!(res[0].1.as_deref(), Some("/lib/pkg/a.py"));
        assert_eq!(res[1].0, FOUND);
        assert_eq!(res[1].1.as_deref(), Some("/lib/pkg/b.py"));
    }

    #[test]
    fn resolve_many_respects_follow_untyped_per_id() {
        // Per-id `follow_untyped_imports` must take effect: the same untyped
        // package (no `py.typed` marker) on `package_path` resolves as
        // FOUND_WITHOUT_TYPE_HINTS without follow, and as FOUND with it, in
        // the same batched call. Inlined (not via the `resolve_many` helper)
        // because the helper passes `search` as `mypy_path`, which doesn't
        // trigger the non-stub-helper branch.
        let f = fs().file("/lib/pkg1/untyped/__init__.py", "");
        let pkg: Vec<String> = vec!["/lib/pkg1".to_string()];
        let flat = BTreeSet::<String>::new();
        let ns_map = BTreeMap::<String, String>::new();
        let stdlib_map = HashMap::<String, ((u8, u8), Option<(u8, u8)>)>::new();
        let input: Vec<(String, bool)> =
            vec![("untyped".to_string(), false), ("untyped".to_string(), true)];
        let ic = TestRefCell::new(HashMap::new());
        let ns_anc = TestRefCell::new(HashMap::new());
        let res = resolve_many_with(
            &f,
            &input,
            &ic,
            &ns_anc,
            &[],
            &[],
            &pkg,
            &[],
            &stdlib_map,
            (3, 10),
            &flat,
            &ns_map,
            false,
            false,
        )
        .unwrap();
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].0, REASON_FOUND_WITHOUT_TYPE_HINTS);
        assert_eq!(res[0].1, None);
        assert_eq!(res[1].0, FOUND);
        assert_eq!(res[1].1.as_deref(), Some("/lib/pkg1/untyped/__init__.py"));
    }
}
