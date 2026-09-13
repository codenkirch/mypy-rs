"""Phase G1 expression dual-write node shadow (issues #1572, #1576).

Python stays canonical. With the AST-mirror gate on, writes to the first
shadowed node family are recorded into Rust storage behind the type
kernel's `rust_node_mirror_*` pyfunctions: the `RefExpr` binding scalars
(`kind`, target `node` fullname, `_fullname`, `is_new_def`,
`is_inferred_def`) and the last `analyzed` replacement class name
(record-only, CallExpr-like expression classes). G1.0b (#1576) extends
the store to the remaining G1 expression fields: `method_type`
(OpExpr/IndexExpr/UnaryExpr), `method_types` (ComparisonExpr), `as_type`
(OpExpr/IndexExpr/StrExpr), `right_always`/`right_unreachable` (OpExpr),
`def_var` (MemberExpr) and the RefExpr/NameExpr `is_special_form`,
`is_alias_rvalue`, `type_guard` and `type_is` flags. Each field keeps a
presence marker plus a shape record (class name, bool, fullname, or
kind list); the payload type graph is G1.1, which extends these records
with wire bytes. No consumer reads the store in G1.0a/G1.0b; it exists
to prove the capture path before any read flip, exactly like the F1
type mirror proved construction capture.

Design notes:
- Capture is via class-level monkeypatching of ``__setattr__`` on
  ``RefExpr``, ``CallExpr``, ``IndexExpr`` and ``OpExpr``. These classes
  use ``__slots__`` but define no ``__setattr__`` of their own, so the
  patch composes cleanly and ``type(x) is NameExpr`` keeps working. When
  the gate is off nothing is patched, so the cost in normal runs is one
  ``if`` at build activation and reset.
- Registration is lazy: an unregistered node's writes only mint an entry
  when a tracked field leaves its constructor default, so building a
  ``NameExpr`` never pins anything. The first binding write (semanal's
  ``lvalue.kind = ...`` etc.) adopts the node and stores the full
  post-write record; later tracked writes refresh it. Writes to
  non-shadowed fields (``name``, ``line``, ``value``, ...) pass
  straight through.
- Strong pins: the Rust store pins each captured node until reset, so
  the ``id()``-keyed ``_NODE_HANDLES`` map cannot go stale; no Python
  pin dict is needed. ``reset`` drops entries and pins but does NOT
  touch ``identity``: ``rust_mirror_reset`` remains the single owner of
  ``identity::reset`` (the proxy contract).
- Capture failures never propagate into the write path: the original
  write has already been applied, and a failure only increments an audit
  counter so a partial object never breaks a build.
- ``analyzed`` capture covers the expression classes that carry the
  field (``CallExpr``, ``IndexExpr``, ``OpExpr``). ``ClassDef.analyzed``
  is a G2 statement-family input and deliberately out of scope here.

Shadowed write sites (the G0 brief's G1 table, all captured through the
patched ``__setattr__``; semanal.py anchors from the #1572 base):
- semanal.py:4347-4351 import-dummy lvalue/rvalue binding,
- semanal.py:4572-4573 module-import fallback binding,
- semanal.py:4789 special-form fullname write,
- semanal.py:4994 inferred-def reset,
- semanal.py:5501 alias-rvalue ``analyzed`` write,
- semanal.py:5773-5776 new-def binding,
- semanal.py:5964-5965 implicit-attribute new-def binding,
- semanal.py:6020 declared-types inferred reset,
- semanal.py:7247-7249 visit_name_expr binding,
- checker.py:2372/2416/2525 NameExpr binder construction bindings,
- checker.py:4088 ``__init_subclass__`` base binding,
- checker.py:6726 redefinition binder binding,
- checker.py:10716-10717 forward-reference binding,
- server/astmerge.py:308-311 CallExpr ``analyzed`` fixup.

G1.0b write sites (anchors from the #1574 base, all through the patched
``__setattr__`` except the ``touch`` sites):
- checkexpr.py:5620 OpExpr ``method_type`` (``visit_op_expr``),
- checkexpr.py:6555 OpExpr ``method_type`` (``check_list_multiply``),
- checkexpr.py:5714/5718/5747/5785 ComparisonExpr ``method_types``
  appends (in place; covered by ``touch`` at the end of
  ``visit_comparison_expr``),
- checkexpr.py:6582 UnaryExpr ``method_type``,
- checkexpr.py:6716/6788 IndexExpr ``method_type`` (``visit_index_with_type``
  native and Python tails),
- checkexpr.py:2721/2723 RefExpr ``type_guard``/``type_is`` (callee),
- checker.py:6701 IndexExpr ``method_type`` (``check_indexed_assignment``),
- semanal.py:4355/5501 RefExpr ``is_alias_rvalue``,
- semanal.py:4789 NameExpr ``is_special_form``,
- semanal.py:5972 MemberExpr ``def_var`` (``analyze_member_lvalue``),
- semanal.py:7676/7681 OpExpr ``right_unreachable``/``right_always``,
- semanal.py:9779-9893 ``as_type`` writes (StrExpr/IndexExpr/OpExpr),
- server/astmerge.py:274-275 MemberExpr ``def_var`` fixup,
- treetransform.py:485/493 ``is_special_form``/``def_var`` copies.
"""

from __future__ import annotations

from typing import Any, Final

from mypy.nodes import (
    CallExpr,
    ComparisonExpr,
    IndexExpr,
    NotParsed,
    OpExpr,
    RefExpr,
    StrExpr,
    UnaryExpr,
)

# The five RefExpr binding scalars. `fullname` is a property writing
# `_fullname`, so the slot name is the tracked key.
_REF_FIELDS: Final[frozenset[str]] = frozenset(
    {"kind", "node", "_fullname", "is_new_def", "is_inferred_def"}
)
# The expression classes that carry `analyzed`.
_ANALYZED_CLASSES: Final[tuple[type, ...]] = (CallExpr, IndexExpr, OpExpr)
# G1.0b (#1576) field sets: kind = class name (None when cleared),
# flag = bool, name = `def_var` fullname, kinds = `method_types` list.
# Only `method_types` is append-only and needs `touch`.
_KIND_FIELDS: Final[frozenset[str]] = frozenset(
    {"method_type", "as_type", "type_guard", "type_is"}
)
_FLAG_FIELDS: Final[frozenset[str]] = frozenset(
    {"right_always", "right_unreachable", "is_special_form", "is_alias_rvalue"}
)
_NAME_FIELDS: Final[frozenset[str]] = frozenset({"def_var"})
_KINDS_FIELDS: Final[frozenset[str]] = frozenset({"method_types"})
_FIELD_NAMES: Final[frozenset[str]] = _KIND_FIELDS | _FLAG_FIELDS | _NAME_FIELDS | _KINDS_FIELDS

_kernel_mod: Any = None
_active = False
_audit_mode = False
# Re-entrancy guard: a capture reads live fields, never writes them, but
# the guard keeps a future field-property hook from recursing.
_in_capture = False
_ORIG_SETATTR: Any = object.__setattr__
# id(node) -> native handle for every node the store holds a record for.
# The Rust store pins the node, so the id() keys cannot be recycled while
# an entry exists; the patched __setattr__ is the only minting path.
_NODE_HANDLES: dict[int, int] = {}
_audit: dict[str, int] = {}


def _count(key: str, n: int = 1) -> None:
    if not _audit_mode:
        return
    _audit[key] = _audit.get(key, 0) + n


def _is_baseline(name: str, value: Any) -> bool:
    """True for a constructor-default write of a tracked field.

    Skipping these keeps unbound nodes (the overwhelming majority at
    construction time) out of the store; the first non-default write
    adopts the node with the full post-write record.
    """
    if name == "kind" or name == "node":
        return value is None
    if name == "_fullname":
        return isinstance(value, str) and value == ""
    return value is False  # is_new_def / is_inferred_def


def _capture_ref(node: RefExpr) -> None:
    global _in_capture
    _in_capture = True
    try:
        target = node.node
        node_fullname: str | None = None
        if target is not None:
            try:
                fullname = target.fullname
            except Exception:
                fullname = None
            if isinstance(fullname, str):
                node_fullname = fullname
        handle = _kernel_mod.rust_node_mirror_capture_ref(
            node, node.kind, node_fullname, node._fullname, node.is_new_def, node.is_inferred_def
        )
        _NODE_HANDLES[id(node)] = handle
        _count("capture_ref")
    except Exception:
        _count("capture_fail.ref")
    finally:
        _in_capture = False


def _capture_analyzed(node: Any) -> None:
    global _in_capture
    _in_capture = True
    try:
        value = node.analyzed
        analyzed_kind = None if value is None else type(value).__name__
        handle = _kernel_mod.rust_node_mirror_capture_analyzed(node, analyzed_kind)
        _NODE_HANDLES[id(node)] = handle
        _count("capture_analyzed")
    except Exception:
        _count("capture_fail.analyzed")
    finally:
        _in_capture = False


def _is_field_baseline(name: str, value: Any) -> bool:
    """True for a constructor-default write of a G1.0b field.

    `as_type` defaults to the `NotParsed.VALUE` sentinel while an
    analysis write of None is meaningful; the other type-valued fields
    default to None; the flags default to False and `method_types` to
    an empty list.
    """
    if name == "as_type":
        return isinstance(value, NotParsed)
    if name in ("method_type", "type_guard", "type_is", "def_var"):
        return value is None
    if name == "method_types":
        return not value
    return value is False


def _capture_field(node: Any, name: str) -> None:
    global _in_capture
    _in_capture = True
    try:
        value = getattr(node, name)
        handle: int
        if name in _KIND_FIELDS:
            kind = None if value is None else type(value).__name__
            handle = _kernel_mod.rust_node_mirror_capture_field_kind(node, name, kind)
        elif name in _FLAG_FIELDS:
            handle = _kernel_mod.rust_node_mirror_capture_flag(node, name, bool(value))
        elif name in _NAME_FIELDS:
            target = value
            fullname: str | None = None
            if target is not None:
                try:
                    raw = target.fullname
                except Exception:
                    raw = None
                fullname = raw if isinstance(raw, str) else None
            handle = _kernel_mod.rust_node_mirror_capture_field_name(node, name, fullname)
        else:
            # method_types is the only list-valued shadowed field.
            kinds = [None if item is None else type(item).__name__ for item in value]
            handle = _kernel_mod.rust_node_mirror_capture_field_kinds(node, name, kinds)
        _NODE_HANDLES[id(node)] = handle
        _count("capture_" + name)
    except Exception:
        _count("capture_fail." + name)
    finally:
        _in_capture = False


def _node_setattr(self: Any, name: str, value: Any) -> None:
    # Apply the write first: the store records the post-write state, and
    # any AttributeError from an unknown slot propagates exactly as the
    # unpatched object.__setattr__ would.
    _ORIG_SETATTR(self, name, value)
    if not _active or _in_capture:
        return
    if name in _REF_FIELDS:
        if _NODE_HANDLES.get(id(self)) is None and _is_baseline(name, value):
            # Constructor-default write on a never-adopted node: nothing
            # to shadow yet (lazy adoption baseline).
            _count("baseline_skip.ref")
            return
        _capture_ref(self)
    elif name == "analyzed" and value is None and id(self) not in _NODE_HANDLES:
        _count("baseline_skip.analyzed")
    elif name == "analyzed":
        _capture_analyzed(self)
    elif name in _FIELD_NAMES:
        if _NODE_HANDLES.get(id(self)) is None and _is_field_baseline(name, value):
            _count("baseline_skip." + name)
            return
        _capture_field(self, name)


def activate(*, audit: bool = False) -> None:
    """Enable node-shadow capture; a missing extension leaves it off.

    Activation is one-shot (un-patching mid-run would desync live
    records), matching the type mirror. A later activate call may still
    turn counters on.
    """
    global _active, _audit_mode, _kernel_mod
    if _active:
        if audit:
            _audit_mode = True
        return
    try:
        import type_kernel as _km
    except ImportError:
        _count("activate_failed.no_type_kernel")
        return
    _kernel_mod = _km
    _audit_mode = audit
    # G1.0b adds ComparisonExpr / StrExpr / UnaryExpr; NameExpr and
    # MemberExpr already route through the RefExpr patch.
    for cls in (
        RefExpr,
        CallExpr,
        IndexExpr,
        OpExpr,
        ComparisonExpr,
        StrExpr,
        UnaryExpr,
    ):
        try:
            cls.__setattr__ = _node_setattr  # type: ignore[method-assign]
        except Exception:
            # A compiled (mypyc) class refuses class-level patching; a
            # partial install stays inert because `_active` never flips.
            _count("activate_failed.patch")
            return
    _active = True
    _count("activate")


def touch(node: Any, field: str) -> None:
    """Record an in-place mutation of a shadowed field (G1.0b).

    `__setattr__` cannot observe list mutations, so a mutating site
    calls this after the write; `ComparisonExpr.method_types` is the
    only such field. Off-gate calls return immediately, so callers need
    no guard. Unknown fields and values that are still the constructor
    default (an empty list) are ignored, keeping lazy adoption intact.
    """
    if not _active or field not in _FIELD_NAMES:
        return
    if _is_field_baseline(field, getattr(node, field)):
        return
    _capture_field(node, field)
    _count("touch." + field)


def reset(*, clear_counts: bool = False) -> None:
    """Drop node-shadow storage and pins (per-build boundary).

    Deliberately does not touch ``identity``: ``rust_mirror_reset`` alone
    owns the raw handle registry, so other seams' handles survive a node
    reset. Activation is one-shot and kept across reset, like the mirror.
    """
    if _kernel_mod is not None:
        _kernel_mod.rust_node_mirror_reset()
    _NODE_HANDLES.clear()
    _count("reset")
    if clear_counts:
        _audit.clear()


def report() -> dict[str, int]:
    """Return a copy of the audit counters."""
    return dict(_audit)
