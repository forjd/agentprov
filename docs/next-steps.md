# Next Steps

AgentProv now has a working Rust CLI MVP for signed, tamper-evident provenance
records. The core run-log primitive exists: records can be generated, appended,
hashed, signed with local development keys, verified as an event chain, checked
against static policy, imported from Codex or Claude Code JSONL streams, and
exported to experimental OpenTelemetry-style and OpenInference-style JSON.

Repository: https://github.com/forjd/agentprov

## Current state

Implemented:

- Rust CLI crate
- MIT license
- CI and release workflows
- changelog, release notes template, and release process notes
- research summary and detailed findings
- MVP scope, threat model, and OpenTelemetry/OpenInference mapping notes
- versioned spec notes for manifest, run envelope, event, policy, local key, and
  AgentProv OTel attributes
- JSON Schemas for manifest, run envelope, event, and policy examples
- example manifest, run envelope, event, and policy records
- append-only JSONL run logs
- canonical BLAKE3 event hashing
- whole-run chain verification
- local Ed25519 development keys
- manifest and event signing
- manifest signature verification
- optional run-log verification against a supplied manifest digest
- signature verification for events and signed run logs
- static policy checks with allow, deny, and require-approval rules
- policy rule expiry support
- approval request event emission for `require_approval` decisions
- approval grant and deny event append commands with optional signatures
- deterministic manual tool-run demo
- Codex and Claude Code JSONL importers
- importer redaction rules for command text, assistant text, and tool results
- experimental OTel-shaped and OpenInference-shaped JSON exports
- export shape tests for OTel-style and OpenInference-style JSON
- embedded schema validation command for manifest, run envelope, event, and
  policy records
- run verification rejects wrong event schema versions and mixed `run_id` values
- typed Rust inputs for event construction and run-log appending
- deterministic OpenAI-style, Anthropic-style, LiteLLM-style, GitHub tool,
  Discord tool, and scheduled-run Rust examples
- local SQLite collector with CLI ingest, query, and verification commands
- local HTTP collector endpoints for JSONL ingest, run listing, event lookup, and
  verification
- streaming HTTP collector endpoint for verified single-event appends
- bounded collector event listing by event sequence
- structured JSON errors for collector HTTP requests
- static read-only collector dashboard export
- trust semantics note for local signatures, manifest binding, run envelopes, and
  future trust roots
- observability consumer notes for OTel-shaped and OpenInference-shaped exports

Current CLI commands:

```text
agentprov manifest example
agentprov manifest hash <file>
agentprov manifest sign <file> --key <key> --out <file>
agentprov manifest verify-signature <file>
agentprov run example
agentprov run init --agent <manifest> --trigger <type> --out <file>
agentprov run verify <jsonl> [--require-signatures] [--manifest <manifest>]
agentprov event hash <file>
agentprov event verify <file>
agentprov event append --run <jsonl> --type <event-type>
agentprov event sign <file> --key <key> --out <file>
agentprov event verify-signature <file>
agentprov key generate --out <file>
agentprov key public --key <file>
agentprov key inspect --key <file>
agentprov policy check --policy <file> --agent <id> --action <action> --resource <resource>
agentprov approval grant --run <jsonl> --approval-id <id> --approver <id> --agent <id> --action <action> --resource <resource> [--reason <text>] [--key <key>]
agentprov approval deny --run <jsonl> --approval-id <id> --approver <id> --agent <id> --action <action> --resource <resource> [--reason <text>] [--key <key>]
agentprov demo manual-tool-run --out <dir>
agentprov export otel <jsonl> --out <file>
agentprov export openinference <jsonl> --out <file>
agentprov import codex <jsonl-or-> --out <jsonl> [--key <key>]
agentprov import claude <jsonl-or-> --out <jsonl> [--key <key>]
agentprov validate <manifest|run-envelope|event|policy> <file>
agentprov collector ingest <jsonl> --db <db>
agentprov collector runs --db <db>
agentprov collector events <run_id> --db <db> [--after-sequence <n>] [--limit <n>]
agentprov collector verify <run_id> --db <db> [--require-signatures]
agentprov collector ui --db <db> --out <html>
agentprov collector serve --addr <addr> --db <db>
```

## Recommended direction

Do not try to become another LLM observability product.

The positioning should stay focused:

> Signed provenance records for AI agent runs.

Or:

> OpenTelemetry-compatible identity and provenance for AI agents.

The useful comparison line is:

> Langfuse, Phoenix and AgentOps show what happened. AgentProv proves who ran it,
> with what authority, and whether the record still verifies.

## Milestone 1: Tighten the current MVP

The current primitive works, so the next work should make it harder to misuse.

Completed:

- keep `docs/roadmap.md`, this file, and the README aligned with implemented
  behaviour
- add runtime schema validation for manifests, run envelopes, events, and
  policies
- strengthen `run verify` so it rejects inconsistent `run_id` values and wrong
  schema versions
- add negative tests for malformed run logs and signature requirements
- add a manifest signature verification command
- support explicit `run verify --manifest <file>` binding checks

Local MVP gap status: complete. Production profiles should make
manifest binding and trusted key sources mandatory.

Acceptance criteria:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`

## Milestone 2: Clarify trust semantics

Local signatures currently prove that a record matches the embedded public key.
They do not establish that the key is trusted by an organisation.

Completed:

- bind run logs to manifest digests and key IDs in a documented production trust
  model
- document exactly what local signatures prove and what they do not prove
- decide whether run logs remain event-only or whether a signed run envelope is
  stored alongside the JSONL event chain
- add design notes for future key registries, KMS/HSM, workload identity, or
  transparency logs

## Milestone 3: Improve policy and approval records

The static policy MVP is useful, but approval flows are still mostly schema
shape rather than workflow.

Completed:

- expand tests for deny priority, require-approval priority, unmatched agents,
  and wildcard edge cases
- decide whether `require_approval` should emit an approval-needed event
- implement `expires_at` support
- add examples for `human.approval.request`, `human.approval.grant`, and
  `human.approval.deny`
- add first-class CLI append helpers for signed approval grant and deny events

## Milestone 4: Harden imports and exports

Codex and Claude imports intentionally avoid copying full prompts, assistant
text, command output, and tool-result content. That privacy boundary should be
kept explicit and tested.

Completed:

- document importer redaction rules field by field
- review `action`, `resource`, and metadata fields for sensitive leakage
- add golden tests for OTel and OpenInference exports
- add fixture-backed golden tests for Codex and Claude imports

Future production work:

- add an end-to-end tested consumer import for one target backend, such as
  Phoenix, Jaeger, or Tempo

## Milestone 5: Extract a stable Rust API

The CLI still owns much of the product surface, but the Rust crate now exposes
typed inputs for event construction and run-log appending.

Completed:

- expose stable functions for building events, appending logs, verifying logs,
  signing, verifying signatures, and policy checks

Future API work:

- keep the CLI as a thin wrapper over the library
- add library-level examples
- define the minimum API needed by Python and TypeScript SDKs

## Milestone 6: Add first real integrations

Start with examples before committing to full SDK maintenance.

Completed:

- OpenAI model-call wrapper example
- Anthropic model-call wrapper example
- LiteLLM wrapper example
- GitHub tool event example
- Discord tool event example
- scheduled or cron-triggered run example
- every example should produce a verifiable AgentProv run log

## Milestone 7: Collector

The first local collector is implemented as a SQLite-backed MVP.

Completed:

- local HTTP ingest server
- SQLite persistence
- query API for runs and events
- verification endpoint
- import/export path between JSONL files and stored runs
- streaming append endpoint for one verified event at a time
- bounded event listing by stable event sequence
- structured JSON error responses for malformed HTTP requests

Future production work:

- authentication and transport security design
- richer pagination metadata and larger-run query ergonomics
- Postgres persistence option if needed

## Milestone 8: Read-only UI

The first UI is a static read-only HTML dashboard exported from the collector
database.

Completed:

- run list
- permission timeline
- event timeline
- verification status

Future UI work:

- live collector-backed web UI
- richer run detail pages
- actor chain visualization
- trace/event tree layout
- filtering and pagination

Keep UI write paths out of scope until the trust and storage semantics are
settled.
