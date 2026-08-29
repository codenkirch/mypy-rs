# ADR-0004: E1 Stage B Rust-owned Type proxy (reflect-into-Python-on-write)

- Status: Proposed
- Date: 2026-08-29
- Issue: #1139 (ADR-0002 Decision 4b, Stage B of the E1 backplane)
- Follows: ADR-0003 (Stage A wire Type totality), ADR-0001 (Decision 1 storage
  model), ADR-0002 (Decision 4: design-only and gated)

## Context

ADR-0002 Decision 4 defines the terminal line of the migration: a Rust-owned
Type proxy, design-only and behind the `native_type_kernel` gate, with no
Python-fallback removal while the strangler per-call gates still carry the
load. The measured motivation is in ADR-0002 Decision 3: the residual
per-call `encode`/`fixup` of whole type graphs at the wire seams is the
dominant Python-side cost of the on-by-default seams, and
`docs/remaining-migration-plan.md` records total work share as negative
today. A proxy replaces "re-serialize the same graph on every seam call"
with "walk the live graph once per scope, then read the shadow".

Stage A (ADR-0003, #1138, merged as design via #1186) is the prerequisite:
it fixes the field set every Rust-side Type representation may rely on
(the 20-variant inventory set 1) and settles the ErasedType decode contract.
Stage B consumes that contract; it does not change the codec.

### Discovered state (supersedes stale premises)

Two corrections against the filed text, both already recorded in ADR-0003:

- `mypy/types.py` `read_type` now *has* an `ERASED_TYPE` branch
  (`mypy/types.py:5364-5367`), decoded only when the module-level opt-in
  flag `_ALLOW_WIRE_ERASED_TYPE` (`mypy/types.py:5321`, default `False`)
  is set; `ErasedType.write`/`read` exist (`mypy/types.py:1622-1629`).
  This ADR never assumes "ErasedType is not on the wire".
- Kernel comments asserting ErasedType unreachable
  (`crates/type_kernel/src/meet.rs:264`, `setops.rs:264-265,430-431`,
  `subtypes.rs:171`, `checker_stmts.rs:396-398`) are stale and tracked for
  re-audit as #1185. Every "mutates in place" claim below is grounded in the
  mutating site itself, never in those comments.

A third discovery made during this review and load-bearing below: the
existing wire-byte cache already encodes the phase-mutability split this
ADR needs. `_type_wire_cache_enabled` is documented as "only cache during
type checking, when Type objects are frozen. Semanal/typeanal mutate
Instance.args and friends in-place; caching there would serve stale bytes"
(`mypy/types.py:129-137`), and `TypeVarId.meta_level` taint tracking
excludes typevar-carrying graphs because meta_level is mutated during
checking (`mypy/types.py:5407-5421`). The mutability boundary the proxy
must respect is therefore already known, named, and proven in production.

### Scope of "proxy"

A **proxy** is a Rust-side shadow of a live Python `Type` node tree: one
Rust structure per proxied node holding the fields of the Stage A wire set
plus a `PyObject` handle to the live node. Proxies serve reads for the hot
seams; Python objects remain the single source of truth for identity.
Three field classes govern every field:

- **O (owned-by-Rust)**: plain data with no in-place mutator anywhere in
  the audited corpus. Rust is authoritative within the proxy scope;
  write-back (Decision 4) mirrors the value into Python.
- **R (reflected-through-PyO3)**: has an identified in-place mutator, or
  holds an arbitrary Python object, or carries position/truthiness
  semantics. Read via `getattr` on the node's own `PyObject` at access
  time; never cached beyond the call. This is the ADR-0002 Decision 3
  scalar-facts pattern applied per field instead of per seam.
- **S (snapshot-backed)**: holds a `TypeInfo`/`TypeAlias` identity.
  Stored as a fullname key, resolved through `TypeResolver.snapshots`
  (`crates/type_kernel/src/typeinfo.rs:200-212`) with the
  `live_info_map` -> `live_typeinfo` escape hatch for stale fields
  (`:211`, `:235-243`). No `TypeInfo` handle is ever stored in a proxy.

## Decision 1: shadow, never replacement (proxy shape and identity map)

**Decision: the proxy is a parallel structure. A proxied node is
`(PyObject live handle, Rust enum, validity class)`. Python objects are
never replaced, wrapped, or interposed; the identity map holds the live
handle and the `id()` key side by side, mirroring the `_type_wire_cache`
id-keyed-strong-ref precedent (`mypy/types.py:129-137`, identity
re-verify at `:5430-5434`), with the live map itself installed into Rust
as a single `PyObject` exactly like `TypeResolver.live_info_map`
(`typeinfo.rs:211`, `set_live_typeinfo_map` at `:1131-1135`).**

Concretely, the crate gains a scope-lifetime `TypeProxyStore` (new file,
e.g. `crates/type_kernel/src/proxy.rs`), holding:

- `roots: HashMap<i64 /* id(live) */, ProxyNode>` keyed on every
  proxied node, not only roots; each `ProxyNode` carries its own
  `PyObject` handle so R-fields are reachable per node without
  materializing anything.
- One installed identity map `Option<PyObject>`, a Python-side
  `dict[int, Type]` that pins the live objects (strong refs) so `id()` keys
  cannot be recycled mid-scope, the same protection `_type_wire_cache`
  relies on (`mypy/types.py:130-131`).

Why a shadow instead of pure reflection: the per-call wire traffic that
ADR-0002 Decision 3 names is structural, walking the whole graph,
`write_type`, and the decode+fixup tail (`mypy/wirefixup.py:113-139`) per
seam call. Once per scope, the proxy walk reads the same graph via `getattr`
(no buffer, no fixup); every subsequent seam call on the same graph hits the
shadow for O/S fields and pays only cheap R-field `getattr`s.

**Proxy lifetime is the checking-phase scope, not the build.** The store is
installed at target-check entry (or the first gated seam contact within a
target) and dropped at scope exit, plus unconditionally at every boundary
where the existing native state is cleared (Decision 4). This inherits,
instead of inventing, the mutability boundary `_type_wire_cache_enabled`
already draws (`mypy/types.py:129-137`): semanal/typeanal mutate
`Instance.args` (`mypy/typeanal.py:1588,3203,3288`) and stamp positions
(`_WirePositionStamper`, `mypy/typeanal.py:4013`) with no per-site
notification possible, so no proxy exists in that phase. Checking-phase
mutations are a closed, enumerable set (Decision 2 inventory), which is
what makes coherence tractable.

**Deferral is free.** Because live objects are never replaced, a deferring
seam simply falls back to the pure-Python body operating on the same live
graph, there is no resync, no re-materialization, and no
wire-decode-failure path. The wire path (`write_type` -> `read_type` ->
`fixup_wire_type`) remains for seams not yet proxied; the proxy and wire
paths coexist behind the same `Option`-defers-to-Python contract.

**Rejected alternatives:**

- *Replace live Types with Rust objects (true E1-prime arena)*: rejected
  by ADR-0001 Decision 1 and re-affirmed here, plugin hooks, binder
  state, and daemon `merge_asts` all key on live object identity
  (`mypy/server/update.py:1124-1131`); replacing objects would rewrite
  every consumer at once. Out of scope by charter.
- *Per-build proxy cache (build-lifetime, like the snapshot)*: rejected on
  coherence grounds. The snapshot is sound because `TypeInfo` fields are
  sealed at SCC boundaries; `Type` fields are not, checking mutates them
  (Decision 2). A build-scoped proxy would need per-mutation invalidation
  of a graph no site enumerates. Target scoping bounds the win, but the
  win is precisely the repeated per-target seam traffic the work-share
  measurements identify.
- *Pure reflection (no O fields at all)*: degenerates into per-field
  `getattr` for everything, which is the scalar-facts pattern ADR-0002
  Decision 3 already ships for individual seams; the proxy exists to also
  eliminate the structural walk. Rejected as "design that does nothing".

## Decision 2: field inventory and O/R/S classification

Classification rules, mechanically applied to the corpus:

1. **In-place mutator exists => R** (rule 1). A field assigned by any
   checker-phase code path (`mypy/checker.py`, `checkexpr.py`,
   `checkmember.py`, `checkpattern.py`, `applytype.py`, `typeops.py`,
   `subtypes.py`, `erasetype.py`) is R. Semanal-phase-only mutators
   (`typeanal.py`) do not demote a field because proxies are not live
   during semanal, the field is O from the checking phase's point of
   view, guarded by the phase-scope rule of Decision 1.
2. **Wire-carried => eligible for O; wire-dropped => R or S** (rule 2).
   The wire set (ADR-0003 inventory set 1) is the floor for O eligibility;
   anything the wire drops is either identity (S) or a fact only the live
   object knows (R). This makes the Stage A codec the totality reference
   for the shadow's structural fields.
3. **`TypeInfo`/`TypeAlias` identity => S, always** (rule 3), per
   ADR-0001 Decision 1 (hold by identity; no handles).

The inventory (line numbers at #1186, HEAD `4f499bf1d`; wire = Rust
`Type` enum, `crates/type_kernel/src/wire.rs:485-601`):

**Base classes**

- `Type` (`mypy/types.py:338`): `_can_be_true`/`_can_be_false` slots
  (`:341`), lazy inited `:357-358`, property setters are the mutation
  point (`:360-379`), **R** (truthiness feeds narrowing decisions;
  `UnionType` is the only variant that even writes them to the wire,
  `:3822-3823`). `line`/`column` ride `Context`
  (`mypy/nodes.py:161-170`) and are carried by **no** wire variant:
  **R**, re-stamped from the live node when a consumer keys on positions
  (the `_WirePositionStamper` precedent, `mypy/typeanal.py:4013`).
- `TypeVarId` (`mypy/types.py:618`): `raw_id`/`namespace` plain
  (`:633`,`:644`); `meta_level` **mutated in place during checking**
  (`mypy/applytype.py:211`, `mypy/typeops.py:2109`; poisons the wire
  cache via taint, `mypy/types.py:5415-5421`), **R**.
- `FunctionLike` (`mypy/types.py:2024`): `fallback` slot (`:2027`),
  forces `_can_be_false = False` (`:2033`).

**ProperType leaf variants**

- `AnyType` (`:1388`): `type_of_any` O (`:1391`); `source_any` R (holds a
  `Type`, flattened at `:1405-1407`); `missing_import_name` O (`:1394`).
  Wire: all carried (`wire.rs:538-542`).
- `UninhabitedType` (`:1496`): `ambiguous` O (`:1510`); truthiness R.
  Wire: `ambiguous` carried (`wire.rs:543-545`).
- `NoneType` (`:1556`): no fields; O (unit). `wire.rs:546`.
- `ErasedType` (`:1602`): no fields; O (unit). `wire.rs:547`. Note the
  Python decode gate applies to `read_type`, not to the Rust enum, a
  shadow may hold `ErasedType` freely; re-materialization follows
  Decision 6.
- `DeletedType` (`:1632`): `source` O (`:1638`). `wire.rs:548-550`.
- `LiteralType` (`:3586`): `value` **O** (a `LiteralValue` scalar,
  `:3608`; written via `write_literal`, `:3694`); `fallback` **R**
  (the wire carries the fallback blob, `wire.rs:587-590`, but the
  nested `Instance` itself is not trusted as an embedded shadow:
  consumers resolve nested Instances through
  `map_instance_to_supertype` against the snapshot, so the read path
  stays live-checked rather than re-walking a nested proxy);
  `_hash` memo is never proxied (`:3642-3644` recomputes it).

**Compound variants**

- `Instance` (`:1737`, slots `:1747`):
  - `type`: **S**, `type_ref` fullname key (`:1760`,`:1762`), resolved
    through `TypeInfoSnapshot`; the `NOT_READY` placeholder hazard
    (`Instance.read`, `mypy/types.py:1877-1878`; `InstanceCache`
    `:2005-2011`; fixup healing `mypy/wirefixup.py:166-204`,
    `_TypeRefFixer` `:291-300`) never arises because proxies hold the
    live handle and never reconstruct the object.
  - `args`: **O** structurally, **R at the container level**, the list
    is re-read (length + child identities) before every traversal,
    because typeanal mutates it in place during semanal
    (`mypy/typeanal.py:1588,3203,3288`) and the proxy must not outlive
    that phase in a stale form. Children are proxied by identity.
  - `last_known_value`: **R** (`:1823`; Python mutates via
    `copy_modified`, `mypy/types.py:1962-1992`, which produces a fresh
    object: the mutation creates a *new* identity the map simply does
    not contain yet, so there is no staleness to defend, but the read
    path stays live-checked).
    Wire: carried (`wire.rs:490`).
  - `extra_attrs`: **R** (`:1830`; mutated in place at
    `mypy/checkexpr.py:1488`, `mypy/typeops.py:1278`; an arbitrary
    name->Type dict, `ExtraAttrs` `mypy/types.py:1673-1690`).
    Wire: carried as `ExtraAttrs` (`wire.rs:451-455`).
  - `invalid`: vestigial, declared (`:1747`), never read; ignored.
  - `_hash` memo: ignored.
  - Wire: `type_ref` + fast singleton tags (`mypy/types.py:1885-1911`,
    `wire.rs:487-494`).
- `TypeVarType` (`:737`): `values`/`upper_bound`/`default` **R**
  (mutated by the semanal stamper `mypy/typeanal.py:4058-4060`; but
  `variance` is **R on checking-phase grounds**: variance inference
  mutates it (`mypy/subtypes.py:3102,3120,3177`) and #1098 made the
  build pre-infer variance before snapshots, a live re-read keeps the
  proxy honest if inference lands mid-scope). `id.raw_id`/`namespace`
  O; `id.meta_level` R. Wire: all nine fields carried
  (`wire.rs:498-508`).
- `ParamSpecType` (`:878`): `flavor` O; `prefix` (a `Parameters`) R
  (rewritten by the stamper, `mypy/typeanal.py:4064-4068`). Wire:
  carried, `meta_level` deliberately absent (`:1002-1012`,
  `wire.rs:509-518`).
- `Parameters` (`:2081`): `arg_types`/`variables` R (lists the semanal
  pass rewrites); `arg_kinds`/`arg_names`/`is_ellipsis_args`/
  `imprecise_arg_kinds` O (`:2114-2122`). Wire: carried,
  `is_ellipsis_args` written last before END_TAG (`:2282-2284`,
  `wire.rs:461-468,601`).
- `TypeVarTupleType` (`:1031`): `tuple_fallback` R (stamper,
  `:4072`); `min_len` O (`:1056`). Wire: carried (`wire.rs:519-528`).
- `UnboundType` (`:1148`): `name`/`args`/`original_str_expr`/
  `original_str_fallback` O (`:1175-1199`); `optional` and
  `empty_tuple_index` **R**, the wire drops both (`:1251-1257`,
  `wire.rs:529-534`), so a shadow that trusted the wire set would lose
  facts the live object carries; re-read from the handle.
- `UnpackType` (`:1337`): `type` O (`:1355`); `from_star_syntax` **R**
  (wire-dropped, `:1364-1367`, `wire.rs:535-537`).
- `CallableType` (`:2327`, slots `:2330-2355`):
  - `arg_types`/`arg_kinds`/`arg_names`/`ret_type`/`variables`/
    `type_guard`/`type_is`/`instance_type`: **R**, checker-phase
    in-place writers exist for every one of these
    (`mypy/checkexpr.py:2608-2610`, `mypy/typeanal.py:4090-4096`,
    `mypy/applytype.py:643,690`).
  - `definition`: **R, never O/S**, a live `mypy.nodes.FuncBase`
    identity (`:2399`), mutated during checking
    (`mypy/checker.py:7662`, `mypy/checkmember.py:864,914`), dropped by
    both serializations (`:2852-2876`, `:2907-2931`) and absent from
    `wire.rs:551-571`; the #1169 `_resync_definitions` repair
    (`mypy/expandtype.py`) exists precisely because this field cannot
    ride any wire. A proxy holds the live handle's `getattr` result at
    access time.
  - `special_sig`: **R** (wire-dropped; mutated `mypy/checkexpr.py:1033`,
    `:3161`).
  - `fallback`: **R** (mutated `mypy/checkexpr.py:5015`).
  - scalar flags (`is_ellipsis_args`, `implicit`, `from_type_type`,
    `from_concatenate`, `imprecise_arg_kinds`, `unpack_kwargs`,
    `is_bound`, `name`): **O** (`:2405-2414`,`:2395`).
  - Wire: matches the write side exactly minus `definition`/`special_sig`
    (`wire.rs:551-571`).
- `Overloaded` (`:2977`): `items` **R**, element mutation during
  checking (`mypy/checker.py:1693`); `fallback` derived from
  `items[0]` (`:2993`), R with it. Wire: items only
  (`wire.rs:572-574`).
- `TupleType` (`:3061`): `items` O structurally, R at container level
  (same rule as `Instance.args`; semanal copies/rebuilds via
  `copy_modified`, `:3174`); `partial_fallback` R (`:3089`); `implicit`
  O (`:3091`). Wire: carried (`wire.rs:575-579`).
- `TypedDictType` (`:3272`): `items` R (dict of Types, semanal-owned);
  `required_keys`/`readonly_keys`/`is_closed` O (`:3319-3324`);
  `fallback` R; `extra_items_from`/`to_be_mutated` **R, non-proxiable**:
  semanal/plugin-only facts both serializations drop
  (`:3363-3371`, `:3384-3391`, `wire.rs:580-586`); a proxy defers on any
  seam decision that would need them.
- `UnionType` (`:3707`): `items` O structurally, R at container level
  (construction flattens, `:3730`); `uses_pep604_syntax` O (`:3734`);
  `is_evaluated`/`original_str_*` **R** (wire-dropped, `:3813-3824`);
  `can_be_true`/`can_be_false` R. Wire: items + pep604 + truthiness
  (`wire.rs:591-596`).
- `TypeType` (`:3895`): `item` O (`:3948`, rebuilt via
  `make_normalized`, `:3951`); `is_type_form` O (`:3949`). Wire:
  carried (`wire.rs:597-600`).
- `TypeAliasType` (`:416`): `args` R (stamper mutates in place,
  `mypy/typeanal.py:4121`); `alias` **S**, live `nodes.TypeAlias`
  identity (`:441`), never serialized; `type_ref` fullname is the
  resolver key (`:527`, `:548`), resolved through
  `TypeResolver.alias_resolver()` / the installed alias map
  (`mypy/build.py:1586-1590`); `alias._is_recursive` is mutated through
  the alias node (`mypy/types.py:493`) and stays with the S-side live
  object.

**Runtime-only wrappers (ADR-0003 set 2)**: `TypeGuardedType`
(`mypy/types.py:559`), `RequiredType` (`:577`), `ReadOnlyType` (`:595`),
`PartialType` (`:3837`), `PlaceholderType` (`:4006`): no proxy variant.
A seam that can see one defers, exactly as the visitor kernels do today.

**Summary counts**: O fields are the wire-carried plain-data fields;
S fields are exactly the three identity carriers (`Instance.type`,
`TypeAliasType.alias`, plus fullname keys `type_ref`); R is everything
with an in-place mutator, every wire-dropped fact, and every
arbitrary-object holder. The O/R/S split is mechanical from the two
tables above, which is the point: a future variant needs only its slots
read once to be classified.

## Decision 3: mutation seams and the coherence protocol

**Decision: coherence is enforced by phase-scoping plus drop-on-write,
with the enumerable checker-phase mutator set named in a registry, not by
a write-barrier machinery.**

Three mechanisms, in priority order:

1. **Phase scoping** (Decision 1): proxies exist only during checking of
   one target. Semanal/typeanal mutations (the bulk of the inventory's
   "in semanal" sites: `typeanal.py:1588,3203,3288,4054-4074,4121`) can
   never race a live proxy. The precedent for trusting this boundary is
   `_type_wire_cache_enabled` (`mypy/types.py:129-137`).
2. **Drop-on-write**: when a proxied seam's decision mutates a proxied
   node (write-back, Decision 4), the shadow entry for that node and all
   nodes below it are dropped from the store; the next access re-walks
   from the live handle. Dropping is always sound; it costs only the
   re-walk.
3. **Touch points for the residual checker-phase mutators**: the
   enumerable in-place mutators that are *not* the gated seams
   themselves: `meta_level` (`applytype.py:211`, `typeops.py:2109`),
   `variance` (`subtypes.py:3102`), `extra_attrs`
   (`checkexpr.py:1488`, `typeops.py:1278`), property-setter item
   rewrites (`checker.py:1692-1693`), `definition` repairs
   (`checkmember.py:864,914`). Each calls a module-level
   `mypy/nativeproxy.touch(*nodes)` (a dict-mark no-op when no proxy is
   installed for the scope), which drops the affected entries. This is a
   handful of one-line calls at sites already touching the same objects;
   every one of them is inside a module whose other gates
   (`_native_checker_active` etc.) are already wired from
   `mypy/build.py:1086-1114`, so the wiring pattern exists.

**Invalidation beyond touches** reuses the existing boundary machinery
unchanged: `_clear_native_resolvers` (`mypy/build.py:1655-1724`) drops
the proxy store at every daemon recheck boundary
(`mypy/server/update.py:706-708`, `:1117-1119`) and per-SCC semanal
handoff (`mypy/build.py:5701-5739`); build-start clears ride
`BuildManager` setup. No new invalidation machinery is introduced; the
proxy store is one more map in the same clear list.

## Decision 4: write-back (reflect-into-Python-on-write)

**Decision: Rust never mutates a proxied node's Python object directly
from inside a decision. A decision that writes returns ownership to
Python in the same seam call; the Python shim applies the mutation to the
live object, then drops the node from the proxy store (drop-on-write).**

This mirrors the classifier pattern that already dominates the crate
(ADR-0001 Decision 2): Rust decides, Python mutates. The only new element
is that the "decide" step now reads the shadow instead of the wire, so
the per-call encode/fixup disappears. Structure-producing seams (a Rust
decision that *constructs* a new Type, e.g. an expansion or join result)
encode the result with `write_type` (`wire.rs:2023`) and the Python shim
decodes through `read_type` + `fixup_wire_type`
(`mypy/wirefixup.py:113-139`), the existing materialization path, now
paid once per decision instead of once per call. The live graph keeps
authority; the shadow refreshes lazily on next access.

## Decision 5: plugin-visible identity preservation

**Decision: plugin hooks always receive and return live Python objects;
the proxy never crosses a hook boundary, and a hook return is a drop
boundary for the nodes the hook touched.**

Evidence that hooks are live-object boundaries today:

- `ClassDefContext` carries the live `cls: ClassDef` and is constructed
  per decorator/metaclass/base at `mypy/plugin.py:511-514`, invoked from
  `apply_class_plugin_hooks` (`mypy/semanal.py:2962-3010`) and the MRO
  hook (`mypy/semanal.py:3731-3744`); `get_class_decorator_hook` documents
  in-place `TypeInfo` mutation (`mypy/plugin.py:736`), the MRO hook
  in-place MRO rewriting (`mypy/plugin.py:789`). `add_method_to_class` /
  `add_attribute_to_class` append into `cls.info.names` and
  `cls.info.defn.defs.body` (`mypy/plugins/common.py:260-288,426`).
  These are semanal-phase: **no proxy is live**, per Decision 3.1.
- Checker-phase hooks receive live `Type`s: `FunctionContext.arg_types` /
  `default_return_type` (`mypy/plugin.py:439-470`) are built from live
  lists at `mypy/checkexpr.py:2344-2377`; `AttributeContext` at
  `mypy/checkmember.py:1308-1313` (also `:1719-1738`, `:1847-1866`,
  `:2377`). The protocol: when a gated seam is about to hand types to a
  hook context, it dereferences its shadow to the live objects it already
  holds (zero-copy, the handles exist); after the hook returns, any node
  whose type appears in the context is dropped from the store, so a
  plugin mutation (e.g. an adjusted `default_return_type`) can never be
  shadowed by stale structure.
- The Rust-side builtin-hook fast path (`PluginHookRegistry`,
  `crates/type_kernel/src/plugin_hooks.rs:37-146`,
  `mypy/build.py:1795`) already encodes "user plugin present => defer to
  Python"; the proxy layer sits behind the same
  `plugin_hook_known_absent` gates (`mypy/semanal.py:2972,2992,3007`),
  so a plugin-visible build sees the proxy path no more than the wire
  path does today.

The invariant to test (Decision 7): for a corpus with a user plugin
(dataclasses, attrs), gate-on decisions equal gate-off decisions, the
same assertion class the existing `Native*Suite` differentials run.

## Decision 6: Stage A (wire totality) as prerequisite

**Decision: Stage B consumes the Stage A contract in three places; none
of them changes the codec.**

1. **Field-set totality**: the shadow's structural (O) fields are exactly
   the ADR-0003 inventory set 1, 20 variants. Stage A's byte-level
   round-trip suite (ADR-0003 Decision 3, direction B) is the proof that
   every field the shadow trusts is reconstructable; Stage B inherits it
   rather than re-proving.
2. **ErasedType re-materialization**: a proxy may hold
   `Type::ErasedType` freely (`wire.rs:547`); when a decision
   re-materializes result bytes that may carry it, the Python shim
   decodes under the ADR-0003 Decision 1 context manager
   (`_ALLOW_WIRE_ERASED_TYPE`, `mypy/types.py:5318-5321`, opt-in shape
   per `mypy/erasetype.py:185-203`), owned-result decodes only, the
   refuse-and-defer default untouched everywhere else, pinned by the
   ungated Direction C test.
3. **Set-2 wrappers stay codec-excluded** (`NotImplementedError` by
   construction, `mypy/types.py:408-413`); Stage B's deferral for them
   mirrors the visitor kernels, so no new wire requirement is ever
   created by the proxy.

No `CACHE_VERSION` change: proxies are session state; nothing proxy-shaped
reaches the incremental cache (`mypy/cache.py` stores JSON-serialized
types; the Type wire stream exists only at seam boundaries, ADR-0003
Decision 1, item 3).

## Decision 7: memory and ownership model

- **Refcounts**: each proxied node pins its live object with one strong
  `PyObject` ref (the store's map is the Python-side
  `dict[int, Type]`, plus the Rust-side `PyObject` clones inside
  `ProxyNode`). This is the crate's first scope-lifetime per-node
  Python retention, precedent is `live_info_map` holding one dict for
  a build (`typeinfo.rs:211`) and `plugin_helpers.rs` per-call
  extraction; the per-node version is justified by the target-scope
  clearing of Decision 3.
- **Cycles**: pinning a node keeps its whole transitive Type graph, and
  any `TypeInfo`/`Type` cycle it participates in (recursive aliases,
  `self_type`, `type_object_type`), alive until the scope exits. The
  bound is the scope; the daemon profile this must survive is one
  target's checked types, strictly smaller than what
  `_type_wire_cache` already pins per session today
  (`mypy/types.py:5407-5433`). Rust-side recursion uses owned `Box`/`Vec`
  (as `wire.rs` does); no Rust->Python cycle can form because S fields
  are fullname keys, never handles.
- **Weakrefs**: **rejected for Stage B.** There is no weakref identity
  map anywhere in `mypy/` today (only the debug blacklist in
  `mypy/server/objgraph.py:6,32`), PyO3 offers no cheap weakref handle
  for arbitrary non-`pyclass` objects, and a weakref-only map admits the
  resurrection hazard: a proxy outliving its object yields stale
  decisions instead of a crash. Strong refs + scope clearing give the
  safety for free; if daemon memory profiling (out of scope here) shows
  the pins dominating, a Python-level `weakref.ref` value in the identity
  dict is the named evolution path, chosen at that point with evidence.
- **Coherence audit**: a test-only pyfunction exposing store size
  (mirroring the `NativeTypeWireSuite` engagement style) lets the parity
  suite assert the store is empty after every scope, the memory
  contract is testable, not aspirational.

## Decision 8: parity test surface

The ADR-0001 Decision 4 four-movement standard, concretized:

- `NativeTypeProxySuite` in `mypy/test/testtypes.py`
  (`@skipUnless(_NATIVE_WIRE_ENABLED, ...)`, styled after
  `NativeTypeWireSuite`, `mypy/test/testtypes.py:1651-1652`):
  - Gate-off/on differential through the public functions for each
    proxied seam, on fixtures covering every O/R/S class (an
    `extra_attrs` carrier, a `last_known_value` instance, a
    recursive-alias graph, a callable with `definition`).
  - **Coherence differential**: apply one of the Decision 3 touch-point
    mutations (e.g. `extra_attrs` rewrite, `meta_level` bump) between
    two proxied seam calls; assert the second decision equals the
    gate-off result. This is the test that would have caught a stale
    shadow.
  - `test_store_empty_after_scope` + engagement direct calls.
- Rust `#[cfg(test)]` unit tests on the shadow builder/classifier with
  no PyO3 (the `typeops.rs:2561` pattern).
- Parity runs via the AGENTS.md type-kernel recipe (`testtypes.py` +
  `testcheck.py` under `TEST_NATIVE_TYPE_KERNEL=1`), plus the standard
  fine-grained / cache / incremental checks and a 0-error self-check.

## Decision 9: out of scope

- No Python-fallback removal; no `Options.native_type_kernel` flip; no
  per-seam default change. The proxy path is an additional branch inside
  the existing gates (ADR-0002 Decision 4 is terminal here).
- No `is_subtype` port (Stage 3c); no kernel decision-coverage work for
  ErasedType inside join/meet/setops (they keep deferring exactly as
  today; #1185 owns the comment re-audit).
- No changes to `mypy/astwire.py` / the `ast_serialize` crate (ADR-0003
  set 3 stays out of the Type wire).
- No wire forms for set-2 wrappers; no `TypeAliasType.alias` inlining
  (ADR-0002 Decision 1 stands).
- No plugin API change, no persisted-state change, no daemon protocol
  change.

## Consequences

- The hot-seam Python cost that ADR-0002 Decision 3 identifies becomes
  per-scope instead of per-call for O/S structure, while R fields cost
  one `getattr`, strictly below the encode+decode+fixup baseline the
  scalar-fact seams already beat. Success is measured by
  `scripts/measure_work_share.py` wall-clock, per ADR-0002 Decision 3;
  a proxy that does not move wall-clock is dropped, not defended.
- The O/R/S classification is a maintained artifact: new `Type` fields
  or new in-place mutators must update the inventory in this ADR's
  successor or the field's class defaults to R (the safe choice).
- Stage C (fallback removal, not designed here) inherits a coherent
  story: phase-scoped shadows, live-identity authority, drop-on-write,
  and the Stage A totality contract, with the strangler deferral
  contract unchanged until then.
- Open risks carried forward: (a) the Decision 3 touch-point list is an
  audit artifact, not a proof; the coherence differential is the
  enforcement. (b) Scope-granularity is a tuning knob, per-target first,
  per-call-chain only if the measurements demand finer drops. (c) The
  `PyObject` per node doubles handle traffic relative to the wire path
  for small graphs; the proxy must engage only above a measured size
  threshold, decided at implementation time, not by this ADR.
