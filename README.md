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
- Approval request events for actions that require human review
- Append-only provenance events linked by canonical hashes
- Optional local signatures for manifests, events, and run logs
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
cargo run -- run verify runs/run_123.jsonl --manifest examples/manifest.json
```

Generate a local key and sign records:

```bash
cargo run -- key generate --out agentprov.key
cargo run -- manifest sign examples/manifest.json --key agentprov.key --out manifest.signed.json
cargo run -- manifest verify-signature manifest.signed.json
cargo run -- event sign examples/event.json --key agentprov.key --out event.signed.json
cargo run -- event verify-signature event.signed.json
```

Check a static policy:

```bash
cargo run -- policy check --policy examples/policy.json --agent agent_01hxexample --action discord.message.create --resource discord://guild/148756/channel/456
```

Validate records against the embedded schemas:

```bash
cargo run -- validate manifest examples/manifest.json
cargo run -- validate run-envelope examples/run.json
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

Run deterministic integration examples:

```bash
cargo run --example openai_wrapper -- runs/openai-wrapper.jsonl
cargo run -- run verify runs/openai-wrapper.jsonl
cargo run --example anthropic_wrapper -- runs/anthropic-wrapper.jsonl
cargo run --example litellm_wrapper -- runs/litellm-wrapper.jsonl
cargo run --example github_tool_event -- runs/github-tool-event.jsonl
cargo run --example discord_tool_event -- runs/discord-tool-event.jsonl
cargo run --example scheduled_run -- runs/scheduled-run.jsonl
```

Use the local collector:

```bash
cargo run -- collector ingest demo-output/run.jsonl --db agentprov.sqlite
cargo run -- collector runs --db agentprov.sqlite
cargo run -- collector events run_demo_manual_tool --db agentprov.sqlite
cargo run -- collector verify run_demo_manual_tool --db agentprov.sqlite
cargo run -- collector ui --db agentprov.sqlite --out collector.html
cargo run -- collector serve --addr 127.0.0.1:8787 --db agentprov.sqlite
```

## Repository map

- `src/` - Rust CLI and provenance primitives
- `examples/` - sample manifest, run, event, and policy records
- `schemas/` - machine-readable JSON Schemas
- `CHANGELOG.md` - release history
- `docs/spec/` - versioned spec notes
- `docs/agent-tool-integrations.md` - Codex and Claude Code import guide
- `docs/research/` - research notes from existing OSS tools
- `docs/mvp-scope.md` - MVP product scope
- `docs/next-steps.md` - current delivery status and future work
- `docs/collector.md` - local SQLite collector and HTTP endpoint notes
- `docs/release-process.md` - release checklist and tag workflow
- `docs/trust-semantics.md` - local signature and trust-root semantics
- `docs/observability-consumers.md` - OTel/OpenInference export consumer notes
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

AgentProv is an early MVP. The record formats, CLI, and schemas are still expected to change.

Current limitations:

- Local key files are for experimentation only, not production key management.
- Signatures prove a record matches the embedded public key; they do not yet establish an organisational trust root.
- Run logs are event-chain JSONL files. A formal run envelope can be generated, but it is not yet stored as a separate signed run-log header.
- OpenTelemetry and OpenInference exports are JSON-shaped interoperability experiments, not a full OTLP collector implementation.
- The importer privacy model avoids copying full prompts, assistant text, command output, and tool-result content, but it is not a complete DLP system.

## License

MIT
