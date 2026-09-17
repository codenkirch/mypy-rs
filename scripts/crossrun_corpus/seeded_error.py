"""Corpus fixture: exactly one deliberate diagnostic.

The `errors` leg must not be able to pass because both arms printed nothing
(two empty artifacts are equal for free), so the pinned corpus produces one
message on purpose. The negative control adds a second one to prove the leg
bites. The diagnostic is a type error, not a syntax error, so the module
still parses, analyses and type-checks to completion in both arms.
"""


def seeded() -> int:
    value: int = "not an int"
    return value
