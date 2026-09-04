# F3 slice 9 profiling (#1397): count TypeVarType + UnionType per-field writes on
# a self-check run, after the Instance/CallableType splice ops landed.

# Usage: PYTHONPATH=<mirror dirs> uv run --no-sync python misc/f3s9_tvar_union.py
#   -- --config-file mypy_self_check.ini -n0 --no-incremental -p mypy

import sys