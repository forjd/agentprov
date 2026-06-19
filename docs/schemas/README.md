# Schemas

Machine-readable JSON Schemas live in the repository-level `schemas/`
directory:

- `schemas/manifest-v1.schema.json`
- `schemas/run-envelope-v1.schema.json`
- `schemas/event-v1.schema.json`
- `schemas/policy-v1.schema.json`

The CLI embeds those schemas and can validate records without needing schema
files beside the installed binary:

```bash
agentprov validate manifest examples/manifest.json
agentprov validate run-envelope examples/run.json
agentprov validate event examples/event.json
agentprov validate policy examples/policy.json
```

The versioned prose specs live in `docs/spec/`.
