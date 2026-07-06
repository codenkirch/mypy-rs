"""Python adapter for the native Rust module resolver.

Mirrors the shape of ``mypy.nativeparse``: an unconditional top-level import
of the compiled extension (the "extension missing -> skip tests" gate lives
in the test file, not here), and a thin wrapper that unpacks the plain
record returned by Rust into the mypy-native ``ModuleSearchResult`` type.

Policy (options interpretation, diagnostics, result caching, the
WRONG_WORKING_DIRECTORY decoration, follow_imports policy) stays in Python;
this module only resolves a module id to a path or ``ModuleNotFoundReason``.

Filesystem access is owned by Rust: the resolver reads the real filesystem
via ``std::fs`` directly, with no per-call PyO3 callbacks. The dispatch gate
in ``FindModuleCache._resolve`` routes only cold, real-filesystem runs here;
daemon (``fine_grained_incremental``) and Bazel runs stay on the Python
``_find_module`` path so the daemon VFS and Bazel fake-init synthesis remain
Python-owned until they are ported or retired.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import module_resolver

from mypy.modulefinder import ModuleNotFoundReason

if TYPE_CHECKING:
    from mypy.errors import Errors
    from mypy.modulefinder import SearchPaths, StdlibVersions
    from mypy.nodes import ImportBase, MypyFile
    from mypy.options import Options

# Rust result-kind sentinels. Must match crates/module_resolver/src/lib.rs.
_FOUND = 255


def make_resolver(
    *,
    options: Options,
    search_paths: SearchPaths,
    stdlib_versions: StdlibVersions,
    stub_flat: set[str],
    stub_namespace: dict[str, str],
) -> module_resolver.NativeResolver:
    """Construct a long-lived ``NativeResolver`` owned by ``FindModuleCache``.

    All stable resolver config (search paths, stubinfo tables, resolver
    flags) is set once here and reused across every ``find_module`` call on
    the owning cache. Only per-call varying args (``id``, ``use_typeshed``,
    ``follow_untyped_imports``) cross the PyO3 boundary on each resolve.
    """
    stdlib_list = [
        (name, lo, hi) for name, (lo, hi) in stdlib_versions.items()
    ]
    return module_resolver.NativeResolver(
        options.namespace_packages,
        options.use_builtins_fixtures,
        list(search_paths.python_path),
        list(search_paths.mypy_path),
        list(search_paths.package_path),
        list(search_paths.typeshed_path),
        options.python_version,
        stdlib_list,
        sorted(stub_flat),
        sorted(stub_namespace.items()),
    )


def resolve_module(
    resolver: module_resolver.NativeResolver,
    id: str,
    *,
    use_typeshed: bool,
    options: Options,
) -> tuple[str | ModuleNotFoundReason, bool]:
    """Resolve a module id using a long-lived ``NativeResolver``.

    Returns ``(result, can_cache)`` mirroring the contract of
    ``FindModuleCache._find_module``: ``result`` is either the resolved path
    or a ``ModuleNotFoundReason``, and ``can_cache`` indicates whether the
    caller may memoize the result.
    """
    # ``follow_untyped_imports`` is per-module; compute it once here so Rust
    # never touches the ``Options`` object.
    follow_untyped = options.clone_for_module(id).follow_untyped_imports

    kind, path, can_cache = resolver.resolve(id, use_typeshed, follow_untyped)
    if kind == _FOUND:
        assert path is not None
        return path, can_cache
    return ModuleNotFoundReason(kind), can_cache


# Rust ImportRecord kinds. Must match crates/module_resolver/src/lib.rs.
_IMP_IMPORT = 0
_IMP_IMPORTFROM = 1
_IMP_IMPORTALL = 2


def _import_to_record(
    imp: ImportBase,
) -> tuple[int, str, int, list[tuple[str, str | None]], int, bool, bool, bool, bool]:
    """Flatten an ``ImportBase`` node into the plain-record shape Rust expects.

    The field order must match the ``ImportRecord`` struct's declaration
    order in ``crates/module_resolver/src/lib.rs`` (the struct derives
    ``FromPyObject``, so PyO3 reads fields positionally from the tuple).
    """
    from mypy import nodes

    if isinstance(imp, nodes.Import):
        return (
            _IMP_IMPORT,
            imp.ids[0][0] if imp.ids else "",
            0,  # relative — Import is always absolute
            imp.ids,
            imp.line,
            imp.is_top_level,
            imp.is_unreachable,
            imp.is_unreachable_dependency,
            imp.is_mypy_only,
        )
    if isinstance(imp, nodes.ImportFrom):
        return (
            _IMP_IMPORTFROM,
            imp.id,
            imp.relative,
            imp.names,
            imp.line,
            imp.is_top_level,
            imp.is_unreachable,
            imp.is_unreachable_dependency,
            imp.is_mypy_only,
        )
    if isinstance(imp, nodes.ImportAll):
        return (
            _IMP_IMPORTALL,
            imp.id,
            imp.relative,
            [],  # ImportAll has no names list
            imp.line,
            imp.is_top_level,
            imp.is_unreachable,
            imp.is_unreachable_dependency,
            imp.is_mypy_only,
        )
    raise TypeError(f"Unexpected import type: {type(imp).__name__}")


def compute_dep_records(
    resolver: module_resolver.NativeResolver,
    *,
    file: MypyFile,
    known_modules: set[str],
    errors: Errors,
    options: Options,
) -> list[tuple[int, str, int]]:
    """Compute ``(priority, module_id, line)`` records for ``file``'s imports.

    Mirrors ``BuildManager.all_imported_modules_in_file``: walks the import
    list, computes priorities, corrects relative imports, expands ancestor
    packages, and discriminates submodule-vs-name. The walk runs entirely in
    Rust (via the long-lived ``NativeResolver``); only the import records
    (already deserialized AST nodes) and the known-modules set cross the
    boundary.

    The known-modules set is rebuilt per call from
    ``manager.modules`` + ``source_set.source_modules`` so Rust's
    ``is_module`` check mirrors the build-graph fast path before falling back
    to filesystem resolution.
    """
    import_records = [_import_to_record(imp) for imp in file.imports]
    records, error = resolver.compute_dep_records(
        file.fullname,
        file.path,
        import_records,
        known_modules,
    )
    if error is not None:
        # Report the blocking relative-import error, mirroring
        # ``BuildManager.correct_rel_imp``'s ``self.error(...)`` call.
        line, message = error
        errors.set_file(file.path, file.fullname, options=options)
        errors.report(line, 0, message, blocker=True)
    return records
