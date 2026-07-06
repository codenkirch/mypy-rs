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

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use pyo3::prelude::*;

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
// FsProbe trait
// ---------------------------------------------------------------------------

trait FsProbe {
    fn isfile(&self, path: &str) -> bool;
    fn isdir(&self, path: &str) -> bool;
    fn listdir(&self, path: &str) -> Vec<String>;
    fn isfile_case(&self, path: &str, prefix: &str) -> bool;
    fn exists_case(&self, path: &str, prefix: &str) -> bool;
    fn read(&self, path: &str) -> Vec<u8>;
}

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
        self.files.insert(path.to_string(), content.as_bytes().to_vec());
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
// std::fs-backed FsProbe for production
// ---------------------------------------------------------------------------

/// Native module resolver: owns the FS caches, stubinfo tables, search
/// paths, and resolver config for the lifetime of a `FindModuleCache`.
///
/// Exposed to Python as `NativeResolver`. Constructed once when the
/// dispatch gate in `FindModuleCache._resolve` first routes to Rust, then
/// reused for every subsequent `find_module` call on that cache. This keeps
/// the FS caches (listdir, isfile_case, exists_case, stat) AND the stubinfo
/// tables (stub_flat BTreeSet, stub_namespace BTreeMap) alive across calls,
/// mirroring `FileSystemCache`'s cache lifetime and avoiding per-call
/// reconstruction of the lookup tables.
///
/// The dispatch gate routes only cold, real-filesystem runs here: daemon
/// (`fine_grained_incremental`) and Bazel runs stay on the Python
/// `_find_module` path. So the resolver never needs to impersonate the
/// daemon's VFS or synthesize Bazel fake `__init__` files — those concerns
/// remain Python-owned until the daemon is retired or the Bazel VFS is
/// ported.
#[pyclass(name = "NativeResolver")]
struct NativeResolver {
    // --- Filesystem caches (mirror FileSystemCache's cache fields) ---
    /// listdir cache: path -> entry names (or None if the dir read errored,
    /// mirroring fscache.listdir raising OSError).
    listdir_cache: RefCell<HashMap<String, Option<Vec<String>>>>,
    /// isfile_case cache: path -> bool.
    isfile_case_cache: RefCell<HashMap<String, bool>>,
    /// exists_case cache: path -> bool.
    exists_case_cache: RefCell<HashMap<String, bool>>,
    /// stat cache: path -> (is_file, is_dir). None means "does not exist".
    stat_cache: RefCell<HashMap<String, Option<(bool, bool)>>>,
    // --- Resolver config (stable for the lifetime of the FindModuleCache) ---
    namespace_packages: bool,
    use_builtins_fixtures: bool,
    python_path: Vec<String>,
    mypy_path: Vec<String>,
    package_path: Vec<String>,
    typeshed_path: Vec<String>,
    /// (module_name, min_version, max_version) for stdlib version gating.
    /// (Currently unused at the Rust layer; `use_typeshed` is computed
    /// Python-side and passed in as a bool. Kept for future Rust-side
    /// version gating.)
    #[allow(dead_code)]
    stdlib_versions: Vec<(String, (u8, u8), Option<(u8, u8)>)>,
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
    #[new]
    #[pyo3(signature = (
        namespace_packages,
        use_builtins_fixtures,
        python_path,
        mypy_path,
        package_path,
        typeshed_path,
        stdlib_versions,
        stub_flat,
        stub_namespace,
    ))]
    fn new(
        namespace_packages: bool,
        use_builtins_fixtures: bool,
        python_path: Vec<String>,
        mypy_path: Vec<String>,
        package_path: Vec<String>,
        typeshed_path: Vec<String>,
        stdlib_versions: Vec<(String, (u8, u8), Option<(u8, u8)>)>,
        stub_flat: Vec<String>,
        stub_namespace: Vec<(String, String)>,
    ) -> Self {
        NativeResolver {
            listdir_cache: RefCell::new(HashMap::new()),
            isfile_case_cache: RefCell::new(HashMap::new()),
            exists_case_cache: RefCell::new(HashMap::new()),
            stat_cache: RefCell::new(HashMap::new()),
            namespace_packages,
            use_builtins_fixtures,
            python_path,
            mypy_path,
            package_path,
            typeshed_path,
            stdlib_versions,
            stub_flat: stub_flat.into_iter().collect(),
            stub_namespace: stub_namespace.into_iter().collect(),
            initial_components: RefCell::new(HashMap::new()),
            ns_ancestors: RefCell::new(HashMap::new()),
        }
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
            fs: self,
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
}

impl FsProbe for NativeResolver {
    fn isfile(&self, path: &str) -> bool {
        self.stat_cached(path).map(|(f, _)| f).unwrap_or(false)
    }
    fn isdir(&self, path: &str) -> bool {
        self.stat_cached(path).map(|(_, d)| d).unwrap_or(false)
    }
    fn listdir(&self, path: &str) -> Vec<String> {
        self.listdir_cached(path).unwrap_or_default()
    }
    fn isfile_case(&self, path: &str, prefix: &str) -> bool {
        // Mirror fscache.isfile_case: fast-fail on non-files, then verify
        // the tail's case via case_check (which also recurses upward).
        if !self.isfile(path) {
            return false;
        }
        if let Some(cached) = self.isfile_case_cache.borrow().get(path) {
            return *cached;
        }
        let result = self.case_check(path, prefix);
        self.isfile_case_cache
            .borrow_mut()
            .insert(path.to_string(), result);
        result
    }
    fn exists_case(&self, path: &str, prefix: &str) -> bool {
        // Mirror fscache.exists_case: no precheck for file/dir-ness; the
        // case_check walk implies existence (a missing tail won't appear
        // in its parent's listing).
        self.case_check(path, prefix)
    }
    fn read(&self, path: &str) -> Vec<u8> {
        std::fs::read(path).unwrap_or_default()
    }
}

impl NativeResolver {
    /// Stat a path, returning (is_file, is_dir). Cached so repeated probes
    /// of the same path within a resolution call tree are free.
    fn stat_cached(&self, path: &str) -> Option<(bool, bool)> {
        if let Some(st) = self.stat_cache.borrow().get(path) {
            return *st;
        }
        let result = std::fs::metadata(path)
            .ok()
            .map(|m| (m.is_file(), m.is_dir()));
        self.stat_cache.borrow_mut().insert(path.to_string(), result);
        result
    }

    /// Read a directory's immediate entry names. Cached: each unique path
    /// is listed at most once for the resolver's lifetime. Returns None on
    /// any I/O error (mirroring fscache.listdir raising OSError).
    fn listdir_cached(&self, path: &str) -> Option<Vec<String>> {
        if let Some(cached) = self.listdir_cache.borrow().get(path) {
            return cached.clone();
        }
        let result = self.listdir_uncached(path);
        self.listdir_cache
            .borrow_mut()
            .insert(path.to_string(), result.clone());
        result
    }

    fn listdir_uncached(&self, path: &str) -> Option<Vec<String>> {
        let rd = std::fs::read_dir(Path::new(path)).ok()?;
        let mut out = Vec::new();
        for entry in rd.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                out.push(name.to_string());
            }
        }
        Some(out)
    }

    /// Case-sensitive existence check mirroring `FileSystemCache.exists_case`:
    /// walk path components from the tail up to `prefix`, requiring each
    /// component to appear (exact case) in its parent's listing. Components
    /// at or above `prefix` are trusted and not checked.
    fn case_check(&self, path: &str, prefix: &str) -> bool {
        if let Some(cached) = self.exists_case_cache.borrow().get(path) {
            return *cached;
        }
        let result = self.case_check_uncached(path, prefix);
        self.exists_case_cache
            .borrow_mut()
            .insert(path.to_string(), result);
        result
    }

    fn case_check_uncached(&self, path: &str, prefix: &str) -> bool {
        let (head, tail) = split_head_tail(path);
        // Stop once we climb above prefix, or at a component with no tail
        // (e.g. the root). fscache returns True here: prefix and above are
        // trusted.
        if !head.starts_with(prefix) || tail.is_empty() {
            return true;
        }
        let names = match self.listdir_cached(&head) {
            Some(n) => n,
            None => return false,
        };
        if !names.contains(&tail) {
            return false;
        }
        self.case_check(&head, prefix)
    }
}

/// Mirror `os.path.split` on POSIX: strip trailing slashes, then split at
/// the last `/`. Used by `case_check` to walk components the same way
/// `FileSystemCache.exists_case` does.
fn split_head_tail(path: &str) -> (String, String) {
    let bytes = path.as_bytes();
    let mut end = bytes.len();
    while end > 1 && bytes[end - 1] == b'/' {
        end -= 1;
    }
    if end == 0 {
        return (String::new(), String::new());
    }
    match bytes[..end].iter().rposition(|&b| b == b'/') {
        Some(idx) => {
            let head = if idx == 0 {
                "/".to_string()
            } else {
                String::from_utf8_lossy(&bytes[..idx]).to_string()
            };
            let tail = String::from_utf8_lossy(&bytes[idx + 1..end]).to_string();
            (head, tail)
        }
        None => (String::new(), String::from_utf8_lossy(&bytes[..end]).to_string()),
    }
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
    /// (module_name, min_version, max_version) for stdlib version gating.
    /// (Currently unused at the Rust layer; `use_typeshed` is computed
    /// Python-side and passed in as a bool. Kept for future Rust-side
    /// version gating.)
    #[allow(dead_code)]
    stdlib_versions: &'a [(String, (u8, u8), Option<(u8, u8)>)],
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
    fn get_toplevel_possibilities(
        &mut self,
        lib_path: &[String],
        id: &str,
    ) -> Vec<String> {
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
    fn find_module_non_stub_helper(
        &self,
        id: &str,
        pkg_dir: &str,
    ) -> Result<(String, bool), u8> {
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
            stdlib_versions: &[],
            stub_flat: &flat_set,
            stub_namespace: &ns_map,
            fs,
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
        let f = fs()
            .file("/lib/pkg1/c.pyi", "")
            .file("/lib/pkg1/c.py", "");
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
        let (kind, _, _) = resolve(&f, "someapproved", &["/lib"], false, false, &["someapproved"]);
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
}
