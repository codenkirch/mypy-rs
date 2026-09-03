# Force Options.native_type_mirror for a cold CLI run (self-check gate).
import os
from typing import Any

from mypy.options import Options

_orig = Options.__init__


def _init(self: Options, *args: Any, **kwargs: Any) -> None:
    _orig(self, *args, **kwargs)
    self.native_type_mirror = os.environ.get("FORCE_NATIVE_TYPE_MIRROR") == "1"
    # Phase F2 (#1393) read flip tracks the mirror switch on this runner:
    # both come from the environment so an audit/strict run picks the
    # mode without repository changes.
    self.native_type_mirror_read = os.environ.get("FORCE_NATIVE_TYPE_MIRROR_READ") == "1"
    # Phase F3 (#1397) write flip tracks the mirror switch on this runner.
    self.native_type_instance_write = os.environ.get("FORCE_NATIVE_TYPE_INSTANCE_WRITE") == "1"


Options.__init__ = _init  # type: ignore[method-assign]

from mypy.main import main

main()
