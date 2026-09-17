"""Corpus fixture: partial types, narrowing and an unreachable tail.

The runner compares two gate states of the *same* corpus, so the fixture
exists to make the compared artifacts non-vacuous: attribute types that are
only known after inference, a narrowing branch that rebinds a name, and a
statement the checker never visits reachably.
"""


class Box:
    """An attribute whose type is only known once inference has run."""

    def __init__(self) -> None:
        self.items = []

    def add(self, value: int) -> None:
        self.items.append(value)

    def total(self) -> int:
        return sum(self.items)


def narrow(value: int | None) -> int:
    if value is None:
        return 0
    return value + 1


def optional_operand(value: int | None) -> int:
    # Errors under `strict_optional` (the default) and type-checks under
    # `--no-strict-optional`, so an `opt:strict_optional` arm pair can show the
    # configuration taking effect on both the build and the daemon path.
    return value + 1


def inferred_list() -> list[int]:
    out = []
    out.append(1)
    return out


def unreachable_tail(flag: bool) -> int:
    if flag:
        return 1
        print("after return")
    return 2
