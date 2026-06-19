# Agent Manifest v1

## Purpose

An Agent Manifest describes an AI agent as an identity-bearing principal rather than a loose name in telemetry metadata.

## Required fields

- `schema`: must be `agentprov.dev/manifest/v1`
- `agent_id`: stable agent identifier
- `name`: human-readable name
- `description`: short purpose statement
- `version`: agent implementation version
- `owner`: object with `type` and `id`
- `source`: object with `repo`, `commit`, and optional `image_digest`
- `runtime`: object with `type` and `environment`
- `capabilities`: list of declared capability strings
- `policy`: object with `id` and `version`

## Optional fields

- `public_key`: key material or key reference used to verify manifests and events
- implementation-specific metadata may be added under a future `metadata` object

## Canonicalisation

For hashing or signing, serialise JSON after recursively sorting object keys. Do not reorder arrays.

## Signing

A signed manifest should include a top-level `signature` object with:

- `algorithm`
- `key_id`
- `public_key`
- `signature`
- `signed_hash`

The CLI can sign and verify local MVP manifest signatures:

```bash
agentprov manifest sign examples/manifest.json --key agentprov.key --out manifest.signed.json
agentprov manifest verify-signature manifest.signed.json
```

## Privacy considerations

Manifests should not contain secrets. Store secret scopes/capabilities, not secret values.

## OpenTelemetry/OpenInference mapping

- `agent_id` maps to `gen_ai.agent.id`
- `name` maps to `gen_ai.agent.name`
- `version` maps to `gen_ai.agent.version`
- `description` maps to `gen_ai.agent.description`
