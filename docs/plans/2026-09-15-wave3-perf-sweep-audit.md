# Wave 3 perf sweep audit: scalar-only wire seams (#1637)

Status: measurement + ranked disposition. Counts only; no wall-clock A/B
is claimed. Companion comment: issue #1637.

- Head: `ce6e1ae3f` (2026-09-15).
- Kernel: built from the same tree into
  `/private/tmp/mypy-rs-1637-typekernel`
  (`cargo rustc -p mypy-type-kernel --features extension-module --lib
  --crate-type cdylib --release`). AST/resolver scratch dirs from the
  standard `native-kernel-parity` setup.
- Corpus: cold self-check, `mypy_self_check.ini -n0 --no-incremental
  -p mypy -p mypyc`, 353 files, "Success: no issues found".
- Scope: `typeops.py`, `checkmember.py`, `checkpattern.py`,
  `messages.py`, `meet.py`, `typeanal.py`. `subtypes.py`,
  `checkexpr.py`, `checker.py` (first pass), and `types.py` are
  excluded (owned by the F/G graph work or already audited by #1624).

## Method

A harness wraps every candidate `rust_*` pyfunction on the
`type_kernel` module before `mypy.main` imports them, counting calls
and summing bytes-typed argument sizes. The harness is env-gated
(`MYPY_TK_1637_AUDIT=1`) and prints at exit via `atexit`. The cost
proxy follows the #1624 model:

```
proxy_us = calls * 0.63            # fixed PyO3 crossing (#1135)
         + call_bytes * 0.04       # payload re-decoded per call
         + encoded_bytes * 0.04    # distinct blob encoded once
```

`encoded_bytes` is estimated as `call_bytes * 0.445` (the aggregate
writes/calls ratio: 1,063,657 distinct writes / 2,388,108 serialize
calls). Per-seam hit rates vary, so this is an approximation; the
ranking is stable under a 2x sensitivity test on the byte term.

Classification was done by reading every Rust function body behind
each seam and labeling it SCALAR-ONLY (the wire round-trip's only
purpose is to read one or two fields off the decoded node), TREE-WALK
(the Rust body traverses the type tree, making the wire cost
amortized), or LIVE-OBJECT (the seam already uses PyO3 live reads,
zero wire bytes). Only SCALAR-ONLY seams are deletion candidates.

Aggregate serialize stats for context: 2,388,108 calls, 32.9MB
total bytes, 1,063,657 distinct writes, 952,278 wire-cache hits.

## Ranked candidate seams (cold self-check)

| # | seam | module | calls | call KB | enc KB | proxy | shape | Python fallback |
|--:|---|---|--:|--:|--:|--:|---|---|
| 1 | `rust_descriptor_has_get_set` | checkmember.py:1513 | 23,414 | 515.1 | 229.4 | 45.25ms | scalar: 2x `has_readable_member` MRO dict walk | `has_readable_member("__get__"/"__set__")` |
| 2 | `rust_is_singleton_equality_type` | typeops.py:2009 | 10,561 | 270.2 | 120.3 | 22.65ms | scalar: `isinstance(LiteralType)` OR identity inner | `isinstance(typ, LiteralType) or is_singleton_identity_type(typ)` |
| 3 | `rust_is_recursive_pair` | typeops.py:371 | 1,982 | 115.9 | 51.6 | 8.11ms | scalar: `is_recursive` flags on 2 top-level nodes | `get_proper_type` + isinstance chain (shallow, 1-step alias) |
| 4 | `rust_is_singleton_identity_type` | typeops.py:1978 | 7,511 | 40.5 | 18.0 | 7.13ms | scalar: NoneType tag, Instance fullname/name-set | isinstance chain: NoneType/Instance/LiteralType/TypeType/FunctionLike |
| 5 | `rust_analyze_none_member_access` | checkmember.py:1252 | 996 | 3.9 | 1.7 | 0.86ms | tag check (`NoneType`) + `name == "__bool__"` dispatch | `__bool__` construction; else recursion into `_analyze_member_access` |
| 6 | `rust_simple_literal_type` | typeops.py:1257 | 174 | 4.6 | 2.0 | 0.38ms | scalar: `last_known_value` field read | `last_known_value` extraction + LiteralType construction |
| 7 | `rust_make_inferred_type_note` | messages.py:4145 | 91 | 5.1 | 2.3 | 0.36ms | scalar: type_ref/args-nonempty reads | format "perhaps you need a type annotation" message |
| 8 | `rust_append_numbers_notes` | messages.py:4112 | 131 | 3.7 | 1.6 | 0.30ms | scalar: type_ref + name-set membership | decode blob, read `type_ref`, check name set |
| 9 | `rust_bind_self_fast` | checkmember.py:2734 | 5 | 3.6 | 1.6 | 0.21ms | special: shim discards decoded result, rebuilds from live method | `bind_self` non-generic strip path (live object) |
| 10 | `rust_analyze_typeddict_access` | checkmember.py:2519 | 14 | 1.7 | 0.8 | 0.11ms | tag-only (`TypedDictType`), returns fixed CallableType by `name` | `name`-dispatched CallableType construction |
| 11 | `rust_erase_to_bound` | typeops.py:1193 | 9 | 0.7 | 0.3 | 0.05ms | tag dispatch: TypeVarType->upper_bound, TypeType->item | isinstance + field read |
| | **TOTAL** | | **44,888** | **988.1** | **439.7** | **85.40ms** | | |

## Zero-engagement candidates (cold self-check)

Nine candidate seams had zero calls on the cold self-check corpus.
The self-check has no `match` statements (confirmed in AGENTS.md),
so all `checkpattern.py` seams are zero-engagement here. They would
show on the `testcheck` corpus; measurement against testcheck is out
of scope for this audit (the issue scopes to the self-check corpus).

| seam | module | reason for zero engagement |
|---|---|---|
| `rust_is_simple_literal` | typeops.py:1279 | not reached on self-check (gate skips to `simple_literal_type`) |
| `rust_instance_fallback` | checkmember.py:2836 | not reached (all Instance accesses have readable fallbacks) |
| `rust_meta_has_operator` | checkmember.py:2868 | not reached (no operator member accesses in self-check corpus) |
| `rust_is_uninhabited` | checkpattern.py:1347 | no match statements in self-check |
| `rust_get_type_range` | checkpattern.py:1318 | no match statements in self-check |
| `rust_filter_or_match_types` | checkpattern.py:252 | no match statements in self-check |
| `rust_classify_sequence_tuple_result` | checkpattern.py:413 | no match statements in self-check |
| `rust_expand_starred_pattern_types` | checkpattern.py:687 | no match statements in self-check |
| `rust_unknown_unpack` | typeanal.py:4541 | not reached (no unpack-in-type errors in self-check) |

These remain deletion candidates if they fire on other corpora; the
classification (SCALAR-ONLY) holds regardless of call count. A
follow-up measurement against `testcheck` would quantify them.

## `meet.py`: zero candidates

All six wire seams in `meet.py` were classified as TREE-WALK. The Rust
bodies drive the subtype engine (`is_subtype`), the overlap kernel,
or tuple-element recursion. The wire round-trip is amortized over the
tree walk; deleting these would lose native engine coverage with no
serialization savings.

## `typeanal.py`: one candidate

`rust_unknown_unpack` (typeanal.py:4541) is SCALAR-ONLY: a top-level
variant tag check plus one target tag read. Zero calls on the
self-check. All other typeanal seams are TREE-WALK (alias expansion,
type analysis recursion).

## Notes on the `rust_bind_self_fast` special case

`rust_bind_self_fast` (checkmember.py:2734) is not a scalar-read
pattern. The seam serializes the callable, Rust strips the first
parameter and sets `is_bound=True`, and the Python shim **discards
the decoded result** and rebuilds the final object via
`copy_modified` on the live method. The wire round-trip is pure
overhead: the shim could call `bind_self`'s non-generic strip path
directly (a few live-object attribute reads). Only 5 calls on the
self-check, so the proxy is negligible (0.21ms), but the pattern is
the same class as the #1640 deletions: the native path does nothing
the Python body can't do cheaper on a live object.

## Total potential savings

Summing the proxy costs of all 11 measured candidates: **85.4ms** on
the cold self-check. The top 2 seams (`rust_descriptor_has_get_set`
and `rust_is_singleton_equality_type`) account for **67.9ms (79%)**.

For context, the #1624 audit measured 15.88s proxy across the top-45
seams. This audit's candidates are all below that cut: they are
smaller seams where the wire cost is modest in absolute terms but the
benefit/cost ratio is strongly negative (serialize a full type tree to
read one field).

The savings are small relative to the #1624 hot seams, but the
deletions are mechanical (same shape as #1640): remove the Python
gate, keep the Rust function registered for direct-seam tests, add a
parity differential pin. No Rust ownership question arises because the
Python fallback is the full pure-Python body, behavior-preserving by
construction.

## Recommendations

1. **Delete `rust_descriptor_has_get_set` gate** (checkmember.py:1504-1523).
   45.25ms proxy, 23K calls. Python fallback is `has_readable_member`
   (MRO dict walk). Highest-impact single deletion in this audit.

2. **Delete `rust_is_singleton_equality_type` + `rust_is_singleton_identity_type` gates** (typeops.py:1975-2015).
   29.78ms combined proxy, 18K calls. Python fallbacks are isinstance
   chains. These two are tightly coupled (equality calls identity);
   delete together.

3. **Delete `rust_is_recursive_pair` gate** (typeops.py:364-376).
   8.11ms proxy, 2K calls. Python fallback is a shallow `get_proper_type`
   + isinstance chain. The alias-chain expansion is 1-step, not a tree walk.

4. **Delete `rust_analyze_none_member_access` gate** (checkmember.py:1242-1273).
   0.86ms proxy, 1K calls. Not a pure scalar read (else-path recurses),
   but the gate only decides the `name == "__bool__"` split; the
   Python body handles both branches. Low impact but clean.

5. **Delete remaining low-call gates** (items 6-11). These sum to
   1.71ms; individually negligible. Batch with the above or defer.

6. **Defer checkpattern.py candidates** pending testcheck measurement.
   Zero engagement on self-check; classify after a testcheck corpus
   run if pattern-matching volume warrants.

7. **Keep all TREE-WALK seams** in meet.py and the remaining typeanal.py
   sites. The wire cost is amortized over genuine tree traversal.

## Caveats

- `call_bytes` counts the blob passed to the seam on every call,
  including wire-cache hits. The `enc KB` column estimates distinct
  blobs via the aggregate writes/calls ratio (0.445); per-seam hit
  rates vary, so this is an approximation. The ranking is stable
  under a 2x sensitivity test on the byte term.
- The proxy is an attribution model, not a wall-clock measurement.
  The actual savings from deleting a seam are the wire cost minus
  the Python body cost; for scalar-only seams the Python body is a
  few dict lookups, so the savings approximate the full proxy.
- The self-check corpus has no `match` statements, so checkpattern.py
  seams are unmeasured. A testcheck measurement would fill the gap.
- Head `ce6e1ae3f` includes the #1640 deletions (9 hot short-call
  seams already retired from types.py). The candidates here are the
  next tier below the #1624 top-45 cut.
