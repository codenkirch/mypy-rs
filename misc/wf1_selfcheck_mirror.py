# Force Options.native_type_mirror for a cold CLI run (self-check gate).
import os
import sys

from mypy.options import Options

_orig = Options.__init__


def _init(self, *args, **kwargs):
    _orig(self, *args, **kwargs)
    self.native_type_mirror = os.environ.get("FORCE_NATIVE_TYPE_MIRROR") == "1"


Options.__init__ = _init

from mypy.main import main

sys.exit(main())
