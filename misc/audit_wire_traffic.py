#!/usr/bin/env python3
"""#1624/#1673 wire-traffic audit: rank kernel seams by calls x bytes.

Pinned invocation (run from the repo root; the extension dirs are the
standard scratch builds and `MYPY_AUDIT_ROOT` re-points `mypy` at a
worktree, since the shared `.venv`'s editable finder otherwise resolves
the main checkout):

    MYPY_AUDIT_ROOT=$PWD MYPY_SERIALIZE_STATS=1 \
    MYPY_AUDIT_ARGS="-n0 --no-incremental --dump-build-stats -p mypy -p mypyc" \
    PYTHONPATH=/private/tmp/mypy-rs-<lane>-tk:/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver \
      .venv/bin/python misc/audit_wire_traffic.py

The ranking method and the cost-proxy constants are documented in
`docs/plans/2026-09-14-perf-wire-traffic-audit.md`.

`-n0` above is load-bearing, not decoration (#1820). This proxy counts
seam calls in *this* process, and `mypy_self_check.ini` sets
`num_workers = 4`, so a run without it fans out and the parent observes
0 calls for every seam while still printing a full report. The script
now refuses to start when the effective worker count is not 0, and
exits non-zero when it counted no seam call at all, rather than letting
a hollow zero pass as evidence. It refuses the same way when
`type_kernel` is not importable (#1821), instead of dying in a bare
`ModuleNotFoundError` raised before the hollow-zero guard can run.

Throwaway-style instrumentation, kept in-tree so the ranking is
reproducible. Exact, load-insensitive counters: every serializer
invocation is registered in a pending map keyed by the
identity of the returned bytes; a `rust_*` seam consumes that identity
when it receives the blob as an argument. At exit every serialization
event lands in exactly one bucket:

  useful        consumed by a seam that returned a decision
  deferred      consumed by a seam that returned None / a -1 slot
  unconsumed    never reached any seam at all, for one of three reasons: the
                bytes were built to key the subtype dedup cache and the lookup
                answered from it (`mypy/subtypes.py:916`), the batch buffer
                holding them was dropped at a build boundary, or the
                serialization sat under a dark gate (parity-only)

Buckets 2 and 3 are the waste. Both are attributed to the Python call
site that registered the blob *first*, which is the site that had the
bytes built: a wire-cache hit hands the same object back to a later call
site, and that later site used to take the row instead (#1837).
Re-registrations are counted and marked on the affected rows, so a shared
blob is visible rather than silently moving between call sites. Section B2
splits the unconsumed bucket by the causes above instead of glossing all of
it as "gated-off / parity-only" (#1827).

The report also names the audited run's outcome and the extension build
behind the numbers (#1828). `source tree:` alone cannot distinguish a
rebuilt `type_kernel` from a stale one, and a stale build is invisible a
second time over: the eager `from type_kernel import ...` blocks fall
back on ImportError and `patch_kernel()` registers only the names
`dir(type_kernel)` holds, so a seam the build lacks never appears in
section Z either.

An operator Ctrl-C is not the audited run's failure (#1846): the census is
skipped and the interrupt is re-raised, because the report walks `pending`
and `dedup_hit_blobs` for a run the operator just asked to stop.
"""

from __future__ import annotations

# Worktree prelude: `MYPY_AUDIT_ROOT` pins `mypy` to a worktree, since the
# shared `.venv`'s editable finder otherwise resolves the main checkout.
import os as _os0
import sys as _sys0

_AUDIT_ROOT = _os0.environ.get("MYPY_AUDIT_ROOT")
if _AUDIT_ROOT:
    _sys0.path.insert(0, _AUDIT_ROOT)
    for _f in list(_sys0.meta_path):
        _mod = type(_f).__module__ or ""
        if "editable" in _mod or "editable" in repr(_f):
            _sys0.meta_path.remove(_f)
    import mypy as _mypy_pre

    assert _mypy_pre.__file__.startswith(_AUDIT_ROOT), _mypy_pre.__file__


import collections
import configparser
import os
import sys
import time
import types
from collections.abc import Callable, Mapping
from typing import Any

# rust_* classifier seams whose None is a decided negative, not a deferral.
CLASSIFIER_NEGATIVE_SEAMS: frozenset[str] = frozenset(
    {
        "rust_get_typevarlike_declaration",
        "rust_find_dataclass_transform_spec",
        "rust_find_duplicate",
        "rust_classify_protocol_test_callee",
    }
)
UNIT_HANDLED_SEAMS: frozenset[str] = frozenset(
    {
        "rust_calculate_class_abstract_status",
        "rust_check_protocol_status",
        "rust_calculate_class_vars",
        "rust_add_type_promotion",
    }
)
_BATCH_SLOT_SEAMS: frozenset[str] = frozenset({"rust_is_subtype_batch"})

SERIALIZER_PREFIX = "_serialize"

# Ranked-proxy constants (see report()).
FIXED_FFI_US = 0.63
ENCODE_US_PER_BYTE = 0.04

# id(blob) -> [caller, nbytes, blob]
pending: dict[int, list[Any]] = {}
consumed_ids: set[int] = set()
# Strong references to every registered blob. The buckets key on `id()`, which
# is only stable while the object lives: a freed consumed blob can hand its
# number to a fresh allocation and read as a wire-cache hit.
_tracked_blobs: dict[int, Any] = {}
# id(blob) -> nbytes, kept for every blob ever serialized (incl. wire-cache
# hits) so per-call payload bytes can be attributed to the consuming seam.
blob_lens: dict[int, int] = {}
# High-water mark of pinned blob bytes (pins are never released, so a
# full-repo audit grows them unboundedly): warn once past it (#1715).
_TRACKED_WARN_BYTES = 1 << 30
_tracked_bytes = 0
_tracked_warned = False

useful: collections.Counter[str] = collections.Counter()  # caller -> events
deferred: collections.Counter[str] = collections.Counter()
unconsumed: collections.Counter[str] = collections.Counter()
useful_bytes: collections.Counter[str] = collections.Counter()
deferred_bytes: collections.Counter[str] = collections.Counter()
unconsumed_bytes: collections.Counter[str] = collections.Counter()
# (caller, seam) -> events, for the deferred list only: a useful pair list
# was collected and never read, so it is not collected at all.
deferred_pair: collections.Counter[tuple[str, str]] = collections.Counter()
seam_calls: collections.Counter[str] = collections.Counter()
seam_defers: collections.Counter[str] = collections.Counter()
seam_bytes: collections.Counter[str] = collections.Counter()
# Credited caller -> re-registration events (#1837). A wire-cache hit returns
# the same bytes object to a later call site, so the event stays keyed by
# identity and the site that had it built keeps the row.
reregistered: collections.Counter[str] = collections.Counter()
# Every `rust_*` name wrapped in patch_kernel(), called or not. A
# call-counter census is otherwise silent about seams whose call sites
# are shadowed by another answering seam (they leave no row to rank).
registered_seams: set[str] = set()
# Blob bytes passed to the seam on EVERY call (wire-cache hits included);
# this is the per-call payload the Rust side re-decodes, the dominant
# per-call cost for whole-tree wire seams.
seam_call_bytes: collections.Counter[str] = collections.Counter()
probe_calls: collections.Counter[str] = collections.Counter()
probe_stats: dict[str, dict[str, Any]] = {}
# #1827 cause split for the unconsumed bucket: `mypy.subtypes._subtype_answers`
# answers a repeated pair without calling any seam, so the two keys built for
# that lookup at `mypy/subtypes.py:879-880` never reach a seam by construction.
subtype_dedup: dict[str, int] = {"probes": 0, "hits": 0}
# id(blob) -> the blob itself, for every pair key a dedup hit turned away.
# Pinned like `_tracked_blobs`, since the split keys on `id()`; bounded by the
# keys `_subtype_answers` holds, which the run keeps alive anyway.
dedup_hit_blobs: dict[int, Any] = {}
# Never a silent zero: the report says which of these holds when it prints the
# split, so an uninstrumented run cannot be read as "0 dedup hits" (#1827).
subtype_dedup_status = "NOT INSTALLED: patch_probes() has not run"

# Python-side sites worth counting directly: parity-only oracles and the
# per-SCC resolver-snapshot upkeep the issue names as root cause (a).
PROBE_FUNCS: tuple[tuple[str, str], ...] = (
    ("mypy.checker", "_try_native_stmt_outcome"),
    ("mypy.checker", "_try_native_type_requires_usage"),
    ("mypy.checker", "_try_native_is_unreachable_map"),
    ("mypy.checker", "_try_native_with_exit_suppresses"),
    ("mypy.checker", "_try_native_try_handler_union"),
    ("mypy.checker", "_try_native_except_handler_tests"),
    ("mypy.checkexpr", "_try_native_normalize_callable"),
    ("mypy.checkexpr", "_try_native_classify_call"),
    # Cross-stream claim: with a user plugin loaded every native hook
    # seam is inert. Count the Python-side attempts and the FFI-armed path.
    ("mypy.checkexpr", "_try_native_plugin_hook"),
    ("mypy.checkexpr", "plugin_call_hook_known_absent"),
    ("mypy.checkexpr", "plugin_hook_known_absent"),
)


def site() -> str:
    """`file:line:function` of the frame that made the tracked call."""
    f = sys._getframe(2)
    code = f.f_code
    return f"{os.path.basename(code.co_filename)}:{f.f_lineno}:{code.co_name}"


def _iter_blob_ids(args: tuple[Any, ...], kwargs: dict[str, Any]) -> list[int]:
    out: list[int] = []
    for a in args:
        if isinstance(a, (bytes, bytearray, memoryview)):
            out.append(id(a))
        elif isinstance(a, (list, tuple)):
            for x in a:
                if isinstance(x, (bytes, bytearray, memoryview)):
                    out.append(id(x))
    for a in kwargs.values():
        if isinstance(a, (bytes, bytearray, memoryview)):
            out.append(id(a))
        elif isinstance(a, (list, tuple)):
            for x in a:
                if isinstance(x, (bytes, bytearray, memoryview)):
                    out.append(id(x))
    return out


def register_serializer_result(result: Any, caller: str) -> None:
    if isinstance(result, (bytes, bytearray, memoryview)):
        blobs = [result]
    elif isinstance(result, (list, tuple)):
        blobs = [b for b in result if isinstance(b, (bytes, bytearray, memoryview))]
    else:
        return
    global _tracked_bytes, _tracked_warned
    for b in blobs:
        key = id(b)
        if key not in _tracked_blobs:
            _tracked_bytes += len(b)
        _tracked_blobs[key] = b
        blob_lens[key] = len(b)
        if not _tracked_warned and _tracked_bytes >= _TRACKED_WARN_BYTES:
            _tracked_warned = True
            print(
                f"audit_wire_traffic: pinned blob bytes passed "
                f"{_TRACKED_WARN_BYTES // (1 << 20)} MiB "
                f"({_tracked_bytes} B); the audit may OOM before report()",
                file=sys.stderr,
            )
        if key in consumed_ids:
            # A wire-cache hit returns an already-consumed blob; not a new event.
            continue
        entry = pending.get(key)
        if entry is None:
            pending[key] = [caller, len(b), b]
        elif caller != entry[0]:
            # A wire-cache hit on a blob nothing has consumed yet: the same
            # object registered again, so this is one event and the site that
            # had the bytes built keeps the row (#1837), not the last one.
            # Only a *different* site is a share: a site re-registering its own
            # still-pending blob is a self-hit, which the report line (by
            # "another call site") and the [shared: N] legend must not count.
            reregistered[entry[0]] += 1


def consume(blob: Any, useful_: bool, seam: str) -> None:
    # The only caller passes a blob fetched by `_lookup_blob`, which returns
    # non-None only while `pending` holds that id, so the pop cannot miss.
    key = id(blob)
    entry = pending.pop(key)
    consumed_ids.add(key)
    origin, nbytes = entry[0], entry[1]
    seam_bytes[seam] += nbytes
    if useful_:
        useful[origin] += 1
        useful_bytes[origin] += nbytes
    else:
        deferred[origin] += 1
        deferred_bytes[origin] += nbytes
        deferred_pair[(origin, seam)] += 1


def make_seam(name: str, fn: Callable[..., Any]) -> Callable[..., Any]:
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        # Counted before the call: a seam that raises is still a crossing, and
        # no accounting path may be reachable only on the happy path.
        seam_calls[name] += 1
        result = fn(*args, **kwargs)
        deferred_ = False
        if name in _BATCH_SLOT_SEAMS:
            if isinstance(result, list) and any(s == -1 for s in result):
                deferred_ = True
        elif result is None and name not in CLASSIFIER_NEGATIVE_SEAMS:
            if name not in UNIT_HANDLED_SEAMS:
                deferred_ = True
        if deferred_:
            seam_defers[name] += 1
        for blob_id in _iter_blob_ids(args, kwargs):
            blob = _lookup_blob(blob_id)
            if blob is not None:
                consume(blob, not deferred_, name)
            n = blob_lens.get(blob_id)
            if n is not None:
                seam_call_bytes[name] += n
        return result

    wrapper.__name__ = name
    return wrapper


def _lookup_blob(blob_id: int) -> Any:
    entry = pending.get(blob_id)
    return entry[2] if entry else None


def make_serializer(name: str, fn: Callable[..., Any]) -> Callable[..., Any]:
    def wrapper(*args: Any, **kwargs: Any) -> Any:
        caller = site()
        result = fn(*args, **kwargs)
        register_serializer_result(result, caller)
        return result

    wrapper.__name__ = name
    return wrapper


def patch_kernel() -> int:
    try:
        import type_kernel
    except ImportError as exc:
        # The one cause #1820's message names that never reached the guard:
        # a missing extension dies here, before any counting starts (#1821).
        raise KernelUnavailableError(missing_kernel_reason(exc)) from exc

    n = 0
    for name in dir(type_kernel):
        if not name.startswith("rust_"):
            continue
        attr = getattr(type_kernel, name)
        if not callable(attr):
            continue
        registered_seams.add(name)
        setattr(type_kernel, name, make_seam(name, attr))
        n += 1
    return n


def patch_serializers() -> int:
    n = 0
    seen: set[int] = set()
    for modname, mod in list(sys.modules.items()):
        if not modname.startswith("mypy.") or mod is None:
            continue
        for name in dir(mod):
            if not name.startswith(SERIALIZER_PREFIX):
                continue
            attr = getattr(mod, name)
            if not isinstance(attr, types.FunctionType) or id(attr) in seen:
                continue
            seen.add(id(attr))
            setattr(mod, name, make_serializer(name, attr))
            n += 1
    return n


class _CountingAnswers(dict[Any, Any]):
    """`mypy.subtypes._subtype_answers` with its dedup lookups counted (#1827).

    The dict answers an identical pair from `mypy/subtypes.py:916-920`, which
    returns before any single-pair or batch seam call: the two blobs built at
    `:879-880` for that lookup are unconsumed by construction. Counting `in`
    on the dict is the only place that says so; the buckets cannot.
    """

    def __contains__(self, key: object) -> bool:
        subtype_dedup["probes"] += 1
        hit = super().__contains__(key)
        if hit:
            subtype_dedup["hits"] += 1
            if isinstance(key, tuple) and len(key) >= 2:
                # The key is (left_bytes, right_bytes, ctx_key): pinning the
                # two blobs is what lets report() join them against `pending`
                # by identity instead of extrapolating from the hit count.
                dedup_hit_blobs[id(key[0])] = key[0]
                dedup_hit_blobs[id(key[1])] = key[1]
        return hit


def _reinstall_after_reset(subtypes_mod: Any, orig: Callable[..., Any]) -> Any:
    """Wrap a build-boundary reset so the counting dict survives it."""

    def reset(*args: Any, **kwargs: Any) -> Any:
        result = orig(*args, **kwargs)
        subtypes_mod._subtype_answers = _CountingAnswers(subtypes_mod._subtype_answers)
        return result

    reset.__name__ = orig.__name__
    return reset


def patch_subtype_dedup_probe(subtypes_mod: Any = None) -> bool:
    """Count `_subtype_answers` lookups for the #1827 cause split.

    `_subtype_answers` is *reassigned* (not cleared) at every build boundary
    (`_clear_subtype_batch`, `_set_native_subtype_active`), so the counting
    subclass is re-installed after each of those rather than installed once.
    A probe that counts nothing because a reset replaced its dict would be a
    structural zero in the report, so the install outcome is recorded in
    `subtype_dedup_status` and printed either way.
    """
    global subtype_dedup_status
    if subtypes_mod is None:
        import mypy.subtypes as subtypes_mod

    answers = getattr(subtypes_mod, "_subtype_answers", None)
    if not isinstance(answers, dict):
        subtype_dedup_status = (
            "NOT INSTALLED: mypy.subtypes._subtype_answers is not a dict, so the "
            "dedup share is unmeasured, not zero"
        )
        return False
    # Validated before anything is written, so a refusal cannot leave the
    # module half-patched with a counter that stops at the first reset.
    resets: list[tuple[str, Any]] = []
    for name in ("_clear_subtype_batch", "_set_native_subtype_active"):
        orig = getattr(subtypes_mod, name, None)
        if not isinstance(orig, types.FunctionType):
            subtype_dedup_status = (
                f"NOT INSTALLED: mypy.subtypes.{name} is not a function, so a "
                f"build-boundary reset would drop the counter"
            )
            return False
        resets.append((name, orig))
    subtypes_mod._subtype_answers = _CountingAnswers(answers)
    for name, orig in resets:
        setattr(subtypes_mod, name, _reinstall_after_reset(subtypes_mod, orig))
    subtype_dedup_status = "installed"
    return True


def install_shim(target: Any, name: str, fn: Callable[..., Any]) -> None:
    """Install a monkey-patch on a class without a `method-assign` error.

    The attribute name is a parameter rather than a literal because ruff's B010
    rewrites `setattr(x, "attr", v)` straight back to an assignment, and an
    assignment to a method is the error this avoids (#1838): a type check that
    is already red cannot report a regression. Same shape as the serializer and
    probe installs below, which name their attribute at runtime too.
    """
    setattr(target, name, fn)


def patch_probes() -> int:
    n = 0
    import importlib

    for modname, funcname in PROBE_FUNCS:
        try:
            mod = importlib.import_module(modname)
        except ImportError:
            continue
        fn = getattr(mod, funcname, None)
        if not isinstance(fn, types.FunctionType):
            continue
        key = f"{modname}.{funcname}"

        def wrapper(*args: Any, __fn: Any = fn, __key: str = key, **kwargs: Any) -> Any:
            probe_calls[__key] += 1
            return __fn(*args, **kwargs)

        setattr(mod, funcname, wrapper)
        n += 1
    # Per-SCC resolver snapshot upkeep (issue root cause (a)).
    try:
        from mypy.build import BuildManager

        orig = BuildManager._collect_incremental
        stats: dict[str, Any] = {
            "calls": 0,
            "module_visits": 0,
            "infos": 0,
            "aliases": 0,
            "new_infos": 0,
            "re_pushed_builtins": 0,
            "other_infos": 0,
            "resolver_build_s": 0.0,
        }

        def collect(self: Any, scc: list[str]) -> Any:
            infos, aliases = orig(self, scc)
            snap = self._native_snapshotted
            stats["calls"] += 1
            stats["module_visits"] += len(self.modules)
            stats["infos"] += len(infos)
            stats["aliases"] += len(aliases)
            for info in infos:
                if info.fullname not in snap:
                    stats["new_infos"] += 1
                elif info.fullname.startswith("builtins."):
                    stats["re_pushed_builtins"] += 1
                else:
                    stats["other_infos"] += 1
            return infos, aliases

        def build_resolvers(self: Any, scc: list[str]) -> Any:
            t0 = time.perf_counter()
            try:
                return orig_build(self, scc)
            finally:
                stats["resolver_build_s"] += time.perf_counter() - t0

        orig_build = BuildManager._build_native_resolvers
        install_shim(BuildManager, "_collect_incremental", collect)
        install_shim(BuildManager, "_build_native_resolvers", build_resolvers)
        probe_stats["_collect_incremental"] = stats
        n += 2
    except ImportError:
        pass
    # #1827: the subtype dedup lookups that turn a freshly built pair away.
    n += 1 if patch_subtype_dedup_probe() else 0
    return n


# Extension modules whose build produced the counted numbers (#1828). A stale
# `.so` (the #228 class) otherwise reads exactly like a rebuilt one: same
# report, same `source tree:`.
_EXTENSION_MODULES: tuple[str, ...] = ("type_kernel", "ast_serialize", "module_resolver")


def _stamp(mtime: float) -> str:
    return time.strftime("%Y-%m-%d %H:%M", time.localtime(mtime))


def _newest_source_mtime(root: str | None) -> float | None:
    """Newest `crates/type_kernel/src/**/*.rs` mtime under `root`, else None.

    Same comparison `mypy/test/conftest.py` makes before a parity run: the
    installed `.so` is stale when it predates the sources it should carry.
    None covers a tree without the Rust sources (an installed wheel).
    """
    if root is None:
        return None
    src = os.path.join(root, "crates", "type_kernel", "src")
    newest: float | None = None
    for dirpath, _dirnames, filenames in os.walk(src):
        for filename in filenames:
            if not filename.endswith(".rs"):
                continue
            try:
                mtime = os.path.getmtime(os.path.join(dirpath, filename))
            except OSError:
                continue
            newest = mtime if newest is None or mtime > newest else newest
    return newest


def extension_identity(mypy_mod: types.ModuleType) -> list[str]:
    """Report lines naming the extension builds behind these numbers (#1828).

    Paths and mtimes, because "which build produced this ranking" is the one
    thing `source tree:` cannot answer. `type_kernel` is additionally compared
    against the newest `crates/type_kernel/src` mtime: a stale build is
    invisible twice over, since the eager `from type_kernel import ...` blocks
    fall back silently on ImportError and `patch_kernel()` registers only the
    names `dir(type_kernel)` holds, so a seam the build lacks never even shows
    up in the zero-call section.
    """
    mypy_path = mypy_mod.__file__
    root = os.path.dirname(os.path.dirname(os.path.abspath(mypy_path))) if mypy_path else None
    lines: list[str] = []
    mtimes: dict[str, float] = {}
    for name in _EXTENSION_MODULES:
        module = sys.modules.get(name)
        path = getattr(module, "__file__", None) if module is not None else None
        if not path:
            lines.append(f"  {name}: NOT LOADED (this run used no part of it)")
            continue
        try:
            mtimes[name] = os.path.getmtime(path)
            shown = _stamp(mtimes[name])
        except OSError:
            shown = "mtime unreadable"
        lines.append(f"  {name}: {path} (mtime {shown})")
    newest = _newest_source_mtime(root)
    if newest is None:
        lines.append("  crates/type_kernel/src: not in this source tree, staleness not checked")
    else:
        lines.append(f"  crates/type_kernel/src newest: {_stamp(newest)}")
        built = mtimes.get("type_kernel")
        if built is not None and built < newest:
            lines.append(
                f"  STALE: type_kernel predates {_stamp(newest)}; rebuild it "
                f"(AGENTS.md, 'Type kernel build order') before ranking"
            )
    return lines


def _buffered_blob_ids() -> set[int] | None:
    """ids of the blobs `mypy.subtypes._subtype_batch` still holds, else None.

    Settles the competing explanation for the unconsumed bucket (#1827): a
    buffered pair whose buffer was dropped at a build boundary never reached a
    seam either. None (unreadable buffer) must not print as a zero.
    """
    subtypes_mod = sys.modules.get("mypy.subtypes")
    batch = getattr(subtypes_mod, "_subtype_batch", None) if subtypes_mod is not None else None
    if not isinstance(batch, list):
        return None
    ids: set[int] = set()
    for row in batch:
        if isinstance(row, tuple) and len(row) >= 2:
            ids.add(id(row[0]))
            ids.add(id(row[1]))
    return ids


def _shared_marker(caller: str) -> str:
    """Row suffix for a credited site whose blobs a second site also registered.

    A wire-cache hit hands the same bytes object back, so the event stays with
    the site that built it and the later site only counts here (#1837). Without
    the marker the sharing is invisible, and the row reads as that site's own
    cost.
    """
    n = reregistered.get(caller, 0)
    return f"  [shared: {n}]" if n else ""


def report(run_status: str) -> None:
    out = sys.stderr
    for entry in pending.values():
        origin, nbytes = entry[0], entry[1]
        unconsumed[origin] += 1
        unconsumed_bytes[origin] += nbytes
    tot = sum(useful.values()) + sum(deferred.values()) + sum(unconsumed.values())
    import mypy as _mypy

    n_reg = len(registered_seams)
    n_called = len(seam_calls)
    n_zero = n_reg - n_called
    print("\n=== #1624 wire-waste audit (self-check) ===", file=out)
    print(f"[audit] {n_reg} seams registered, {n_called} called, {n_zero} zero-call", file=out)
    print(f"source tree: {_mypy.__file__}", file=out)
    print(f"audited run: {run_status}", file=out)
    for line in extension_identity(_mypy):
        print(line, file=out)
    print(f"serialization events: {tot}", file=out)
    print(f"  useful:      {sum(useful.values())} ({sum(useful_bytes.values())} B)", file=out)
    print(f"  deferred:    {sum(deferred.values())} ({sum(deferred_bytes.values())} B)", file=out)
    print(
        f"  unconsumed:  {sum(unconsumed.values())} ({sum(unconsumed_bytes.values())} B)", file=out
    )
    # Printed even at zero: a site's row is a bound, not an exact attribution,
    # whenever a wire-cache hit handed its blob to another call site (#1837).
    print(
        f"  wire-cache shares: {sum(reregistered.values())} re-registration(s) by "
        f"another call site (#1837); the blob keeps its builder's row, marked "
        f"[shared: N] in sections A, B and C",
        file=out,
    )

    print("\n--- A. serialized then SEAM DEFERRED (top 25 call sites) ---", file=out)
    for site_, cnt in deferred.most_common(25):
        print(
            f"  {cnt:8d}  {deferred_bytes[site_]:10d}B  {site_}{_shared_marker(site_)}", file=out
        )
    print("\n--- A2. same, top 25 (call site, seam) pairs ---", file=out)
    for (site_, seam), cnt in deferred_pair.most_common(25):
        print(f"  {cnt:8d}  {seam:<45} {site_}", file=out)

    print("\n--- B. serialized, NEVER consumed by any seam (top 25) ---", file=out)
    for site_, cnt in unconsumed.most_common(25):
        print(
            f"  {cnt:8d}  {unconsumed_bytes[site_]:10d}B  {site_}{_shared_marker(site_)}", file=out
        )

    print("\n--- B2. unconsumed bucket by cause (#1827) ---", file=out)
    n_unconsumed = sum(unconsumed.values())
    if n_unconsumed:
        site_, cnt = unconsumed.most_common(1)[0]
        share = 100.0 * cnt / n_unconsumed
        print(
            f"  dominant call site: {site_} {cnt} events, "
            f"{unconsumed_bytes[site_]}B ({share:.1f}% of the bucket)",
            file=out,
        )
    print(f"  subtype dedup probe: {subtype_dedup_status}", file=out)
    if subtype_dedup_status == "installed":
        hits, probes = subtype_dedup["hits"], subtype_dedup["probes"]
        print(f"  mypy.subtypes._subtype_answers: {hits} dedup hits of {probes} lookups", file=out)
        # The join is by blob identity against this run's `pending` map, not by
        # extrapolating from the hit count: one wire-cached blob is registered
        # once and looked up many times.
        turned_away = [entry for entry in pending.values() if id(entry[2]) in dedup_hit_blobs]
        n_dedup = len(turned_away)
        n_dedup_bytes = sum(entry[1] for entry in turned_away)
        share = 100.0 * n_dedup / n_unconsumed if n_unconsumed else 0.0
        print(
            f"  a dedup hit turned these away: {n_dedup} of the {n_unconsumed} "
            f"unconsumed events ({share:.1f}%, {n_dedup_bytes} B)",
            file=out,
        )
        residual = n_unconsumed - n_dedup
        if residual > 0:
            print(
                f"  other causes: {residual} events, a batch dropped at a build "
                f"boundary or a serialization under a dark gate; not split further here",
                file=out,
            )
        if n_dedup:
            # Recorded because the share dominates: no gate flip or reordering
            # removes these bytes, the dedup key IS the wire bytes built for it.
            print(
                "  reduction path: the cache key is the serialization, so it cannot "
                "be skipped by reordering; only a memo keyed on object identity, "
                "taken before the bytes are built, avoids rebuilding them",
                file=out,
            )
    buffered_ids = _buffered_blob_ids()
    if buffered_ids is None:
        print("  mypy.subtypes._subtype_batch: NOT READABLE, buffered share unmeasured", file=out)
    else:
        n_buffered = sum(1 for key in pending if key in buffered_ids)
        print(f"  still in mypy.subtypes._subtype_batch at exit: {n_buffered} blobs", file=out)

    print("\n--- C. useful: consumed and decided (top 15) ---", file=out)
    for site_, cnt in useful.most_common(15):
        print(f"  {cnt:8d}  {useful_bytes[site_]:10d}B  {site_}{_shared_marker(site_)}", file=out)

    print("\n--- ALL seams with calls (calls / defers / bytes first-consumed) ---", file=out)
    print(
        f"  total seam calls (armed gates only; "
        f"lower bound on the registered surface): {sum(seam_calls.values())}",
        file=out,
    )
    for seam, cnt in seam_calls.most_common():
        d = seam_defers.get(seam, 0)
        b = seam_bytes.get(seam, 0)
        print(f"  {seam}: {cnt} calls, {d} defers, {b}B", file=out)

    zero = sorted(registered_seams - seam_calls.keys())
    print(f"\n--- Z. registered seams with ZERO calls ({len(zero)}) ---", file=out)
    for seam in zero:
        print(f"  {seam}", file=out)

    # Ranked cost proxy: calls * fixed FFI + per-call payload bytes (re-decoded
    # on every wire call) + first-consumed bytes. The constants and their
    # calibration are documented in the 2026-09-14 wire-traffic audit doc.
    print(
        "\n--- RANKED proxy: calls*0.63us + call_bytes*0.04us/B + encode_bytes*0.04us/B ---",
        file=out,
    )
    print(
        "  proxy_us   seam                                      calls  callMB  encMB  B/call  wire?",
        file=out,
    )
    rows = []
    for seam, cnt in seam_calls.items():
        cb = seam_call_bytes.get(seam, 0)
        eb = seam_bytes.get(seam, 0)
        proxy = cnt * FIXED_FFI_US + (cb + eb) * ENCODE_US_PER_BYTE
        rows.append((proxy, seam, cnt, cb, eb))
    rows.sort(reverse=True)
    for proxy, seam, cnt, cb, eb in rows[:45]:
        bpc = cb / cnt if cnt else 0.0
        wire = "wire" if cb or eb else "live"
        print(
            f"  {proxy:8.1f}us  {seam:<40} {cnt:>8} {cb/1e6:7.2f} {eb/1e6:6.2f} {bpc:7.1f}  {wire}",
            file=out,
        )

    print("\n--- probed python-side sites ---", file=out)
    for key, cnt in probe_calls.most_common():
        print(f"  {cnt:8d}  {key}", file=out)
    for key, st in probe_stats.items():
        print(f"  {key}: {st}", file=out)
    try:
        from mypy import checkexpr as _ce

        plugins = _ce._native_plugin_hook_plugins
        print(
            "\n--- plugin-hook registry state at exit ---\n"
            f"  _native_plugin_hook_has_user_plugins = "
            f"{_ce._native_plugin_hook_has_user_plugins}\n"
            f"  registry installed = {_ce._native_plugin_hook_registry is not None}\n"
            f"  plugin instances = {len(plugins) if plugins is not None else None}",
            file=out,
        )
    except Exception as exc:
        print(f"  plugin-state probe failed: {exc!r}", file=out)


# --- fail-loud guards (#1820/#1821) ----------------------------------------
# An in-process proxy cannot observe a fanned-out run: that is a hollow zero,
# so refuse it the #1789/#1799 way (exit non-zero, name the setting).
DEFAULT_CONFIG_FILE = "mypy_self_check.ini"
FANOUT_REFUSAL = (
    "audit_wire_traffic: REFUSING TO RUN (#1820).\n"
    "  the audited check would fan out to worker processes, and this audit counts\n"
    "  seam calls with an in-process proxy: every seam would report 0 calls.\n"
    "  effective num_workers = {value}\n"
    "  setting: {source}\n"
    "  remedy: run single-process. Pass -n0 (the pinned invocation in the module\n"
    "  docstring does exactly that), or set num_workers = 0 in the config.\n"
)
HOLLOW_ZERO_REFUSAL = (
    "audit_wire_traffic: HOLLOW ZERO (#1820).\n"
    "  the proxy recorded {calls} calls across {seams} registered seam(s), so the\n"
    "  report above is NOT evidence of a dark kernel. Either the run fanned out to\n"
    "  workers (num_workers > 0), or it registered no rust_* seam at all (a stub or\n"
    "  missing extension), or every native gate was dark, or the run aborted before\n"
    "  checking any code.\n"
    "  remedy: re-run single-process with -n0 and the extension .so dirs on\n"
    "  PYTHONPATH, and read the mypy status in the report above.\n"
)
AUDITED_RUN_FAILED_REFUSAL = (
    "audit_wire_traffic: THE AUDITED RUN FAILED (#1828).\n"
    "  the report above was printed from a run that did not finish its work, so it\n"
    "  is a truncated census and not a ranking: every seam it is missing was never\n"
    "  reached. This is decided from the run's outcome, not from the tally, because\n"
    "  a run that aborted after one counted call still produced a partial report.\n"
    "  failure: {failure}\n"
    "  remedy: fix the crash (a stale extension build is the usual cause, see the\n"
    "  extension identity lines in the report above) and re-run.\n"
)
OPERATOR_ABORT_NOTICE = (
    "\naudit_wire_traffic: ABORTED by the operator (#1846).\n"
    "  KeyboardInterrupt reached the audited run: that is not the run's own\n"
    "  failure, so it is not recorded as one, and no census is printed. The\n"
    "  report walks `pending` and `dedup_hit_blobs`, which is the work Ctrl-C\n"
    "  asked to stop. The interrupt is re-raised, so the exit status is the\n"
    "  interpreter's (130), not this tool's.\n"
    "  seam calls counted before the interrupt: {calls}\n"
)
MISSING_KERNEL_REFUSAL = (
    "audit_wire_traffic: {headline} (#1821).\n"
    "  the audit wraps every `rust_*` seam with a counting proxy, so an importable\n"
    "  type_kernel must exist before any work starts. `import type_kernel` failed.\n"
    "  A failure of the extension itself and a failure inside its own import\n"
    "  (a missing `librt`, say) are indistinguishable from the exception class\n"
    "  alone, so the headline names the extension as one cause and `cause:` below\n"
    "  carries the failure verbatim.\n"
    "  cause: {cause}\n"
    "  effect: nothing would be registered and the report would be a hollow zero,\n"
    "  and this failure precedes the #1820 hollow-zero guard, so it must refuse here.\n"
    "  remedy: build the extension (AGENTS.md, 'Type kernel build order') and put\n"
    "  its scratch dir on PYTHONPATH together with the ast_serialize and\n"
    "  module_resolver dirs, exactly as pinned in the module docstring:\n"
    "  PYTHONPATH=/private/tmp/mypy-rs-<lane>-tk"
    ":/private/tmp/mypy-rs-local-ast:/private/tmp/mypy-rs-local-resolver\n"
    "  a no-scratch run is not a remedy on its own: the shared .venv ships a\n"
    "  type-stub-only ast_serialize (#1800), so that run dies on the parser instead.\n"
)


class KernelUnavailableError(RuntimeError):
    """`type_kernel` is not importable: refuse before any work (#1821)."""


# The headline is derived from the cause (#1831): `except ImportError` also
# catches a failure raised inside type_kernel's own initialisation (the missing
# `librt` case above), where the extension itself is present.
MISSING_EXTENSION_HEADLINE = "MISSING type_kernel EXTENSION"
KERNEL_UNAVAILABLE_HEADLINE = "type_kernel UNAVAILABLE"


def missing_kernel_reason(cause: BaseException) -> str:
    """The refusal message for an unimportable `type_kernel`.

    The cause is carried verbatim, class name included: a transitive
    ImportError (a missing `librt`, say) must stay diagnosable instead of
    being flattened into "no type_kernel extension". The headline follows the
    cause for the same reason: only `name == "type_kernel"` is the extension
    missing, anything else failed while type_kernel imported something.
    """
    headline = (
        MISSING_EXTENSION_HEADLINE
        if getattr(cause, "name", None) == "type_kernel"
        else KERNEL_UNAVAILABLE_HEADLINE
    )
    return MISSING_KERNEL_REFUSAL.format(
        headline=headline, cause=f"{type(cause).__name__}: {cause}"
    )


def audit_argv(extra: str | None) -> list[str]:
    """The argv handed to `mypy.main.main`. The guards never rewrite it."""
    if extra:
        return ["mypy", "--config-file", DEFAULT_CONFIG_FILE] + extra.split()
    return [
        "mypy",
        "--config-file",
        DEFAULT_CONFIG_FILE,
        "-n0",
        "--no-incremental",
        "-p",
        "mypy",
        "-p",
        "mypyc",
    ]


def _int_or_none(raw: str) -> int | None:
    try:
        return int(raw)
    except ValueError:
        return None


def _config_path(argv: list[str]) -> str:
    """The config file mypy will read: last `--config-file`, else the default."""
    path = DEFAULT_CONFIG_FILE
    i = 0
    while i < len(argv):
        tok = argv[i]
        if tok == "--config-file" and i + 1 < len(argv):
            path = argv[i + 1]
            i += 1
        elif tok.startswith("--config-file="):
            path = tok.split("=", 1)[1]
        i += 1
    return path


def _cli_worker_spelling(argv: list[str]) -> tuple[str, str] | None:
    """The last `-n`/`--num-workers` in argv as (spelling, raw value), else None.

    Last occurrence wins, matching argparse's store action. Covers `-n<k>`,
    `-n <k>`, `--num-workers <k>` and `--num-workers=<k>`. A `-n<k>` token
    whose remainder is not an integer is left to mypy's own parser.
    """
    found: tuple[str, str] | None = None
    i = 0
    while i < len(argv):
        tok = argv[i]
        if tok in ("-n", "--num-workers"):
            if i + 1 < len(argv):
                found = (f"{tok} {argv[i + 1]}", argv[i + 1])
            i += 2
            continue
        if tok.startswith("--num-workers="):
            found = (tok, tok.split("=", 1)[1])
        elif len(tok) > 2 and tok.startswith("-n") and _int_or_none(tok[2:]) is not None:
            found = (tok, tok[2:])
        i += 1
    return found


def _config_num_workers(path: str) -> str | None:
    """`num_workers` from the config file's `[mypy]` section, else None.

    None also covers an absent file, which mypy itself rejects loudly
    (`Cannot find config file`); nothing is read here that mypy will not
    read as well, so the guard cannot disagree with the run it gates.
    """
    parser = configparser.ConfigParser(interpolation=None)
    if not parser.read(path):
        return None
    return parser.get("mypy", "num_workers", fallback=None)


def _fanout_decision(raw: str, source: str) -> str | None:
    """Refuse unless `raw` resolves to num_workers == 0."""
    value = _int_or_none(raw.strip())
    if value == 0:
        return None
    shown = str(value) if value is not None else f"{raw!r} (not an integer)"
    return FANOUT_REFUSAL.format(value=shown, source=source)


def fanout_reason(argv: list[str], environ: Mapping[str, str]) -> str | None:
    """The refusal message when the audited run would fan out, else None.

    Precedence mirrors mypy's `process_options`: an explicit `-n` beats
    MYPY_NUM_WORKERS, which beats the config file. Checked before any
    work, so a fan-out is never "measured" first.
    """
    spelling = _cli_worker_spelling(argv)
    if spelling is not None:
        return _fanout_decision(spelling[1], f"MYPY_AUDIT_ARGS {spelling[0]!r}")
    env_raw = environ.get("MYPY_NUM_WORKERS", "").strip()
    if env_raw:
        return _fanout_decision(env_raw, "MYPY_NUM_WORKERS")
    config_path = _config_path(argv)
    config_raw = _config_num_workers(config_path)
    if config_raw is not None and config_raw.strip():
        return _fanout_decision(config_raw, f"{config_path} [mypy] num_workers")
    return None


def hollow_zero_reason(calls: int, seams: int) -> str | None:
    """The refusal message when the proxy observed nothing, else None."""
    if calls > 0:
        return None
    return HOLLOW_ZERO_REFUSAL.format(calls=calls, seams=seams)


def classify_run_exit(code: object) -> str | None:
    """The failure detail for a `SystemExit` code, else None (#1828).

    `clean_exit=True` makes the audited run's own exits raise as well, so a
    `SystemExit` is not automatically a crash: mypy returns normally on a clean
    run, exits 1 when it found type errors (every module was still checked, and
    a census does not care), and exits 2 for a serious error or when blockers
    stopped analysis. Only the last one truncates the census.
    """
    if code is None or code == 0 or code == 1:
        return None
    return f"mypy exited {code!r}"


def run_failure_reason(failure: str | None) -> str | None:
    """The refusal when the audited run itself failed, else None (#1828).

    Independent of the seam tally, because the tally is exactly what a
    truncated run still produces: one counted call before an abort was enough
    to exit 0 with a partial report.
    """
    if failure is None:
        return None
    return AUDITED_RUN_FAILED_REFUSAL.format(failure=failure)


def evidence_refusals(total_calls: int, seams: int, run_failure: str | None) -> list[str]:
    """Every reason the printed report cannot be read as evidence (#1820/#1828).

    An empty list means the report is readable. Collecting all of them instead
    of returning the first keeps the crashed-run message and the hollow-zero
    message visible together: a run that aborted before counting anything is
    both.
    """
    reasons = []
    failed = run_failure_reason(run_failure)
    if failed is not None:
        reasons.append(failed)
    hollow = hollow_zero_reason(total_calls, seams)
    if hollow is not None:
        reasons.append(hollow)
    return reasons


def main() -> int:
    argv = audit_argv(os.environ.get("MYPY_AUDIT_ARGS"))
    refusal = fanout_reason(argv, os.environ)
    if refusal is not None:
        # Before any work: a fanned-out run must not produce a report at all.
        print(refusal, file=sys.stderr)
        return 1
    _t0 = time.perf_counter()
    try:
        n_seams = patch_kernel()
    except KernelUnavailableError as exc:
        # Before any work, like the fan-out refusal: no report at all.
        print(str(exc), file=sys.stderr)
        return 1
    import mypy.main

    n_ser = patch_serializers()
    n_probe = patch_probes()
    print(
        f"[audit] patched {n_seams} seams, {n_ser} serializers, {n_probe} probes", file=sys.stderr
    )
    semanal_flag = os.environ.get("MYPY_ENABLE_NATIVE_SEMANAL")
    if semanal_flag:
        gate_state = "armed (native_type_kernel default on)"
    else:
        gate_state = "DARK"
    print(
        f"[audit] MYPY_ENABLE_NATIVE_SEMANAL={semanal_flag!r}; "
        f"semanal/semanal-visitor gates {gate_state}",
        file=sys.stderr,
    )
    sys.argv = argv
    # The audited run's outcome is part of the evidence (#1828), and it is
    # recorded on the abnormal path: every exit and every raise sets it.
    run_failure: str | None = None
    run_status = "not started"
    try:
        mypy.main.main(clean_exit=True)
        run_status = "completed, mypy.main returned"
    except SystemExit as exc:
        run_failure = classify_run_exit(exc.code)
        if run_failure is None:
            run_status = f"completed, mypy exit {exc.code!r}"
        else:
            run_status = f"FAILED, {run_failure}"
    except KeyboardInterrupt:
        # A Ctrl-C is an operator action, not the audited run's failure
        # (#1846): recording it as one attributed the abort to mypy and let
        # the census below walk `pending` for a run already asked to stop.
        print(OPERATOR_ABORT_NOTICE.format(calls=sum(seam_calls.values())), file=sys.stderr)
        raise
    except BaseException as exc:
        run_failure = f"{type(exc).__name__}: {exc}"
        run_status = f"FAILED, raised {run_failure}"
        # The counters are the whole point: report them on the error path too.
        import traceback

        traceback.print_exc()
    finally:
        total_run_s = time.perf_counter() - _t0
        print(f"\n[audit] total run wall {total_run_s:.1f}s (load-contaminated)", file=sys.stderr)
    # The report is printed on every path that reaches this line; the guards
    # only decide whether it may be read as evidence. It used to sit in the
    # `finally`, which is what made an operator abort print a census (#1846).
    report(run_status)
    total_calls = sum(seam_calls.values())
    refusals = evidence_refusals(total_calls, len(registered_seams), run_failure)
    for reason in refusals:
        print(reason, file=sys.stderr)
    return 1 if refusals else 0


if __name__ == "__main__":
    sys.exit(main())
