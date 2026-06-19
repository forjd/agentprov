# Provenance Event v1

## Purpose

A Provenance Event is an append-only record in an agent run. Events form a hash chain so accidental or casual mutation is detectable.

## Required fields

- `schema`: must be `agentprov.dev/event/v1`
- `event_id`
- `run_id`
- `sequence`
- `timestamp`
- `event_type`
- `subject`
- `previous_event_hash`
- `event_hash`

## Optional fields

- `action`
- `resource`
- `payload_digest`
- `metadata`
- `signature`
- `key_id`

## Hashing rules

To calculate `event_hash`:

1. Remove `event_hash`.
2. Remove `signature`.
3. Recursively sort JSON object keys.
4. Serialise to JSON.
5. Hash with BLAKE3.
6. Prefix with `blake3:`.

The first event in a run must have `previous_event_hash = null`. Each later event must set `previous_event_hash` to the prior event's `event_hash`.

## Signing rules

The MVP signs the event hash with Ed25519 and stores the signature in the top-level `signature` object.

## Recommended event types

- `run.start`
- `agent.invoke`
- `agent.plan`
- `llm.call`
- `permission.check`
- `tool.execute`
- `memory.read`
- `memory.write`
- `artifact.create`
- `artifact.update`
- `artifact.delete`
- `human.approval.request`
- `human.approval.grant`
- `human.approval.deny`

See `examples/approval-request-event.json`,
`examples/approval-grant-event.json`, and
`examples/approval-deny-event.json` for the initial approval event shape.

## Privacy considerations

Store digests and redacted previews by default. Full prompts, tool inputs, tool outputs and retrieved data should be opt-in.
