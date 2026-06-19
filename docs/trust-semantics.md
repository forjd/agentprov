# Trust Semantics

AgentProv's local MVP signatures are useful for tamper evidence, but they are
not a complete organisational trust system.

## What local signatures prove

For a signed manifest or event, `agentprov * verify-signature` proves:

- the record still hashes to the `signature.signed_hash` value
- the Ed25519 signature verifies against the embedded public key
- the embedded key ID is the key ID that was used when the local key signed the
  record

For a run log, `agentprov run verify --require-signatures` additionally proves
that every event has a valid local signature and that the event chain has not
been mutated.

## What local signatures do not prove

Local signatures do not prove:

- that the embedded public key belongs to a trusted organisation
- that the key was protected by KMS, HSM, workload identity, or hardware-backed
  attestation
- that the runtime host was uncompromised
- that an action was safe or correct
- that a human approver was authorised outside the recorded policy context

## Manifest binding

`agentprov run init --agent <manifest>` records
`metadata.agent_manifest_digest` in the first `run.start` event. If the manifest
is signed, the digest is the signed payload hash.

`agentprov run verify <run.jsonl> --manifest <manifest>` verifies that binding
and verifies the manifest signature when one is present.

For the local MVP, manifest binding is explicit and optional. For production
profiles, signed run logs should require an expected manifest and a trusted key
source.

## Run envelope decision

The current run log is event-chain JSONL. The first `run.start` event carries
the key run metadata needed for verification and manifest binding.

The separate Run Envelope schema remains useful for interoperability and future
collector APIs, but the MVP does not store a signed envelope header in every
JSONL run log. A future format can add an envelope prelude or sidecar without
breaking event-chain verification.

## Future trust roots

Production trust should be added through one or more explicit trust roots:

- key registry mapping `key_id` to trusted principals
- cloud KMS or HSM-backed signing
- workload identity or OIDC-bound signing
- transparency log inclusion for manifests and key rotations
- revocation and expiry metadata for keys
- organisation policy for which keys may sign which agent manifests
