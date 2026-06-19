# Run Envelope v1

## Purpose

A Run Envelope describes one execution of an agent: who or what triggered it, which agent version ran, where it ran, and what authority was available.

## Required fields

- `schema`: must be `agentprov.dev/run-envelope/v1`
- `run_id`: stable run identifier
- `trace_id`: trace identifier for observability correlation
- `trigger`: object with `type` and `id`
- `agent`: object with `agent_id`, `version`, and `manifest_digest`
- `actor_chain`: ordered list of actors in the responsibility chain
- `runtime`: host/runtime/environment details
- `authority`: capabilities and policy reference
- `started_at`
- `status`

## Optional fields

- `parent_run_id`: parent run when delegated by another agent
- `ended_at`: completion timestamp

## Actor chain

The actor chain should preserve responsibility, for example:

```text
user -> service -> agent -> tool
```

Each actor must have `type` and `id`; `auth_method` is optional.

## Trigger types

Initial trigger types:

- `manual`
- `scheduled`
- `webhook`
- `api`
- `ci`
- `delegated`

## OpenTelemetry/OpenInference mapping

The run ID can be used as the trace ID or as an AgentProv attribute on exported spans. The actor chain should be preserved under `agentprov.actor_chain`.

## Manifest binding

`agentprov run init --agent <manifest>` records the manifest digest in the first
`run.start` event under `metadata.agent_manifest_digest`. For signed manifests,
the digest is the signed payload hash, not a hash of the signature wrapper.

`agentprov run verify <run.jsonl> --manifest <manifest>` verifies that recorded
digest against the supplied manifest and verifies the manifest signature when
one is present.
