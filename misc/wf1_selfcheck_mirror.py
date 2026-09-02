# Force Options.native_type_mirror for a cold CLI run (self-check gate).
import os
from typing import Any

from mypy.options import Options

_orig = Options.__init__


def _init(self: Options, *args: Any, **kwargs: Any) -> None:
    _orig(self, *args, **kwargs)
    self.native_type_mirror = os.environ.get("FORCE_NATIVE_TYPE_MIRROR") == "1"


Options.__init__ = _init  # type: ignore[method-assign]

from mypy.main import main

main()
