# ADR-0006: one-family replacement view for `Instance` (F reopening experiment)

- Status: Draft. Measured; the maintainer accepts or rejects.
- Date: 2026-09-15
- Issue: #1671 (F reopening experiment, from the wave-70B close-out
  `docs/plans/2026-09-11-f-program-close-out.md`).
- Follows: ADR-0001 Decision 1 (storage model), ADR-0002 Decision 4, ADR-0004
  (Decision 1, which this ADR succeeds *for the `Instance` family* only),
  ADR-0005 (the proxy scaffold this experiment deletes).

## Context

Phase F closed on 2026-09-11 unclaimed (#1573): every shipped F mechanism
measured zero or worse, and the ownership-transfer claim was not made. The
close-out named exactly one falsifiable route back:

> The one non-noise mechanism is replacement views for one family, requiring a
> successor to ADR-0004 Decision 1. Target: the one-family view prototype beats
> the current native default by >=10% relative total work share with
> capture/read off, all parity green.

ADR-0004 declined that architecture on contract grounds (plugins, `isinstance`,
`__slots__`, astmerge), not on measurement. #1671 is the measurement. This ADR
is its design record and its verdict.

Two premises are inherited rather than re-tested, because the close-out already
bounded them:

- The proxy read *shadow* is measured-exhausted (P2 wall -0.5% / CPU +5.6%;
  P2b corpus A +8.97% wall, corpus B -11% inside noise, ceiling <=0.5%).
- Capture overhead is fixed per-object semantics, not tunable slack (F1
  dual-write capture: +78.7s median, +85%).

This experiment therefore does **not** re-run either. It tests the one thing
neither measured: whether moving the field *storage* itself into Rust makes the
wire seam cheaper.

## The prototype

One family, `Instance`, and only `Instance`.

**Rust storage** (`crates/type_kernel/src/typeview.rs`): one `InstanceView` per
live object, holding `Instance.type.fullname`, the identity handles of the
arguments, the live argument objects themselves, a `fixed_up` flag (`type_ref
is None`) and an `args_tvar_clean` flag. Entries are keyed by the shared
`identity::handle_for_stable` handle (`crates/type_kernel/src/identity.rs:160`),
never by `id()`, and each entry pins its object, so a handle cannot outlive its
referent (identity guarantee 4).

**Python gate** (`mypy/typeview.py`): default off. Activation installs a
`__setattr__` interceptor on `Instance` and, in the read arm, an `args`
descriptor. Off is genuinely free: with no activation the class shape and the
attribute bytecode are exactly the pre-existing ones, and the one funnel hook
(`mypy/types.py:4798`) is a `None`-checked module global, so the inactive cost
is a single `is not None` test per seam call.

**The seam** (`mypy/types.py:4796-4804`): `_serialize_type_for_visitor` probes
the view before its own wire-cache probe, and returns the view's bytes when the
store can serve them.

**The serve rule.** The Rust encoder
(`crates/type_kernel/src/typeview.rs`, `encode_instance`) emits the wire bytes
straight from the store, recursing into registered `Instance` children, and
returns `None` on any doubt. It is served only when every one of these holds:

| Condition | Why |
|---|---|
| entry is present at the current stamp | the epoch/stamp gate |
| `Instance.type_ref is None` | mirrors the F3 cache's refusal of pre-fixup instances (`mypy/types.py:6284-6286`) |
| no argument is tvar-like, recursively | `TypeVarId.meta_level` is mutated during inference, so tvar bytes go stale (the `_type_wire_cache_saw_tvar` rule, `mypy/types.py:6287-6293`) |
| no `last_known_value`, no `extra_attrs` | the encoder emits the two `LITERAL_NONE` clears unconditionally |
| every argument is itself a registered, fresh entry | Rust owns no non-`Instance` leaf encoder |
| the stored fullname equals the live `TypeInfo.fullname` | `Instance.type` has no writer hook (see below) |

A miss returns `None` and the existing Python walk runs unchanged. A miss can
therefore cost a lookup; it cannot produce a wrong byte.

**Coherence.** Three mechanisms, deliberately not a write barrier:

1. `args`, `type_ref`, `last_known_value` and `extra_attrs` all pass through the
   `Instance.__setattr__` interceptor, so every write re-registers the object
   from its new field set. This covers the in-place mutators the F2 mirror had
   to enumerate by hand (`mypy/typeanal.py:1570,1579,3321,3406`,
   `mypy/semanal_typeargs.py:101,149,157`, `mypy/exprtotype.py:132`,
   `mypy/fixup.py:300,321`) and the constructor path
   (`mypy/types.py:1874-1957`).
2. `Instance.type` is the one field with **no** hook: it is read on nearly every
   seam, and a descriptor there would tax the whole build. It is instead
   re-verified against the live object on every serve. A retyped-in-place
   instance is a miss, not stale bytes.
3. `_clear_native_resolvers` (`mypy/build.py:1945`) drops the whole store at
   every daemon recheck boundary, alongside the other native resets.
4. The miss path makes one re-registration attempt
   (`mypy/typeview.py`, `encode`), because an argument may become registrable
   after its parent was built. That is the normal shape after a wire decode,
   which clears `type_ref` only post-fixup (`mypy/fixup.py:297-321`).

## Contract surfaces

The four surfaces ADR-0004 Decision 1 named as the reason to decline.

### 1. Plugin visibility

**Answer: unchanged, and structurally so.**

Plugins never see the store. They receive live `Type` objects exactly as before:
`FunctionContext.arg_types` / `default_return_type` (`mypy/plugin.py:439-470`)
are built from live lists (`mypy/checkexpr.py:2344-2377`), `AttributeContext` at
`mypy/checkmember.py:1308-1313`. The view changes no object a plugin can reach
and no class a plugin can `isinstance`. What it changes is which bytes the
funnel computes, and those bytes are byte-identical (below).

The `proper_plugin` corpus is a user-plugin corpus
(`mypy_self_check.ini:22`), and the corpus gate runs it. Result: see
"Measured verdict".

### 2. Identity and handles

**Answer: the view consumes the existing service; it mints nothing.**

Handles come from `identity::handle_for_stable`
(`crates/type_kernel/src/identity.rs:160`), the same layer the type mirror and
the node shadow use. There is no second handle namespace, and ADR-0005
correction 1 ("handles come from the existing `identity::handle_for` raw layer")
holds unchanged. Identity is not owned by the store: `reset` clears entries and
pins only, never `identity::reset`, which stays with `rust_mirror_reset`.

The store pins its entries, so a stale handle can never be adopted by a new
object even though the underlying key is `id()`-derived. That is the same
strong-pin protocol as the mirror, including its documented residual: an object
retained by a reference cycle keeps its pin until the next non-preserving reset
(identity guarantee 5).

Identity *semantics* do change in one measurable way: in the read arm,
`inst.args` returns a freshly built tuple rather than the slot's own tuple, so
`inst.args is inst.args` is no longer `True`. Nothing in mypy depends on that
identity, and the whole-graph differential below pins the *values*; but it is a
real API difference and this ADR records it rather than hiding it.

### 3. astmerge

**Answer: unaffected, and this is a design constraint, not a happy accident.**

`merge_asts` (`mypy/server/astmerge.py:117`) preserves the identities of
externally visible nodes and re-homes `TypeInfo` identities. The store holds no
node identity and no `TypeInfo` identity: it holds a fullname string and the
handles of `Instance` objects. There is nothing for a merge to re-home, which is
why no `_rehome`-style hook is needed next to the wirefixup one
(`mypy/build.py:2149`).

The conservative consequence: an instance whose `TypeInfo` is re-homed but whose
fullname is unchanged stays servable and is still correct, because the wire
carries the fullname and nothing else about the type.

### 4. Cache and daemon

**Answer: no cache participation; daemon boundary handled by the standard reset.**

- `CACHE_VERSION` is untouched, no `Options` field exists, and the gate is
  deliberately absent from `OPTIONS_AFFECTING_CACHE`. No view state can reach
  `mypy.cache`.
- Daemon: `_clear_native_resolvers` (`mypy/build.py:1945`) resets the store
  before semantic analysis on a fine-grained recheck, first in the chain so the
  mirror sweep sees the pins released. The gate is activated per `BuildManager`
  and re-activation is idempotent.
- Incremental: the store is session state; a build that never sets the env
  variable has no store at all.

## Measurement

Corpus: cold self-check `mypy_self_check.ini -n0 --no-incremental -p mypy -p
mypyc` (354 source files, `Success: no issues found` in all three legs), plus
`--dump-build-stats` with `MYPY_SERIALIZE_STATS=1`. All three legs ran in one
corpus-slot hold on the same kernel build and the same tree (`7d60e86c2`), with
`mypy.__file__` verified to point at the worktree.

Two legs, reported separately on purpose.

### Load-invariant leg (primary)

Counts and the RSS probe are load-invariant. `serialize_funnel_s` is a clock and
is read as a *share*, not as an absolute.

| Counter | baseline (`MYPY_TYPE_VIEW` unset) | arm 1 (store + serve) | arm 2 (+ routed reads) |
|---|---|---|---|
| `serialize_calls` | 2,839,640 | 2,839,692 | 2,839,637 |
| `serialize_view` (served from the store) | 0 | 712,772 | 712,728 |
| `serialize_view_bytes` | 0 | 10,100,326 | 10,099,690 |
| `serialize_writes` (walk encodes) | 1,204,139 | 814,068 | 814,061 |
| `serialize_bytes` (walk bytes) | 50,523,844 | 46,086,873 | 46,085,745 |
| `serialize_hits` (F3 wire cache) | 1,163,415 | 920,305 | 920,304 |
| `serialize_funnel_s` | 1.347 | 1.852 | 1.990 |
| `typeview_encodes` / `defers` | - | 712,772 / 0 | 712,728 / 0 |
| `typeview_entries` | - | 3,207,019 | 3,206,888 |
| `typeview_read_routes` | - | 0 | 17,431,189 |
| `parse + semanal + type_check` | 63.572 | 83.063 | 87.218 |
| max RSS | 889,176,064 | 2,558,312,448 | 2,509,160,448 |

Derived:

| | value |
|---|---|
| served share of funnel calls | 712,772 / 2,839,692 = **25.1%** |
| served share of encoded bytes | 10.10 MB / 56.19 MB = **18.0%** |
| walk encodes removed | 1,204,139 -> 814,068 = **-32.4%** |
| walk bytes removed | 50,523,844 -> 46,086,873 = **-8.8%** |
| funnel share of total work | 1.347 / 63.572 = **2.12%** (baseline), 2.23% (arm 1), 2.28% (arm 2) |
| RSS cost | 889 MB -> 2,558 MB = **+1.67 GB, +188%** |

Four things this says.

1. **The mechanism works, and it is small.** The store serves a quarter of the
   funnel calls and removes a third of the walk's encodes. `defers` is 0: every
   registered instance was servable, so the refusal paths are not eating the
   result. But the served instances are the *cheap* ones: 18.0% of the bytes for
   25.1% of the calls, i.e. 14.2 bytes per serve against 41.9 bytes per walk
   write. Leaf singletons are two bytes. The view is best at exactly the work
   that was already cheap.

2. **The whole funnel is 2.1% of total work.** The clock is a share, but the
   denominator and the numerator come from the same run and the *ceiling* claim
   does not depend on the load: `serialize_funnel_s` covers cache probe, fast
   path and encode walk for every seam call. Removing 100% of it, with zero
   replacement cost, moves total work by 2.12%.

3. **The two halves of a replacement view separate cleanly, and neither pays.**
   This is the experiment's most decision-relevant result.

   *Capture buys the traffic.* Arms 1 and 2 differ only in whether `Instance.args`
   reads route through the store. Their wire counters are indistinguishable
   (814,068 vs 814,061 walk writes, a 7-write difference), so **the entire -32.4%
   write reduction comes from capture alone**, and routing reads adds no further
   wire saving.

   *The read route is a net cost.* Arm 2 routes 17,431,189 `args` reads through
   the store - 24x the funnel call count, and 6.1x the number of instances ever
   registered. Every routed read is a pyO3 attribute round-trip against a CPython
   slot read, and the wire saving it buys is zero. This is the effect the F risk
   register predicted for cold pyO3 attribute access, now isolated to one axis.

   *Capture cannot be free, and its cost lands outside the funnel clock.*
   Registration happens in `Instance.__setattr__`, so it is invisible to
   `serialize_funnel_s`: 3.21M `rust_view_put` calls, each minting a handle,
   pinning the object and its argument tuple, and inserting into two Rust
   `HashMap`s plus two Python dicts. The funnel it is trying to shrink is 1.347s
   in total. Measured on a single run per arm the `type_check_time` effect is
   large - 40.163s (baseline) -> 60.183s (arm 1) -> 65.968s (arm 2) - but this
   box ran three sequential single runs whose `parse_time`, on a code path that
   is *identical* in all three, moved 15.764s -> 12.922s -> 10.551s. A 33%
   swing on invariant code means a cross-run timing delta of this size is not
   attributable, and it is not claimed as the bar's measurement.

   The attributed part is structural and load-invariant: the mechanism whose
   entire addressable surface is 1.347s of funnel work requires 3.21M FFI
   registrations plus a per-access read route applied 17.43M times. Those counts
   do not improve under a quiet host.

4. **It costs 1.67 GB of RSS to do it.** 3.2M live entries, each pinning its
   object and its argument tuple. On a 64 GB machine, for a bound of 2.12%.

### Wall-clock leg: DEFERRED, not measured

The reopening bar names a *quiet host* for this leg: "cold self-check (...,
3+ interleaved pairs, quiet host, `scripts/measure_work_share.py`)". This host
was not quiet, and the artifacts prove it: the three legs are sequential single
runs, not the bar's interleaved pairs, and `parse_time` moved 15.764s -> 12.922s
-> 10.551s across them on a code path that is identical in all three. Load
averages recorded in the `.env` files were 13.79/13.37, 16.67 and 13.77, with
combined uid-501 RSS between 20 and 44 GB against the 51 GB cap and other lanes'
corpora starting and stopping inside the window.

No wall-clock figure is reported and none decides PASS/NO-GO. The `type_check_time`
column is recorded as measured and marked *not attributable*, for the reason in
item 3 above. It would not
change the verdict: the wall leg measures the same quantity the `type_check_time`
row already bounds, and a 10% wall improvement is arithmetically impossible when
the entire mechanism's addressable surface is 2.12% of the phases that dominate
the wall.

## Measured verdict: NO-GO

**The bar is missed by a measured factor of ~4.7, and the blocker is arithmetic,
not contract.**

The prototype is parity-clean on all four surfaces ADR-0004 named (see
"Measured contract status" below), which is the one thing the close-out did not
expect. What blocks it is the bound in item 2 above:

> The `Instance` family's wire funnel is 2.12% of total work on the cold
> self-check. A replacement view that removed *all* of it at *zero* cost would
> beat the native default by 2.12%, against a bar of 10%. The measured prototype
> removes 32.4% of the walk's encodes, and turns the funnel share into 2.23%
> (arm 1) / 2.28% (arm 2) because the probe runs on every call.

Which contract made it impossible: **none**. ADR-0004 Decision 1 declined
replacement views for plugin / `isinstance` / `__slots__` / astmerge reasons.
This experiment shows all four are satisfiable, and that the architecture is
nonetheless not worth building. The close-out's premise ("the only architecture
that removes per-object Python work is replacement views") is true and
immaterial: the per-object Python work it removes is a two-percent cost.

### Which half of a replacement view is the wall

Two separate walls, and a successor decision needs both:

| Half | Verdict | Evidence |
|---|---|---|
| Rust-owned field *storage* feeding the wire encode | delivers the traffic reduction, and cannot pay for itself | -32.4% walk writes for 3.21M registrations against a 1.347s funnel |
| Rust-owned field *storage* serving Python attribute *reads* | strictly negative | 17.43M routed reads, zero additional wire saving, +5.8s over arm 1 |

So the architecture is not "declined on contract" and not "viable but
unmeasured". It is measured, both halves are bounded, and the binding constraint
is that the wire funnel is a two-percent cost while a view's bookkeeping is
charged against every instance and every attribute access.

### Measured contract status

All four surfaces were exercised rather than argued:

- **All four CI parity jobs pass on this PR** (`parity`, `parity-mirror`,
  `parity-typeops`, `parity-ast`), on the arm-2 gate.
- **All three cold self-checks are clean** (`Success: no issues found in 354
  source files`), including the two gate-on legs, on a corpus that loads a user
  plugin (`mypy_self_check.ini:22` -> `mypy.plugins.proper_plugin`).
- The `typeview` suite's 26 tests include the gate-off/on funnel differential
  over a mixed graph and byte-parity for leaf, singleton, generic, two-arg and
  nested recursion, plus the Rust store's 13 unit tests.

So the honest successor to ADR-0004 Decision 1 is: the decision stands, for a
*measured* reason, and the four contract objections can be retired as objections.

## Consequences

1. **F stays closed.** `docs/plans/2026-09-11-f-program-close-out.md` should
   record this experiment as run, with the 2.12% ceiling as the reason, so the
   reopening route is closed on evidence rather than left open on a hypothesis.
2. **The prototype does not graduate and must not be defaulted on.** It is
   default-off, adds 1.67 GB RSS, and makes the funnel slower. If the maintainer
   rejects this ADR, its fate is deletion (a third orphan is not acceptable) or a
   separate successor issue.
3. **The measurement is reusable.** `MYPY_SERIALIZE_CLOCK` and
   `MYPY_SERIALIZE_STATS` both survive; any future "make the seam cheaper" claim
   should be checked against the funnel share first, because a design confined to
   the funnel cannot clear a 10% bar by construction.
4. **Retire "the funnel is the cost" as a hypothesis.** The remaining cost in
   `type_check_time` is elsewhere, and it is not addressable by type storage.

## What this does not change

- `CACHE_VERSION`, the incremental cache format, and `OPTIONS_AFFECTING_CACHE`.
- Any default. The gate is env-only (`MYPY_TYPE_VIEW`), default off.
- `Type.__slots__`, `Instance.__slots__`, `isinstance`, plugin hook surface,
  and the attribute API when the gate is off.
- The F3 wire cache and the F2 mirror read flip, which keep their ordering
  behind the view probe.

## Superseded material

The `type_proxy.py` / `proxy.rs` P1 scaffold (#1553) and its six
`rust_proxy_*` pyfunctions are deleted here, at the issue's instruction ("wire
it or delete it, do not leave a third orphan"). ADR-0005's "P1 landed" record
stays historically accurate for its date; the scaffold is retired because P2
and P2b measured its ceiling at <=0.5% wall, which is what motivated testing
replacement views instead of a shadow at all.