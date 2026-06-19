# Local Key v1

## Purpose

`agentprov.dev/local-key/v1` is an MVP local development key format for signing AgentProv manifests and events.

It is not a production key-management system.

## Required fields

- `schema`: must be `agentprov.dev/local-key/v1`
- `algorithm`: currently `ed25519`
- `key_id`: stable identifier for this key file
- `public_key`: hex-encoded Ed25519 public key
- `secret_key`: hex-encoded Ed25519 signing key

## CLI commands

```bash
agentprov key generate --out agentprov.key
agentprov key public --key agentprov.key
agentprov key inspect --key agentprov.key
agentprov manifest verify-signature manifest.signed.json
agentprov event verify-signature event.signed.json
```

`key public` prints only public material.

`key inspect` prints metadata and whether secret material exists, but does not print the secret key value.

## Security considerations

- Treat files in this format as secrets.
- Do not commit local key files.
- Use OS secret stores, HSMs, cloud KMS, workload identity, or signing services for production systems.
- Future versions should support external signing providers rather than requiring local private key material.
