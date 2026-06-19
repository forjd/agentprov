# Implementation Roadmap

This roadmap tracks the product contract at a high level. For the active task
queue and acceptance criteria, see `docs/next-steps.md`.

## Phase 0: Repository and research

Status: done.

- Rust CLI crate
- docs/research findings
- MVP scope
- schema sketches
- OTel/OpenInference mapping
- threat model
- CI and release workflows
- changelog and release process notes

## Phase 1: Core data structures

Status: done for the MVP, with future typing improvements planned.

- `AgentManifest`
- `RunEnvelope`
- `Actor`
- `PermissionDecision`
- `ProvenanceEvent`
- canonical JSON hashing
- event verification

The implementation still uses `serde_json::Value` for several runtime paths.
That is acceptable for the MVP, but a more stable Rust API should add stronger
typed boundaries.

## Phase 2: Event chains

Status: done for local JSONL run logs.

- append events to a run log
- validate sequence numbers
- validate previous hash links
- verify whole run chains
- add example fixtures and integration tests
- validate event schema during run verification
- reject mixed `run_id` values in a single log

Future work:

- decide whether a formal run envelope is stored alongside or inside a run log

## Phase 3: Signing

Status: partial.

- generate local Ed25519 keypair
- sign manifest digests
- sign event hashes
- verify event signatures
- verify manifest signatures
- verify signed run logs
- verify run logs against supplied manifest digests
- document local key handling limitations
- document local signature trust semantics
- document manifest binding and run-envelope decisions

Future work:

- implement trusted key registries or external signing providers

## Phase 4: Policy MVP

Status: done for static policy checks.

- static policy file
- allow/deny/require-approval rules
- optional rule expiry
- CLI policy check command
- permission decision event generation
- approval request event generation
- approval grant/deny append commands with optional local signatures
- approval request/grant/deny examples

Future work:

- external approval workflow integrations

## Phase 5: SDK shape

Status: partial.

- typed Rust event and run-log append inputs
- stable Rust functions for event hashing, run-log verification, signing,
  signature verification, and policy checks

Future work:

- thin CLI wrappers over the library surface
- library-level examples
- Python SDK prototype
- TypeScript SDK prototype
- decorators/context managers in dynamic SDKs

## Phase 6: OpenTelemetry export

Status: partial.

- map provenance events to OTel-shaped JSON spans
- map AgentProv fields to OTel GenAI and OpenInference attributes
- export OpenInference-shaped JSON
- observability consumer notes for OTel-shaped and OpenInference-shaped exports

Future work:

- end-to-end tested import for Phoenix, Jaeger, Tempo, or a similar tool
- decide how close the MVP export should get to full OTLP payloads

## Phase 7: Agent tool integrations

Status: partial.

- Codex JSONL importer
- Claude Code JSONL importer
- optional local signatures for imported events
- privacy-oriented import tests for selected source payloads
- documented importer redaction rules
- fixture-backed golden tests for Codex and Claude imports
- export shape tests for OTel-style and OpenInference-style JSON
- deterministic OpenAI-style and Anthropic-style wrapper examples
- deterministic LiteLLM-style wrapper example
- deterministic GitHub tool event example
- deterministic Discord tool event example
- deterministic scheduled run example
- observability consumer notes

Future work:

- end-to-end Langfuse/Phoenix export examples
- AgentOps/Helicone interop notes

## Phase 8: Collector

Status: partial.

- local HTTP ingest server
- SQLite persistence
- query API for runs/events
- verification endpoint
- streaming append endpoint for one verified event at a time
- bounded event listing by stable event sequence
- event listing filtered by event type
- pagination metadata with `has_more`
- structured JSON error responses for malformed HTTP requests

Future work:

- authentication and transport security design
- larger-run query ergonomics
- Postgres persistence option if needed

## Phase 9: UI

Status: partial.

- run list
- permission timeline
- event timeline
- verification status

Future work:

- live collector-backed web UI
- richer run detail pages
- actor chain visualization
- trace/event tree layout
- filtering and pagination
