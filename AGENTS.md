# Agent Instructions

This repository is a working branch for migrating mypy toward Rust while keeping
the existing Python behavior stable.

## Commit Style

Use Conventional Commits for all commits:

- `feat: ...` for new user-visible functionality
- `fix: ...` for bug fixes
- `docs: ...` for documentation-only changes
- `test: ...` for tests
- `refactor: ...` for behavior-preserving internal changes
- `perf: ...` for performance changes
- `build: ...` for packaging, dependency, and build-system changes
- `ci: ...` for CI workflow changes
- `chore: ...` for maintenance that does not fit the above

Prefer a single clear subject line under 72 characters. Add a body when the
change has non-obvious motivation, migration notes, or test coverage details.

## Development Workflow

Use `uv` for local development. Do not add tox-based workflows.

Common commands:

```bash
uv sync
uv run test
uv run pytest -n0 -k test_name
uv run lint
uv run format
uv run typecheck
uv run docs
```

`uv run test` delegates to `runtests.py` so the existing grouped test behavior
is preserved. Use `uv run pytest ...` when you need direct pytest arguments.

## Rust Migration Direction

The migration plan is recorded in `docs/rust-migration-strangler.md`.

Follow a strangler-fig approach:

- Keep Python-facing behavior stable while adding Rust behind narrow interfaces.
- Prefer Rust adapters that exchange plain records, bytes, or stable IDs with
  Python.
- Do not start by porting `mypy.nodes` or `mypy.types`; they are widely shared
  mutable object graphs and plugin-visible.
- Treat the native parser path as the first production migration seam.
- Next candidates are module discovery/import graph prepass, cache indexing and
  validation, and only later selected pure type-operation kernels.
- Preserve daemon, cache, plugin, and incremental-mode semantics unless a change
  is explicitly called out and tested.

## Design Principles

Use the following design principles when changing the codebase:

- Prefer deep modules: small, stable interfaces hiding meaningful complexity.
- Avoid shallow pass-through modules that merely split code without reducing the
  caller's burden.
- Optimize for locality: keep related decisions, invariants, and error handling
  close to the code that owns them.
- Design interfaces around what callers need to know, including invariants,
  ordering constraints, error modes, and performance expectations.
- Make complexity explicit where it is essential, and hide accidental complexity
  behind well-named modules.
- Do not leak implementation details across seams. If callers must understand
  the implementation to use the module correctly, improve the interface.
- Prefer consistency and boring structure over cleverness.
- Add comments for non-obvious reasoning and invariants, not for restating what
  the code already says.
- When changing shared behavior, test through the public interface rather than
  testing internal incidental structure.

## Verification Expectations

For workflow or infrastructure changes, run the smallest relevant uv commands
first, then the broader suite when practical:

```bash
uv lock --check
uv run typecheck
uv run docs
uv run lint
uv run test
```

For Rust migration work, add targeted parity tests and include native-parser,
daemon, cache, and incremental-mode checks when affected.
