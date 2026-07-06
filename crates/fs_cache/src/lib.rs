//! Rust backing for `mypy.fscache.FileSystemCache`.
//!
//! This crate implements the transactional memoizing filesystem cache that
//! `mypy.fscache.FileSystemCache` previously implemented in pure Python.
//! The Python class becomes a thin delegate forwarding each method to the
//! `FsCache` `#[pyclass]` defined here.
//!
//! Two responsibilities, mirroring the Python original:
//!
//!   * **Memoize syscalls** within a transaction so the same `stat`/`listdir`/
//!     `read` is never issued twice against the real filesystem.
//!   * **Snapshot consistency** within a transaction: repeated reads of the
//!     same path return the same result even if the real filesystem changes
//!     underneath. `flush()` starts a new transaction by clearing the caches.
//!
//! One synthetic overlay is preserved: the Bazel fake `__init__.py`
//! synthesis (`init_under_package_root` / `fake_init`), used when
//! `--package-root` is set so that Bazel-style empty packages appear to
//! contain an `__init__.py` even when the file does not exist on disk.
//!
//! Invariants preserved verbatim from `mypy/fscache.py`:
//!   * The contents of a file are always from the same or later instant
//!     compared to the reported mtime, even if mtime is queried after
//!     reading the file. (`read` stats before opening.)
//!   * Repeating an operation produces the same result as the first one
//!     during a transaction.
//!   * `flush()` clears every cache except `package_root`.
//!
//! `FsCache` also implements `module_resolver::FsProbe` so that Phase 2 can
//! wire `NativeResolver` to read through the same shared cache (eliminating
//! the dual-cache hazard that today forces `_native_gate_active` to exclude
//! daemon and Bazel modes).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

use fs_probe::FsProbe;

// ---------------------------------------------------------------------------
// Stat result
// ---------------------------------------------------------------------------

/// Cached fields from `std::fs::metadata`. Carries everything mypy actually
/// reads off `os.stat_result`: `st_mode` (for `S_ISREG`/`S_ISDIR`), `st_size`,
/// `st_mtime`, and `st_ino`/`st_dev` (for `samefile`).
#[derive(Clone, Copy, Debug)]
struct StatResult {
    mode: u32,
    size: u64,
    mtime: f64,
    ino: u64,
    dev: u64,
    nlink: u64,
}

impl StatResult {
    fn is_file(&self) -> bool {
        // S_IFREG = 0o100000
        (self.mode & 0o170000) == 0o100000
    }
    fn is_dir(&self) -> bool {
        // S_IFDIR = 0o040000
        (self.mode & 0o170000) == 0o040000
    }
    fn from_metadata(meta: fs::Metadata) -> Self {
        // std::fs::Metadata already has cross-platform accessors. On Unix
        // these map to the underlying stat fields; on Windows they are
        // synthesized but mypy doesn't run there in practice.
        let mode = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                meta.mode()
            }
            #[cfg(not(unix))]
            {
                if meta.is_dir() {
                    0o040000
                } else if meta.is_file() {
                    0o100000
                } else {
                    0
                }
            }
        };
        let ino = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                meta.ino()
            }
            #[cfg(not(unix))]
            {
                0
            }
        };
        let dev = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                meta.dev()
            }
            #[cfg(not(unix))]
            {
                0
            }
        };
        let nlink = {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                meta.nlink()
            }
            #[cfg(not(unix))]
            {
                1
            }
        };
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        StatResult {
            mode,
            size: meta.len(),
            mtime,
            ino,
            dev,
            nlink,
        }
    }
}

// ---------------------------------------------------------------------------
// FsCache
// ---------------------------------------------------------------------------

/// Transactional memoizing filesystem cache.
///
/// All caches are per-transaction; `flush()` clears them. `package_root`
/// survives across flushes (matches the Python contract).
#[pyclass(name = "FsCache")]
struct FsCache {
    /// Long-lived across transactions; only mutated by `set_package_root`.
    /// Each entry is a normalized relative path with a trailing separator.
    package_root: RefCell<Vec<String>>,
    stat_cache: RefCell<HashMap<String, Option<StatResult>>>,
    /// `None` here mirrors an OSError from `listdir`; the corresponding
    /// errno lives in `listdir_error_cache`.
    listdir_cache: RefCell<HashMap<String, Option<Vec<String>>>>,
    listdir_error_cache: RefCell<HashMap<String, PyOSErrorEntry>>,
    isfile_case_cache: RefCell<HashMap<String, bool>>,
    exists_case_cache: RefCell<HashMap<String, bool>>,
    /// Bytes are stored as Python-owned `Py<PyBytes>` so reads can be
    /// returned to Python without copying.
    read_cache: RefCell<HashMap<String, Py<PyBytes>>>,
    read_error_cache: RefCell<HashMap<String, PyOSErrorEntry>>,
    hash_cache: RefCell<HashMap<String, String>>,
    /// Directory paths under which a synthetic empty `__init__.py` exists
    /// (Bazel fake-init). Survives only within the transaction that created
    /// it, because `flush()` clears it; the Python original clears it too.
    fake_package_cache: RefCell<HashSet<String>>,
}

/// A captured `OSError` we can re-raise. We store errno + filename so the
/// re-raised exception looks identical to the original; `strerror` is
/// regenerated from errno to avoid holding a Python exception object.
#[derive(Clone)]
struct PyOSErrorEntry {
    errno: i32,
    filename: String,
}

impl PyOSErrorEntry {
    fn from_io(err: &io::Error) -> Self {
        PyOSErrorEntry {
            errno: err.raw_os_error().unwrap_or(0),
            filename: String::new(),
        }
    }
    fn raise(&self, _py: Python<'_>) -> PyErr {
        PyOSError::new_err((self.errno, io::Error::from_raw_os_error(self.errno).to_string(), self.filename.clone()))
    }
}

#[pymethods]
impl FsCache {
    #[new]
    fn new() -> Self {
        let cache = FsCache {
            package_root: RefCell::new(Vec::new()),
            stat_cache: RefCell::new(HashMap::new()),
            listdir_cache: RefCell::new(HashMap::new()),
            listdir_error_cache: RefCell::new(HashMap::new()),
            isfile_case_cache: RefCell::new(HashMap::new()),
            exists_case_cache: RefCell::new(HashMap::new()),
            read_cache: RefCell::new(HashMap::new()),
            read_error_cache: RefCell::new(HashMap::new()),
            hash_cache: RefCell::new(HashMap::new()),
            fake_package_cache: RefCell::new(HashSet::new()),
        };
        // The Python original sets package_root = [] then calls flush();
        // both leave the caches empty, so we just ensure package_root is set.
        let _ = cache.package_root.borrow_mut(); // touch for consistency
        cache
    }

    fn set_package_root(&self, package_root: Vec<String>) {
        *self.package_root.borrow_mut() = package_root;
    }

    /// Start a new transaction and empty all caches. `package_root` survives.
    fn flush(&self) {
        self.stat_cache.borrow_mut().clear();
        self.listdir_cache.borrow_mut().clear();
        self.listdir_error_cache.borrow_mut().clear();
        self.isfile_case_cache.borrow_mut().clear();
        self.exists_case_cache.borrow_mut().clear();
        self.read_cache.borrow_mut().clear();
        self.read_error_cache.borrow_mut().clear();
        self.hash_cache.borrow_mut().clear();
        self.fake_package_cache.borrow_mut().clear();
    }

    /// Return stat for `path`, or `None` if it does not exist. On a missing
    /// `__init__.py` under a package root, synthesizes a fake stat (Bazel).
    /// The `None`/`Some` result is cached for the transaction.
    ///
    /// Returns a tuple `(st_mode, st_size, st_mtime, st_ino, st_dev, st_nlink)`
    /// matching the fields mypy reads off `os.stat_result`.
    fn stat_or_none(&self, _py: Python<'_>, path: String) -> PyResult<Option<(u32, u64, f64, u64, u64, u64)>> {
        if let Some(cached) = self.stat_cache.borrow().get(&path) {
            return Ok(cached.map(|s| (s.mode, s.size, s.mtime, s.ino, s.dev, s.nlink)));
        }
        let result = match fs::metadata(&path) {
            Ok(meta) => Some(StatResult::from_metadata(meta)),
            Err(_) => {
                if self.init_under_package_root(&path) {
                    match self.fake_init(&path) {
                        Ok(st) => Some(st),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            }
        };
        self.stat_cache.borrow_mut().insert(path, result);
        Ok(result.map(|s| (s.mode, s.size, s.mtime, s.ino, s.dev, s.nlink)))
    }

    /// Is `path` an `__init__.py` under a package root (for Bazel synthesis)?
    fn init_under_package_root(&self, path: &str) -> bool {
        let package_root = self.package_root.borrow();
        if package_root.is_empty() {
            return false;
        }
        let p = Path::new(path);
        let basename = match p.file_name().and_then(|s| s.to_str()) {
            Some(name) => name,
            None => return false,
        };
        if basename != "__init__.py" {
            return false;
        }
        let dirname = match p.parent() {
            Some(d) => d,
            None => return false,
        };
        // dirname basename must be a Python identifier (can't hold an
        // __init__.py in a non-identifier directory).
        let dirname_basename = dirname
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if !dirname_basename.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false)
            || !dirname_basename
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_')
        {
            return false;
        }
        // The directory itself must exist and be a directory.
        let dir_stat = match fs::metadata(dirname) {
            Ok(m) => m,
            Err(_) => return false,
        };
        if !dir_stat.is_dir() {
            return false;
        }
        // Skip if on a different drive (Windows-relevant; on Unix splitdrive
        // is a no-op so this never trips).
        let cwd = std::env::current_dir().unwrap_or_default();
        let cwd_drive = splitdrive(&cwd.to_string_lossy()).0;
        let path_drive = splitdrive(path).0;
        if cwd_drive != path_drive {
            return false;
        }
        // Normalize to a relative path and check against each package root.
        let abs_path = if Path::new(path).is_absolute() {
            cwd.join(path.strip_prefix(&cwd_drive).unwrap_or(path))
        } else {
            cwd.join(path)
        };
        let rel = match abs_path.strip_prefix(&cwd) {
            Ok(r) => r,
            Err(_) => return false,
        };
        let rel_str = rel.to_string_lossy();
        let norm = normalize_path(&rel_str);
        for root in package_root.iter() {
            if norm.starts_with(root) {
                // A package root itself is never a package.
                if norm == format!("{}{}", root, basename) {
                    return false;
                }
                return true;
            }
        }
        false
    }

    fn listdir(&self, py: Python<'_>, path: String) -> PyResult<Vec<String>> {
        let norm = normalize_path(&path);
        if let Some(cached) = self.listdir_cache.borrow().get(&norm) {
            if let Some(entries) = cached {
                let mut entries = entries.clone();
                if self.fake_package_cache.borrow().contains(&norm)
                    && !entries.iter().any(|e| e == "__init__.py")
                {
                    entries.push("__init__.py".to_string());
                }
                return Ok(entries);
            } else {
                return Err(self
                    .listdir_error_cache
                    .borrow()
                    .get(&norm)
                    .cloned()
                    .unwrap_or_else(|| PyOSErrorEntry {
                        errno: 2,
                        filename: norm.clone(),
                    })
                    .raise(py));
            }
        }
        match fs::read_dir(&path) {
            Ok(rd) => {
                let mut entries: Vec<String> = Vec::new();
                for entry in rd {
                    match entry {
                        Ok(e) => {
                            if let Some(name) = e.file_name().to_str() {
                                entries.push(name.to_string());
                            }
                        }
                        Err(err) => {
                            let entry_err = PyOSErrorEntry::from_io(&err);
                            self.listdir_error_cache
                                .borrow_mut()
                                .insert(norm.clone(), entry_err.clone());
                            self.listdir_cache.borrow_mut().insert(norm, None);
                            return Err(entry_err.raise(py));
                        }
                    }
                }
                self.listdir_cache.borrow_mut().insert(norm.clone(), Some(entries.clone()));
                if self.fake_package_cache.borrow().contains(&norm)
                    && !entries.iter().any(|e| e == "__init__.py")
                {
                    entries.push("__init__.py".to_string());
                }
                Ok(entries)
            }
            Err(err) => {
                let entry_err = PyOSErrorEntry::from_io(&err);
                self.listdir_error_cache
                    .borrow_mut()
                    .insert(norm.clone(), entry_err.clone());
                self.listdir_cache.borrow_mut().insert(norm, None);
                Err(entry_err.raise(py))
            }
        }
    }

    fn isfile(&self, path: String) -> bool {
        self.stat_cached(&path).map(|s| s.is_file()).unwrap_or(false)
    }

    fn isdir(&self, path: String) -> bool {
        self.stat_cached(&path).map(|s| s.is_dir()).unwrap_or(false)
    }

    /// Return whether `path` exists and is a file, with case-sensitive
    /// matching of the last path component up to `prefix` on case-insensitive
    /// filesystems. Mirrors `FileSystemCache.isfile_case`.
    #[pyo3(signature = (path, prefix))]
    fn isfile_case(&self, py: Python<'_>, path: String, prefix: String) -> PyResult<bool> {
        // Fast path: if it's not a file at all, skip the case check.
        if !self.isfile(path.clone()) {
            return Ok(false);
        }
        if let Some(cached) = self.isfile_case_cache.borrow().get(&path) {
            return Ok(*cached);
        }
        let p = Path::new(&path);
        let tail = match p.file_name().and_then(|s| s.to_str()) {
            Some(t) => t.to_string(),
            None => {
                self.isfile_case_cache.borrow_mut().insert(path, false);
                return Ok(false);
            }
        };
        let head = match p.parent() {
            Some(h) => normalize_path(&h.to_string_lossy()),
            None => String::new(),
        };
        let result = match self.listdir(py, head.clone()) {
            Ok(names) => names.iter().any(|n| n == &tail),
            Err(_) => false,
        };
        let result = if result {
            self.exists_case(py, head, prefix)?
        } else {
            false
        };
        self.isfile_case_cache.borrow_mut().insert(path, result);
        Ok(result)
    }

    /// Return whether `path` exists, checking path components in case
    /// sensitive fashion up to `prefix`.
    #[pyo3(signature = (path, prefix))]
    fn exists_case(&self, py: Python<'_>, path: String, prefix: String) -> PyResult<bool> {
        if let Some(cached) = self.exists_case_cache.borrow().get(&path) {
            return Ok(*cached);
        }
        let p = Path::new(&path);
        let head = match p.parent() {
            Some(h) => normalize_path(&h.to_string_lossy()),
            None => String::new(),
        };
        let tail = match p.file_name().and_then(|s| s.to_str()) {
            Some(t) => t.to_string(),
            None => {
                self.exists_case_cache.borrow_mut().insert(path, true);
                return Ok(true);
            }
        };
        // Only walk components under `prefix`; above the prefix we trust the
        // filesystem's own case handling.
        if !head.starts_with(&prefix) || tail.is_empty() {
            self.exists_case_cache.borrow_mut().insert(path, true);
            return Ok(true);
        }
        let result = match self.listdir(py, head.clone()) {
            Ok(names) => names.iter().any(|n| n == &tail),
            Err(_) => false,
        };
        let result = if result {
            self.exists_case(py, head, prefix)?
        } else {
            false
        };
        self.exists_case_cache.borrow_mut().insert(path, result);
        Ok(result)
    }

    fn exists(&self, path: String, real_only: Option<bool>) -> bool {
        let real_only = real_only.unwrap_or(false);
        let st = match self.stat_cached(&path) {
            Some(st) => st,
            None => return false,
        };
        if real_only {
            let dirname = Path::new(&path)
                .parent()
                .map(|p| normalize_path(&p.to_string_lossy()))
                .unwrap_or_default();
            !self.fake_package_cache.borrow().contains(&dirname)
        } else {
            let _ = st;
            true
        }
    }

    /// Read file contents. Stats before opening so the cached mtime is from
    /// an instant no earlier than the contents. Returns a `PyBytes` borrowed
    /// from the cache (no copy across the PyO3 boundary).
    fn read<'py>(&self, py: Python<'py>, path: String) -> PyResult<&'py PyBytes> {
        // Clone the Py<PyBytes> out of the cache before releasing the
        // RefCell borrow; the underlying bytes are not copied (Py<PyBytes>
        // is a refcounted handle into the Python heap).
        if let Some(cached) = self.read_cache.borrow().get(&path).cloned() {
            return Ok(cached.into_ref(py));
        }
        if let Some(err) = self.read_error_cache.borrow().get(&path) {
            return Err(err.clone().raise(py));
        }
        // Stat first: the Python original documents that the contents of a
        // file must be from the same or later instant than the reported
        // mtime. We stat into the stat cache so a subsequent stat_or_none
        // call returns the pre-read mtime.
        self.stat_cached(&path);
        let dirname = Path::new(&path)
            .parent()
            .map(|p| normalize_path(&p.to_string_lossy()))
            .unwrap_or_default();
        let basename = Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let fake = basename == "__init__.py"
            && self.fake_package_cache.borrow().contains(&dirname);
        let data: Vec<u8> = if fake {
            Vec::new()
        } else {
            match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    let entry = PyOSErrorEntry {
                        errno: err.raw_os_error().unwrap_or(0),
                        filename: path.clone(),
                    };
                    self.read_error_cache.borrow_mut().insert(path, entry.clone());
                    return Err(entry.raise(py));
                }
            }
        };
        // Compute hash now so a later hash_digest() returns the same value
        // without re-reading.
        let hash = sha1_hex(&data);
        let pybytes: Py<PyBytes> = PyBytes::new(py, &data).into();
        self.read_cache
            .borrow_mut()
            .insert(path.clone(), pybytes.clone());
        self.hash_cache.borrow_mut().insert(path, hash);
        Ok(pybytes.into_ref(py))
    }

    fn hash_digest(&self, py: Python<'_>, path: String) -> PyResult<String> {
        if let Some(h) = self.hash_cache.borrow().get(&path) {
            return Ok(h.clone());
        }
        // Mirrors the Python original: read() fills hash_cache as a side
        // effect.
        self.read(py, path.clone())?;
        Ok(self
            .hash_cache
            .borrow()
            .get(&path)
            .cloned()
            .unwrap_or_default())
    }

    fn samefile(&self, f1: String, f2: String) -> bool {
        match (self.stat_cached(&f1), self.stat_cached(&f2)) {
            (Some(s1), Some(s2)) => s1.ino == s2.ino && s1.dev == s2.dev,
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl FsCache {
    /// Stat a path, populating the stat cache (mirrors the Python original's
    /// `stat_or_none` which both `isfile` and `isdir` go through).
    fn stat_cached(&self, path: &str) -> Option<StatResult> {
        if let Some(cached) = self.stat_cache.borrow().get(path) {
            return *cached;
        }
        let result = match fs::metadata(path) {
            Ok(meta) => Some(StatResult::from_metadata(meta)),
            Err(_) => {
                if self.init_under_package_root(path) {
                    self.fake_init(path).ok()
                } else {
                    None
                }
            }
        };
        self.stat_cache.borrow_mut().insert(path.to_string(), result);
        result
    }

    /// Synthesize a fake `__init__.py` stat under a package root (Bazel).
    /// Adds the directory to `fake_package_cache` so `listdir`/`read`/`exists`
    /// also see the synthesized file.
    fn fake_init(&self, path: &str) -> io::Result<StatResult> {
        let p = Path::new(path);
        let basename = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        debug_assert_eq!(basename, "__init__.py", "fake_init on non-init path: {}", path);
        let dirname = p.parent().unwrap_or(Path::new("."));
        let dirname_norm = normalize_path(&dirname.to_string_lossy());
        let meta = fs::metadata(dirname)?;
        // Synthesize a regular file: mode S_IFREG | 0o444, size 0, ino 1,
        // nlink 1, dev/mtime from the directory (matches the Python original:
        // it copies the dirname stat then overrides the file-relevant fields).
        let mut st = StatResult::from_metadata(meta);
        st.mode = 0o100444; // S_IFREG | 0o444
        st.size = 0;
        st.ino = 1;
        st.nlink = 1;
        self.fake_package_cache.borrow_mut().insert(dirname_norm);
        Ok(st)
    }
}

/// `os.path.normpath`-equivalent for the subset of paths we deal with:
/// collapse `.`/`..`/double separators. The Python original uses
/// `os.path.normpath` on every listdir key, so the cache hits line up.
fn normalize_path(path: &str) -> String {
    // Use std::path to do most of the work, then normalize separators.
    let p = Path::new(path);
    let normalized = p
        .components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect::<std::path::PathBuf>();
    let mut s = normalized.to_string_lossy().into_owned();
    // Collapse trailing separators.
    while s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    s
}

/// Split a path into (drive, rest). On Unix the drive is always empty.
/// Mirrors `os.path.splitdrive`.
fn splitdrive(path: &str) -> (String, String) {
    #[cfg(windows)]
    {
        // Match os.path.splitdrive on Windows: drive letter or UNC.
        if path.len() >= 2 {
            let bytes = path.as_bytes();
            if bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
                return (path[..2].to_string(), path[2..].to_string());
            }
        }
        if path.starts_with(r"\\") {
            if let Some(idx) = path[2..].find(r"\") {
                let rest = &path[2 + idx + 1..];
                if let Some(end) = rest.find(r"\") {
                    return (path[..2 + idx + 1 + end + 1].to_string(), rest[end + 1..].to_string());
                }
            }
        }
        (String::new(), path.to_string())
    }
    #[cfg(not(windows))]
    {
        (String::new(), path.to_string())
    }
}

/// SHA-1 hex digest, matching `hashlib.sha1(data).hexdigest()`.
fn sha1_hex(data: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut s = String::with_capacity(40);
    for b in result.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ---------------------------------------------------------------------------
// FsProbe impl (Rust-to-Rust, no PyO3 boundary; used by Phase 2)
// ---------------------------------------------------------------------------

impl FsProbe for FsCache {
    fn isfile(&self, path: &str) -> bool {
        FsCache::isfile(self, path.to_string())
    }
    fn isdir(&self, path: &str) -> bool {
        FsCache::isdir(self, path.to_string())
    }
    fn listdir(&self, path: &str) -> Vec<String> {
        Python::with_gil(|py| FsCache::listdir(self, py, path.to_string()).unwrap_or_default())
    }
    fn isfile_case(&self, path: &str, prefix: &str) -> bool {
        Python::with_gil(|py| {
            FsCache::isfile_case(self, py, path.to_string(), prefix.to_string()).unwrap_or(false)
        })
    }
    fn exists_case(&self, path: &str, prefix: &str) -> bool {
        Python::with_gil(|py| {
            FsCache::exists_case(self, py, path.to_string(), prefix.to_string()).unwrap_or(false)
        })
    }
    fn read(&self, path: &str) -> Vec<u8> {
        Python::with_gil(|py| {
            FsCache::read(self, py, path.to_string())
                .map(|b| b.as_bytes().to_vec())
                .unwrap_or_default()
        })
    }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

#[pymodule]
fn fs_cache(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_class::<FsCache>()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmpdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn write_file(root: &PathBuf, rel: &str, content: &[u8]) {
        let full = root.join(rel);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full, content).unwrap();
    }

    #[test]
    fn stat_caches_file_dir_not_found() {
        let dir = tmpdir();
        let path = dir.path().to_path_buf();
        let file_path = path.join("foo.py");
        fs::write(&file_path, b"hi").unwrap();
        let cache = FsCache::new();

        // Cold: file exists.
        let st = cache.stat_cached(file_path.to_str().unwrap()).unwrap();
        assert!(st.is_file());
        assert!(!st.is_dir());
        assert_eq!(st.size, 2);

        // Cache hit.
        let st2 = cache.stat_cached(file_path.to_str().unwrap()).unwrap();
        assert_eq!(st.size, st2.size);

        // Directory.
        let st_dir = cache.stat_cached(path.to_str().unwrap()).unwrap();
        assert!(st_dir.is_dir());

        // Not found.
        let missing = path.join("missing.py");
        assert!(cache.stat_cached(missing.to_str().unwrap()).is_none());
    }

    #[test]
    fn listdir_caches_and_injects_fake_init() {
        let dir = tmpdir();
        let pkg = dir.path().join("pkg");
        fs::create_dir_all(&pkg).unwrap();
        fs::write(pkg.join("a.py"), b"").unwrap();
        let cache = FsCache::new();

        let pkg_str = pkg.to_string_lossy().to_string();
        let entries = Python::with_gil(|py| cache.listdir(py, pkg_str.clone())).unwrap();
        assert!(entries.iter().any(|e| e == "a.py"));
        assert!(!entries.iter().any(|e| e == "__init__.py"));

        // Synthesize a fake __init__.py via fake_init and verify listdir
        // now injects it.
        let init_path = pkg.join("__init__.py");
        cache.fake_init(init_path.to_str().unwrap()).unwrap();
        let entries2 = Python::with_gil(|py| cache.listdir(py, pkg_str)).unwrap();
        assert!(
            entries2.iter().any(|e| e == "__init__.py"),
            "fake __init__.py should be injected into listdir"
        );
    }

    #[test]
    fn listdir_raises_on_missing_dir() {
        let cache = FsCache::new();
        let path = "/nonexistent-dir-xyz".to_string();
        let r1 = Python::with_gil(|py| cache.listdir(py, path.clone()));
        assert!(r1.is_err(), "listdir on missing dir should raise");
        // Re-raise should hit the cached error, not re-stat.
        let r2 = Python::with_gil(|py| cache.listdir(py, path));
        assert!(r2.is_err());
    }

    #[test]
    fn read_caches_and_returns_bytes() {
        let dir = tmpdir();
        let file_path = dir.path().join("src.py");
        fs::write(&file_path, b"x = 1\n").unwrap();
        let cache = FsCache::new();
        let path = file_path.to_string_lossy().to_string();

        let bytes = Python::with_gil(|py| cache.read(py, path.clone()).unwrap().as_bytes().to_vec());
        assert_eq!(bytes, b"x = 1\n");

        // Second call hits the cache (returns the same PyBytes object).
        let bytes2 = Python::with_gil(|py| cache.read(py, path).unwrap().as_bytes().to_vec());
        assert_eq!(bytes2, b"x = 1\n");
    }

    #[test]
    fn read_missing_file_raises_and_caches_error() {
        let cache = FsCache::new();
        let path = "/nonexistent-file-xyz.py".to_string();
        let r1 = Python::with_gil(|py| cache.read(py, path.clone()).map(|_| ()));
        assert!(r1.is_err());
        // Cached error path: the second call must not re-stat (no panic).
        let r2 = Python::with_gil(|py| cache.read(py, path).map(|_| ()));
        assert!(r2.is_err());
    }

    #[test]
    fn hash_digest_caches_after_read() {
        let dir = tmpdir();
        let file_path = dir.path().join("h.py");
        fs::write(&file_path, b"abc").unwrap();
        let cache = FsCache::new();
        let path = file_path.to_string_lossy().to_string();

        let h = Python::with_gil(|py| cache.hash_digest(py, path.clone()).unwrap());
        // sha1("abc")
        assert_eq!(h, "a9993e364706816aba3e25717850c26c9cd0d89d");

        // Second call returns the cached value without re-reading.
        let h2 = Python::with_gil(|py| cache.hash_digest(py, path).unwrap());
        assert_eq!(h, h2);
    }

    #[test]
    fn samefile_compares_inode_and_dev() {
        let dir = tmpdir();
        let file_path = dir.path().join("s.py");
        fs::write(&file_path, b"").unwrap();
        let hardlink = dir.path().join("link.py");
        fs::hard_link(&file_path, &hardlink).unwrap();
        let cache = FsCache::new();

        let same = cache.samefile(
            file_path.to_string_lossy().to_string(),
            hardlink.to_string_lossy().to_string(),
        );
        assert_eq!(same, true);

        let other = dir.path().join("other.py");
        fs::write(&other, b"").unwrap();
        let diff = cache.samefile(
            file_path.to_string_lossy().to_string(),
            other.to_string_lossy().to_string(),
        );
        assert_eq!(diff, false);
    }

    #[test]
    fn flush_clears_caches_but_preserves_package_root() {
        let dir = tmpdir();
        let file_path = dir.path().join("f.py");
        fs::write(&file_path, b"").unwrap();
        let cache = FsCache::new();
        cache.set_package_root(vec!["pkg/".to_string()]);

        // Populate caches.
        let _ = cache.stat_cached(file_path.to_str().unwrap());
        let path = file_path.to_string_lossy().to_string();
        let _ = Python::with_gil(|py| cache.read(py, path).map(|b| b.as_bytes().to_vec()).unwrap());

        assert!(!cache.stat_cache.borrow().is_empty());
        assert!(!cache.read_cache.borrow().is_empty());
        assert_eq!(cache.package_root.borrow().len(), 1);

        cache.flush();

        assert!(cache.stat_cache.borrow().is_empty());
        assert!(cache.read_cache.borrow().is_empty());
        assert_eq!(
            cache.package_root.borrow().len(),
            1,
            "package_root must survive flush"
        );
    }

    #[test]
    fn snapshot_consistency_within_transaction() {
        let dir = tmpdir();
        let file_path = dir.path().join("snap.py");
        fs::write(&file_path, b"v1").unwrap();
        let cache = FsCache::new();
        let path = file_path.to_string_lossy().to_string();

        let st1 = cache.stat_cached(file_path.to_str().unwrap()).unwrap();
        let bytes1 = Python::with_gil(|py| cache.read(py, path.clone()).unwrap().as_bytes().to_vec());
        assert_eq!(bytes1, b"v1");

        // Mutate the underlying file within the same transaction.
        fs::write(&file_path, b"v2-longer").unwrap();

        // Snapshot: stat returns the cached (older) result.
        let st2 = cache.stat_cached(file_path.to_str().unwrap()).unwrap();
        assert_eq!(st1.size, st2.size, "stat should be cached within transaction");

        // And read returns the cached (older) contents.
        let bytes2 = Python::with_gil(|py| cache.read(py, path.clone()).unwrap().as_bytes().to_vec());
        assert_eq!(bytes2, b"v1", "read should be cached within transaction");

        // After flush, a new transaction sees the new contents.
        cache.flush();
        let st3 = cache.stat_cached(file_path.to_str().unwrap()).unwrap();
        assert_eq!(st3.size, 9, "flush should expose new file size");
    }

    #[test]
    fn bazel_fake_init_under_package_root() {
        let dir = tmpdir();
        let pkg = dir.path().join("pkg");
        fs::create_dir_all(&pkg).unwrap();
        let cache = FsCache::new();
        // Without a package_root, no fake-init.
        let init_path = pkg.join("__init__.py");
        assert!(!cache.init_under_package_root(init_path.to_str().unwrap()));

        // We cannot easily exercise the full init_under_package_root path
        // without chdir'ing (it normalizes against cwd). Instead, exercise
        // fake_init directly and verify it populates fake_package_cache so
        // listdir/read/exists see the synthesized file.
        let st = cache.fake_init(init_path.to_str().unwrap()).unwrap();
        assert!(st.is_file(), "fake_init must produce a regular-file stat");
        assert_eq!(st.size, 0);

        // listdir should now inject __init__.py into the package contents.
        let pkg_str = pkg.to_string_lossy().to_string();
        let entries = Python::with_gil(|py| cache.listdir(py, pkg_str)).unwrap();
        assert!(entries.iter().any(|e| e == "__init__.py"));

        // read on the fake __init__.py should return empty bytes.
        let init_str = init_path.to_string_lossy().to_string();
        let bytes = Python::with_gil(|py| cache.read(py, init_str).unwrap().as_bytes().to_vec());
        assert_eq!(bytes, b"");
    }

    #[test]
    fn fsprobe_impl_delegates_correctly() {
        let dir = tmpdir();
        let file_path = dir.path().join("probe.py");
        fs::write(&file_path, b"data").unwrap();
        let cache = FsCache::new();
        let path = file_path.to_string_lossy().to_string();

        assert!(FsProbe::isfile(&cache, &path));
        assert!(!FsProbe::isdir(&cache, &path));
        assert_eq!(FsProbe::read(&cache, &path), b"data");
    }
}
