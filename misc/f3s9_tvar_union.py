# F3 slice 9 profiling (#1397): count TypeVarType + UnionType per-field writes on
# a self-check run, after the Instance/CallableType splice ops landed.

# Usage: PYTHONPATH=<mirror dirs> uv run --no-sync python misc/f3s9_tvar_union.py
#   -- --config-file mypy_self_check.ini -n0 --no-incremental -p mypy

import sys
import time
from collections import Counter
from typing import Any

from mypy.options import Options

_orig = Options.__init__


def _init(self: Options, *args: Any, **kwargs: Any) -> None:
    _orig(self, *args, **kwargs)
    self.native_type_mirror = True
    self.native_type_mirror_read = True
    self.fast_exit = False


Options.__init__ = _init  # type: ignore[method-assign]

import mypy.types as types_mod

_counters: Counter[str] = Counter()
_TARGETS: tuple[tuple[type, str], ...] = (
    (types_mod.TypeVarType, "tvar"),
    (types_mod.UnionType, "union"),
)


def _hook(cls: type, fam: str) -> None:
    orig_set = cls.__setattr__

    def counting_set(self: Any, name: str, value: Any) -> None:
        _counters[f"{fam}.{name}"] += 1
        orig_set(self, name, value)

    cls.__setattr__ = counting_set  # type: ignore[method-assign]


for cls, fam in _TARGETS:
    _hook(cls, fam)

_orig_union_copy = getattr(types_mod.UnionType, "copy_modified", None)


if _orig_union_copy is None:
    # UnionType has no copy_modified; measure its writes via __setattr__ only.
    pass
else:

    def _union_copy(self: Any, **kwargs: Any) -> Any:
        for k in kwargs:
            _counters[f"union.copy_modified.{k}"] += 1
        return _orig_union_copy(self, **kwargs)

    types_mod.UnionType.copy_modified = _union_copy  # type: ignore[method-assign]

_orig_tvar_copy = types_mod.TypeVarType.copy_modified


def _tvar_copy(self: Any, **kwargs: Any) -> Any:
    for k in kwargs:
        _counters[f"tvar.copy_modified.{k}"] += 1
    return _orig_tvar_copy(self, **kwargs)


types_mod.TypeVarType.copy_modified = _tvar_copy  # type: ignore[method-assign]

from mypy.main import main

argv_rest = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
sys.argv = [sys.argv[0], *argv_rest]

t0 = time.perf_counter()
try:
    main()
except SystemExit:
    pass
elapsed = time.perf_counter() - t0
print(f"F3 slice-9 profile: tvar/union wall={elapsed:.1f}s")
print("per-field setattr / copy_modified kwarg top 40:")
for k, v in _counters.most_common(40):
    print(f"  {k:44} {v}")
print("mirror report captured/defer/family setattr counters:")
import mypy.types_mirror

rep = mypy.types_mirror.report()
rows = sorted(
    ((k, v) for k, v in rep.items() if k.startswith(("setattr_", "unser", "capture"))),
    key=lambda kv: -kv[1],
)
for k, v in rows[:50]:
    print(f"  {k:48} {v}")
