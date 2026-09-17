"""Corpus fixture: an attribute read before its own class body is checked.

The checker defers a *top-level function* when it reads a name or attribute
whose type is not inferred yet (`handle_cannot_determine_type`, documented in
`mypy/checker.py` with the `C().x` example), and that deferral is what puts a
module into a second pass. The runner refuses a `deferral.build` comparison
whose second-pass total is zero, because a budget that nothing spent is not a
budget that was compared.
"""


def deferred_read() -> int:
    return Late().total + 1


class Late:
    def __init__(self) -> None:
        self.total = helper()


def helper() -> int:
    return 1
