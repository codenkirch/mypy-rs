"""Interface for accessing the file system with automatic caching.

The idea is to cache the results of any file system state reads during
a single transaction. This has two main benefits:

* This avoids redundant syscalls, as we won't perform the same OS
  operations multiple times.

* This makes it easier to reason about concurrent FS updates, as different
  operations targeting the same paths can't report different state during
  a transaction.

Note that this only deals with reading state, not writing.

Properties maintained by the API:

* The contents of the file are always from the same or later time compared
  to the reported mtime of the file, even if mtime is queried after reading
  a file.

* Repeating an operation produces the same result as the first one during
  a transaction.

* Call flush() to start a new transaction (flush the caches).

The API is a bit limited. It's easy to add new cached operations, however.
You should perform all file system reads through the API to actually take
advantage of the benefits.
"""

from __future__ import annotations

import os
import stat

from mypy_extensions import mypyc_attr

from mypy.util import hash_digest

# The transactional memoizing cache is implemented in Rust
# (``crates/module_resolver/src/fs_cache.rs``); this class is a thin Python
# delegate that forwards every method to the ``module_resolver.FsCache``
# pyclass. The delegate exists so ``FileSystemCache`` keeps its Python type
# identity (callers subclass it, annotate against it, and
# ``fswatcher``/``build`` import it by name) while the implementation —
# including the per-transaction snapshot semantics and the Bazel fake
# ``__init__.py`` synthesis — lives in Rust.
#
# When the compiled extension is not on PYTHONPATH (e.g. a daemon subprocess
# that overrides PYTHONPATH to the repo root), we fall back to the pure
# Python implementation below. This preserves the strangler-fig contract:
# Python keeps working without the extension; the extension is an
# optimization when present.
try:
    from module_resolver import FsCache as _FsCache

    _HAS_RUST_CACHE = True
except ImportError:
    _FsCache = None  # type: ignore[assignment,misc]
    _HAS_RUST_CACHE = False

# os.stat_result indices (matches the CPython struct stat sequence order
# used by fscache._fake_init when it synthesizes a stat). We only need a
# subset to build a result callers can read st_mode/st_size/st_mtime/st_ino/
# st_dev/st_nlink off; the remaining fields default to 0.
_ST_MODE = stat.ST_MODE
_ST_INO = stat.ST_INO
_ST_DEV = stat.ST_DEV
_ST_NLINK = stat.ST_NLINK
_ST_SIZE = stat.ST_SIZE
_ST_MTIME = stat.ST_MTIME


@mypyc_attr(allow_interpreted_subclasses=True)  # for tests
class FileSystemCache:
    def __init__(self) -> None:
        # The package root is not flushed with the caches.
        # It is set by set_package_root() below.
        self.package_root: list[str] = []
        if _HAS_RUST_CACHE:
            self._rust = _FsCache()
        self.flush()

    def set_package_root(self, package_root: list[str]) -> None:
        self.package_root = package_root
        if _HAS_RUST_CACHE:
            self._rust.set_package_root(package_root)

    def flush(self) -> None:
        """Start another transaction and empty all caches."""
        if _HAS_RUST_CACHE:
            self._rust.flush()
        # Keep the public attribute surface stable for any introspection
        # (the actual state lives in Rust when available).
        self.stat_or_none_cache: dict[str, os.stat_result | None] = {}
        self.listdir_cache: dict[str, list[str]] = {}
        self.listdir_error_cache: dict[str, OSError] = {}
        self.isfile_case_cache: dict[str, bool] = {}
        self.exists_case_cache: dict[str, bool] = {}
        self.read_cache: dict[str, bytes] = {}
        self.read_error_cache: dict[str, Exception] = {}
        self.hash_cache: dict[str, str] = {}
        self.fake_package_cache: set[str] = set()

    def stat_or_none(self, path: str) -> os.stat_result | None:
        if _HAS_RUST_CACHE:
            result = self._rust.stat_or_none(path)
            if result is None:
                return None
            mode, size, mtime, ino, dev, nlink = result
            seq = [0] * 10
            seq[_ST_MODE] = mode
            seq[_ST_INO] = ino
            seq[_ST_DEV] = dev
            seq[_ST_NLINK] = nlink
            seq[_ST_SIZE] = size
            seq[_ST_MTIME] = int(mtime)
            return os.stat_result(seq)
        return self._stat_or_none_py(path)

    def init_under_package_root(self, path: str) -> bool:
        if _HAS_RUST_CACHE:
            return self._rust.init_under_package_root(path)
        return self._init_under_package_root_py(path)

    def listdir(self, path: str) -> list[str]:
        if _HAS_RUST_CACHE:
            return self._rust.listdir(path)
        return self._listdir_py(path)

    def isfile(self, path: str) -> bool:
        if _HAS_RUST_CACHE:
            return self._rust.isfile(path)
        st = self._stat_or_none_py(path)
        if st is None:
            return False
        return stat.S_ISREG(st.st_mode)

    def isfile_case(self, path: str, prefix: str) -> bool:
        """Return whether path exists and is a file.

        On case-insensitive filesystems (like Mac or Windows) this returns
        False if the case of path's last component does not exactly match
        the case found in the filesystem.

        We check also the case of other path components up to prefix.
        For example, if path is 'user-stubs/pack/mod.pyi' and prefix is 'user-stubs',
        we check that the case of 'pack' and 'mod.py' matches exactly, 'user-stubs' will
        be case insensitive on case insensitive filesystems.

        The caller must ensure that prefix is a valid file system prefix of path.
        """
        if _HAS_RUST_CACHE:
            return self._rust.isfile_case(path, prefix)
        if not self.isfile(path):
            # Fast path
            return False
        if path in self.isfile_case_cache:
            return self.isfile_case_cache[path]
        head, tail = os.path.split(path)
        if not tail:
            self.isfile_case_cache[path] = False
            return False
        try:
            names = self._listdir_py(head)
            # This allows one to check file name case sensitively in
            # case-insensitive filesystems.
            res = tail in names
        except OSError:
            res = False
        if res:
            # Also recursively check the other path components in case sensitive way.
            res = self.exists_case(head, prefix)
        self.isfile_case_cache[path] = res
        return res

    def exists_case(self, path: str, prefix: str) -> bool:
        """Return whether path exists - checking path components in case sensitive
        fashion, up to prefix.
        """
        if _HAS_RUST_CACHE:
            return self._rust.exists_case(path, prefix)
        if path in self.exists_case_cache:
            return self.exists_case_cache[path]
        head, tail = os.path.split(path)
        if not head.startswith(prefix) or not tail:
            # Only perform the check for paths under prefix.
            self.exists_case_cache[path] = True
            return True
        try:
            names = self._listdir_py(head)
            # This allows one to check file name case sensitively in
            # case-insensitive filesystems.
            res = tail in names
        except OSError:
            res = False
        if res:
            # Also recursively check other path components.
            res = self.exists_case(head, prefix)
        self.exists_case_cache[path] = res
        return res

    def isdir(self, path: str) -> bool:
        if _HAS_RUST_CACHE:
            return self._rust.isdir(path)
        st = self._stat_or_none_py(path)
        if st is None:
            return False
        return stat.S_ISDIR(st.st_mode)

    def exists(self, path: str, real_only: bool = False) -> bool:
        if _HAS_RUST_CACHE:
            return self._rust.exists(path, real_only)
        st = self._stat_or_none_py(path)
        if st is None:
            return False
        if real_only:
            dirname = os.path.dirname(path)
            return dirname not in self.fake_package_cache
        return True

    def read(self, path: str) -> bytes:
        if _HAS_RUST_CACHE:
            return self._rust.read(path)
        if path in self.read_cache:
            return self.read_cache[path]
        if path in self.read_error_cache:
            raise self.read_error_cache[path]

        # Need to stat first so that the contents of file are from no
        # earlier instant than the mtime reported by self.stat().
        self._stat_or_none_py(path)

        dirname, basename = os.path.split(path)
        dirname = os.path.normpath(dirname)
        # Check the fake cache.
        if basename == "__init__.py" and dirname in self.fake_package_cache:
            data = b""
        else:
            try:
                with open(path, "rb") as f:
                    data = f.read()
            except OSError as err:
                self.read_error_cache[path] = err
                raise

        self.read_cache[path] = data
        self.hash_cache[path] = hash_digest(data)
        return data

    def hash_digest(self, path: str) -> str:
        if _HAS_RUST_CACHE:
            return self._rust.hash_digest(path)
        if path not in self.hash_cache:
            self.read(path)
        return self.hash_cache[path]

    def samefile(self, f1: str, f2: str) -> bool:
        if _HAS_RUST_CACHE:
            return self._rust.samefile(f1, f2)
        s1 = self._stat_or_none_py(f1)
        s2 = self._stat_or_none_py(f2)
        if s1 is None or s2 is None:
            return False
        return os.path.samestat(s1, s2)

    # --- Pure-Python fallback implementations (used when the Rust extension
    # is not on PYTHONPATH). These preserve the original fscache semantics.

    def _stat_or_none_py(self, path: str) -> os.stat_result | None:
        if path in self.stat_or_none_cache:
            return self.stat_or_none_cache[path]

        st = None
        try:
            st = os.stat(path)
        except OSError:
            if self._init_under_package_root_py(path):
                try:
                    st = self._fake_init_py(path)
                except OSError:
                    pass

        self.stat_or_none_cache[path] = st
        return st

    def _init_under_package_root_py(self, path: str) -> bool:
        """Is this path an __init__.py under a package root?"""
        if not self.package_root:
            return False
        dirname, basename = os.path.split(path)
        if basename != "__init__.py":
            return False
        if not os.path.basename(dirname).isidentifier():
            return False

        st = self._stat_or_none_py(dirname)
        if st is None:
            return False
        else:
            if not stat.S_ISDIR(st.st_mode):
                return False
        ok = False

        # skip if on a different drive
        current_drive, _ = os.path.splitdrive(os.getcwd())
        drive, _ = os.path.splitdrive(path)
        if drive != current_drive:
            return False
        if os.path.isabs(path):
            path = os.path.relpath(path)
        path = os.path.normpath(path)
        for root in self.package_root:
            if path.startswith(root):
                if path == root + basename:
                    ok = False
                    break
                else:
                    ok = True
        return ok

    def _fake_init_py(self, path: str) -> os.stat_result:
        """Prime the cache with a fake __init__.py file."""
        dirname, basename = os.path.split(path)
        assert basename == "__init__.py", path
        assert not os.path.exists(path), path  # Not cached!
        dirname = os.path.normpath(dirname)
        st = os.stat(dirname)  # May raise OSError
        seq: list[float] = list(st)
        seq[stat.ST_MODE] = stat.S_IFREG | 0o444
        seq[stat.ST_INO] = 1
        seq[stat.ST_NLINK] = 1
        seq[stat.ST_SIZE] = 0
        st = os.stat_result(seq)
        self.fake_package_cache.add(dirname)
        return st

    def _listdir_py(self, path: str) -> list[str]:
        path = os.path.normpath(path)
        if path in self.listdir_cache:
            res = self.listdir_cache[path]
            if path in self.fake_package_cache and "__init__.py" not in res:
                res.append("__init__.py")  # Updates the result as well as the cache
            return res
        if path in self.listdir_error_cache:
            raise copy_os_error(self.listdir_error_cache[path])
        try:
            results = os.listdir(path)
        except OSError as err:
            self.listdir_error_cache[path] = copy_os_error(err)
            raise err
        self.listdir_cache[path] = results
        if path in self.fake_package_cache and "__init__.py" not in results:
            results.append("__init__.py")
        return results


def copy_os_error(e: OSError) -> OSError:
    new = OSError(*e.args)
    new.errno = e.errno
    new.strerror = e.strerror
    new.filename = e.filename
    if e.filename2:
        new.filename2 = e.filename2
    return new
