# G4 graduation criteria (proposed, 2026-09-16)

Phase G is the active ownership track and its first two rungs have landed
(G3.1 read flip `ac68f0eaf`; G3.2 write-time seed `33c7a5b79`), yet `G4` in
`docs/remaining-migration-plan.md:746` is a single line — *"graduation: 'the AST
executes on Rust storage'"* — with no falsifiable criteria. Phase F's `F4` had
concrete ones (`docs/remaining-migration-plan.md:704-707`: cache writes
serialize directly from Rust; the daemon preserves identity through handles; the
plugin bridge serves view objects unchanged), plus a risk register whose items
each needed an answer *before the step merged*.

Without the equivalent, two things go wrong: lanes cannot tell when a family is
"done" (G1.0b/G2.0 are record-only *by design* and have been since wave 71), and
the claim ladder's policy — claims move only when a phase graduates — has nothing
to test. This brief proposes the criteria so the node read channel now in flight
(N1) and the Phase H design (H1) can be judged against a definition of done.

## Proposed criteria

Each is falsifiable, and each names the receipt that would settle it. The
receipts deliberately follow the G-track's own evidence rules: counters, never
return values, because a deferred path still returns a correct-looking result.

1. **Write ownership for at least one family.** Writes land in Rust storage and
   the Python object is served from it — the *write* flip, not dual-write.
   Receipt: for that family the dual-write leg is removed and its defer counters
   are zero for the covered shapes. Until this holds, the store is a shadow.
2. **The cache path serializes from the store** for the flipped family, the
   analogue of F4's first clause. Receipt: the AST cache write/read path's Python
   involvement eliminated for that family, with the `ast_serialize` differential
   green in both gate states (and `CACHE_VERSION` treated as a T3 change if it
   moves).
3. **Identity survives incremental updates.** astmerge, aststrip and
   fine-grained preserve identity through handles for the flipped family.
   Receipt: the fine-grained + daemon batteries green in both gate states, plus
   the `@`-key class of pin — G3.2's aststrip hazard was an *ordering* divergence
   (`D@5` keeping its dict position while the store re-minted an ordinal), and it
   is exactly the kind of thing a value-only differential misses.
4. **The Python-facing contract is unchanged**, with F4's risk-register gates
   reused rather than reinvented: the DefaultPlugin suite plus the plugin-driven
   `testcheck` subsets, `isinstance`/`__slots__`/repr fidelity, and the cold-path
   overhead gate (self-check wall clock within 10%). This is the criterion Phase
   F was declined on — contract, not measurement — so G must answer it in
   writing, not by assertion.
5. **Graduation is per family, and the rung needs the families it claims.**
   A partially-flipped G is not the G4 rung; state which families the claim
   covers.

## Where the plan's text no longer matches the field

- **The family order is inverted by history.** The plan says expression nodes
  first, statement nodes second, symbol tables *last* "because semanal mutates
  them mid-pass". What actually happened: symbol tables served first (G3.1,
  G3.2) and expression nodes are being served now. The doc should state what
  governs the order now — likely "cheapest complete family first", since G3.2
  measured `defer_no_handle` = 124 as the *entire* remaining symtable defer
  volume, which makes symbol tables one optional step from full serve coverage.
- **G0 completeness is unverified here.** The plan requires the AST writer to
  cover the fields Python-only paths mutate; whether that is complete for
  statement nodes is not stated anywhere I could find, and criterion 2 depends on
  it.

## Open forks for the owner (not decided here)

1. **Does G4 require the write flip, or does read-serving suffice?** F4's text
   required write ownership; G4's slogan does not say. This is the highest-stakes
   ambiguity, because read-only serving is much cheaper and partially true
   already; choosing "read-serving suffices" would let G4 arrive with the store
   still a mirror.
2. **Is expression-first still the order?** See above; history says the opposite.
3. **Which family does the claim cover first**, if the rung may be claimed
   per family rather than for all three at once.

## What G4 explicitly does not require

- Phase H's driver move (H is the *next* rung: "the type-checking pipeline
  executes in Rust").
- Anything from Phase F, which was closed unclaimed; F is not on G's critical
  path.
- A standalone binary (Phase J).

Filed as an issue and left for the owner to ratify or amend; this document is a
proposal, and the criteria above are derived from F4's precedent plus the G-track
evidence contracts that are already in force.