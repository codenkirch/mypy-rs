# Perf wire-traffic audit and ranked hot-seam targets (#1624)

Status: measurement + ranked disposition. Counts only; no wall-clock
A/B is claimed. Companion comment: issue #1624.

- Head: `origin/main` = `501663e5c` (2026-09-14). The previous session's
  Phase-1 numbers were taken at `656cddbe3`; this document re-measures
  on the current head.
- Kernel: built from the same tree into `/private/tmp/mypy-rs-tk-1624`
  (`cargo rustc -p mypy-type-kernel --features extension-module --lib
  --crate-type cdylib --release`). AST/resolver scratch dirs from the
  standard `native-kernel-parity` setup.
- Corpus: cold self-check, `mypy_self_check.ini -n0 --no-incremental
  -p mypy -p mypyc`, 353 files, "Success: no issues found".

## Method

The harness wraps every `rust_*` attribute of `type_kernel` and every
module-level `_serialize*` function before `mypy.main` imports them.
Each serializer invocation is registered against the identity of the
bytes it produced (wire-cache hits return the same object and are
counted as repeat passes, not new events). A seam call "consumes" the
blob identity it receives. Every event lands in exactly one bucket:

- **useful**: consumed by a seam that returned a decision;
- **deferred**: consumed by a seam that returned `None`/-1;
- **unconsumed**: never reached a seam.

Cost proxy per seam, calibrated to issue #1624's own figures:

```
proxy_us = calls * 0.63            # fixed PyO3 crossing (documented #1135
                                   # measurement; AGENTS.md wave 60B)
         + call_bytes * 0.04       # payload re-decoded on EVERY wire call
         + encoded_bytes * 0.04    # distinct blob encoded once (Python side)
```

`0.04us/byte` (~25 MB/s) is the rate implied by #1624's "~2.7s residual
wire serialize/deserialize traffic" over the 69.7MB the previous audit
measured. The proxy is an attribution model, not an independent
measurement: it ranks and sizes, it does not predict. The top-45 rows
total 15.9s against the issue's measured +15.7s total-share regression,
which is corroborating but partly circular by calibration.

Hard counts only in the tables; the run wall (125.6s) and
`resolver_build_s` (1.86-2.44s across two runs) are load-contaminated
single-run attributions and are labelled as such.

## Headline (head `501663e5c`)

| bucket | events | bytes | share |
|---|---:|---:|---:|
| useful (consumed, decided) | 1,249,398 | 63,744,700 | 80.5% |
| **deferred (serialized, seam said `None`)** | **435** | **70,673** | **0.028%** |
| unconsumed (never reached a seam) | 302,065 | 5,750,195 | 19.5% |
| total serialization events | 1,551,898 | 69,565,568 | |

Seam census: **344 seams called, 8,307,304 calls, 2,858 defers
(0.034%)**. The kernel still decides 99.966% of seam calls; the
remaining cost is the price of the calls themselves.

## Ranked proxy (top 15 of 45)

`wire` = bytes cross the boundary; `live` = live-object seam, fixed
cost only. Disposition letters: (a) live-object, (b) batched call,
(c) resolver incrementalism, (d) Python-side gating/deletion. Slice
references are the follow-up issues filed from this audit.

| # | seam | calls | call MB | enc MB | proxy | shape | slice | seam anchor | Python caller |
|--:|---|--:|--:|--:|--:|---|---|---|---|
| 1 | `rust_has_recursive_types` | 903,792 | 45.4 | 14.3 | 2.96s | (d) memo / (a) | #1640 | `crates/type_kernel/src/visitor.rs:163` | `mypy/types.py:5660` |
| 2 | `rust_callable_is_generic` | 418,803 | 52.2 | 10.7 | 2.78s | (d) delete | #1640 | `crates/type_kernel/src/types_impl.rs:658` | `mypy/types.py:4940` |
| 3 | `rust_check_argument_types_plan` | 175,416 | 29.0 | 4.3 | 1.44s | (a)/(b) | #1642 | `crates/type_kernel/src/checkexpr_argtypes.rs:65` | `mypy/checkexpr.py:4356` |
| 4 | `rust_copy_modified` | 254,445 | 18.5 | 2.7 | 1.01s | (a) floor | — | `crates/type_kernel/src/copymodified.rs:379` | `mypy/types.py:5495` |
| 5 | `rust_expand_type` | 116,272 | 10.6 | 5.8 | 0.73s | (a) floor | — | `crates/type_kernel/src/expandtype.rs:239` | `mypy/expandtype.py:366` |
| 6 | `rust_flatten_nested_unions` | 276,749 | 9.0 | 3.5 | 0.68s | (a) live return | — | `crates/type_kernel/src/visitor.rs:938` | `mypy/types.py:5992` |
| 7 | `rust_classify_check_arg` | 233,755 | 10.3 | 1.1 | 0.60s | (d) delete | #1642 | `crates/type_kernel/src/checkexpr_functions.rs:7643` | `mypy/checkexpr.py:4551` |
| 8 | `rust_can_be_true_default` | 285,979 | 6.1 | 2.3 | 0.52s | (a)/(d) | #1640 | `crates/type_kernel/src/types_impl.rs:100` | `mypy/types.py:4867` |
| 9 | `rust_analyze_instance_member_dispatch` | 99,763 | 6.7 | 0.7 | 0.36s | (d)/(a) | — | `crates/type_kernel/src/checkmember.rs:2295` | `mypy/checkmember.py:786` |
| 10 | `rust_is_literal_type_like` | 245,695 | 3.8 | 0.3 | 0.32s | (d) gate | #1640 | `crates/type_kernel/src/typeops.rs:651` | `mypy/typeops.py:1946` |
| 11 | `rust_check_overload_call` | 14,358 | 5.7 | 2.0 | 0.32s | (a)/(b) | #1642 | `crates/type_kernel/src/overload.rs:142` | `mypy/checkexpr.py:4647` |
| 12 | `rust_callable_is_var_arg` | 33,172 | 5.8 | 0.2 | 0.26s | (d) delete | #1640 | `crates/type_kernel/src/types_impl.rs:582` | `mypy/types.py:2656` |
| 13 | `rust_can_be_false_default` | 213,404 | 3.0 | 0.1 | 0.26s | (a)/(d) | #1640 | `crates/type_kernel/src/types_impl.rs:163` | `mypy/types.py:4893` |
| 14 | `rust_classify_analyze_var` | 99,268 | 3.6 | 0.4 | 0.22s | (d)/(a) | — | `crates/type_kernel/src/checkmember.rs:4838` | `mypy/checkmember.py:943` |
| 15 | `rust_is_subtype` | 25,345 | 3.7 | 1.2 | 0.21s | (a)/(d) | — | `crates/type_kernel/src/subtypes.rs:4215` | `mypy/subtypes.py:939` |

Category sums (top-45 rows, total 15.88s):

| category | proxy |
|---|---:|
| `mypy/types.py` whole-tree bool predicates | 3.73s |
| `mypy/types.py` trivial scalar property seams | 3.22s |
| `mypy/checkexpr.py` + `argmap.py` call-check cluster | 3.43s |
| `mypy/types.py` tree transforms (`copy_modified`, `flatten`) | 1.69s |
| `expandtype` transforms | 0.97s |
| subtype / join / meet / solve | 1.06s |
| remaining 299 seams (below the table cut; 2.84M calls, 4.96MB; fixed-cost lower bound) | ~1.8s |

Live seams total 0.99s; wire seams total 14.89s (of the top-45 rows).
The top 10 rows are 72% of the proxy; the top 20 are 85%.

## Two seam families that can only lose

1. **`CallableType.is_generic` (#2).** The Python body is
   `bool(self.variables)` (`mypy/types.py:2830-2834`); the native path
   serializes the entire callable (avg 124.5 B/call, 52.2MB total,
   4.9 repeat passes per distinct blob) so Rust can read
   `variables.is_empty()` (`types_impl.rs:666-672`). At 418,803 calls
   this is 2.78s of proxy for a list truthiness check. The same shape
   covers `is_var_arg` (`ARG_STAR in self.arg_kinds`), `min_args`
   (`arg_kinds.count(ARG_POS)`), `is_kw_arg`, and
   `max_possible_positional_args`; long signatures serialize 174-421
   B/call for those reads.
2. **`ExpressionChecker.check_arg` (#7).** The Rust body
   (`checkexpr_functions.rs:7622-7633`) only tests
   `isinstance(caller, DeletedType)` plus two booleans the shim already
   computes; the Python fallback at `checkexpr.py:4586` is the same
   three-`if` dispatch and short-circuits the eager booleans. 233,755
   calls / 10.3MB / 0.60s proxy for an `isinstance`.

Both are older than the perf blocker's diagnosis: the property family
landed 2026-08-13 (`d46e71015`, #571), `classify_check_arg` on
2026-08-28 (`7e44b3320`, #1052, titled "perf"), and both are the exact
class `#1624` now names: "the semanal seams are net losses because
wire-encode dominates for short calls".

## Root cause (a): per-SCC resolver upkeep

Probe over the same run (the re-push count reproduces the previous
session exactly; `new_infos` 6,360 vs 6,359):

| quantity | value |
|---|---:|
| `_collect_incremental` calls (SCCs) | 521 |
| module-dict visits | 424,094 |
| infos fed to `resolver.update` | 64,856 |
| genuinely new | 6,360 (9.8%) |
| **`builtins.*` re-pushes** | **53,563 (82.6%)** |
| other (in-SCC, already snapshotted) | 4,933 |
| wall inside `_build_native_resolvers` | 1.86-2.44s (load-contaminated) |

`crates/type_kernel/src/typeinfo.rs:1210-1222` re-snapshots any
`builtins.*` info handed to `update` because promotion sinks
(`builtins.int`, `builtins.float`, `builtins.bytearray`,
`builtins.memoryview`) accumulate `_promote` entries from later SCCs;
`mypy/build.py:1660-1667` feeds all of them on every SCC call. The
mutation sites are `mypy/semanal_classprop.py:189-231`
(`add_type_promotion`), which fires 3,133 times. A dirty set of mutated
sinks replaces the 521x blanket re-push. This is fix shape (c) and
slice #1641.

## Fix direction 4 (residual Python-side wire traffic): exhausted

The class "serialized then the seam deferred" is 435 events / 70.7KB =
0.028% of serializations. Top rows are low double digits each
(`checkexpr.py:1067` container-type 154, `checkmember.py:640` 41,
`checkmember.py:639` 40, ...). There is no population left to gate.

The "unconsumed" bucket (302,065 events / 5.75MB) is 88% of events at
one site: `mypy/types.py:5747:_restore_list_identity`, which
re-serializes decoded rows to byte-match the live inputs for identity
restoration (the wire round-trip loses object identity). That is #1623
territory, not a gate. The next rows are
`types.py:5608:_serialize_type_list_for_visitor` (13,026) and the
`rust_is_subtype_batch` accumulator (`subtypes.py:879-880`, 13,864
together). A live-object return interface
(shape a) for the list-returning seams (`flatten_nested_unions` #6,
`copy_modified` #4) would delete this pass wholesale; it is the F2-class
follow-up after the slices above.

## Recommended slices (filed)

1. **#1640** - retire or live-wire the hot short-call seams in
   `mypy/types.py` (~7.3s proxy; `is_generic`, `has_recursive_types`,
   truthiness defaults, `is_literal_type_like`, `is_var_arg`,
   `min_args`). Mechanical on the delete side, no Rust ownership
   question for the O(1) reads.
2. **#1641** - dirty-driven per-SCC resolver update (53,563 -> ~3K
   builtins re-snapshots, -94%; most of the 1.9-2.4s). Python-side
   only, counts already reproduce twice.
3. **#1642** - cut the `check_call` seam cluster (delete
   `classify_check_arg`, live/plan `check_argument_types_plan`, batch
   the live scalar crossings) (~2.8s proxy).

Not yet sliced (documented floors): `copy_modified` / `expand_type`
tree transforms and `flatten_nested_unions` (all need the live-object
return interface and land with or after the F2/identity work), and the
subtype/join/meet residual defers tracked by the existing blocker
issues (#1618-#1623).

## Caveats

- Constant wire traffic now: a fresh `.so` must be built for any new
  counts (a stale extension silently nulls whole import blocks; this
  audit's `.so` resolved all 775 `rust_*` symbols).
- The harness keys serialization events by `id(bytes)`; consumed ids
  are not held alive, so id reuse adds low-double-digit noise. The
  headline buckets have been stable across the two runs on different
  heads (1,558,496 events at `656cddbe3` / 1,551,898 at `501663e5c`).
- `call_bytes` counts the blob passed to the seam on every call,
  including wire-cache hits, because the Rust side decodes per call;
  `enc MB` counts distinct blobs. The 0.63us and 0.04us/B constants are
  documented estimates, not measurements. Sensitivity: doubling the
  byte term against the fixed cost leaves ranks 1-7 unchanged
  (byte-dominated rows); ranks 8-12 swap among the fixed-cost-heavy
  rows (`can_be_true_default`, `analyze_instance_member_dispatch`,
  `check_overload_call`, `is_literal_type_like`). No category
  attribution changes.
