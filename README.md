# AgentProv

[![CI](https://github.com/forjd/agentprov/actions/workflows/ci.yml/badge.svg)](https://github.com/forjd/agentprov/actions/workflows/ci.yml)

Signed, tamper-evident provenance records for AI agent runs.

AgentProv is a Rust-first MVP for answering the audit questions that ordinary LLM observability usually leaves open:

> Who or what ran this agent, with what authority, where did it run, and does the record still verify?

It is designed to sit beside systems such as Langfuse, Phoenix/OpenInference, AgentOps, Helicone, MLflow, and Weave. Those tools show prompts, model calls, tool calls, latency, cost, and traces. AgentProv focuses on identity, authority, policy decisions, and tamper evidence.

## What it records

- Agent identity, owner, source repository, version, runtime, capabilities, and policy reference
- Run trigger, actor chain, runtime context, available tools, and policy version
- Permission checks for agent actions against scoped resources
- Append-only provenance events linked by canonical hashes
- Optional local signatures for manifests, events, and run records
- JSONL imports from Codex and Claude Code agent runs
- Experimental exports to OpenTelemetry-style and OpenInference-style JSON

## Quick start

Run a complete local demo:

```bash
cargo run -- demo manual-tool-run --out demo-output/
cargo run -- run verify demo-output/run.jsonl
```

Expected output shape:

```text
Run verifies
Events: 4
Event chain: valid
Signatures: not present
```

## Install

Install from GitHub:

```bash
cargo install --git https://github.com/forjd/agentprov
agentprov --version
```

Or clone the repository for local development:

```bash
git clone https://github.com/forjd/agentprov.git
cd agentprov
cargo run -- --version
```

## CLI examples

Generate example records:

```bash
cargo run -- manifest example
cargo run -- run example
```

Hash and verify event records:

```bash
cargo run -- event hash examples/event.json
cargo run -- event verify examples/event.json
```

Create and verify an append-only run log:

```bash
cargo run -- run init --agent examples/manifest.json --trigger manual --out runs/run_123.jsonl
cargo run -- event append --run runs/run_123.jsonl --type permission.check --action discord.message.create --resource discord://guild/123/channel/456
cargo run -- run verify runs/run_123.jsonl
```

Generate a local key and sign an event:

```bash
cargo run -- key generate --out agentprov.key
cargo run -- event sign examples/event.json --key agentprov.key --out event.signed.json
cargo run -- event verify-signature event.signed.json
```

Check a static policy:

```bash
cargo run -- policy check --policy examples/policy.json --agent agent_01hxexample --action discord.message.create --resource discord://guild/148756/channel/456
```

Export a run log:

```bash
cargo run -- export otel demo-output/run.jsonl --out run.otlp.json
cargo run -- export openinference demo-output/run.jsonl --out run.openinference.json
```

Import Codex or Claude Code JSONL streams:

```bash
codex exec --ephemeral --json --sandbox read-only "Summarize this repo." \
  | cargo run -- import codex - --out runs/codex-run.jsonl

claude -p --output-format stream-json --verbose --no-session-persistence \
  "Summarize this repo." \
  | cargo run -- import claude - --out runs/claude-run.jsonl
```

## Repository map

- `src/` - Rust CLI and provenance primitives
- `examples/` - sample manifest, run, event, and policy records
- `schemas/` - machine-readable JSON Schemas
- `docs/spec/` - versioned spec notes
- `docs/agent-tool-integrations.md` - Codex and Claude Code import guide
- `docs/research/` - research notes from existing OSS tools
- `docs/mvp-scope.md` - MVP product scope
- `docs/otel-mapping.md` - OpenTelemetry/OpenInference mapping notes
- `docs/threat-model.md` - threat model

## Quality gates

Run these before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Status

AgentProv is an early MVP. The record formats, CLI, and schemas are still expected to change. Local key handling is for experimentation only, not production key management.

## License

MIT
