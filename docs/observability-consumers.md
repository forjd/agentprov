# Observability Consumer Examples

AgentProv exports are JSON-shaped interoperability artifacts, not a full OTLP
collector implementation yet.

## OTel-shaped JSON

Create a run and export it:

```bash
agentprov demo manual-tool-run --out demo-output/
agentprov export otel demo-output/run.jsonl --out run.otlp.json
```

The output uses an OTLP-inspired `resourceSpans[].scopeSpans[].spans[]` shape.
It is intended as a bridge for tools such as Jaeger or Tempo examples, but it is
not yet a network exporter.

## OpenInference-shaped JSON

```bash
agentprov export openinference demo-output/run.jsonl --out run.openinference.json
```

The output contains spans with OpenInference span-kind attributes where AgentProv
event types map clearly:

- `llm.call` -> `LLM`
- `tool.execute` -> `TOOL`
- `agent.invoke` / `run.start` -> `AGENT`
- `agent.plan` -> `AGENT`

## Current limitations

- Exports do not push to Phoenix, Jaeger, Tempo, Langfuse, or AgentOps directly.
- Span IDs are derived from AgentProv event IDs and are not yet normalized to all
  backend-specific ID constraints.
- The exporter keeps AgentProv identity, permission, and verification attributes
  rather than trying to mimic a complete tracing SDK.
- Backend-specific import guides should be added once one target backend is
  selected for end-to-end testing.
