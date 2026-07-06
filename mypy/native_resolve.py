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
    from mypy.modulefinder import SearchPaths, StdlibVersions
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
