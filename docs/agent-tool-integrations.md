# Agent Tool Integrations

AgentProv can import JSONL streams from Codex and Claude Code into verifiable
AgentProv run logs. The importers preserve the lifecycle shape of an agent run
without copying full prompts, assistant text, command output, or tool-result
content into the provenance log. Each source event is represented by a
`payload_digest` and selected metadata.

## Codex

Capture a non-interactive Codex run:

```bash
codex exec --ephemeral --json --sandbox read-only \
  "Inspect Cargo.toml and summarize the package name." \
  > /tmp/codex.jsonl
```

Import and verify the run:

```bash
agentprov import codex /tmp/codex.jsonl --out runs/codex-run.jsonl
agentprov run verify runs/codex-run.jsonl
```

You can also stream directly from Codex:

```bash
codex exec --ephemeral --json --sandbox read-only "Summarize this repo." \
  | agentprov import codex - --out runs/codex-run.jsonl
```

The Codex importer maps `thread.started` to `run.start`, `turn.started` to
`agent.invoke`, command/MCP/web-search items to `tool.execute`, assistant
messages to `artifact.create`, reasoning/plan updates to `agent.plan`, and
`turn.completed` to `run.end`.

## Claude Code

Claude Code does not read `AGENTS.md` directly, so this repository includes a
`CLAUDE.md` file that imports the shared guidance:

```markdown
@AGENTS.md
```

Capture a non-interactive Claude Code run:

```bash
claude -p --output-format stream-json --verbose --no-session-persistence \
  --permission-mode dontAsk --tools Read \
  "Read Cargo.toml and summarize the package name." \
  > /tmp/claude.jsonl
```

Import and verify the run:

```bash
agentprov import claude /tmp/claude.jsonl --out runs/claude-run.jsonl
agentprov run verify runs/claude-run.jsonl
```

You can also stream directly from Claude Code:

```bash
claude -p --output-format stream-json --verbose --no-session-persistence \
  --permission-mode dontAsk --tools Read \
  "Summarize this repo." \
  | agentprov import claude - --out runs/claude-run.jsonl
```

Add `--include-hook-events` when you want Claude Code hook lifecycle events in
the source stream as well.

The Claude importer maps `system:init` to `run.start`, tool-use messages to
`tool.execute`, tool-result messages to `tool.execute` result records, assistant
text messages to `artifact.create`, thinking-token events to `agent.plan`, rate
limit events to `permission.check`, and final `result` events to `run.end` or
`run.error`.

## Signing Imported Runs

Both importers support local MVP signatures:

```bash
agentprov key generate --out agentprov.key
agentprov import codex /tmp/codex.jsonl --out runs/codex-run.jsonl --key agentprov.key
agentprov run verify runs/codex-run.jsonl --require-signatures
```

Local key files are for experimentation only. Do not commit generated keys.
