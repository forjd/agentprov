# Next Steps

AgentProv currently has a useful seed repository: research notes, MVP scope, schema sketches, OpenTelemetry/OpenInference mapping notes, a threat model, examples, CI, and a small Rust CLI for example records and single-event hashing/verification.

The next goal is to turn it from a research-backed skeleton into a small, demonstrable provenance primitive.

## Current state

Repository: https://github.com/forjd-hermes-bot/agentprov

Implemented:

- Rust CLI crate
- MIT licence
- CI workflow
- research summary and detailed findings
- MVP scope
- schema sketches
- threat model
- roadmap
- OpenTelemetry/OpenInference mapping notes
- example manifest/run/event JSON files
- CLI commands:
  - `agentprov manifest example`
  - `agentprov manifest hash <file>`
  - `agentprov run example`
  - `agentprov event hash <file>`
  - `agentprov event verify <file>`

Verified:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build --release`
- GitHub Actions CI is passing

## Recommended direction

Do not try to become another LLM observability product.

The positioning should stay focused:

> Signed provenance records for AI agent runs.

Or:

> OpenTelemetry-compatible identity and provenance for AI agents.

The useful comparison line is:

> Langfuse, Phoenix and AgentOps show what happened. AgentProv proves who ran it, with what authority, and whether the record still verifies.

## Phase 1: Make run verification real

The current CLI verifies one event hash. The next milestone should verify a full append-only run log.

### Target behaviour

```bash
agentprov run init --agent examples/manifest.json --trigger manual --out runs/run_123.jsonl
agentprov event append --run runs/run_123.jsonl --type permission.check --action discord.message.create --resource discord://guild/123/channel/456
agentprov event append --run runs/run_123.jsonl --type tool.execute --action discord.message.create --resource discord://guild/123/channel/456
agentprov run verify runs/run_123.jsonl
```

Expected output:

```text
Run verifies
Events: 2
Event chain: valid
Signatures: not present
```

### Required work

- Add a `RunLog` concept based on newline-delimited JSON events.
- Add `run init` command.
- Add `event append` command.
- Each appended event should:
  - increment sequence number
  - copy the previous event hash
  - compute its own event hash
  - write one JSON line
- Add `run verify` command.
- Verification should fail if:
  - sequence numbers skip or repeat
  - `previous_event_hash` does not match the prior event
  - an event hash is wrong
  - the file contains invalid JSON

### Acceptance criteria

- Integration tests cover valid and tampered run logs.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.

## Phase 2: Add signing

Once event chains work, add Ed25519 signing.

### Target behaviour

```bash
agentprov key generate --out agentprov.key
agentprov manifest sign examples/manifest.json --key agentprov.key --out examples/manifest.signed.json
agentprov event sign examples/event.json --key agentprov.key --out examples/event.signed.json
agentprov event verify-signature examples/event.signed.json
agentprov run verify runs/run_123.jsonl --require-signatures
```

### Required work

- Add key generation.
- Add public key export.
- Add manifest signing.
- Add event signing.
- Add signature verification.
- Decide how keys are represented on disk.
- Document that MVP key handling is for local experimentation, not production key management.

### Acceptance criteria

- Signed events verify.
- Modified signed events fail verification.
- `run verify --require-signatures` fails if unsigned events are present.

## Phase 3: Formalise the spec

Move from informal schema sketches to explicit versioned spec documents.

Create:

```text
docs/spec/manifest-v1.md
docs/spec/run-envelope-v1.md
docs/spec/event-v1.md
docs/spec/policy-v1.md
docs/spec/otel-attributes-v1.md
```

Each spec should include:

- purpose
- required fields
- optional fields
- canonicalisation rules
- hashing/signing rules where relevant
- privacy considerations
- OpenTelemetry/OpenInference mappings where relevant
- JSON examples

## Phase 4: Add formal JSON Schema files

Create machine-readable schemas:

```text
schemas/manifest-v1.schema.json
schemas/run-envelope-v1.schema.json
schemas/event-v1.schema.json
schemas/policy-v1.schema.json
```

Add tests that validate all examples against the schemas.

## Phase 5: Add a static policy MVP

The permission model is a key differentiator, so add a small policy engine before adding more observability features.

### Target behaviour

```bash
agentprov policy check \
  --policy examples/policy.json \
  --agent agent_01hxexample \
  --action discord.message.create \
  --resource discord://guild/148756/channel/148756
```

Expected output:

```json
{
  "decision": "allow",
  "policy_id": "policy_research_agent",
  "policy_version": "v1",
  "reason": "matched allow rule"
}
```

### MVP policy rules

Support three rule lists:

- `allow`
- `deny`
- `require_approval`

Rule fields:

- `action`
- `resource`
- optional `expires_at`

Initial matching can be deliberately simple:

- exact match
- `*` wildcard
- prefix wildcard suffix such as `discord://guild/123/*`

## Phase 6: Add the first end-to-end demo

A deterministic demo will make the project easier to explain.

### Target command

```bash
agentprov demo manual-tool-run --out demo-output/
agentprov run verify demo-output/run.jsonl
```

### Demo story

The demo should show:

1. A user triggers an agent manually.
2. The agent has a manifest and capabilities.
3. The agent checks permission to call a tool.
4. The tool call is recorded.
5. The event chain verifies.

Example final output:

```text
Run verifies
Agent: research-agent v0.1.0
Trigger: manual
Actor chain: danjdewhurst -> hermes -> research-agent
Events: 4
Permission checks: 1 allowed
Tool calls: 1
Event chain: valid
Signatures: valid
```

## Phase 7: Improve README for external readers

The README should become a product landing page, not just a project note.

Add:

- clear problem statement
- short comparison with Langfuse/Phoenix/AgentOps
- 30-second demo
- example run log
- explanation of event-chain verification
- installation/build instructions
- contribution roadmap

## Phase 8: OpenTelemetry export

Once the core provenance model works, add export rather than a custom dashboard.

Target:

```bash
agentprov export otel runs/run_123.jsonl --out run_123.otlp.json
agentprov export openinference runs/run_123.jsonl --out run_123.openinference.json
```

The first export can be JSON rather than a full OTLP network exporter.

## Phase 9: Integrations

After the core is credible, add examples for real agent environments:

- OpenAI/Anthropic/LiteLLM model call wrapper
- Discord tool example
- GitHub tool example
- scheduled/cron run example
- Phoenix or Langfuse export example

## Suggested GitHub issues

Create these issues first:

1. Implement append-only run logs and `run verify`
2. Add Ed25519 key generation and event signing
3. Split schema sketches into versioned spec docs
4. Add machine-readable JSON Schemas
5. Add static policy check command
6. Add deterministic manual tool-run demo
7. Improve README with 30-second demo
8. Add OpenTelemetry/OpenInference export format

## Near-term priority

The most important next task is full run-chain verification.

Without it, AgentProv is mostly a documented idea. With it, the project has a concrete primitive:

> A portable run log that can prove whether its own history has been altered.
