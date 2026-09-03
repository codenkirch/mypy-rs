# F3 slice 4 (#1397) battery: strict audited cold self-check driver.
# Patches Options.__init__ so native_type_mirror is on before main();
# strict + audit come from the env (MYPY_TK_MIRROR*, F3S4_MIRROR=0 off).
import os
import time
from typing import Any

from mypy.options import Options

_orig = Options.__init__


def _init(self: Options, *args: Any, **kwargs: Any) -> None:
    _orig(self, *args, **kwargs)
    self.native_type_mirror = os.environ.get("F3S4_MIRROR", "1") == "1"
    self.native_type_mirror_read = False
    self.native_type_instance_write = True


Options.__init__ = _init  # type: ignore[method-assign]

import mypy.types_mirror
from mypy.main import main

# clean_exit disables fast_exit so main() raises SystemExit instead of
# hard-killing the process before the report tail can run.
t0 = time.perf_counter()
try:
    main(clean_exit=True)
except SystemExit:
    pass
elapsed = time.perf_counter() - t0

main_proc = sorted(mypy.types_mirror.report().items(), key=lambda kv: -kv[1])[:12]
print(f"F3S4 battery: wall={elapsed:.1f}s")
print("report top:", main_proc)
