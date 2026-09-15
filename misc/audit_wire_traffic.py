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

Throwaway-style instrumentation, kept in-tree so the ranking is
reproducible. Exact, load-insensitive counters: every serializer
invocation is registered in a pending map keyed by the
identity of the returned bytes; a `rust_*` seam consumes that identity
when it receives the blob as an argument. At exit every serialization
event lands in exactly one bucket:

  useful        consumed by a seam that returned a decision
  deferred      consumed by a seam that returned None / a -1 slot
  unconsumed    never reached any seam at all (gated-off / parity-only)

Buckets 2 and 3 are the waste. Both are attributed to the Python call
site that paid for the serialization.
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
import os
import sys
import types
from typing import Any, Callable

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

useful: collections.Counter = collections.Counter()  # caller -> events
deferred: collections.Counter = collections.Counter()
unconsumed: collections.Counter = collections.Counter()
useful_bytes: collections.Counter = collections.Counter()
deferred_bytes: collections.Counter = collections.Counter()
unconsumed_bytes: collections.Counter = collections.Counter()
# (caller, seam) -> events, for the deferred list only: a useful pair list
# was collected and never read, so it is not collected at all.
deferred_pair: collections.Counter = collections.Counter()
seam_calls: collections.Counter = collections.Counter()
seam_defers: collections.Counter = collections.Counter()
seam_bytes: collections.Counter = collections.Counter()
# Blob bytes passed to the seam on EVERY call (wire-cache hits included);
# this is the per-call payload the Rust side re-decodes, the dominant
# per-call cost for whole-tree wire seams.
seam_call_bytes: collections.Counter = collections.Counter()
probe_calls: collections.Counter = collections.Counter()
probe_stats: dict[str, dict[str, Any]] = {}

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
    for b in blobs:
        key = id(b)
        _tracked_blobs[key] = b
        blob_lens[key] = len(b)
        if key in consumed_ids:
            # A wire-cache hit returns an already-consumed blob; not a new event.
            continue
        pending[key] = [caller, len(b), b]


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
    import type_kernel

    n = 0
    for name in dir(type_kernel):
        if not name.startswith("rust_"):
            continue
        attr = getattr(type_kernel, name)
        if not callable(attr):
            continue
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
        import time as _time

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
            t0 = _time.perf_counter()
            try:
                return orig_build(self, scc)
            finally:
                stats["resolver_build_s"] += _time.perf_counter() - t0

        orig_build = BuildManager._build_native_resolvers
        BuildManager._collect_incremental = collect
        BuildManager._build_native_resolvers = build_resolvers
        probe_stats["_collect_incremental"] = stats
        n += 2
    except ImportError:
        pass
    return n


def report() -> None:
    out = sys.stderr
    for entry in pending.values():
        origin, nbytes = entry[0], entry[1]
        unconsumed[origin] += 1
        unconsumed_bytes[origin] += nbytes
    tot = sum(useful.values()) + sum(deferred.values()) + sum(unconsumed.values())
    import mypy as _mypy

    print("\n=== #1624 wire-waste audit (self-check) ===", file=out)
    print(f"source tree: {_mypy.__file__}", file=out)
    print(f"serialization events: {tot}", file=out)
    print(f"  useful:      {sum(useful.values())} ({sum(useful_bytes.values())} B)", file=out)
    print(f"  deferred:    {sum(deferred.values())} ({sum(deferred_bytes.values())} B)", file=out)
    print(f"  unconsumed:  {sum(unconsumed.values())} ({sum(unconsumed_bytes.values())} B)", file=out)

    print("\n--- A. serialized then SEAM DEFERRED (top 25 call sites) ---", file=out)
    for site_, cnt in deferred.most_common(25):
        print(f"  {cnt:8d}  {deferred_bytes[site_]:10d}B  {site_}", file=out)
    print("\n--- A2. same, top 25 (call site, seam) pairs ---", file=out)
    for (site_, seam), cnt in deferred_pair.most_common(25):
        print(f"  {cnt:8d}  {seam:<45} {site_}", file=out)

    print("\n--- B. serialized, NEVER consumed by any seam (top 25) ---", file=out)
    for site_, cnt in unconsumed.most_common(25):
        print(f"  {cnt:8d}  {unconsumed_bytes[site_]:10d}B  {site_}", file=out)

    print("\n--- C. useful: consumed and decided (top 15) ---", file=out)
    for site_, cnt in useful.most_common(15):
        print(f"  {cnt:8d}  {useful_bytes[site_]:10d}B  {site_}", file=out)

    print("\n--- ALL seams with calls (calls / defers / bytes first-consumed) ---", file=out)
    for seam, cnt in seam_calls.most_common():
        d = seam_defers.get(seam, 0)
        b = seam_bytes.get(seam, 0)
        print(f"  {seam}: {cnt} calls, {d} defers, {b}B", file=out)

    # Ranked cost proxy: calls * fixed FFI + per-call payload bytes (re-decoded
    # on every wire call) + first-consumed bytes. The constants and their
    # calibration are documented in the 2026-09-14 wire-traffic audit doc.
    print("\n--- RANKED proxy: calls*0.63us + call_bytes*0.04us/B + encode_bytes*0.04us/B ---", file=out)
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
    except Exception as exc:  # noqa: BLE001
        print(f"  plugin-state probe failed: {exc!r}", file=out)


def main() -> int:
    import time as _time

    _t0 = _time.perf_counter()
    n_seams = patch_kernel()
    import mypy.main

    n_ser = patch_serializers()
    n_probe = patch_probes()
    print(
        f"[audit] patched {n_seams} seams, {n_ser} serializers, {n_probe} probes",
        file=sys.stderr,
    )
    extra = os.environ.get("MYPY_AUDIT_ARGS")
    if extra:
        sys.argv = ["mypy", "--config-file", "mypy_self_check.ini"] + extra.split()
    else:
        sys.argv = [
            "mypy",
            "--config-file",
            "mypy_self_check.ini",
            "-n0",
            "--no-incremental",
            "-p",
            "mypy",
            "-p",
            "mypyc",
        ]
    try:
        mypy.main.main(clean_exit=True)
    except SystemExit:
        pass
    except Exception:
        # The counters are the whole point: report them on the error path too.
        import traceback

        traceback.print_exc()
    finally:
        total_run_s = _time.perf_counter() - _t0
        print(
            f"\n[audit] total run wall {total_run_s:.1f}s (load-contaminated)",
            file=sys.stderr,
        )
        report()
    return 0


if __name__ == "__main__":
    sys.exit(main())