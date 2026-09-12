# Phase G0 next steps (scoping brief, wave 65B / issue #1540)

Date: 2026-09-11. Basis: read-only audit at `7a2b2f850` (pre-wave-65 main)
plus probes against the shared `ast_serialize` extension. Persisted by the
orchestrator from the wave-65B brief.

## Bottom line

1. **No AST node enum exists in production.** `ast_serialize` writes wire
   bytes directly from the ruff AST (`crates/ast_serialize/src/lib.rs:735,989`).
   `nodes_full.rs` / `nodes_codec.rs` / `full_ast_codec.rs` /
   `visitor_engine.rs` are dead scaffolding (2,040 lines, zero callers).
   G0's first job is the enum, not extending one.
2. **Three confirmed parser-parity bugs on main**, invisible because their
   suites are not in CI:
   - `FuncDef.docstring` / `ClassDef.docstring` never serialized
     (`lib.rs:2048-2087`, `lib.rs:1644-1695`) -> `stubgen
     --include-docstrings` drops them (`mypy/stubgen.py:892-894`);
     `teststubgen -k Docstring` fails today (`test-data/unit/stubgen.test:3634`).
   - `--custom-typing-module` ignored by the native branch: `parse()` never
     receives the option, `serialize_import` / `serialize_import_from`
     (`lib.rs:906-987`) write raw names; probe diverges from fastparse.
   - `options.pos_only_special_methods=False` ignored: `serialize_parameter`
     elides unconditionally (`lib.rs:2531`, `nativeparse.py:704-706`).
   - Latent: `options.transform_source` is only applied on the fastparse
     branch (`mypy/parse.py:52-65` vs `:71-72`); no reproduced divergence.
3. **CI blind spots**: `test_nativeparse.py` (250 cases), `testparse.py`,
   `teststubgen.py`, and `cargo test -p mypy-ast-serialize` run in no
   workflow.
4. Plan G's family order (expressions, statements, symbol tables) is
   correct; risk ranks in the opposite order.

## First slice (G0.1): AST wire v5

Thread three options through `ast_serialize.parse` and
`nativeparse.parse_to_binary_ast`:

- `include_docstrings: bool` -> `FUNC_DEF_STMT` / `CLASS_DEF` records gain
  an `Option<String>` docstring (value must equal
  `ast.get_docstring(node, clean=False)`); readers set
  `func_def.docstring` / `class_def.docstring`.
- `custom_typing_module: Option<String>` -> translate import names and set
  the implicit asname when translation occurred (fastparse.py:1419-1426,
  1439-1444).
- `pos_only_special_methods: bool` -> skip elision when false.
- `AST_WIRE_VERSION = 5` returned in the parse data dict and enforced at
  the entry (today `cache_version` is accepted and ignored, `lib.rs:433`,
  while `nativeparse.py:317` hardcodes 4; a stale `.so` fails late with the
  `AssertionError: 255` class).
- Route `options.transform_source` through the native branch
  (`mypy/parse.py`) to match stubgen semantics.

Acceptance: `test_nativeparse.py` + `testparse.py` green, `teststubgen -k
Docstring` flips to green, testcheck 8,198/15/7 exact, cold self-check 347.

## Family order and risk

1. **G1 expressions** (medium risk): already fully serialized; missing
   analysis fields are the `RefExpr` binding set, `analyzed` replacements
   (`CastExpr`/`RevealExpr`/...), `method_type(s)`, `as_type`, `def_var`.
2. **G2 statements** (high): `FuncDef`/`ClassDef`/`OverloadedFuncDef`
   metadata lists (`nodes.py:1326`, `:1865`, `:994`), `AssignmentStmt`
   type fields, `IfStmt.unreachable_else`, identity replacement in
   `astmerge.py:210-217`, `aststrip.py:112-220` reset sites.
3. **G3 symbol tables** (critical, last): deferral lifecycle creates and
   replaces `PlaceholderNode`s (`semanal.py:9025-9083`), queued
   `class_type` closures (`semanal.py:9572`), and astmerge rewrites
   identities wholesale (`astmerge.py:210-241,352-361`); `TypeInfo` mixes
   node fields with the F-phase type graph.

## Ordered PR sequence

1. **G0.1 AST wire v5** (docstrings + custom-typing-module + version pin +
   transform_source routing) - the three confirmed bugs above.
2. **G0.2 dead-scaffolding deletion + AST CI** - remove the 2,040 dead
   lines; add `cargo test/fmt/clippy -p mypy-ast-serialize` and a parity
   job step for `test_nativeparse.py` / `testparse.py` / `teststubgen.py`;
   fix the stale tag-range comments (`astwire.py:39-43`,
   `cache.py:35-37` - 150-152 are NOT free; AST-only tags must use 153-159
   and 230-253).
3. **G0.3 expression enum + writer** (byte parity over the 250-case
   corpus + golden blobs).
4. **G0.4 statement/pattern enum + writer** (plus `Block` location and
   elif-flattening parity; testmerge 41/1, testdiff 79).
5. **G0.5 symbol-node enum + writer + cache payload contract** (byte
   round-trip on a real cache data file; warm-run cache consumption).
6. **G1.0 expression dual-write shadow + accessors** (no read flip).

## Risk-register deltas

- Pin Phase G to the ADR-0004 shadow model (Python objects authoritative):
  plugins, `isinstance`, `__slots__`, and astmerge's
  `replace_object_state` all assume live Python nodes. The F/G plan text
  that describes view-routed attribute access contradicts it and should be
  reconciled.
- `NodeId` table, not weakrefs: `identity.rs handle_for` is mirror-only and
  daemon-stable handles are blocked on #1528's strong-pin work. Keep G0's
  writer fullname-keyed (`cross_ref`) so the cache path does not depend on
  #1528.
- Cache: today only symbol tables are cached, defs are re-parsed; a Rust
  AST cache payload is a new format (bump `CACHE_VERSION`, currently 13,
  and keep `FileRawData` byte-stable for worker handoff).
- CI blindness is now on the critical path (see G0.2).
