# Changelog

All notable changes to AgentProv are tracked here.

This project follows Conventional Commits for commit messages and uses tag
releases named `vMAJOR.MINOR.PATCH`.

## Unreleased

- Added embedded JSON Schema validation for manifests, run envelopes, events,
  and policies.
- Hardened run verification with event schema checks, mixed `run_id` detection,
  signature requirements, and optional manifest binding.
- Added manifest signature verification and clearer local trust semantics.
- Expanded static policy handling with rule expiry and approval request events.
- Added approval request, grant, and deny examples.
- Added privacy-oriented Codex and Claude Code importer redaction rules and
  tests.
- Added OpenTelemetry-style and OpenInference-style export shape tests and
  consumer notes.
- Added typed Rust inputs for event creation and run-log appending.
- Added deterministic integration examples for OpenAI-style, Anthropic-style,
  LiteLLM-style, GitHub tool, Discord tool, and scheduled runs.
- Added a local SQLite collector with CLI ingest, query, verification, HTTP
  endpoints, and static dashboard export.
- Updated roadmap, next-steps, schema, spec, collector, and README docs to match
  the implemented MVP.

## 0.1.0

- Initial Rust CLI MVP for signed, tamper-evident provenance records for AI
  agent runs.
