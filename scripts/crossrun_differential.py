#!/usr/bin/env python3
"""Cross-run differential runner: compare two mypy configurations on one corpus.

WHY CROSS-RUN. The checker is not idempotent: it writes `var.type`,
`var.is_inferred`, `defn.type` and `lvalue_node.type`, pushes and pops binder
frames, and drains `deferred_nodes` and `_type_maps` (#1770 section 1.4).
Running two gate states inside one process would double-apply those writes, so
G3.1's in-run mode-2 comparison is unavailable here and the differential has to
be two processes compared from outside (#1770 section 4.3).

LEGS. Each is compared and reported under its own name, so a failure says which
leg broke rather than only that something differed:

  kernel            provenance and gate arming: the arm imported mypy from the
                    tree under test, ran single-process, the mode each gate
                    should hold (its env value, or the option-default serving
                    mode #1860 when the env gate is unset) was in force, read
                    back from the kernel rather than silently inert, and no
                    in-run differential counter reported a mismatch.
  errors            the build's error output, byte-for-byte.
  ast               `mypy.exportjson`'s MypyFile JSON per module, both the
                    in-memory tree (`convert_mypy_file_to_json`, which carries
                    the symbol table through `convert_symbol_table`) and the
                    written cache (`convert_binary_cache_to_json`),
                    byte-identical. This leg catches the node-field writes the
                    errors leg misses. `mypy/test/testexportjson.py` is the
                    existing consumer this invocation is modelled on.
  typemap           `BuildState.type_map()` per module, in iteration order, with
                    its `len(_type_maps) == 1` invariant asserted.
  deferral.build    the build-path deferral budget (`last_pass = 2`): per-module
                    `pass_num` / `last_pass` / `deferred_nodes`.
  deferral.daemon   the daemon-path budget (`last_pass = 3`), from a real
                    fine-grained update driven through `mypy.dmypy_server`.
  suites            `testfinegrained.py` and `testfinegrainedcache.py` run under
                    each arm and compared per test.

CONFIGURATIONS. An arm is a name plus three token kinds, so a future flip is
expressible without touching this file:

  env:MYPY_TK_NODE_READ_FLIP=2   an environment variable for the arm process
  opt:native_ast_mirror=1        an `Options` attribute; the mirror gates have
                                 no CLI flag, and the test harnesses set the
                                 same field from `TEST_NATIVE_*`
  flag:--no-native-type-kernel   a mypy command line flag

Presets: `flips` (default), `kernel`, `self`. Explicit `--a` / `--b` tokens
replace the preset on both sides.

DEGENERATE-PASS GUARD. Two empty or truncated dumps compare equal for free,
which is the hollow-zero failure this repo filed as #1820. Every artifact is
therefore read back through the arm's `status.json`, parsed, and required to be
non-empty; a leg whose artifact is missing, empty or malformed fails as
`degenerate` and names the artifact. A leg that could not run at all fails as
`not-run`.

CONTROLS. `--corpus-a` / `--corpus-b` point an arm at its own copy of the
corpus, which is the negative-control affordance: the file *names* must still
match, so only content can differ, and the runner says so loudly in the report.
The positive control is the default preset pair, which must agree today.

PINNED INVOCATION (from the repo root; run the whole thing inside one heavy-op
pool slot and never let the runner re-enter the pool, since a pool slot is not
reentrant -- `run 2` when the suites leg is selected, `run 1` otherwise):

  PYTHONPATH=/private/tmp/crossrun-ext .venv/bin/python \
    scripts/crossrun_differential.py --out /private/tmp/crossrun-out

Exit status: 0 all selected legs agree, 1 any selected leg failed, 2 usage.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parent.parent
DEFAULT_CORPUS = REPO / "scripts" / "crossrun_corpus"
DEFAULT_SUITES = ("mypy/test/testfinegrained.py", "mypy/test/testfinegrainedcache.py")

# The three serving gates that exist today (#1770 section 4.3). A new gate is
# declared on the command line; this tuple only seeds the default preset.
FLIP_GATES = ("MYPY_TK_NODE_READ_FLIP", "MYPY_TK_STMT_READ_FLIP", "MYPY_TK_VAR_KEY_FLIP")

# Ambient variables that would decide an arm's state outside the declaration the
# report is about. Declared tokens are applied after these are stripped.
STRIPPED_PREFIXES = ("MYPY_TK_", "TEST_NATIVE_")
STRIPPED_EXACT = ("MYPY_NUM_WORKERS", "MYPY_CACHE_DIR")

# `mypy/test/helpers.py` sets these Options fields from TEST_NATIVE_* gates and
# `parse_options` applies them after any `# flags:` line, so the gate env is the
# only channel that reaches a DataSuite: translate tokens, never pass flags.
SUITE_OPT_GATES = {
    "native_ast_mirror": "TEST_NATIVE_AST_MIRROR",
    "native_symtable_mirror": "TEST_NATIVE_SYMTABLE_MIRROR",
    "native_ast_mirror_read": "TEST_NATIVE_AST_MIRROR_READ",
    "native_ast_mirror_stmt_read": "TEST_NATIVE_AST_MIRROR_STMT_READ",
    "native_symtable_read_flip": "TEST_NATIVE_SYMTABLE_READ_FLIP",
    "native_symtable_read_flip_verify": "TEST_NATIVE_SYMTABLE_READ_FLIP_VERIFY",
}
SUITE_FLAG_GATES = {
    "--native-type-kernel": ("TEST_NATIVE_TYPE_KERNEL", "1"),
    "--no-native-type-kernel": ("TEST_NATIVE_TYPE_KERNEL", "0"),
    "--native-parser": ("TEST_NATIVE_PARSER", "1"),
    "--no-native-parser": ("TEST_NATIVE_PARSER", "0"),
    "--native-resolver": ("TEST_NATIVE_RESOLVER", "1"),
    "--no-native-resolver": ("TEST_NATIVE_RESOLVER", "0"),
}

# `mypy/test/testexportjson.py` skips these; keeping the same list keeps the ast
# leg about the corpus and its own closure rather than the bundled stubs.
SKIP_MODULES = frozenset(
    {"builtins", "typing", "_typeshed", "__future__", "typing_extensions", "sys", "collections"}
)

LEG_KERNEL = "kernel"
LEG_ERRORS = "errors"
LEG_AST = "ast"
LEG_TYPEMAP = "typemap"
LEG_DEFER_BUILD = "deferral.build"
LEG_DEFER_DAEMON = "deferral.daemon"
LEG_SUITES = "suites"
ALL_LEGS = (
    LEG_KERNEL,
    LEG_ERRORS,
    LEG_AST,
    LEG_TYPEMAP,
    LEG_DEFER_BUILD,
    LEG_DEFER_DAEMON,
    LEG_SUITES,
)
# Legs the arm worker can run. The ones that are always part of a build arm are
# not optional; `deferral.daemon` costs a second build and is.
WORKER_LEGS = tuple(leg for leg in ALL_LEGS if leg != LEG_SUITES)
OPTIONAL_LEGS = (LEG_DEFER_DAEMON,)

VERDICT_AGREE = "agree"
VERDICT_MISMATCH = "mismatch"
VERDICT_DEGENERATE = "degenerate"
VERDICT_NOT_RUN = "not-run"
VERDICT_SKIPPED = "skipped"

CONTROL_BANNER = (
    "CONTROL RUN: the two arms were pointed at different corpus directories.\n"
    "This is the negative-control affordance, not a baseline comparison: the\n"
    "corpus is supposed to be identical across arms, so a mismatch here is the\n"
    "expected outcome and the exit status is not a verdict on the tree."
)


class UsageError(Exception):
    """A command line the runner refuses to act on."""


class ArmError(Exception):
    """An arm could not run, or ran something other than what was declared."""


@dataclasses.dataclass(frozen=True)
class Arm:
    name: str
    env: tuple[tuple[str, str], ...]
    options: tuple[tuple[str, str], ...]
    flags: tuple[str, ...]

    def env_dict(self) -> dict[str, str]:
        return dict(self.env)

    def declared(self) -> str:
        parts = [f"env:{key}={value}" for key, value in self.env]
        parts += [f"opt:{key}={value}" for key, value in self.options]
        parts += [f"flag:{flag}" for flag in self.flags]
        return " ".join(parts) if parts else "(no tokens: production defaults)"


@dataclasses.dataclass
class Comparison:
    leg: str
    verdict: str
    detail: str


def parse_arm_token(token: str) -> tuple[str, str, str]:
    """Split an arm token into `(kind, name, value)`; refuse anything else."""
    kind, sep, body = token.partition(":")
    if not sep:
        raise UsageError(
            f"arm token {token!r} must be prefixed: env:NAME=VALUE, "
            f"opt:attribute=VALUE or flag:--some-mypy-flag"
        )
    if kind == "flag":
        if not body.startswith("-"):
            raise UsageError(f"arm token {token!r}: a mypy flag must start with '-'")
        return (kind, body, "")
    if kind not in ("env", "opt"):
        raise UsageError(f"arm token {token!r}: prefix must be env:, opt: or flag:, not {kind!r}")
    name, eq, value = body.partition("=")
    if not eq or not name:
        raise UsageError(f"arm token {token!r}: expected {kind}:NAME=VALUE")
    return (kind, name, value)


def arm_from_tokens(name: str, tokens: list[str]) -> Arm:
    env: dict[str, str] = {}
    options: dict[str, str] = {}
    flags: list[str] = []
    for token in tokens:
        kind, key, value = parse_arm_token(token)
        if kind == "env":
            env[key] = value
        elif kind == "opt":
            options[key] = value
        else:
            flags.append(key)
    return Arm(name, tuple(sorted(env.items())), tuple(sorted(options.items())), tuple(flags))


def preset(pair: str) -> tuple[Arm, Arm]:
    """The named arm pairs. `flips` is the default agreement control."""
    if pair == "flips":
        mirror = "opt:native_ast_mirror=1"
        off = [f"env:{gate}=0" for gate in FLIP_GATES] + [mirror]
        serve = [f"env:{gate}=2" for gate in FLIP_GATES] + [mirror]
        return (arm_from_tokens("flips-off", off), arm_from_tokens("flips-serve", serve))
    if pair == "kernel":
        return (
            arm_from_tokens("kernel-on", []),
            arm_from_tokens("kernel-off", ["flag:--no-native-type-kernel"]),
        )
    if pair == "self":
        return (arm_from_tokens("self-a", []), arm_from_tokens("self-b", []))
    raise UsageError(f"unknown preset {pair!r}: expected one of flips, kernel, self")


def resolve_legs(raw: str, known: tuple[str, ...], allow_empty: bool = False) -> tuple[str, ...]:
    """Parse a leg list, refusing unknown names and an empty selection."""
    wanted = [part.strip() for part in raw.split(",") if part.strip()]
    if not wanted and not allow_empty:
        raise UsageError("--legs is empty: select at least one leg")
    unknown = [leg for leg in wanted if leg not in known]
    if unknown:
        raise UsageError(f"leg(s) {unknown} are unknown; known legs are {list(known)}")
    return tuple(leg for leg in known if leg in wanted)


def corpus_digests(corpus: Path) -> dict[str, str]:
    """`name -> sha256` for every `.py` in a corpus directory."""
    return {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(corpus.glob("*.py"))
    }


def stage_corpus(source: Path, destination: Path) -> None:
    """Copy a corpus into an arm directory; refuse an empty one."""
    files = sorted(source.glob("*.py"))
    if not files:
        raise UsageError(f"corpus {source} has no .py files")
    destination.mkdir(parents=True, exist_ok=True)
    for path in files:
        shutil.copyfile(path, destination / path.name)


def normalize_text(text: str, arm_dir: Path, tree: Path) -> str:
    """Erase host-specific roots so a comparison cannot see path spelling.

    Both arms stage the corpus at `<arm>/corpus`, so the only difference the
    normalisation has to remove is which scratch root the arm ran in.
    """
    return text.replace(str(arm_dir), "<ARM>").replace(str(tree), "<TREE>")


def first_difference(left: str, right: str) -> str:
    """A pointed first difference, for a mismatch message."""
    left_lines = left.splitlines()
    right_lines = right.splitlines()
    for index in range(max(len(left_lines), len(right_lines))):
        left_line = left_lines[index] if index < len(left_lines) else "<missing>"
        right_line = right_lines[index] if index < len(right_lines) else "<missing>"
        if left_line != right_line:
            return f"line {index + 1}: {left_line!r} != {right_line!r}"
    if len(left) != len(right):
        return f"byte length differs: {len(left)} != {len(right)}"
    return "no line differs (byte-level difference only)"


# --- artifact readers ----------------------------------------------------


def read_artifact(path: Path, what: str) -> tuple[str | None, str | None]:
    """Read a text artifact, refusing a missing or empty one."""
    if not path.is_file():
        return None, f"{what} artifact {path} is missing"
    raw = path.read_text(encoding="utf8")
    if not raw.strip():
        return None, f"{what} artifact {path} is empty"
    return raw, None


def read_json_artifact(path: Path, what: str) -> tuple[Any | None, str | None]:
    """Read a JSON artifact, refusing a missing, empty or malformed one."""
    raw, refusal = read_artifact(path, what)
    if refusal is not None:
        return None, refusal
    assert raw is not None
    try:
        return json.loads(raw), None
    except json.JSONDecodeError as exc:
        return None, f"{what} artifact {path} is not valid JSON: {exc}"


def diagnose_list(left: list[str], right: list[str]) -> str:
    """Point at the first differing element of two message lists."""
    for index in range(min(len(left), len(right))):
        if left[index] != right[index]:
            return f"first differing message {index}: {left[index]!r} != {right[index]!r}"
    if len(left) != len(right):
        longer, shorter = (left, right) if len(left) > len(right) else (right, left)
        extra = longer[len(shorter) :]
        return (
            f"message counts differ ({len(left)} vs {len(right)}): first extra entry "
            f"{extra[0]!r} (plus {len(extra) - 1} more)"
        )
    return "the lists are equal element-wise (byte-level difference only)"


def compare_text_leg(leg: str, left: str, right: str, what: str, artifact: str) -> Comparison:
    if left == right:
        return Comparison(leg, VERDICT_AGREE, f"{what}: {len(left)} bytes byte-identical")
    return Comparison(
        leg, VERDICT_MISMATCH, f"{what}: {artifact} differs ({first_difference(left, right)})"
    )


def compare_dump_dirs(leg: str, left: Path, right: Path, label: str) -> tuple[Comparison, int]:
    """Byte-compare two directories of per-module JSON dumps.

    The module *sets* must match: a module dumped on one side only is a
    mismatch, not merely a smaller comparison.
    """
    left_files = {path.name: path for path in sorted(left.glob("*.json"))}
    right_files = {path.name: path for path in sorted(right.glob("*.json"))}
    if not left_files and not right_files:
        return Comparison(leg, VERDICT_DEGENERATE, f"{label}: both dump directories are empty"), 0
    if not left_files or not right_files:
        absent = "left" if not left_files else "right"
        present = len(left_files) or len(right_files)
        return (
            Comparison(
                leg,
                VERDICT_DEGENERATE,
                f"{label}: the {absent} dump directory is empty while the other holds "
                f"{present} module dump(s)",
            ),
            0,
        )
    if set(left_files) != set(right_files):
        only_left = sorted(set(left_files) - set(right_files))
        only_right = sorted(set(right_files) - set(left_files))
        return (
            Comparison(
                leg,
                VERDICT_MISMATCH,
                f"{label}: module sets differ (only left: {only_left[:5]}, "
                f"only right: {only_right[:5]})",
            ),
            0,
        )
    refusals: list[str] = []
    differing: list[str] = []
    first = ""
    for name in sorted(left_files):
        left_raw, refusal = read_artifact(left_files[name], f"{label} {name}")
        if refusal is not None:
            refusals.append(refusal)
            continue
        right_raw, refusal = read_artifact(right_files[name], f"{label} {name}")
        if refusal is not None:
            refusals.append(refusal)
            continue
        assert left_raw is not None and right_raw is not None
        if left_raw != right_raw:
            differing.append(name)
            if not first:
                first = first_difference(left_raw, right_raw)
    compared = len(left_files) - len(refusals)
    if refusals:
        return Comparison(leg, VERDICT_DEGENERATE, f"{label}: {refusals[0]}"), compared
    if differing:
        return (
            Comparison(
                leg,
                VERDICT_MISMATCH,
                f"{label}: {len(differing)} of {compared} module dump(s) differ, first "
                f"{differing[0]}: {first}",
            ),
            compared,
        )
    return (
        Comparison(leg, VERDICT_AGREE, f"{label}: {compared} module dump(s) byte-identical"),
        compared,
    )


def merge(leg: str, parts: list[Comparison], detail: str) -> Comparison:
    """One verdict per leg: the worst part wins, the detail carries the counts."""
    for verdict in (VERDICT_MISMATCH, VERDICT_DEGENERATE, VERDICT_NOT_RUN):
        for part in parts:
            if part.verdict == verdict:
                return Comparison(leg, verdict, f"{detail}; {part.detail}")
    return Comparison(leg, VERDICT_AGREE, detail)


# --- the runner ----------------------------------------------------------


def split_ambient(base: dict[str, str]) -> tuple[dict[str, str], list[str]]:
    """The ambient environment with gate-deciding variables removed.

    What a run declares is what decided it, so `STRIPPED_PREFIXES` never reach an
    arm unless the arm declares them.
    """
    stripped = sorted(
        key for key in base if key.startswith(STRIPPED_PREFIXES) or key in STRIPPED_EXACT
    )
    removed = set(stripped)
    return {key: value for key, value in base.items() if key not in removed}, stripped


def arm_environment(base: dict[str, str], arm: Arm, ext_dirs: list[str]) -> dict[str, str]:
    """One arm's process environment, from an already-split base.

    This takes the base and applies *that arm's* tokens and nothing else. The
    earlier shape returned the stripped base together with one arm's tokens and
    let the caller reuse it for both arms, which leaked arm A's undeclared
    `env:` tokens into arm B's process and made a served mode in B look declared.
    """
    env = dict(base)
    env.update(arm.env_dict())
    if ext_dirs:
        inherited = env.get("PYTHONPATH", "")
        env["PYTHONPATH"] = os.pathsep.join(ext_dirs + ([inherited] if inherited else []))
    return env


def arm_command(
    python: str, arm: Arm, arm_dir: Path, tree: Path, corpus_name: str, optional: tuple[str, ...]
) -> list[str]:
    command = [
        python,
        str(Path(__file__).resolve()),
        "--worker",
        "--arm",
        arm.name,
        "--arm-dir",
        str(arm_dir),
        "--tree",
        str(tree),
        "--corpus",
        corpus_name,
        "--optional-legs",
        ",".join(optional),
    ]
    for key, value in arm.env:
        command += ["--token", f"env:{key}={value}"]
    for key, value in arm.options:
        command += ["--token", f"opt:{key}={value}"]
    for flag in arm.flags:
        command += ["--token", f"flag:{flag}"]
    return command


def run_arm(
    python: str,
    arm: Arm,
    arm_dir: Path,
    corpus_source: Path,
    tree: Path,
    optional: tuple[str, ...],
    env: dict[str, str],
    timeout: int,
) -> dict[str, Any]:
    """Stage the corpus, run the arm worker, and read its status back."""
    stage_corpus(corpus_source, arm_dir / "corpus")
    command = arm_command(python, arm, arm_dir, tree, "corpus", optional)
    log_path = arm_dir / "worker.log"
    started = time.time()
    with open(log_path, "w", encoding="utf8") as log:
        try:
            completed = subprocess.run(
                command,
                cwd=str(arm_dir),
                env=env,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=timeout,
            )
            returncode: int | None = completed.returncode
        except subprocess.TimeoutExpired:
            returncode = None
    status_path = arm_dir / "status.json"
    status: dict[str, Any] | None = None
    if status_path.is_file():
        try:
            status = json.loads(status_path.read_text(encoding="utf8"))
        except json.JSONDecodeError:
            status = None
    return {
        "arm": arm.name,
        "dir": arm_dir,
        "log": log_path,
        "seconds": time.time() - started,
        "returncode": returncode,
        "status": status,
    }


def arm_leg_status(run: dict[str, Any], leg: str) -> tuple[bool, str]:
    """Whether the arm reports a leg as run, with a pointed reason when not."""
    status = run["status"]
    if status is None:
        return False, f"arm {run['arm']} wrote no readable status.json (see {run['log']})"
    if run["returncode"] is None:
        return False, f"arm {run['arm']} exceeded its timeout"
    entry = status.get("legs", {}).get(leg)
    if entry is None:
        return (
            False,
            f"arm {run['arm']} never reached the {leg} leg (stopped at stage "
            f"{status.get('stage')!r}, error {status.get('error')!r})",
        )
    if not entry.get("ok"):
        return False, f"arm {run['arm']} reports {leg} as not run: {entry.get('detail')}"
    return True, str(entry.get("detail", ""))


def compare_kernel_leg(runs: dict[str, dict[str, Any]], names: tuple[str, str]) -> Comparison:
    provenance: dict[str, Any] = {}
    for name in names:
        value, refusal = read_json_artifact(runs[name]["dir"] / "dump" / "provenance.json", name)
        if refusal is not None:
            return Comparison(LEG_KERNEL, VERDICT_DEGENERATE, refusal)
        provenance[name] = value

    problems: list[str] = []
    for name in names:
        data = provenance[name]
        if data["num_workers"] != 0:
            problems.append(
                f"arm {name} ran with num_workers={data['num_workers']}: an in-process dump of a "
                f"fanned-out build keeps its state in worker processes and is hollow"
            )
        if not data["tree_ok"]:
            problems.append(
                f"arm {name} imported mypy from {data['mypy_file']}, outside the tree under "
                f"test {data['tree']}"
            )
        for gate, state in sorted(data["gates"].items()):
            if state["declared"] != state["requested"]:
                problems.append(
                    f"arm {name} holds {gate}={state['requested']} in its process environment but "
                    f"declared {state['declared']}: an undeclared channel decided this arm, so the "
                    f"mode in force is not the one the report claims"
                )
            expected = state.get("effective", state["requested"])
            if expected != state["in_force"]:
                problems.append(
                    f"arm {name} should hold {gate}={expected} (env {state['requested']}, "
                    f"option default {state.get('default', 0)}) but the mode in force is "
                    f"{state['in_force']}: the gate is inert, so the leg it belongs to compared "
                    f"nothing"
                )
            mismatched = state.get("counters", {}).get("mismatched", 0)
            if mismatched:
                problems.append(
                    f"arm {name}: the in-run differential for {gate} reported {mismatched} "
                    f"mismatch(es) (counters {state['counters']})"
                )
    for field in ("tree", "mypy_file", "python", "kernel_file"):
        if provenance[names[0]][field] != provenance[names[1]][field]:
            problems.append(
                f"the arms disagree on {field}: {provenance[names[0]][field]} vs "
                f"{provenance[names[1]][field]}"
            )
    if provenance[names[0]]["corpus_files"] != provenance[names[1]]["corpus_files"]:
        problems.append(
            f"the arms ran different corpus file lists: {provenance[names[0]]['corpus_files']} vs "
            f"{provenance[names[1]]['corpus_files']}"
        )
    if problems:
        return Comparison(LEG_KERNEL, VERDICT_NOT_RUN, "; ".join(problems))

    data = provenance[names[0]]
    per_arm: list[str] = []
    inert: list[str] = []
    for name in names:
        states = provenance[name]["gates"]
        modes = " ".join(
            f"{gate.removeprefix('MYPY_TK_').removesuffix('_FLIP')}={state['in_force']}"
            for gate, state in sorted(states.items())
        )
        counters = " ".join(
            f"{gate.removeprefix('MYPY_TK_').removesuffix('_FLIP')}[consulted "
            f"{state['counters'].get('consulted')}, served {state['counters'].get('served')}, "
            f"compared {state['counters'].get('compared')}]"
            for gate, state in sorted(states.items())
            if state.get("counters") and state["counters"].get("consulted")
        )
        per_arm.append(f"{name}: {modes}" + (f"; {counters}" if counters else ""))
        inert += [
            f"{name}/{gate.removeprefix('MYPY_TK_').removesuffix('_FLIP')}"
            for gate, state in sorted(states.items())
            if state.get("effective", state["requested"]) != 0
            and not state.get("counters", {}).get("consulted")
        ]
    deltas = option_deltas(provenance, names)
    # A requested mode that nothing consulted is not evidence about that channel,
    # but it does not fail the leg: the cross-run comparison is the runner's
    # claim, while the flip's engagement is the flip's own counter gate.
    inert_clause = f"; ARMED BUT INERT (0 consulted): {', '.join(inert)}" if inert else ""
    return Comparison(
        LEG_KERNEL,
        VERDICT_AGREE,
        f"tree={data['tree']}, single-process, gates in force: {'; '.join(per_arm)}"
        + (f"; declared option deltas: {deltas}" if deltas else "")
        + inert_clause,
    )


def option_deltas(provenance: dict[str, Any], names: tuple[str, str]) -> str:
    left = provenance[names[0]].get("effective_options", {})
    right = provenance[names[1]].get("effective_options", {})
    return ", ".join(
        f"{key}={left.get(key)}/{right.get(key)}"
        for key in sorted(left)
        if left.get(key) != right.get(key)
    )


def compare_errors_leg(runs: dict[str, dict[str, Any]], names: tuple[str, str]) -> Comparison:
    values: dict[str, Any] = {}
    for name in names:
        value, refusal = read_json_artifact(runs[name]["dir"] / "dump" / "errors.json", name)
        if refusal is not None:
            return Comparison(LEG_ERRORS, VERDICT_DEGENERATE, refusal)
        values[name] = value
    if not values[names[0]] or not values[names[1]]:
        return Comparison(
            LEG_ERRORS,
            VERDICT_DEGENERATE,
            f"the error output is empty ({names[0]}: {len(values[names[0]])}, "
            f"{names[1]}: {len(values[names[1]])} message(s)): an empty artifact compares equal "
            f"for free",
        )
    what = f"error output ({len(values[names[0]])} vs {len(values[names[1]])} message(s))"
    left = json.dumps(values[names[0]], indent=1)
    right = json.dumps(values[names[1]], indent=1)
    if left == right:
        return Comparison(LEG_ERRORS, VERDICT_AGREE, f"{what}: {len(left)} bytes byte-identical")
    return Comparison(
        LEG_ERRORS,
        VERDICT_MISMATCH,
        f"{what}: {diagnose_list(values[names[0]], values[names[1]])} "
        f"(artifacts {runs[names[0]]['dir']}/dump/errors.json, "
        f"{runs[names[1]]['dir']}/dump/errors.json)",
    )


def compare_ast_leg(runs: dict[str, dict[str, Any]], names: tuple[str, str]) -> Comparison:
    parts: list[Comparison] = []
    counts: list[int] = []
    for sub, label in (("ast", "in-memory trees"), ("cache", "cache payloads")):
        part, count = compare_dump_dirs(
            LEG_AST,
            runs[names[0]]["dir"] / "dump" / sub,
            runs[names[1]]["dir"] / "dump" / sub,
            label,
        )
        parts.append(part)
        counts.append(count)
    return merge(
        LEG_AST,
        parts,
        f"{counts[0]} in-memory tree(s) + {counts[1]} cache payload(s) byte-identical",
    )


def compare_typemap_leg(runs: dict[str, dict[str, Any]], names: tuple[str, str]) -> Comparison:
    maps, count = compare_dump_dirs(
        LEG_TYPEMAP,
        runs[names[0]]["dir"] / "dump" / "typemap",
        runs[names[1]]["dir"] / "dump" / "typemap",
        "inferred type maps",
    )
    summaries: dict[str, Any] = {}
    for name in names:
        value, refusal = read_json_artifact(
            runs[name]["dir"] / "dump" / "typemap-summary.json", name
        )
        if refusal is not None:
            return Comparison(LEG_TYPEMAP, VERDICT_DEGENERATE, refusal)
        summaries[name] = value
    for name in names:
        summary = summaries[name]
        if summary["invariant_violations"]:
            return Comparison(
                LEG_TYPEMAP,
                VERDICT_NOT_RUN,
                f"arm {name}: BuildState.type_map()'s len(_type_maps) == 1 invariant failed for "
                f"{summary['invariant_violations']}",
            )
        if summary["checked"] == 0 or summary["entries"] == 0:
            return Comparison(
                LEG_TYPEMAP,
                VERDICT_DEGENERATE,
                f"arm {name}: {summary['checked']} module(s) checked with "
                f"{summary['entries']} type-map entr(ies), so the compared artifact is empty",
            )
        if summary["captured_entries"] != summary["export_types_total"]:
            return Comparison(
                LEG_TYPEMAP,
                VERDICT_NOT_RUN,
                f"arm {name}: the per-module capture holds {summary['captured_entries']} entries "
                f"while the build's own export_types total is {summary['export_types_total']}: the "
                f"capture missed a module, so it is not evidence",
            )
    detail = (
        f"{count} module(s), {summaries[names[0]]['entries']}/{summaries[names[1]]['entries']} "
        f"entries, {len(summaries[names[0]]['unchecked'])} unchecked, "
        f"{len(summaries[names[0]]['empty_maps'])} empty map(s), len(_type_maps) == 1 throughout"
    )
    return merge(LEG_TYPEMAP, [maps], detail)


def compare_deferral_build_leg(
    runs: dict[str, dict[str, Any]], names: tuple[str, str]
) -> Comparison:
    stats: dict[str, Any] = {}
    for name in names:
        value, refusal = read_json_artifact(
            runs[name]["dir"] / "dump" / "deferral-build.json", name
        )
        if refusal is not None:
            return Comparison(LEG_DEFER_BUILD, VERDICT_DEGENERATE, refusal)
        stats[name] = value
    for name in names:
        data = stats[name]
        if data["second_pass_total"] == 0:
            return Comparison(
                LEG_DEFER_BUILD,
                VERDICT_DEGENERATE,
                f"arm {name}: no module entered a second pass, so the {data['last_pass']}-pass "
                f"budget was never spent and the comparison is vacuous",
            )
        unexpected = sorted(
            value for value in data["last_pass_values"] if int(value) != data["last_pass"]
        )
        if unexpected:
            return Comparison(
                LEG_DEFER_BUILD,
                VERDICT_NOT_RUN,
                f"arm {name}: the build path reports last_pass={unexpected} as well as "
                f"{data['last_pass']}, so this leg is not comparing the build budget alone",
            )
    detail = (
        f"{stats[names[0]]['checked']} module(s), last_pass={stats[names[0]]['last_pass']}, "
        f"second-pass total {stats[names[0]]['second_pass_total']} "
        f"({', '.join(stats[names[0]]['second_pass_modules']) or 'none'}), "
        f"{stats[names[0]]['deferred_nodes_remaining']} deferred node(s) left"
    )
    return compare_text_leg(
        LEG_DEFER_BUILD,
        json.dumps(stats[names[0]], indent=1, sort_keys=True),
        json.dumps(stats[names[1]], indent=1, sort_keys=True),
        detail,
        f"{runs[names[0]]['dir']}/dump/deferral-build.json",
    )


def compare_deferral_daemon_leg(
    runs: dict[str, dict[str, Any]], names: tuple[str, str]
) -> Comparison:
    stats: dict[str, Any] = {}
    for name in names:
        value, refusal = read_json_artifact(
            runs[name]["dir"] / "dump" / "deferral-daemon.json", name
        )
        if refusal is not None:
            return Comparison(LEG_DEFER_DAEMON, VERDICT_DEGENERATE, refusal)
        stats[name] = value
    for name in names:
        if not stats[name]["increment"]["budgets"].get("3"):
            return Comparison(
                LEG_DEFER_DAEMON,
                VERDICT_NOT_RUN,
                f"arm {name}: the fine-grained increment never ran a second pass with "
                f"last_pass=3 (observed budgets {stats[name]['increment']['budgets']}), so the "
                f"daemon budget is unmeasured rather than equal",
            )
    increment = stats[names[0]]["increment"]
    detail = (
        f"increment budgets {increment['budgets']}, {increment['second_pass_calls']} second-pass "
        f"call(s), updated modules {increment['updated_modules']}"
    )
    for step in ("initialize", "increment"):
        left_messages = stats[names[0]][step]["messages"]
        right_messages = stats[names[1]][step]["messages"]
        if left_messages != right_messages:
            return Comparison(
                LEG_DEFER_DAEMON,
                VERDICT_MISMATCH,
                f"{detail}: the {step} step's output differs "
                f"({diagnose_list(left_messages, right_messages)})",
            )
    if stats[names[0]]["increment"]["budgets"] != stats[names[1]]["increment"]["budgets"]:
        return Comparison(
            LEG_DEFER_DAEMON,
            VERDICT_MISMATCH,
            f"{detail}: the second-pass budgets differ "
            f"({stats[names[0]]['increment']['budgets']} vs "
            f"{stats[names[1]]['increment']['budgets']})",
        )
    # `options` is reported, not compared: it says what each arm's daemon ran
    # with, and two arms may declare different `opt:` values on purpose.
    left = json.dumps(
        {key: value for key, value in stats[names[0]].items() if key != "options"},
        indent=1,
        sort_keys=True,
    )
    right = json.dumps(
        {key: value for key, value in stats[names[1]].items() if key != "options"},
        indent=1,
        sort_keys=True,
    )
    if left == right:
        applied = stats[names[0]].get("options", {})
        return Comparison(
            LEG_DEFER_DAEMON,
            VERDICT_AGREE,
            f"{detail}: {len(left)} bytes byte-identical"
            + (f", daemon options {applied}" if applied else ""),
        )
    return Comparison(
        LEG_DEFER_DAEMON,
        VERDICT_MISMATCH,
        f"{detail}: {runs[names[0]]['dir']}/dump/deferral-daemon.json differs "
        f"({first_difference(left, right)})",
    )


def compare_selected_legs(
    legs: tuple[str, ...], runs: dict[str, dict[str, Any]], names: tuple[str, str]
) -> list[Comparison]:
    """Compare every selected worker leg across the two arms."""
    comparators = {
        LEG_KERNEL: compare_kernel_leg,
        LEG_ERRORS: compare_errors_leg,
        LEG_AST: compare_ast_leg,
        LEG_TYPEMAP: compare_typemap_leg,
        LEG_DEFER_BUILD: compare_deferral_build_leg,
        LEG_DEFER_DAEMON: compare_deferral_daemon_leg,
    }
    results: list[Comparison] = []
    for leg in legs:
        comparator = comparators.get(leg)
        if comparator is None:
            continue
        ok_a, why_a = arm_leg_status(runs[names[0]], leg)
        ok_b, why_b = arm_leg_status(runs[names[1]], leg)
        if not ok_a or not ok_b:
            results.append(Comparison(leg, VERDICT_NOT_RUN, why_a if not ok_a else why_b))
            continue
        results.append(comparator(runs, names))
    return results


# --- the suites leg (runner side) ---------------------------------------


def suite_environment(arm: Arm) -> tuple[dict[str, str], list[str]]:
    """Translate an arm's tokens into the test harness's own gate variables."""
    translated: dict[str, str] = {}
    unmappable: list[str] = []
    for name, value in arm.options:
        gate = SUITE_OPT_GATES.get(name)
        if gate is None:
            unmappable.append(f"opt:{name}={value}")
        else:
            on = value.strip().lower() in ("1", "true", "yes", "on")
            translated[gate] = "1" if on else "0"
    for flag in arm.flags:
        entry = SUITE_FLAG_GATES.get(flag)
        if entry is None:
            unmappable.append(f"flag:{flag}")
        else:
            translated[entry[0]] = entry[1]
    return translated, unmappable


def run_suites_leg(
    python: str,
    arm: Arm,
    arm_dir: Path,
    tree: Path,
    suites: tuple[str, ...],
    env: dict[str, str],
    timeout: int,
) -> dict[str, Any]:
    """Run the fine-grained suites under one arm and record per-test outcomes."""
    translated, unmappable = suite_environment(arm)
    if unmappable:
        return {
            "ok": False,
            "detail": (
                f"the suites leg cannot express {unmappable} to pytest: DataSuites read their "
                f"Options from the TEST_NATIVE_* gates, so an arm carrying these tokens cannot "
                f"be exercised here. Declare an equivalent opt:/flag: token or deselect the leg."
            ),
            "suites": {},
            "gates": translated,
        }
    suite_env = dict(env)
    suite_env.update(translated)
    command = [
        python,
        "-m",
        "pytest",
        *suites,
        "-n0",
        "--tb=no",
        "-v",
        "-p",
        "no:cacheprovider",
        # Generous because this leg is compared as an outcome set on a shared
        # host: a load spike must not time out one arm's test and not the
        # other's, which would read as a divergence that is not one.
        "--timeout=300",
        "--timeout-method=thread",
    ]
    log_path = arm_dir / "suites.log"
    with open(log_path, "w", encoding="utf8") as log:
        try:
            completed = subprocess.run(
                command,
                cwd=str(tree),
                env=suite_env,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=timeout,
            )
            returncode: int | None = completed.returncode
        except subprocess.TimeoutExpired:
            returncode = None
    outcomes: dict[str, dict[str, str]] = {}
    for line in log_path.read_text(encoding="utf8", errors="replace").splitlines():
        node, outcome = parse_pytest_line(line)
        if node is None or outcome is None:
            continue
        outcomes.setdefault(node.split("::", 1)[0], {})[node] = outcome
    counts = ", ".join(f"{suite}: {len(tests)}" for suite, tests in sorted(outcomes.items()))
    return {
        "ok": True,
        "detail": f"exit {returncode}, {counts or 'no outcomes parsed'}",
        "returncode": returncode,
        "suites": outcomes,
        "gates": translated,
        "log": str(log_path),
    }


PYTEST_OUTCOMES = frozenset({"PASSED", "FAILED", "SKIPPED", "ERROR", "XFAIL", "XPASS"})


def parse_pytest_line(line: str) -> tuple[str | None, str | None]:
    """`nodeid OUTCOME` from a pytest `-v` line, else `(None, None)`."""
    stripped = line.strip()
    if not stripped or "::" not in stripped:
        return None, None
    parts = stripped.split()
    if len(parts) < 2 or parts[1] not in PYTEST_OUTCOMES:
        return None, None
    return parts[0], parts[1]


def compare_suites_leg(
    suite_a: dict[str, Any], suite_b: dict[str, Any], names: tuple[str, str]
) -> Comparison:
    for name, record in ((names[0], suite_a), (names[1], suite_b)):
        if not record["ok"]:
            return Comparison(LEG_SUITES, VERDICT_NOT_RUN, f"arm {name}: {record['detail']}")
        if record["returncode"] is None:
            return Comparison(LEG_SUITES, VERDICT_NOT_RUN, f"arm {name}: the suites timed out")
        if record["returncode"] not in (0, 1):
            return Comparison(
                LEG_SUITES,
                VERDICT_NOT_RUN,
                f"arm {name}: pytest exited {record['returncode']}, a collection or internal "
                f"error rather than a test result (see {record['log']})",
            )
        if not record["suites"]:
            return Comparison(
                LEG_SUITES,
                VERDICT_DEGENERATE,
                f"arm {name}: no test outcomes were parsed from {record['log']}, so the compared "
                f"artifact is empty",
            )
        for suite, tests in record["suites"].items():
            if not tests:
                return Comparison(
                    LEG_SUITES, VERDICT_DEGENERATE, f"arm {name}: suite {suite} reported no tests"
                )
    missing = sorted(set(suite_a["suites"]) - set(suite_b["suites"]))
    extra = sorted(set(suite_b["suites"]) - set(suite_a["suites"]))
    if missing or extra:
        return Comparison(
            LEG_SUITES,
            VERDICT_MISMATCH,
            f"the arms ran different suite sets (only {names[0]}: {missing}, only {names[1]}: {extra})",
        )
    total = 0
    for suite in sorted(suite_a["suites"]):
        tests_a = suite_a["suites"][suite]
        tests_b = suite_b["suites"][suite]
        total += len(tests_a)
        if set(tests_a) != set(tests_b):
            only_a = sorted(set(tests_a) - set(tests_b))[:3]
            only_b = sorted(set(tests_b) - set(tests_a))[:3]
            return Comparison(
                LEG_SUITES,
                VERDICT_MISMATCH,
                f"suite {suite}: test sets differ (only {names[0]}: {only_a}, "
                f"only {names[1]}: {only_b})",
            )
        differing = sorted(node for node in tests_a if tests_a[node] != tests_b[node])
        if differing:
            node = differing[0]
            return Comparison(
                LEG_SUITES,
                VERDICT_MISMATCH,
                f"suite {suite}: {len(differing)} test(s) changed outcome, first {node} "
                f"{tests_a[node]} -> {tests_b[node]}",
            )
    gates = suite_a.get("gates", {})
    return Comparison(
        LEG_SUITES,
        VERDICT_AGREE,
        f"{len(suite_a['suites'])} suite(s), {total} test(s), identical outcomes, harness gates "
        f"{gates or '{}'}",
    )


# --- report --------------------------------------------------------------


def render_report(
    arms: tuple[Arm, Arm],
    runs: dict[str, dict[str, Any]],
    results: list[Comparison],
    legs: tuple[str, ...],
    corpora: dict[str, Path],
    tree: Path,
    stripped: list[str],
    out: Path,
) -> str:
    lines: list[str] = []
    lines.append("=== cross-run differential (#1770 section 4.3) ===")
    lines.append(f"tree: {tree}")
    lines.append(f"output: {out}")
    control = corpora[arms[0].name].resolve() != corpora[arms[1].name].resolve()
    if control:
        lines.append("")
        lines.append(CONTROL_BANNER)
    lines.append("")
    lines.append("arms:")
    for arm in arms:
        lines.append(f"  {arm.name}: {arm.declared()}")
        run = runs[arm.name]
        lines.append(f"    exit {run['returncode']} in {run['seconds']:.1f}s, log {run['log']}")
    if stripped:
        lines.append(
            f"  ambient gate variables stripped from both arms: {', '.join(stripped)} "
            f"(declare one with env: to use it)"
        )
    lines.append("")
    lines.append("corpus:")
    for arm in arms:
        source = corpora[arm.name]
        digests = corpus_digests(source)
        summary = ", ".join(f"{name}={digest[:12]}" for name, digest in sorted(digests.items()))
        lines.append(f"  {arm.name}: {source} ({len(digests)} file(s)) {summary}")
    lines.append("")
    width = max(len(leg) for leg in ALL_LEGS)
    lines.append(f"{'leg':<{width}}  {'verdict':<11}  detail")
    lines.append(f"{'-' * width}  {'-' * 11}  {'-' * 40}")
    by_leg = {result.leg: result for result in results}
    for leg in ALL_LEGS:
        result = by_leg.get(leg)
        if result is None:
            verdict = VERDICT_SKIPPED if leg not in legs else VERDICT_NOT_RUN
            detail = "(not selected)" if leg not in legs else "(selected but not compared)"
            lines.append(f"{leg:<{width}}  {verdict:<11}  {detail}")
        else:
            lines.append(f"{leg:<{width}}  {result.verdict:<11}  {result.detail}")
    failed = [result for result in results if result.verdict != VERDICT_AGREE]
    lines.append("")
    if failed:
        lines.append(f"VERDICT: {len(failed)} of {len(results)} compared leg(s) failed")
        for result in failed:
            lines.append(f"  {result.leg}: {result.verdict}: {result.detail}")
    else:
        lines.append(f"VERDICT: all {len(results)} compared leg(s) agree")
    return "\n".join(lines) + "\n"


# --- the arm worker ------------------------------------------------------

OPTION_PROBE = (
    "native_parser",
    "native_resolver",
    "native_type_kernel",
    "native_ast_mirror",
    "native_symtable_mirror",
    "native_binder",
    "num_workers",
    "incremental",
    "namespace_packages",
    "follow_imports",
    "local_partial_types",
    "python_version",
)


def worker_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="crossrun-arm", description="internal: one arm of the differential"
    )
    parser.add_argument("--arm", required=True)
    parser.add_argument("--arm-dir", required=True)
    parser.add_argument("--tree", required=True)
    parser.add_argument("--corpus", default="corpus")
    parser.add_argument("--optional-legs", default="")
    parser.add_argument("--token", action="append", default=[])
    return parser


def pin_tree(tree: Path) -> str:
    """Import mypy from the tree under test, or refuse (#1789)."""
    sys.meta_path[:] = [
        finder
        for finder in sys.meta_path
        if "editable"
        not in (finder if isinstance(finder, type) else type(finder)).__module__.lower()
        and "editable"
        not in (finder if isinstance(finder, type) else type(finder)).__name__.lower()
    ]
    cwd_entries = {"", os.getcwd(), os.path.realpath(os.getcwd())}
    sys.path[:] = [entry for entry in sys.path if entry not in cwd_entries]
    resolved = str(tree.resolve())
    sys.path.insert(0, resolved)
    import mypy

    location = getattr(mypy, "__file__", None)
    if location is None or not location.startswith(resolved + os.sep):
        raise ArmError(
            f"mypy resolved to {location}, outside the tree under test {resolved} (#1789); run "
            f"the runner from inside the worktree with the scratch extension dirs on PYTHONPATH"
        )
    return str(location)


def coerce_option(options: Any, name: str, raw: str) -> Any:
    """Set one Options attribute, refusing an unknown name or a bad spelling."""
    value = coerced_value(options, name, raw)
    setattr(options, name, value)
    return value


def coerced_value(options: Any, name: str, raw: str) -> Any:
    """The value `opt:name=raw` means for `options`, without setting it."""
    if not hasattr(options, name):
        raise ArmError(f"Options has no attribute {name!r}: an opt: token must name a real option")
    current = getattr(options, name)
    lowered = raw.strip().lower()
    if isinstance(current, bool) or lowered in ("true", "false"):
        if lowered in ("1", "true", "yes", "on"):
            return True
        if lowered in ("0", "false", "no", "off"):
            return False
        raise ArmError(f"opt:{name}={raw} is not a boolean spelling")
    if isinstance(current, int):
        try:
            return int(raw)
        except ValueError:
            raise ArmError(f"opt:{name}={raw} is not an integer") from None
    if isinstance(current, str):
        return raw
    raise ArmError(f"opt:{name}={raw}: {type(current).__name__} options are not settable here")


def gate_mode(raw: str | None, gate: str, source: str) -> int:
    """One flip gate's mode, refused loudly when it is not a mode."""
    try:
        return int(raw or "0")
    except ValueError:
        raise ArmError(f"{gate} from {source} must be 0, 1 or 2, got {raw!r}") from None


def dump_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=1, sort_keys=True) + "\n", encoding="utf8")


def typemap_key(node: Any) -> str:
    """A stable, order-preserving key for one type-map expression."""
    return (
        f"{type(node).__name__}@{getattr(node, 'line', -1)}:{getattr(node, 'column', -1)}:{node}"
    )


def worker_main(argv: list[str]) -> int:
    args = worker_parser().parse_args(argv)
    arm_dir = Path(args.arm_dir).resolve()
    tree = Path(args.tree).resolve()
    arm_dir.mkdir(parents=True, exist_ok=True)
    state: dict[str, Any] = {
        "arm": args.arm,
        "stage": "start",
        "legs": {},
        "error": None,
        "started": time.time(),
    }

    def flush() -> None:
        dump_json(arm_dir / "status.json", state)

    def record(leg: str, ok: bool, detail: str) -> None:
        state["legs"][leg] = {"ok": ok, "detail": detail}
        flush()

    flush()
    try:
        optional = resolve_legs(args.optional_legs, OPTIONAL_LEGS, allow_empty=True)
        mypy_file = pin_tree(tree)
        os.chdir(arm_dir)
        import mypy.version as mypy_version
        from mypy import nodes_mirror
        from mypy.build import (
            State as BuildState,
            _cache_dir_prefix,
            build as build_program,
            get_cache_names,
        )
        from mypy.exportjson import Config, convert_binary_cache_to_json, convert_mypy_file_to_json
        from mypy.main import process_options
        from mypy.modules_state import modules_state

        tokens = [parse_arm_token(token) for token in args.token]
        overrides = {key: value for kind, key, value in tokens if kind == "opt"}
        flags = [key for kind, key, _ in tokens if kind == "flag"]
        # The arm's own declaration, kept apart from what the process environment
        # ends up holding: a served mode that nothing declared means some other
        # channel decided this arm, and the report may not call that declared.
        declared_env = {key: value for kind, key, value in tokens if kind == "env"}
        for kind, key, value in tokens:
            if kind == "env":
                os.environ[key] = value

        corpus_files = sorted(path.name for path in Path(args.corpus).glob("*.py"))
        if not corpus_files:
            raise ArmError(f"corpus {args.corpus} has no .py files")
        state["tree"] = str(tree)
        state["mypy_file"] = mypy_file
        state["corpus_files"] = corpus_files
        flush()

        cache_dir = arm_dir / "cache"
        argv = [
            "--config-file=",
            "-n0",
            "--no-site-packages",
            "--cache-dir",
            str(cache_dir),
            "--no-color-output",
            *flags,
            *[str(Path(args.corpus) / name) for name in corpus_files],
        ]
        sources, options = process_options(argv, require_targets=True)
        requested_options = {
            name: coerce_option(options, name, value) for name, value in overrides.items()
        }
        # Worker-internal harness settings, not part of the compared config: the
        # fixed-format cache is what `convert_binary_cache_to_json` reads (the
        # same pair `testexportjson.py` sets), and `export_types` keeps the total.
        options.sqlite_cache = False
        options.fixed_format_cache = True
        options.export_types = True
        if options.num_workers != 0:
            raise ArmError(
                f"the arm runs with num_workers={options.num_workers}: an in-process dump of a "
                f"fanned-out build keeps its state in worker processes, so every artifact would "
                f"be hollow. Pass -n0 (this runner does)."
            )
        state["stage"] = "build"
        flush()
        # `finish_passes()` releases the checker before `build.build` returns, so the
        # master type map and the deferral counters exist only inside that method;
        # the hook reads them there, and `export_types` cross-checks the total.
        captured: dict[str, dict[str, Any]] = {}
        original_free_state = BuildState.free_state

        def capturing_free_state(self: Any) -> None:
            checker = self._type_checker
            if checker is not None:
                maps = checker._type_maps
                entries: list[list[str]] = []
                if len(maps) == 1:
                    entries = [[typemap_key(node), str(value)] for node, value in maps[0].items()]
                captured[self.id] = {
                    "module": self.id,
                    "type_maps": len(maps),
                    "entries": entries,
                    "last_pass": int(checker.last_pass),
                    "pass_num": int(checker.pass_num),
                    "deferred_nodes": len(checker.deferred_nodes),
                }
            original_free_state(self)

        BuildState.free_state = capturing_free_state  # type: ignore[method-assign]
        try:
            result = build_program(sources=sources, options=options)
        finally:
            BuildState.free_state = original_free_state  # type: ignore[method-assign]
        state["build_errors"] = len(result.errors)
        flush()

        # Serving gates have two channels: the env var, and the production
        # default the build wiring serves when the env gate is unset (#1860).
        # The node and statement defaults are read back from `nodes_mirror`
        # (#1863, #1869), never re-derived from kernel presence.
        production_defaults = {
            "MYPY_TK_NODE_READ_FLIP": 1 if nodes_mirror.production_read_flip() else 0,
            "MYPY_TK_STMT_READ_FLIP": 1 if nodes_mirror.production_stmt_flip() else 0,
            "MYPY_TK_VAR_KEY_FLIP": 0,
        }
        gates: dict[str, dict[str, Any]] = {}
        for gate, mode, counters in zip(
            FLIP_GATES,
            (nodes_mirror.read_flip(), nodes_mirror.stmt_read_flip(), nodes_mirror.var_key_flip()),
            (
                nodes_mirror.read_counters(),
                nodes_mirror.stmt_read_counters(),
                nodes_mirror.var_key_counters(),
            ),
            strict=True,
        ):
            raw_env = os.environ.get(gate)
            default = production_defaults.get(gate, 0)
            gates[gate] = {
                "declared": gate_mode(declared_env.get(gate, "0"), gate, "the arm's tokens"),
                "requested": gate_mode(raw_env or "0", gate, "the process environment"),
                "default": default,
                "effective": (
                    gate_mode(raw_env, gate, "the process environment")
                    if raw_env is not None
                    else default
                ),
                "in_force": mode,
                "counters": counters,
            }
        kernel_module = nodes_mirror._kernel()
        provenance = {
            "arm": args.arm,
            "tree": str(tree),
            "tree_ok": mypy_file.startswith(str(tree) + os.sep),
            "mypy_file": mypy_file,
            "kernel_file": getattr(kernel_module, "__file__", None),
            "python": sys.executable,
            "mypy_version": getattr(mypy_version, "__version__", None),
            "corpus_files": corpus_files,
            "declared_tokens": args.token,
            "requested_options": requested_options,
            "effective_options": {
                name: getattr(options, name) for name in OPTION_PROBE if hasattr(options, name)
            },
            "num_workers": options.num_workers,
            "gates": gates,
        }
        dump_json(arm_dir / "dump" / "provenance.json", provenance)
        record(
            LEG_KERNEL,
            provenance["tree_ok"]
            and all(data["effective"] == data["in_force"] for data in gates.values()),
            f"tree_ok={provenance['tree_ok']}, kernel={provenance['kernel_file']}, gates="
            + ", ".join(f"{gate}={data['in_force']}" for gate, data in sorted(gates.items())),
        )

        normalized_errors = [normalize_text(line, arm_dir, tree) for line in result.errors]
        dump_json(arm_dir / "dump" / "errors.json", normalized_errors)
        record(LEG_ERRORS, True, f"{len(normalized_errors)} message(s)")

        modules = sorted(result.files)
        dumped_modules = [module for module in modules if module not in SKIP_MODULES]
        memory_count = 0
        tree_missing: list[str] = []
        for module in dumped_modules:
            module_tree = result.graph[module].tree
            if module_tree is None:
                tree_missing.append(module)
                continue
            memory_json = json.dumps(
                convert_mypy_file_to_json(module_tree, Config(implicit_names=False)),
                indent=1,
                sort_keys=True,
            )
            dump_text(arm_dir / "dump" / "ast" / f"{module}.json", memory_json, arm_dir, tree)
            memory_count += 1
        # testexportjson reads the cache in isolation, with no node fixer: the
        # same reset keeps this dump a read of the written bytes.
        modules_state.node_fixer = None
        modules_state.modules = {}
        # The cache file name is not `<module>.data.ff`: `get_cache_names` mirrors a
        # package layout (`os` -> `os/__init__.data.ff`), so deriving the path by
        # hand silently skipped 21 of 49 modules before this used the helper.
        cache_prefix = Path(_cache_dir_prefix(options))
        cache_count = 0
        cache_missing: list[str] = []
        for module in dumped_modules:
            module_path = result.graph[module].path
            if module_path is None:
                cache_missing.append(module)
                continue
            cache_file = cache_prefix / get_cache_names(module, module_path, options)[1]
            if not cache_file.is_file():
                cache_missing.append(module)
                continue
            cache_json = json.dumps(
                convert_binary_cache_to_json(cache_file.read_bytes(), implicit_names=False),
                indent=1,
                sort_keys=True,
            )
            dump_text(arm_dir / "dump" / "cache" / f"{module}.json", cache_json, arm_dir, tree)
            cache_count += 1
        record(
            LEG_AST,
            memory_count > 0 and cache_count == memory_count,
            f"{memory_count} in-memory tree dump(s), {cache_count} cache payload dump(s), "
            f"{len(tree_missing)} module(s) without a tree, "
            f"{len(cache_missing)} module(s) without a cache file",
        )
        if memory_count == 0:
            raise ArmError("no module tree was dumped: the ast leg has nothing to compare")

        invariant_violations: list[str] = []
        unchecked: list[str] = []
        empty_maps: list[str] = []
        entries = 0
        deferral_modules: dict[str, Any] = {}
        for module in dumped_modules:
            record_stats = captured.get(module)
            if record_stats is None:
                unchecked.append(module)
                continue
            if record_stats["type_maps"] != 1:
                invariant_violations.append(f"{module}: {record_stats['type_maps']} type maps")
                continue
            if not record_stats["entries"]:
                empty_maps.append(module)
            entries += len(record_stats["entries"])
            dump_json(
                arm_dir / "dump" / "typemap" / f"{module}.json",
                {
                    "module": module,
                    "type_maps": record_stats["type_maps"],
                    "entries": record_stats["entries"],
                },
            )
            deferral_modules[module] = {
                "last_pass": record_stats["last_pass"],
                "pass_num": record_stats["pass_num"],
                "deferred_nodes": record_stats["deferred_nodes"],
                "type_maps": record_stats["type_maps"],
            }
        checked = len(deferral_modules)
        # The guard on the capture's completeness: `finish_passes()` feeds the
        # same master maps into `manager.all_types`, so the build's own total is
        # an independent count over every captured module, skipped ones included.
        captured_entries = sum(len(record["entries"]) for record in captured.values())
        export_types_total = len(result.types)
        dump_json(
            arm_dir / "dump" / "typemap-summary.json",
            {
                "checked": checked,
                "unchecked": unchecked,
                "empty_maps": empty_maps,
                "entries": entries,
                "captured_entries": captured_entries,
                "export_types_total": export_types_total,
                "invariant_violations": invariant_violations,
            },
        )
        record(
            LEG_TYPEMAP,
            not invariant_violations and checked > 0 and captured_entries == export_types_total,
            f"{checked} module(s) checked, {len(unchecked)} unchecked, {entries} entries in the "
            f"compared modules ({captured_entries} captured / {export_types_total} via "
            f"export_types), {len(invariant_violations)} invariant violation(s)",
        )

        last_pass_values: dict[str, int] = {}
        for stats in deferral_modules.values():
            key = str(stats["last_pass"])
            last_pass_values[key] = last_pass_values.get(key, 0) + 1
        second_pass_modules = sorted(
            module for module, stats in deferral_modules.items() if stats["pass_num"] > 0
        )
        dump_json(
            arm_dir / "dump" / "deferral-build.json",
            {
                "checked": checked,
                "modules": deferral_modules,
                "last_pass": 2,
                "last_pass_values": last_pass_values,
                "second_pass_modules": second_pass_modules,
                "second_pass_total": len(second_pass_modules),
                "deferred_nodes_remaining": sum(
                    stats["deferred_nodes"] for stats in deferral_modules.values()
                ),
            },
        )
        record(
            LEG_DEFER_BUILD,
            True,
            f"{checked} module(s), last_pass values {last_pass_values}, second-pass total "
            f"{len(second_pass_modules)}",
        )

        if LEG_DEFER_DAEMON in optional:
            daemon = run_daemon_leg(args, arm_dir, tree, argv, overrides)
            dump_json(arm_dir / "dump" / "deferral-daemon.json", daemon)
            record(
                LEG_DEFER_DAEMON,
                True,
                f"increment budgets {daemon['increment']['budgets']}, "
                f"{daemon['increment']['second_pass_calls']} second-pass call(s)",
            )

        state["stage"] = "done"
        flush()
        return 0 if all(entry["ok"] for entry in state["legs"].values()) else 1
    except (ArmError, UsageError) as exc:
        state["stage"] = "refused"
        state["error"] = str(exc)
        flush()
        print(f"crossrun-arm: REFUSING ({args.arm}): {exc}", file=sys.stderr)
        return 2
    except Exception as exc:
        import traceback

        state["stage"] = "crashed"
        state["error"] = f"{type(exc).__name__}: {exc}"
        flush()
        traceback.print_exc()
        return 1


def dump_text(path: Path, text: str, arm_dir: Path, tree: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(normalize_text(text, arm_dir, tree) + "\n", encoding="utf8")


def run_daemon_leg(
    args: argparse.Namespace,
    arm_dir: Path,
    tree: Path,
    build_argv: list[str],
    overrides: dict[str, str],
) -> dict[str, Any]:
    """Drive one real fine-grained update and record the deferral budgets.

    The daemon path (`mypy/server/update.py`) sets `checker.last_pass = 3`
    before calling `check_second_pass`, a different budget from the build's
    `DEFAULT_LAST_PASS = 2`. Counting the observed values on both paths is the
    only way to tell "the daemon budget was exercised and equal" apart from
    "the daemon path never ran".

    The arm's `opt:` tokens are re-applied here because this path builds its own
    `Options` from `build_argv`, which never carried them: without that, a pair
    differing only by an `opt:` token compared two runs at the option defaults,
    a vacuous agreement for exactly the case this leg exists to cover.
    """
    from mypy.checker import TypeChecker
    from mypy.dmypy_server import Server
    from mypy.main import process_options
    from mypy.options import Options

    sources, options = process_options(list(build_argv), require_targets=True)
    defaults = Options()
    declared_options: dict[str, dict[str, Any]] = {}
    for name, raw in sorted(overrides.items()):
        coerce_option(options, name, raw)
        applied = getattr(options, name)
        expected = coerced_value(defaults, name, raw)
        if applied != expected:
            raise ArmError(
                f"opt:{name}={raw} did not take on the daemon path's Options: it holds "
                f"{applied!r} where the declaration means {expected!r}"
            )
        declared_options[name] = {
            "declared": raw,
            "applied": applied,
            "default": getattr(defaults, name),
        }
    original = TypeChecker.check_second_pass
    observed: list[int] = []

    def counting(self: Any, *positional: Any, **keywords: Any) -> Any:
        observed.append(int(getattr(self, "last_pass", -1)))
        return original(self, *positional, **keywords)

    TypeChecker.check_second_pass = counting  # type: ignore[method-assign]
    try:
        server = Server(options, str(arm_dir / "daemon-status.json"))
        initialize = server.check(sources, export_types=False, is_tty=False, terminal_width=-1)
        observed.clear()
        mutated = sorted(path.name for path in Path(args.corpus).glob("*.py"))[0]
        with open(Path(args.corpus) / mutated, "a", encoding="utf8") as handle:
            handle.write("\n\ndef crossrun_probe() -> int:\n    return 1\n")
        increment = server.check(sources, export_types=False, is_tty=False, terminal_width=-1)
        manager = server.fine_grained_manager
        budgets: dict[str, int] = {}
        for value in observed:
            budgets[str(value)] = budgets.get(str(value), 0) + 1
        return {
            "mutated_file": mutated,
            # Reported, not compared: two arms may legitimately declare different
            # `opt:` values, so the compared fields are the behavioural ones.
            "options": declared_options,
            "initialize": summarise_daemon_step(initialize),
            "increment": {
                **summarise_daemon_step(increment),
                "budgets": budgets,
                "second_pass_calls": len(observed),
                "updated_modules": sorted(manager.updated_modules) if manager else [],
                "changed_modules": (
                    sorted(module for module, _ in manager.changed_modules) if manager else []
                ),
            },
        }
    finally:
        TypeChecker.check_second_pass = original  # type: ignore[method-assign]


def summarise_daemon_step(response: dict[str, Any]) -> dict[str, Any]:
    """A deterministic summary of one `dmypy_server.Server.check` response."""
    text = response.get("out") or response.get("err") or ""
    return {
        "status": response.get("status"),
        "messages": [line for line in str(text).splitlines() if line.strip()],
    }


# --- entry point ---------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if argv and argv[0] == "--worker":
        return worker_main(argv[1:])

    parser = argparse.ArgumentParser(
        prog="crossrun_differential.py",
        description="Compare two mypy configurations of one corpus, leg by leg.",
    )
    parser.add_argument(
        "--pair", default="flips", choices=("flips", "kernel", "self"), help="named arm pair"
    )
    parser.add_argument("--a", action="append", default=[], help="arm A token, repeatable")
    parser.add_argument("--b", action="append", default=[], help="arm B token, repeatable")
    parser.add_argument("--name-a", default=None, help="arm A name")
    parser.add_argument("--name-b", default=None, help="arm B name")
    parser.add_argument(
        "--corpus", type=Path, default=DEFAULT_CORPUS, help="pinned corpus directory"
    )
    parser.add_argument("--corpus-a", type=Path, default=None, help="corpus for arm A alone")
    parser.add_argument("--corpus-b", type=Path, default=None, help="corpus for arm B alone")
    parser.add_argument(
        "--legs", default=",".join(ALL_LEGS), help=f"comma list of {','.join(ALL_LEGS)}"
    )
    parser.add_argument("--suites", action="append", default=[], help="suite path, repeatable")
    parser.add_argument("--out", type=Path, default=None, help="output directory")
    parser.add_argument("--tree", type=Path, default=REPO, help="tree under test")
    parser.add_argument("--python", default=sys.executable, help="interpreter for the arms")
    parser.add_argument("--ext-dir", action="append", default=[], help="scratch extension dir")
    parser.add_argument("--timeout", type=int, default=3600, help="per-arm timeout in seconds")
    args = parser.parse_args(argv)

    try:
        legs = resolve_legs(args.legs, ALL_LEGS)
        arm_a, arm_b = preset(args.pair)
        if args.a or args.b:
            if not (args.a and args.b):
                raise UsageError(
                    "--a and --b must be given together: one side alone would silently pair the "
                    "other with the preset"
                )
            arm_a = arm_from_tokens(args.name_a or "a", args.a)
            arm_b = arm_from_tokens(args.name_b or "b", args.b)
        if args.name_a:
            arm_a = dataclasses.replace(arm_a, name=args.name_a)
        if args.name_b:
            arm_b = dataclasses.replace(arm_b, name=args.name_b)
        if arm_a.name == arm_b.name:
            raise UsageError(
                f"both arms are named {arm_a.name!r}: the report could not tell them apart"
            )
        corpora = {
            arm_a.name: (args.corpus_a or args.corpus),
            arm_b.name: (args.corpus_b or args.corpus),
        }
        suites = tuple(args.suites) if args.suites else DEFAULT_SUITES
        out = (args.out or Path(tempfile.mkdtemp(prefix="crossrun-"))).resolve()
        out.mkdir(parents=True, exist_ok=True)
    except UsageError as exc:
        print(f"crossrun_differential: {exc}", file=sys.stderr)
        return 2

    optional = tuple(leg for leg in OPTIONAL_LEGS if leg in legs)
    ambient, stripped = split_ambient(dict(os.environ))
    # One environment per arm, from the same stripped base: an arm's declared
    # tokens must not travel to the other arm (see `arm_environment`).
    envs: dict[str, dict[str, str]] = {
        arm.name: arm_environment(ambient, arm, args.ext_dir) for arm in (arm_a, arm_b)
    }
    names = (arm_a.name, arm_b.name)
    runs: dict[str, dict[str, Any]] = {}
    for arm in (arm_a, arm_b):
        arm_dir = out / arm.name
        arm_dir.mkdir(parents=True, exist_ok=True)
        runs[arm.name] = run_arm(
            args.python,
            arm,
            arm_dir,
            corpora[arm.name],
            args.tree,
            optional,
            envs[arm.name],
            args.timeout,
        )

    results = compare_selected_legs(legs, runs, names)

    if LEG_SUITES in legs:
        # Like every other leg, the suites comparison reads its input back from
        # the arm's artifact, so a record that was not written (or was truncated)
        # is a refusal rather than a comparison against in-memory state.
        records: dict[str, Any] = {}
        refusal: str | None = None
        for arm in (arm_a, arm_b):
            record = run_suites_leg(
                args.python,
                arm,
                runs[arm.name]["dir"],
                args.tree,
                suites,
                envs[arm.name],
                args.timeout,
            )
            artifact = runs[arm.name]["dir"] / "dump" / "suites.json"
            dump_json(artifact, record)
            stored, refusal = read_json_artifact(artifact, arm.name)
            if refusal is not None:
                break
            records[arm.name] = stored
        if refusal is not None:
            results.append(Comparison(LEG_SUITES, VERDICT_DEGENERATE, refusal))
        else:
            results.append(compare_suites_leg(records[names[0]], records[names[1]], names))

    report = render_report((arm_a, arm_b), runs, results, legs, corpora, args.tree, stripped, out)
    (out / "report.txt").write_text(report, encoding="utf8")
    print(report)
    failed = [result for result in results if result.verdict != VERDICT_AGREE]
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
