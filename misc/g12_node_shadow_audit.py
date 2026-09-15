"""G1.2 (#1674) node-shadow fidelity audit generator.

Derives the audit table from source, so the published table cannot drift
from the code:

- Python field sets: ``ast`` over ``mypy/nodes.py`` (``__slots__`` per
  class, with the line anchor of every slot).
- Rust shadow field sets: the tracked-field tables in
  ``mypy/nodes_mirror.py`` / ``mypy/symtables_mirror.py`` (also by
  ``ast``, so each tracked name keeps its line anchor) plus the record
  shape in ``crates/type_kernel/src/node_mirror.rs`` (text scan for the
  ``NodeShadow`` fields and ``FieldValue`` variants).
- Mutation evidence for every gap: assignment sites of that slot outside
  the defining module, with file:line anchors.

Run it from the repo root; ``--check`` exits non-zero when the committed
document no longer matches the derived tables::

    .venv/bin/python misc/g12_node_shadow_audit.py > docs/plans/<doc>.md
    .venv/bin/python misc/g12_node_shadow_audit.py --check docs/plans/<doc>.md
"""

from __future__ import annotations

import ast
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NODES_PY = ROOT / "mypy" / "nodes.py"
NODES_MIRROR_PY = ROOT / "mypy" / "nodes_mirror.py"
SYMTABLES_MIRROR_PY = ROOT / "mypy" / "symtables_mirror.py"
NODE_MIRROR_RS = ROOT / "crates" / "type_kernel" / "src" / "node_mirror.rs"

# Family -> the ``_G2_TRACKED``-style table that claims the class.
G1_REF_STRUCT = {
    "kind": "NodeShadow.kind",
    "node": "NodeShadow.node_fullname",
    "_fullname": "NodeShadow.fullname",
    "is_new_def": "NodeShadow.is_new_def",
    "is_inferred_def": "NodeShadow.is_inferred_def",
}
G1_FIELD_MAP = (
    "is_alias_rvalue",
    "is_special_form",
    "right_always",
    "right_unreachable",
    "method_type",
    "as_type",
    "type_guard",
    "type_is",
    "method_types",
    "def_var",
    "name",
)
G1_ANALYZED = "analyzed"
# Fields that ``ast_serialize`` owns: written by the parser before any
# analysis pass, so the node shadow is not their home (see the doc).
G1_STRUCTURAL = ("name", "expr", "callee", "args", "arg_kinds", "arg_names", "left", "right", "op")
# Expression classes the G1 patch installs on.
G1_PATCHED = (
    "RefExpr",
    "NameExpr",
    "MemberExpr",
    "CallExpr",
    "IndexExpr",
    "OpExpr",
    "UnaryExpr",
    "ComparisonExpr",
    "StrExpr",
)
G2_CLASSES = (
    "ImportBase",
    "Import",
    "ImportFrom",
    "ImportAll",
    "Block",
    "AssignmentStmt",
    "ForStmt",
    "WithStmt",
    "IfStmt",
    "MatchStmt",
    "TypeAliasStmt",
    "FuncDef",
    "OverloadedFuncDef",
    "Decorator",
    "ClassDef",
    "Var",
)
G3_CLASSES = ("SymbolTableNode", "TypeInfo", "SymbolTable")

# Slots whose value is a live object (node, type, symbol table): the
# shadow cannot serve them from a marker-only record. Pinned by hand once
# so the doc's rank ordering is a fact, not a reading.
OBJECT_VALUED = {
    "node",
    "def_var",
    "method_types",
    "expr",
    "index",
    "lvalues",
    "rvalue",
    "body",
    "else_body",
    "target",
    "defs",
    "type_args",
    "ids",
    "items",
    "impl",
    "func",
    "var",
    "info",
    "type",
    "unanalyzed_type",
    "index_type",
    "unanalyzed_index_type",
    "inferred_item_type",
    "inferred_iterator_type",
    "analyzed_types",
    "subject_dummy",
    "alias_node",
    "assignments",
    "unanalyzed_items",
    "metaclass",
    "removed_base_type_exprs",
    "type_vars",
    "base_type_exprs",
    "removed_statements",
    "decorators",
    "original_decorators",
    "setter_type",
    "final_value",
    "bases",
    "mro",
    "metaclass_type",
    "names",
    "dataclass_transform_spec",
    "original_def",
    "functions",
    "aliases",
    "final_iteration",
    "unreachable_else",
    "analyzed",
    "alias_tvars",
    "declared_metaclass",
    "self_type",
    "default_depends",
    "typeddict_type",
    "tuple_type",
    "fallback_to_any",
    "meta_fallback_to_any",
    "abstract_attributes",
    "deletable_attributes",
    "slots",
    "special_alias",
    "alt_promote",
    "type_object_type",
    "typeddict_data",
    "defn",
}


@dataclass
class Slot:
    name: str
    line: int
    owner: str


@dataclass
class Tracked:
    name: str
    line: int
    table: str
    owner: str = ""


@dataclass
class RustShape:
    record: dict[str, int] = field(default_factory=dict)
    variants: dict[str, int] = field(default_factory=dict)


def _literal_strings(node: ast.AST) -> list[tuple[str, int]]:
    """``(value, lineno)`` for every string constant inside ``node``."""
    return [
        (n.value, n.lineno)
        for n in ast.walk(node)
        if isinstance(n, ast.Constant) and isinstance(n.value, str)
    ]


def parse_slots(path: Path) -> dict[str, list[Slot]]:
    """``class -> [(slot, line)]`` from ``__slots__`` literals."""
    tree = ast.parse(path.read_text())
    out: dict[str, list[Slot]] = {}
    for node in ast.walk(tree):
        if not isinstance(node, ast.ClassDef):
            continue
        slots: list[Slot] = []
        for stmt in node.body:
            if not isinstance(stmt, ast.Assign):
                continue
            if not any(
                isinstance(t, ast.Name) and t.id == "__slots__" for t in stmt.targets
            ):
                continue
            slots.extend(Slot(name, line, node.name) for name, line in _literal_strings(stmt.value))
        if slots:
            out[node.name] = slots
    return out


def parse_tracked(path: Path) -> list[Tracked]:
    """Tracked-field literals with line anchors, table name included."""
    tree = ast.parse(path.read_text())
    out: list[Tracked] = []
    for node in tree.body:
        table = _assign_target(node)
        if table is None or not table.startswith("_"):
            continue
        for name, line in _literal_strings(_assign_value(node)):
            out.append(Tracked(name, line, table))
    return out


def _assign_target(node: ast.stmt) -> str | None:
    if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name):
        return node.targets[0].id
    if isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
        return node.target.id
    return None


def _assign_value(node: ast.stmt) -> ast.AST:
    if isinstance(node, ast.Assign):
        return node.value
    assert isinstance(node, ast.AnnAssign) and node.value is not None
    return node.value


def parse_tracked_map(path: Path) -> dict[str, list[Tracked]]:
    """``_G2_TRACKED``-style ``Class -> [Tracked]`` dicts.

    A dict value may name a module-level table (``Var: _G2_VAR``) rather
    than spell a literal, so the flat tables resolve by name first.
    """
    tree = ast.parse(path.read_text())
    flat: dict[str, list[Tracked]] = {}
    for tracked in parse_tracked(path):
        flat.setdefault(tracked.table, []).append(tracked)
    out: dict[str, list[Tracked]] = {}
    for node in tree.body:
        table = _assign_target(node)
        if table is None or not table.startswith("_"):
            continue
        value = _assign_value(node)
        if not isinstance(value, ast.Dict):
            continue
        for key, item in zip(value.keys, value.values):
            if not isinstance(key, ast.Name):
                continue
            if isinstance(item, ast.Name) and item.id in flat:
                entries = [Tracked(t.name, t.line, t.table, key.id) for t in flat[item.id]]
            else:
                entries = [
                    Tracked(name, line, table, key.id) for name, line in _literal_strings(item)
                ]
            out.setdefault(key.id, []).extend(entries)
    return out


def parse_patched_classes(path: Path) -> dict[str, int]:
    """``_ANALYZED_CLASSES``-style tuples: ``class name -> line``.

    The elements are class *names* (``CallExpr``), not strings.
    """
    tree = ast.parse(path.read_text())
    out: dict[str, int] = {}
    for node in tree.body:
        table = _assign_target(node)
        if table is None or table not in ("_ANALYZED_CLASSES", "_G1_PATCHED_CLASSES"):
            continue
        for name in ast.walk(_assign_value(node)):
            if isinstance(name, ast.Name):
                out.setdefault(name.id, name.lineno)
    return out


G1_FIELD_TABLES = (
    "_REF_FIELDS",
    "_KIND_FIELDS",
    "_FLAG_FIELDS",
    "_NAME_FIELDS",
    "_KINDS_FIELDS",
    "_TEXT_FIELDS",
)


def parse_g1_tracked(path: Path) -> dict[str, int]:
    """The G1 field tables only: ``slot -> line``.

    ``_G2_TRACKED`` is deliberately excluded: its class-local names
    (``ForStmt.index``) are not G1 expression fields, and a blanket union
    would mislabel them as shadowed row by row.
    """
    out: dict[str, int] = {}
    for tracked in parse_tracked(path):
        if tracked.table in G1_FIELD_TABLES:
            out.setdefault(tracked.name, tracked.line)
    return out


def parse_rust_shape(path: Path) -> RustShape:
    """``NodeShadow`` record fields and ``FieldValue`` variants (text scan)."""
    text = path.read_text().splitlines()
    shape = RustShape()
    in_enum = False
    for n, line in enumerate(text, start=1):
        if "pub(crate) enum FieldValue" in line:
            in_enum = True
            continue
        if in_enum:
            if line.startswith("}"):
                in_enum = False
                continue
            m = re.match(r"\s+([A-Z]\w*)\b", line)
            if m:
                shape.variants.setdefault(m.group(1), n)
        m = re.match(r"\s+pub\(crate\) (\w+):", line)
        if m:
            shape.record.setdefault(m.group(1), n)
    return shape


def mutation_sites(slot: str, owner: str, limit: int = 3) -> list[str]:
    """Assignment sites for ``owner.slot`` outside its module and the tests.

    Two precision filters, both cheap and both documented in the audit:

    - ``mypyc/`` is scanned out: its IR classes reuse the same slot names
      (``mypyc.ir.ops.Op.op``), so a bare name match there is not a write
      to a ``mypy.nodes`` object.
    - a hit counts only if its file also names ``owner``, which drops the
      remaining same-name-slot collisions.
    """
    pattern = rf"\.{re.escape(slot)}\s*=[^=]"
    proc = subprocess.run(
        ["rg", "-n", "--no-heading", pattern, "mypy"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    hits: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line in proc.stdout.splitlines():
        path, _, rest = line.partition(":")
        if path.endswith("nodes.py") or "/test" in path or "test/" in path:
            continue
        if path in seen:
            continue
        if subprocess.run(
            ["rg", "-q", "-w", owner, path], cwd=ROOT, capture_output=True
        ).returncode != 0:
            continue
        seen.add(path)
        hits.append((path, rest.split(":", 1)[0]))
    # `rg` parallelizes its walk, so its output order is not stable; sort
    # (path, line) so the generated tables are reproducible.
    hits.sort(key=lambda hit: (hit[0], int(hit[1])))
    return [f"`{path}:{line}`" for path, line in hits[:limit]]


def _verdict(slot: str, tracked: bool, writes: int, g1: bool) -> str:
    if tracked:
        if slot in G1_REF_STRUCT:
            return "served"
        if slot in ("method_types", "def_var"):
            return "partial (element fullname/class only)"
        if slot == G1_ANALYZED:
            return "partial (replacement class name only)"
        if slot in ("method_type", "as_type", "type_guard", "type_is"):
            return "served (kind + G1.1 wire bytes)"
        if slot in OBJECT_VALUED:
            return "partial (marker only)"
        return "served"
    # Unshadowed: only a slot some Python path writes needs the shadow,
    # so a slot with no write site outside its own module is structural
    # (the AST wire writer owns it).
    if writes == 0:
        return "AST wire (structural)"
    return "gap" if g1 else "gap"


def emit() -> str:
    slots = parse_slots(NODES_PY)
    g1_lines = parse_g1_tracked(NODES_MIRROR_PY)
    g1_tracked = set(g1_lines)
    g2_map = parse_tracked_map(NODES_MIRROR_PY)
    g3_tracked = parse_tracked(SYMTABLES_MIRROR_PY)
    g3_names = {t.name for t in g3_tracked}
    g3_map = parse_tracked_map(SYMTABLES_MIRROR_PY)
    analyzed_classes = parse_patched_classes(NODES_MIRROR_PY)
    shape = parse_rust_shape(NODE_MIRROR_RS)
    writes_cache: dict[str, list[str]] = {}

    def writes(slot: str, owner: str) -> list[str]:
        key = f"{owner}.{slot}"
        if key not in writes_cache:
            writes_cache[key] = mutation_sites(slot, owner)
        return writes_cache[key]

    def gap_rows(cls: str, slot: Slot, tracked: bool) -> str | None:
        if tracked:
            return None
        hits = writes(slot.name, cls)
        if not hits:
            return None
        return f"| `{cls}` | `{slot.name}` | {', '.join(hits)} |"

    lines: list[str] = []
    lines.append("<!-- generated by misc/g12_node_shadow_audit.py; do not hand-edit -->")
    lines.append("")
    lines.append("### G1 expression shadow (record + field map)")
    lines.append("")
    lines.append("| Python class | slot (`mypy/nodes.py`) | Rust home | served as | verdict |")
    lines.append("|---|---|---|---|---|")
    for cls in G1_PATCHED:
        for slot in slots.get(cls, []):
            tracked = slot.name in g1_tracked
            if slot.name == G1_ANALYZED and cls in analyzed_classes:
                rust = f"`node_mirror.rs:{shape.record.get('analyzed_kind', 0)}`"
                served = (
                    "`NodeShadow.analyzed_kind` "
                    f"(`nodes_mirror.py:{analyzed_classes.get(cls, 0)}`)"
                )
                tracked = True
            elif slot.name in G1_REF_STRUCT:
                rust = f"`node_mirror.rs:{shape.record.get(G1_REF_STRUCT[slot.name].split('.')[1], 0)}`"
                served = G1_REF_STRUCT[slot.name]
            elif tracked:
                variant = {
                    "method_type": "Wire",
                    "as_type": "Wire",
                    "type_guard": "Wire",
                    "type_is": "Wire",
                    "is_alias_rvalue": "Flag",
                    "is_special_form": "Flag",
                    "right_always": "Flag",
                    "right_unreachable": "Flag",
                    "method_types": "Kinds",
                    "def_var": "Name",
                    "name": "Text",
                }.get(slot.name, "?")
                rust = f"`node_mirror.rs:{shape.variants.get(variant, 0)}`"
                served = f"`FieldValue::{variant}` (`nodes_mirror.py:{g1_lines.get(slot.name, 0)}`)"
            else:
                rust = "-"
                served = "-"
            verdict = _verdict(slot.name, tracked, len(writes(slot.name, cls)), True)
            lines.append(
                f"| `{cls}` | `{slot.name}` ({NODES_PY.name}:{slot.line}) | {rust} "
                f"| {served} | {verdict} |"
            )
    lines.append("")
    lines.append("### G2 statement/def shadow (field-name keyed meta record)")
    lines.append("")
    lines.append("| Python class | slot (`mypy/nodes.py`) | home | verdict |")
    lines.append("|---|---|---|---|")
    for cls in G2_CLASSES:
        for slot in slots.get(cls, []):
            entries = [t for t in g2_map.get(cls, []) if t.name == slot.name]
            tracked = bool(entries)
            if tracked:
                anchor = (
                    f"`nodes_mirror.py:{entries[0].line}` via `{entries[0].table}[{cls}]`"
                )
            else:
                anchor = "-"
            verdict = _verdict(slot.name, tracked, len(writes(slot.name, cls)), False)
            lines.append(
                f"| `{cls}` | `{slot.name}` ({NODES_PY.name}:{slot.line}) | {anchor} | {verdict} |"
            )
    lines.append("")
    lines.append("### G3 symbol-table shadow (`symtables_mirror.py`, keyed by table+name)")
    lines.append("")
    lines.append("| Python class | slot (`mypy/nodes.py`) | home | verdict |")
    lines.append("|---|---|---|---|")
    for cls in G3_CLASSES:
        for slot in slots.get(cls, []):
            entries = [t for t in g3_map.get(cls, []) if t.name == slot.name]
            if not entries and slot.name in g3_names:
                entries = [t for t in g3_tracked if t.name == slot.name]
            tracked = bool(entries)
            anchor = f"`symtables_mirror.py:{entries[0].line}` via `{entries[0].table}`" if tracked else "-"
            verdict = _verdict(slot.name, tracked, len(writes(slot.name, cls)), False)
            lines.append(
                f"| `{cls}` | `{slot.name}` ({NODES_PY.name}:{slot.line}) | {anchor} | {verdict} |"
            )
    lines.append("")
    lines.append("### Gap ledger (Python write sites outside `mypy/nodes.py`)")
    lines.append("")
    lines.append("| family | class | slot | writes outside `nodes.py` |")
    lines.append("|---|---|---|---|")
    rows: list[str] = []
    for cls in G1_PATCHED:
        for slot in slots.get(cls, []):
            tracked = slot.name in g1_tracked or (
                slot.name == G1_ANALYZED and cls in analyzed_classes
            )
            row = gap_rows(cls, slot, tracked)
            if row:
                rows.append(f"| G1 | {row.lstrip('| ')}")
    for cls in G2_CLASSES:
        tracked_names = {t.name for t in g2_map.get(cls, [])}
        for slot in slots.get(cls, []):
            row = gap_rows(cls, slot, slot.name in tracked_names)
            if row:
                rows.append(f"| G2 | {row.lstrip('| ')}")
    for cls in G3_CLASSES:
        for slot in slots.get(cls, []):
            row = gap_rows(cls, slot, slot.name in g3_names)
            if row:
                rows.append(f"| G3 | {row.lstrip('| ')}")
    lines.extend(rows or ["| - | - | - | none |"])
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    text = emit()
    if len(argv) >= 3 and argv[1] == "--check":
        doc = Path(argv[2])
        committed = doc.read_text()
        if text not in committed:
            print(f"stale audit tables in {doc}: re-run the generator", file=sys.stderr)
            return 1
        print(f"{doc}: audit tables match the derived state")
        return 0
    sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
