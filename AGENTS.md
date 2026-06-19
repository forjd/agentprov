# Agent Instructions

This repository is a Rust CLI project for signed provenance records for AI agent runs.

## Worktree

- Keep changes focused. Do not reformat unrelated files.
- Do not commit generated demo output, local keys, or temporary run logs.
- Prefer ASCII in new files unless an existing file already uses another style.

## Key paths

- `src/` contains the CLI and core provenance logic.
- `tests/` contains integration and schema tests.
- `examples/` contains sample manifest, run, event, and policy JSON.
- `schemas/` contains JSON Schemas for the example records.
- `docs/` contains specs, research notes, and roadmap material.

## Validation

Run the smallest useful check for the change. For general changes, use:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

For documentation-only changes, a full Rust test run is optional unless commands, examples, schemas, or repository metadata changed.

## Git

- Commit messages must always follow the Conventional Commits standard, such as `docs: add agent instructions`.
- Branch names should use the same Conventional Commits type prefix with a short kebab-case summary, such as `docs/add-agent-instructions`.

## GitHub

The canonical repository is `https://github.com/forjd/agentprov`.
