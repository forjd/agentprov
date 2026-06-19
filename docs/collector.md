# Local Collector

The local collector is an MVP ingest and query surface for AgentProv JSONL run
logs. It stores original event JSON in SQLite and verifies stored runs with the
same event-chain verifier used by `agentprov run verify`.

## CLI

Ingest a run log:

```bash
agentprov collector ingest runs/run_123.jsonl --db agentprov.sqlite
```

List runs:

```bash
agentprov collector runs --db agentprov.sqlite
```

Read events for a run:

```bash
agentprov collector events run_123 --db agentprov.sqlite
```

Verify a stored run:

```bash
agentprov collector verify run_123 --db agentprov.sqlite
agentprov collector verify run_123 --db agentprov.sqlite --require-signatures
```

Export a static read-only dashboard:

```bash
agentprov collector ui --db agentprov.sqlite --out collector.html
```

Start the HTTP collector:

```bash
agentprov collector serve --addr 127.0.0.1:8787 --db agentprov.sqlite
```

## HTTP endpoints

`POST /ingest`

Request body: AgentProv JSONL events.

Response:

```json
{
  "run_id": "run_123",
  "events": 4
}
```

`GET /runs`

Returns known runs.

`GET /runs/<run_id>/events`

Returns stored event JSON for one run.

`GET /runs/<run_id>/verify`

Runs event-chain verification for one stored run.

## Limitations

- The server is intentionally small and local-first.
- There is no authentication, TLS, pagination, or multi-tenant isolation.
- The HTTP surface accepts complete JSONL run logs, not streaming append yet.
- The HTML dashboard is a static read-only export, not a live web application.
- Production deployments should put authentication, transport security, and key
  trust policy in front of or inside a more complete collector.
