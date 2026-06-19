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
agentprov collector events run_123 --db agentprov.sqlite --after-sequence 100 --limit 50
agentprov collector events run_123 --db agentprov.sqlite --type permission.check
```

Export a stored run back to JSONL:

```bash
agentprov collector export run_123 --db agentprov.sqlite --out restored-run.jsonl
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

`POST /runs/<run_id>/events`

Request body: one AgentProv event JSON object. The collector verifies the full
stored event chain plus the new event before inserting it. A sequence-1 event
creates the run; later events must link to the previous stored event hash.

Response:

```json
{
  "run_id": "run_123",
  "sequence": 2,
  "event_hash": "blake3:..."
}
```

`GET /runs`

Returns known runs.

`GET /runs/<run_id>/events`

Returns stored event JSON for one run. Optional `after_sequence`, `limit`, and
`event_type` query parameters return a bounded, filtered page by stable event
sequence:

```text
GET /runs/run_123/events?after_sequence=100&limit=50&event_type=permission.check
```

Responses include `count`, `next_after_sequence`, and `has_more` metadata for
client-side paging.

`GET /runs/<run_id>/verify`

Runs event-chain verification for one stored run.

## HTTP errors

HTTP errors return JSON:

```json
{
  "error": "run not found: run_123"
}
```

The collector returns `400` for malformed JSON, invalid query parameters, and
invalid event appends; `404` for missing routes or missing runs; and `500` for
unexpected server failures.

## Limitations

- The server is intentionally small and local-first.
- There is no authentication, TLS, cursor token API, or multi-tenant isolation.
- Streaming append accepts complete AgentProv event records; it does not yet
  expose a typed event builder over HTTP.
- The HTML dashboard is a static read-only export, not a live web application.
- Production deployments should put authentication, transport security, and key
  trust policy in front of or inside a more complete collector.
