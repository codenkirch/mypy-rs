# F3 (#1397) slice 0 baseline: count Instance attribute traffic on a run.
# Stands in for the PyO3 round-trips an Instance write flip would cost.
# Usage: PYTHONPATH=<dirs> uv run --no-sync python misc/f3_baseline.py [-p mypy]
import time
from typing import Any

from mypy.options import Options

_orig = Options.__init__


def _init(self: Options, *args: Any, **kwargs: Any) -> None:
    _orig(self, *args, **kwargs)
    self.native_type_mirror = True
    self.native_type_mirror_read = True


Options.__init__ = _init  # type: ignore[method-assign]

import mypy.types as types_mod

Inst = types_mod.Instance
_ATTRS = frozenset(
    ("args", "type", "last_known_value", "type_ref", "extra_attrs", "invalid")
)
_counters = {"write": 0}
_orig_set = Inst.__setattr__


def _counting_set(self: Any, name: str, value: Any) -> None:
    if name in _ATTRS:
        _counters["write"] += 1
    _orig_set(self, name, value)


Inst.__setattr__ = _counting_set  # type: ignore[method-assign]

from mypy.main import main

import mypy.types_mirror

t0 = time.perf_counter()
main()
elapsed = time.perf_counter() - t0
report = mypy.types_mirror.report()
top = sorted(report.items(), key=lambda kv: -kv[1])[:15]
print(
    f"F3 baseline: Instance writes={_counters['write']} wall={elapsed:.1f}s"
)
print("mirror report top:", top)
