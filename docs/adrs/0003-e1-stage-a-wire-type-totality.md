# ADR-0003: E1 Stage A wire Type totality (ErasedType tag-122 round-trip)

- Status: Proposed
- Date: 2026-08-29
- Issue: #1138 (ADR-0002 Decision 4a, Stage A of the E1 backplane)
- Follows: ADR-0002 (Decision 4: E1 backplane, design-only and gated),
  ADR-0001 (parity surface)

## Context

ADR-0002 Decision 4 makes wire Type totality the terminal prerequisite for any
Python-fallback removal in the type kernel: "no removal of the Python fallback
until the wire Type enum round-trips every `mypy.types.Type` variant and
gate-off/gate-on parity is byte-identical for 100% of seams". Issue #1138 asks
for the design of the ErasedType (tag 122) part of that claim, plus a full
variant inventory.

### Discovered state (supersedes the filed premise)

The issue body and the AGENTS.md type-kernel section describe the invariant as
"`read_type` deliberately does NOT decode tag 122 (there is no ERASED_TYPE
branch in `read_type`)". On the current tree that is no longer literally true:

- `ERASED_TYPE = 122` is a defined tag (`mypy/types.py:5316`), and
  `ErasedType.write` / `ErasedType.read` exist (`mypy/types.py:1622-1629`).
- `read_type` has an `ERASED_TYPE` branch (`mypy/types.py:5364-5367`), but it
  decodes only when the module-level opt-in flag `_ALLOW_WIRE_ERASED_TYPE`
  (`mypy/types.py:5318-5321`, default `False`) is set; otherwise it still hits
  `assert False` (`mypy/types.py:5364-5367`), so default behavior is unchanged.
- Exactly one production consumer opts in: the `replace_meta_vars` shim
  `_deserialize_type_with_erased` (`mypy/erasetype.py:185-203`), which flips
  the flag around its own decode and restores it in `finally`. A second decode
  site in the same module, the `erase_typevars` shim
  (`mypy/erasetype.py:321-326`), decodes without the flag and keeps the
  refuse-and-defer behavior, demonstrating the per-site split. Both shapes
  shipped in 7cec9c815 (#817/#818, "port replace_meta_vars meta erasure
  incl. ErasedType targets").
- The Rust side is already first-class: `Type::ErasedType` is an enum variant
  (`crates/type_kernel/src/wire.rs:485,547`), read via the `ERASED_TYPE` arm of
  `read_type` (`wire.rs:1147,1171` -> `read_erased_type`, `wire.rs:924-926`,
  tag constant `wire.rs:99`), and encoded by `write_type`
  (`wire.rs:2042-2046`). `rust_replace_meta_vars` depends on the opt-in decode
  of its own result (`crates/type_kernel/src/erase_typevars.rs:56-75`).
- All three blockers of #1138 are closed: #1133, #1134, #1122.

The deep-recursion safety property therefore no longer lives in the branch's
absence; it lives in the opt-in's shape: default-off, flipped per decode
around bytes the caller owns, restored in `finally`. A seam shim that decodes
result bytes without opting in still fails on tag 122 (`AssertionError`,
caught by the shim's `except (AssertionError, NotImplementedError)`, running
the pure-Python body). That is what protects the deep
`is_protocol_implementation <-> is_callable_compatible <-> is_subtype`
recursion chain documented for mypy #21445 (`ziplike` / `f0-overload`;
`NativeHasErasedComponentSuite`, `mypy/test/testtypes.py:12220`). This ADR
names that property, keeps it, and scopes the totality claim around it.

### Current wire variant inventory

Producing side: Python `*.write` methods in `mypy/types.py`. Consuming side:
`read_type` (`mypy/types.py:5324-5372`) and the Rust `Type` enum
(`crates/type_kernel/src/wire.rs:485`). Three disjoint sets:

1. **Type-wire serializable (20 variants)**, round-tripping today except for
   the ErasedType decode gate:
   Instance (incl. `last_known_value` and `extra_attrs`,
   `mypy/types.py:1885-1906`; Rust `ExtraAttrs` field, `wire.rs:451-459`),
   TypeAliasType (`type_ref` resolver key only, `alias` intentionally not on
   the wire, ADR-0002 Decision 1), TypeVarType / ParamSpecType /
   TypeVarTupleType, UnboundType, UnpackType, AnyType / UninhabitedType /
   NoneType / DeletedType, CallableType / Overloaded, TupleType / TypedDict /
   LiteralType / UnionType / TypeType, Parameters, ErasedType
   (encode exists both sides, Python decode opt-in only).
2. **Runtime-only wrappers with no wire form by construction**: they inherit
   `Type.write` / `Type.read`, which raise
   `NotImplementedError("Cannot serialize ...")` (`mypy/types.py:408-413`):
   `TypeGuardedType` (`mypy/types.py:559`, transient wrapper for
   `find_isinstance_check`), `RequiredType` (`:577`),
   `ReadOnlyType` (`:595`), `PartialType` (`:3837`),
   `PlaceholderType` (`:4006`, must be gone after semanal by contract).
   The wire codec cannot produce them, so no Rust variant exists;
   totality excludes them by construction.
3. **AST-only pseudo-types**, valid only in the serialized-AST stream, not the
   Type wire (tag comments at `mypy/types.py:5312-5315`): `TypeList` (118),
   `EllipsisType` (119), `RawExpressionType` (120), `CALL_TYPE` (121). They
   ride `mypy/astwire.py` / the `ast_serialize` crate, not `read_type`.

Follow-up bucket discovered during this review, not fixed here: several kernel
comments still assert ErasedType reachability facts from the pre-slice-48
state ("ErasedType is not on the wire", `crates/type_kernel/src/meet.rs:264`;
"ErasedType has no wire-format variant", `setops.rs:264-265,430-431`;
"ErasedType has no wire representation", `subtypes.rs:171`;
"`ErasedType` has no `write`/`read` in mypy/types.py, so it can never appear
in serialized input", `checker_stmts.rs:396-398`). The reachability
assumptions encoded there must be re-audited before any additional seam opts
in to ErasedType-bearing decodes; tracked separately (see Consequences).

## Decision 1: generalize the opt-in decode; never make tag 122 unconditionally decodable

**Decision: keep `_ALLOW_WIRE_ERASED_TYPE` as the single switch, formalize its
use as a context manager in `mypy/types.py`, and grow the allowed decode sites
seam by seam.** The `replace_meta_vars` pattern (`mypy/erasetype.py:185-203`)
becomes the canonical mechanism: wrap only the decode of result bytes the
caller legitimately owns, restore in `finally`. Concretely: `mypy/types.py`
gains a context manager (suggested name `_wire_erased_roundtrip()`) that sets
the flag, yields, and restores; `mypy/erasetype.py:194-203` is reduced to use
it (no behavior change, same finally semantics).

Usage contract, enforceable by call-site review and the Direction C test
(Decision 3):

1. Default-off. Nothing sets the flag at import time, at build start, or via
   `Options`; only a shim decode wraps itself in the context manager.
2. `finally`-restore, so a decode failure cannot leak the flag past the
   boundary.
3. Never wraps a decode of persisted bytes. `mypy/cache.py` has no `read_type`
   import (the incremental cache stores JSON-serialized types, not the Type
   wire stream), and `_write_type_cached` (`mypy/types.py:5407-5429`) is a
   per-session in-memory wire cache, not persisted state. The Type wire stream
   exists only on the seam boundary, which is what makes a per-decode opt-in
   sound.

A seam may opt in only when Python semantics say that seam's result can
legitimately contain an ErasedType (the `replace_meta_vars` precedent:
`checkexpr` passes `ErasedType()` as the target, `mypy/erasetype.py:188-191`).
Everywhere else the refuse-and-defer behavior stays.

**Rejected alternatives:**

- *Unconditional `ERASED_TYPE` branch in `read_type`*: destroys the safety
  property for every generic decode site. One Rust seam bug that lets an
  ErasedType leak across a boundary it does not own would silently produce a
  (possibly malformed) live `Type` instead of a loud defer, re-opening the
  mypy #21445 recursion fragility over the whole seam surface. Rejected.
- *Keyword parameter `read_type(data, allow_erased=True)`*: backward
  compatible, but churns signatures at every decode call site for no
  correctness gain, while the context manager keeps zero signature changes.
  The flag itself is process-local module state; mypy parallel workers are
  processes, not checker threads, so no cross-thread leak exists. Rejected in
  favor of the context manager.
- *Kernel-side blanket replacement* (emit `UninhabitedType` or `AnyType`
  instead of `ErasedType` before crossing the seam): not semantically
  faithful, and unnecessary where it would count: kernels that can decide
  ErasedType leaves already passthrough or handle them
  (`erase_typevars.rs:538` passthrough, `visitor.rs:197-201`,
  `subtypes.rs` leaf handling). Rejected as a general mechanism; per-seam
  deferral remains the fallback.

## Decision 2: scope the totality claim to the inventory

**Decision: Stage A's "every `mypy.types.Type` variant" means the 20-variant
Type-wire set (inventory set 1).** What this design changes versus today:

- ErasedType: moved from "one opt-in consumer" to "any seam may opt in for its
  own result decode via the Decision 1 context manager". Encode already exists
  on both sides. No new variant bytes; `CACHE_VERSION` is not touched (the
  Type wire stream never reaches persisted caches).
- `extra_attrs`, `last_known_value`: already carried by the Instance wire form
  (`mypy/types.py:1904-1906`, Rust fields at `wire.rs:451-459,489-494`). No
  gap; the known `extra_attrs` deferral inside subtype/join kernels is a
  decision-coverage matter for Stage B/3c, not a codec gap.
- `TypeAliasType.alias` stays a resolver key (ADR-0002 Decision 1; snapshot
  completeness and chain resolution shipped via #1133/#1134/#1149). No target
  inlining.
- Sets 2 and 3 gain no wire forms: set 2 cannot be produced by the codec
  (`NotImplementedError` by construction, `mypy/types.py:408-413`), set 3
  belongs to the AST stream (`mypy/types.py:5312-5315`).

## Decision 3: the totality parity sweep, gated off by default

**Decision: a new `NativeWireTotalitySuite` in `mypy/test/testtypes.py`,
styled after `NativeTypeWireSuite` (`mypy/test/testtypes.py:1651-1652`), which
runs only under `TEST_NATIVE_TYPE_KERNEL=1` with the extension built
(`_NATIVE_WIRE_ENABLED`, `mypy/test/testtypes.py:1644`).** Fixture: one
minimal instance per inventory set-1 variant; relative to the existing suite
this adds TypeAliasType, ParamSpecType, TypeVarTupleType, UnpackType,
Parameters, TypedDictType, an `extra_attrs` carrier, and ErasedType itself.

Three directions:

- **A, Python write -> Rust decode** (str parity): reuse the existing
  `assert_wire_par` pattern: `Type.write` into `librt.internal.WriteBuffer`,
  `read_type_to_str(bytes) == str(t)` (pyfunction registered at
  `crates/type_kernel/src/lib.rs:177`). Proves the Rust reader decodes every
  isinstance-branch, including the `ERASED_TYPE` arm.
- **B, byte-level round-trip both sides** (object totality): add a test-only
  pyfunction `wire::rust_roundtrip_type(bytes) -> Option<Vec<u8>>` that decodes
  with the Rust `read_type` (`wire.rs:1147`) and re-encodes with `write_type`
  (`wire.rs:2023`); the suite asserts byte equality with the Python-written
  buffer for every fixture. Byte equality is strictly stronger than `str()`
  parity: it proves every field survives, including `extra_attrs`, TypeVar
  `meta_level`/`namespace`, and `TypeVarType.id`. The same byte-identity is
  asserted on the Python side under the Decision 1 context manager:
  `read_type` then re-`.write()`, mirroring the existing
  `test_typevar_meta_level_roundtrip` object-level pattern
  (`mypy/test/testtypes.py:1831-1853`).
- **C, invariant negative proof** (no gate, runs in plain CI): with the flag at
  its default, `read_type` on tag-122 bytes raises `AssertionError`
  (`mypy/types.py:5364-5367`), i.e. the safety property is pinned by a test,
  not by folklore. This test is *not* under `skipUnless`, because refusing tag
  122 is precisely the default that must never regress while the rest of the
  suite is opt-in.

Rust-side: `#[cfg(test)]` unit tests in `wire.rs` asserting
`read_erased_type` / `write_type` idempotence on `Type::ErasedType`
(empty-body variant, byte-exact two-tag form).

No production CLI change is needed: `TEST_NATIVE_TYPE_KERNEL=1` plus the
scratch-dir build recipe in AGENTS.md ("Type kernel build order") already
covers the extension build; the suite inherits both.

## Decision 4: out of scope

- No Python-fallback removal. No `Options.native_type_kernel` default flip.
  No `_ALLOW_WIRE_ERASED_TYPE` default flip.
- No wire forms for inventory set 2 (`TypeGuardedType`, `RequiredType`,
  `ReadOnlyType`, `PartialType`, `PlaceholderType`).
- No changes to the AST-only stream (set 3) or to `mypy/astwire.py`.
- The Stage B proxy model (ADR-0002: reflect-into-Python-on-write, hold
  `TypeInfo`/`TypeAlias` by identity) and the 3c `is_subtype` port. ErasedType
  decision semantics inside join/meet/setop kernels keep deferring exactly as
  today; reaching them only changes which decodes refuse.
- No kernel comment rewording or reachability rework in this change; the
  stale-ErasedType-comment bucket above is tracked as a separate issue.

## Consequences

- The safety property survives totality by construction: refusal is the
  default decode behavior for tag 122; only owned-result decodes opt in; a
  regression in the default is pinned by the ungated Direction C test.
- Stage A completion for E1 is then implementable without touching the codec:
  the implementation slice is the context manager, the round-trip pyfunction,
  and the three-direction suite; parity runs via the AGENTS.md type-kernel
  recipe (`testtypes.py` + `testcheck.py` under `TEST_NATIVE_TYPE_KERNEL=1`),
  plus the standard `fine-grained` / cache / incremental checks and a 0-error
  self-check.
- The AGENTS.md type-kernel paragraph on this invariant is updated in the same
  change to describe the opt-in mechanism truthfully.
- Open follow-up: re-audit the ErasedType reachability comments in
  `meet.rs` / `setops.rs` / `subtypes.rs` / `checker_stmts.rs` before any new
  seam opts in to ErasedType-bearing decodes (separate issue, see PR body).
