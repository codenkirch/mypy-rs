//! Issue #540: Pure helpers from mypy/modulefinder.py.
//!
//! Ports standalone functions and simple data classes from the module
//! finder into Rust #[pyfunction]s and #[pyclass]es. Functions that take
//! live Python objects (fscache, options) access needed attributes via
//! PyO3. Each function mirrors the Python semantics exactly.

#![allow(non_local_definitions)]

use pyo3::exceptions::{PySystemExit, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet, PyString, PyTuple};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// ====================================================================
// is_init_file
// ====================================================================

#[pyfunction]
#[pyo3(name = "rust_is_init_file")]
pub fn rust_is_init_file(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .map(|n| {
            n == std::ffi::OsStr::new("__init__.py") || n == std::ffi::OsStr::new("__init__.pyi")
        })
        .unwrap_or(false)
}

// ====================================================================
// parse_version
// ====================================================================

fn parse_version_str(version: &str) -> PyResult<(i64, i64)> {
    let parts: Vec<&str> = version.trim().split('.').collect();
    if parts.len() != 2 {
        return Err(PyValueError::new_err(format!(
            "invalid version string: '{version}'"
        )));
    }
    let major: i64 = parts[0]
        .parse()
        .map_err(|_| PyValueError::new_err(format!("invalid major: '{}'", parts[0])))?;
    let minor: i64 = parts[1]
        .parse()
        .map_err(|_| PyValueError::new_err(format!("invalid minor: '{}'", parts[1])))?;
    Ok((major, minor))
}

#[pyfunction]
#[pyo3(name = "rust_parse_version")]
pub fn rust_parse_version(version: &str) -> PyResult<(i64, i64)> {
    parse_version_str(version)
}

// ====================================================================
// mypy_path
// ====================================================================

#[pyfunction]
#[pyo3(name = "rust_mypy_path")]
pub fn rust_mypy_path() -> Vec<String> {
    match std::env::var("MYPYPATH") {
        Ok(path_env) => {
            let sep = if std::env::consts::OS == "windows" {
                ';'
            } else {
                ':'
            };
            path_env.split(sep).map(|s| s.to_string()).collect()
        }
        Err(_) => vec![],
    }
}

// ====================================================================
// typeshed_py_version
// ====================================================================

#[pyfunction]
#[pyo3(name = "rust_typeshed_py_version")]
pub fn rust_typeshed_py_version(options: &PyAny) -> PyResult<(i64, i64)> {
    let py_version: (i64, i64) = options.getattr("python_version")?.extract()?;
    Ok(if py_version >= (3, 10) {
        py_version
    } else {
        (3, 10)
    })
}

// ====================================================================
// default_lib_path
// ====================================================================

fn py_abspath(py: Python<'_>, path: &str) -> PyResult<String> {
    let os_path = py.import("os.path")?;
    os_path.call_method1("abspath", (path,))?.extract()
}

fn py_join(py: Python<'_>, base: &str, parts: &[&str]) -> PyResult<String> {
    let os_path = py.import("os.path")?;
    let mut args: Vec<PyObject> = vec![PyString::new(py, base).into()];
    for p in parts {
        args.push(PyString::new(py, p).into());
    }
    let tup = PyTuple::new(py, &args);
    os_path.call_method1("join", (tup,))?.extract()
}

/// Internal: default_lib_path without the pyfunction wrapper.
fn internal_default_lib_path(
    py: Python<'_>,
    data_dir: &str,
    custom_typeshed_dir: Option<&str>,
) -> PyResult<Vec<String>> {
    let data_dir = py_abspath(py, data_dir)?;
    let mut path: Vec<String> = Vec::new();

    let (typeshed_dir, mypy_extensions_dir, librt_dir) = if let Some(ctd) = custom_typeshed_dir {
        let ctd_abs = py_abspath(py, ctd)?;
        let ts = py_join(py, &ctd_abs, &["stdlib"])?;
        let me = py_join(py, &ctd_abs, &["stubs", "mypy-extensions"])?;
        let lr = py_join(py, &ctd_abs, &["stubs", "librt"])?;
        let versions_file = py_join(py, &ts, &["VERSIONS"])?;
        if !Path::new(&ts).is_dir() || !Path::new(&versions_file).is_file() {
            let sys = py.import("sys")?;
            let stderr = sys.getattr("stderr")?;
            let msg = format!(
                "error: --custom-typeshed-dir does not point to a \
                     valid typeshed ({ctd_abs})\n"
            );
            stderr.call_method1("write", (msg,))?;
            stderr.call_method0("flush")?;
            return Err(PySystemExit::new_err(2));
        }
        (ts, me, lr)
    } else {
        let auto = py_join(py, &data_dir, &["stubs-auto"])?;
        let dd = if Path::new(&auto).is_dir() {
            auto
        } else {
            data_dir.clone()
        };
        let ts = py_join(py, &dd, &["typeshed", "stdlib"])?;
        let me = py_join(py, &dd, &["typeshed", "stubs", "mypy-extensions"])?;
        let lr = py_join(py, &dd, &["typeshed", "stubs", "librt"])?;
        (ts, me, lr)
    };

    path.push(typeshed_dir);
    path.push(mypy_extensions_dir);
    path.push(librt_dir);

    if std::env::consts::OS != "windows" {
        path.push("/usr/local/lib/mypy".to_string());
    }

    if path.is_empty() {
        let sys = py.import("sys")?;
        let stderr = sys.getattr("stderr")?;
        let msg = format!(
            "Could not resolve typeshed subdirectories. Your mypy install \
             is broken.\nPython executable is located at {exe:?}.\n\
             Mypy located at {data_dir}\n",
            exe = std::env::current_exe()
                .map(|e| e.display().to_string())
                .unwrap_or_default(),
        );
        stderr.call_method1("write", (msg,))?;
        stderr.call_method0("flush")?;
        return Err(PySystemExit::new_err(1));
    }
    Ok(path)
}

#[pyfunction]
#[pyo3(name = "rust_default_lib_path")]
pub fn rust_default_lib_path(
    py: Python<'_>,
    data_dir: &str,
    pyversion: (i64, i64),
    custom_typeshed_dir: Option<String>,
) -> PyResult<Vec<String>> {
    let _ = pyversion; // unused in Python body
    internal_default_lib_path(py, data_dir, custom_typeshed_dir.as_deref())
}

// ====================================================================
// load_stdlib_py_versions
// ====================================================================

#[pyfunction]
#[pyo3(name = "rust_load_stdlib_py_versions")]
pub fn rust_load_stdlib_py_versions(
    py: Python<'_>,
    custom_typeshed_dir: Option<String>,
) -> PyResult<Py<PyDict>> {
    let typeshed_dir: PathBuf = match &custom_typeshed_dir {
        Some(d) => PathBuf::from(d),
        None => {
            let modulefinder = py.import("mypy.modulefinder")?;
            let file: String = modulefinder.getattr("__file__")?.extract()?;
            let os_path = py.import("os.path")?;
            let dir: String = os_path.call_method1("dirname", (file,))?.extract()?;
            PathBuf::from(dir).join("typeshed")
        }
    };
    let stdlib_dir = typeshed_dir.join("stdlib");
    let versions_path = stdlib_dir.join("VERSIONS");

    if !versions_path.is_file() {
        return Err(pyo3::exceptions::PyAssertionError::new_err(format!(
            "({}, {:?}, modulefinder)",
            custom_typeshed_dir.as_deref().unwrap_or("None"),
            versions_path
        )));
    }

    let contents = std::fs::read_to_string(&versions_path)
        .map_err(|e| PyValueError::new_err(format!("Cannot read VERSIONS: {e}")))?;

    let result = PyDict::new(py);
    for line in contents.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (module, version_range) = line
            .split_once(':')
            .ok_or_else(|| PyValueError::new_err(format!("Invalid VERSIONS line: {line}")))?;
        let module = module.trim();
        let versions: Vec<&str> = version_range.split('-').collect();
        let min_version = parse_version_str(versions[0])?;
        let max_version = if versions.len() >= 2 && !versions[1].trim().is_empty() {
            Some(parse_version_str(versions[1])?)
        } else {
            None
        };

        let min_obj: PyObject = PyTuple::new(py, [min_version.0, min_version.1]).into();
        let max_obj: PyObject = match max_version {
            Some(mv) => PyTuple::new(py, [mv.0, mv.1]).into(),
            None => py.None(),
        };
        let entry = PyTuple::new(py, [min_obj, max_obj]);
        result.set_item(module, entry)?;
    }
    Ok(result.into())
}

// ====================================================================
// matches_exclude
// ====================================================================

#[pyfunction]
#[pyo3(name = "rust_matches_exclude")]
pub fn rust_matches_exclude(
    py: Python<'_>,
    subpath: &str,
    excludes: Vec<String>,
    fscache: &PyAny,
    verbose: bool,
) -> PyResult<bool> {
    if excludes.is_empty() {
        return Ok(false);
    }
    let os_path = py.import("os.path")?;
    let relpath: String = os_path.call_method1("relpath", (subpath,))?.extract()?;
    let sep = std::path::MAIN_SEPARATOR;
    let mut subpath_str = relpath.replace(sep, "/");

    let is_dir: bool = fscache.call_method1("isdir", (subpath,))?.extract()?;
    if is_dir {
        subpath_str.push('/');
    }

    let re_mod = py.import("re")?;
    let sys = py.import("sys")?;
    let stderr = sys.getattr("stderr")?;

    for exclude in &excludes {
        match re_mod.call_method1("search", (exclude, &subpath_str)) {
            Ok(result) => {
                if !result.is_none() {
                    if verbose {
                        let msg = format!(
                            "TRACE: Excluding {subpath_str} \
                             (matches pattern {exclude})\n"
                        );
                        stderr.call_method1("write", (msg,))?;
                        stderr.call_method0("flush")?;
                    }
                    return Ok(true);
                }
            }
            Err(e) => {
                let e_msg = e.value(py).str()?.to_str()?.to_string();
                let mut msg = format!(
                    "error: The exclude {exclude} is an invalid regular \
                     expression, because: {e_msg}"
                );
                if exclude.contains('\\') {
                    msg.push_str(
                        "\n(Hint: use / as a path separator, even if \
                         you're on Windows!)",
                    );
                }
                msg.push_str(
                    "\nFor more information on Python's flavor of \
                              regex, see: https://docs.python.org/3/library/re.html\n",
                );
                stderr.call_method1("write", (msg,))?;
                stderr.call_method0("flush")?;
                return Err(PySystemExit::new_err(2));
            }
        }
    }
    Ok(false)
}

// ====================================================================
// get_search_dirs
// ====================================================================

fn internal_get_search_dirs(
    py: Python<'_>,
    python_executable: Option<&str>,
) -> PyResult<(Vec<String>, Vec<String>)> {
    let exe = match python_executable {
        None => return Ok((vec![], vec![])),
        Some(e) => e,
    };
    let sys = py.import("sys")?;
    let sys_executable: String = sys.getattr("executable")?.extract()?;

    if exe == sys_executable {
        let pyinfo = py.import("mypy.pyinfo")?;
        let result = pyinfo.call_method0("getsearchdirs")?;
        return result.extract::<(Vec<String>, Vec<String>)>();
    }

    let subprocess = py.import("subprocess")?;
    let ast_mod = py.import("ast")?;
    let os_mod = py.import("os")?;
    let pyinfo = py.import("mypy.pyinfo")?;
    let pyinfo_file: String = pyinfo.getattr("__file__")?.extract()?;

    let os_env = os_mod.getattr("environ")?;
    let new_env = PyDict::new(py);
    let items = os_env.call_method0("items")?;
    for item in items.iter()? {
        let item = item?;
        let (k, v): (&PyAny, &PyAny) = item.extract()?;
        new_env.set_item(k, v)?;
    }
    new_env.set_item("PYTHONSAFEPATH", "1")?;

    let kwargs = PyDict::new(py);
    kwargs.set_item("env", new_env)?;
    kwargs.set_item("stderr", subprocess.getattr("PIPE")?)?;

    let cmd = vec![exe, pyinfo_file.as_str(), "getsearchdirs"];
    let output = subprocess.call_method("check_output", (cmd,), Some(kwargs));

    match output {
        Ok(out) => {
            let decoded: String = out.call_method0("decode")?.extract()?;
            let parsed = ast_mod.call_method1("literal_eval", (decoded,))?;
            parsed.extract::<(Vec<String>, Vec<String>)>()
        }
        Err(e) => {
            let cpe = subprocess.getattr("CalledProcessError")?;
            if e.value(py).is_instance(cpe)? {
                let err_val = e.value(py);
                let stderr_msg: String = err_val.getattr("stderr")?.extract().unwrap_or_default();
                let stdout_msg: String = err_val.getattr("stdout")?.extract().unwrap_or_default();
                let sys_stderr = sys.getattr("stderr")?;
                sys_stderr.call_method1("write", (format!("{stderr_msg}\n"),))?;
                sys_stderr.call_method1("write", (format!("{stdout_msg}\n"),))?;
                return Err(e);
            }
            let os_err = os_mod.getattr("error")?;
            if e.value(py).is_instance(os_err)? {
                let err_val = e.value(py);
                let errno: i32 = err_val.getattr("errno")?.extract().unwrap_or(0);
                let reason: String = os_mod.call_method1("strerror", (errno,))?.extract()?;
                let compile_err = py.import("mypy.errors")?.getattr("CompileError")?;
                let msg = format!("mypy: Invalid python executable '{exe}': {reason}");
                let err_list = PyList::new(py, [msg]);
                let c_err = compile_err.call1((err_list,))?;
                return Err(pyo3::PyErr::from_value(c_err));
            }
            Err(e)
        }
    }
}

#[pyfunction]
#[pyo3(name = "rust_get_search_dirs")]
pub fn rust_get_search_dirs(
    py: Python<'_>,
    python_executable: Option<&str>,
) -> PyResult<(Vec<String>, Vec<String>)> {
    internal_get_search_dirs(py, python_executable)
}

// ====================================================================
// compute_search_paths
// ====================================================================

#[pyfunction]
#[pyo3(name = "rust_compute_search_paths")]
pub fn rust_compute_search_paths(
    py: Python<'_>,
    sources: &PyAny,
    options: &PyAny,
    data_dir: &str,
    alt_lib_path: Option<String>,
) -> PyResult<SearchPaths> {
    let _python_version: (i64, i64) = options.getattr("python_version")?.extract()?;
    let custom_typeshed_dir: Option<String> = options.getattr("custom_typeshed_dir")?.extract()?;
    let use_builtins_fixtures: bool = options.getattr("use_builtins_fixtures")?.is_true()?;
    let bazel: bool = options.getattr("bazel")?.is_true()?;
    let options_mypy_path: Vec<String> = options.getattr("mypy_path")?.extract()?;
    let python_executable: Option<String> = options.getattr("python_executable")?.extract()?;

    // lib_path (deque in Python, Vec used as stack here)
    let mut lib_path: Vec<String> =
        internal_default_lib_path(py, data_dir, custom_typeshed_dir.as_deref())?;

    if use_builtins_fixtures {
        let root_dir = match std::env::var("MYPY_TEST_PREFIX") {
            Ok(rd) => py_abspath(py, &rd)?,
            Err(_) => {
                let modulefinder = py.import("mypy.modulefinder")?;
                let file: String = modulefinder.getattr("__file__")?.extract()?;
                let os_path = py.import("os.path")?;
                let d1: String = os_path.call_method1("dirname", (file,))?.extract()?;
                let d2: String = os_path.call_method1("dirname", (d1,))?.extract()?;
                py_abspath(py, &d2)?
            }
        };
        let lib_stub = py_join(py, &root_dir, &["test-data", "unit", "lib-stub"])?;
        lib_path.insert(0, lib_stub);
    }

    // python_path from sources
    let mut python_path: Vec<String> = Vec::new();
    if alt_lib_path.is_none() {
        for item in sources.iter()? {
            let item = item?;
            let base_dir = item.getattr("base_dir")?;
            if !base_dir.is_none() {
                if let Ok(dir) = base_dir.extract::<String>() {
                    if !python_path.contains(&dir) {
                        python_path.push(dir);
                    }
                }
            }
        }
        let dir = if bazel {
            ".".to_string()
        } else {
            std::env::current_dir()?.display().to_string()
        };
        if !lib_path.contains(&dir) {
            python_path.insert(0, dir);
        }
    }

    // mypypath
    let mut mypypath = rust_mypy_path();
    mypypath.extend(options_mypy_path);
    if let Some(ref alt) = alt_lib_path {
        mypypath.insert(0, alt.clone());
    }

    // search dirs
    let (sys_path, site_packages) = internal_get_search_dirs(py, python_executable.as_deref())?;

    // check site_packages not in MYPYPATH
    let os_path = py.import("os.path")?;
    let sep: String = os_path.getattr("sep")?.extract()?;
    let altsep_obj = os_path.getattr("altsep")?;
    let altsep: Option<String> = if altsep_obj.is_none() {
        None
    } else {
        altsep_obj.extract().ok()
    };

    let sys = py.import("sys")?;
    let stderr = sys.getattr("stderr")?;
    for site in &site_packages {
        if lib_path.contains(site) {
            // Python asserts this; just skip
            continue;
        }
        let in_mypypath = mypypath.contains(site)
            || mypypath
                .iter()
                .any(|p| p.starts_with(&format!("{site}{sep}")))
            || altsep.as_ref().is_some_and(|as_| {
                mypypath
                    .iter()
                    .any(|p| p.starts_with(&format!("{site}{as_}")))
            });
        if in_mypypath {
            stderr.call_method1(
                "write",
                (format!("{site} is in the MYPYPATH. Please remove it.\n"),),
            )?;
            stderr.call_method1(
                "write",
                ("See https://mypy.readthedocs.io/en/stable/\
                     running_mypy.html#how-mypy-handles-imports \
                     for more info\n",),
            )?;
            stderr.call_method0("flush")?;
            return Err(PySystemExit::new_err(1));
        }
    }

    // Build SearchPaths
    let python_path_rev: Vec<String> = python_path.into_iter().rev().collect();
    let package_path: Vec<String> = sys_path.into_iter().chain(site_packages).collect();
    SearchPaths::new(py, python_path_rev, mypypath, package_path, lib_path)
}

// ====================================================================
// SearchPaths (pyclass)
// ====================================================================

#[pyclass(name = "RustSearchPaths")]
pub struct SearchPaths {
    inner_python_path: Vec<String>,
    inner_mypy_path: Vec<String>,
    inner_package_path: Vec<String>,
    inner_typeshed_path: Vec<String>,
}

impl SearchPaths {
    fn new(
        py: Python<'_>,
        python_path: Vec<String>,
        mypy_path: Vec<String>,
        package_path: Vec<String>,
        typeshed_path: Vec<String>,
    ) -> PyResult<Self> {
        let abs = |p: &str| -> PyResult<String> { py_abspath(py, p) };
        let abs_vec =
            |v: Vec<String>| -> PyResult<Vec<String>> { v.iter().map(|p| abs(p)).collect() };
        Ok(SearchPaths {
            inner_python_path: abs_vec(python_path)?,
            inner_mypy_path: abs_vec(mypy_path)?,
            inner_package_path: abs_vec(package_path)?,
            inner_typeshed_path: abs_vec(typeshed_path)?,
        })
    }
}

#[pymethods]
impl SearchPaths {
    #[new]
    #[pyo3(signature = (python_path=vec![], mypy_path=vec![], package_path=vec![], typeshed_path=vec![]))]
    pub fn py_new(
        py: Python<'_>,
        python_path: Vec<String>,
        mypy_path: Vec<String>,
        package_path: Vec<String>,
        typeshed_path: Vec<String>,
    ) -> PyResult<Self> {
        Self::new(py, python_path, mypy_path, package_path, typeshed_path)
    }

    #[getter(python_path)]
    fn get_python_path(&self, py: Python<'_>) -> Py<PyTuple> {
        PyTuple::new(py, &self.inner_python_path).into()
    }

    #[getter(mypy_path)]
    fn get_mypy_path(&self, py: Python<'_>) -> Py<PyTuple> {
        PyTuple::new(py, &self.inner_mypy_path).into()
    }

    #[getter(package_path)]
    fn get_package_path(&self, py: Python<'_>) -> Py<PyTuple> {
        PyTuple::new(py, &self.inner_package_path).into()
    }

    #[getter(typeshed_path)]
    fn get_typeshed_path(&self, py: Python<'_>) -> Py<PyTuple> {
        PyTuple::new(py, &self.inner_typeshed_path).into()
    }

    #[setter(python_path)]
    fn set_python_path(&mut self, value: &PyAny) -> PyResult<()> {
        self.inner_python_path = value.extract()?;
        Ok(())
    }

    #[setter(mypy_path)]
    fn set_mypy_path(&mut self, value: &PyAny) -> PyResult<()> {
        self.inner_mypy_path = value.extract()?;
        Ok(())
    }

    #[setter(package_path)]
    fn set_package_path(&mut self, value: &PyAny) -> PyResult<()> {
        self.inner_package_path = value.extract()?;
        Ok(())
    }

    #[setter(typeshed_path)]
    fn set_typeshed_path(&mut self, value: &PyAny) -> PyResult<()> {
        self.inner_typeshed_path = value.extract()?;
        Ok(())
    }

    fn asdict(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("python_path", PyTuple::new(py, &self.inner_python_path))?;
        dict.set_item("mypy_path", PyTuple::new(py, &self.inner_mypy_path))?;
        dict.set_item("package_path", PyTuple::new(py, &self.inner_package_path))?;
        dict.set_item("typeshed_path", PyTuple::new(py, &self.inner_typeshed_path))?;
        Ok(dict.into())
    }

    fn __repr__(&self) -> String {
        format!(
            "RustSearchPaths(python_path={:?}, mypy_path={:?}, \
             package_path={:?}, typeshed_path={:?})",
            self.inner_python_path,
            self.inner_mypy_path,
            self.inner_package_path,
            self.inner_typeshed_path
        )
    }
}

// ====================================================================
// BuildSource (pyclass)
// ====================================================================

#[pyclass(name = "RustBuildSource")]
pub struct BuildSource {
    inner_path: Option<String>,
    inner_module: String,
    inner_text: Option<String>,
    inner_base_dir: Option<String>,
    inner_followed: bool,
}

#[pymethods]
impl BuildSource {
    #[new]
    #[pyo3(signature = (path, module, text=None, base_dir=None, followed=false))]
    pub fn new(
        path: Option<String>,
        module: Option<String>,
        text: Option<String>,
        base_dir: Option<String>,
        followed: bool,
    ) -> Self {
        BuildSource {
            inner_path: path,
            inner_module: module.unwrap_or_else(|| "__main__".to_string()),
            inner_text: text,
            inner_base_dir: base_dir,
            inner_followed: followed,
        }
    }

    #[getter(path)]
    fn get_path(&self) -> Option<String> {
        self.inner_path.clone()
    }

    #[setter(path)]
    fn set_path(&mut self, value: Option<String>) {
        self.inner_path = value;
    }

    #[getter(module)]
    fn get_module(&self) -> String {
        self.inner_module.clone()
    }

    #[setter(module)]
    fn set_module(&mut self, value: String) {
        self.inner_module = value;
    }

    #[getter(text)]
    fn get_text(&self) -> Option<String> {
        self.inner_text.clone()
    }

    #[setter(text)]
    fn set_text(&mut self, value: Option<String>) {
        self.inner_text = value;
    }

    #[getter(base_dir)]
    fn get_base_dir(&self) -> Option<String> {
        self.inner_base_dir.clone()
    }

    #[setter(base_dir)]
    fn set_base_dir(&mut self, value: Option<String>) {
        self.inner_base_dir = value;
    }

    #[getter(followed)]
    fn get_followed(&self) -> bool {
        self.inner_followed
    }

    #[setter(followed)]
    fn set_followed(&mut self, value: bool) {
        self.inner_followed = value;
    }

    fn __repr__(&self) -> String {
        format!(
            "BuildSource(path={:?}, module={:?}, has_text={}, \
             base_dir={:?}, followed={})",
            self.inner_path,
            self.inner_module,
            self.inner_text.is_some(),
            self.inner_base_dir,
            self.inner_followed
        )
    }
}

// ====================================================================
// BuildSourceSet (pyclass)
// ====================================================================

#[pyclass(name = "RustBuildSourceSet")]
pub struct BuildSourceSet {
    source_text_present: bool,
    source_modules: HashMap<String, String>,
    source_paths: HashSet<String>,
}

#[pymethods]
impl BuildSourceSet {
    #[new]
    pub fn new(sources: &PyAny) -> PyResult<Self> {
        let mut set = BuildSourceSet {
            source_text_present: false,
            source_modules: HashMap::new(),
            source_paths: HashSet::new(),
        };
        for item in sources.iter()? {
            let item = item?;
            let text = item.getattr("text")?;
            if !text.is_none() {
                set.source_text_present = true;
            }
            let path = item.getattr("path")?;
            if !path.is_none() {
                if let Ok(p) = path.extract::<String>() {
                    set.source_paths.insert(p.clone());
                }
            }
            let module = item.getattr("module")?;
            if !module.is_none() {
                if let Ok(m) = module.extract::<String>() {
                    let p: String = path.extract::<String>().unwrap_or_default();
                    set.source_modules.insert(m, p);
                }
            }
        }
        Ok(set)
    }

    #[getter]
    fn source_text_present(&self) -> bool {
        self.source_text_present
    }

    #[getter]
    fn source_modules(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (k, v) in &self.source_modules {
            dict.set_item(k, v)?;
        }
        Ok(dict.into())
    }

    #[getter]
    fn source_paths(&self, py: Python<'_>) -> PyResult<Py<PySet>> {
        let set = PySet::new(py, &self.source_paths)?;
        Ok(set.into())
    }

    fn is_source(&self, file: &PyAny) -> PyResult<bool> {
        let path = file.getattr("path")?;
        if !path.is_none() {
            if let Ok(p) = path.extract::<String>() {
                if self.source_paths.contains(&p) {
                    return Ok(true);
                }
            }
        }
        let fullname = file.getattr("_fullname")?;
        if !fullname.is_none() {
            if let Ok(f) = fullname.extract::<String>() {
                if self.source_modules.contains_key(&f) {
                    return Ok(true);
                }
            }
        }
        Ok(self.source_text_present)
    }
}
