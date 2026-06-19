use crate::canonical::canonical_hash;
use crate::event::{build_event, event_hash};
use crate::run_log::write_jsonl;
use crate::signing::{LocalKeyFile, sign_value};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use uuid::Uuid;

pub struct ImportReport {
    pub provider: &'static str,
    pub run_id: String,
    pub source_events: usize,
    pub events: usize,
}

#[derive(Clone, Copy)]
enum Provider {
    Codex,
    Claude,
}

struct ImportContext {
    provider: Provider,
    run_id: String,
    session_uri: String,
}

struct EventDraft {
    event_type: String,
    action: Option<String>,
    resource: Option<String>,
    subject: String,
    metadata: Value,
}

pub fn import_codex_jsonl(
    input: &Path,
    out: &Path,
    key: Option<&LocalKeyFile>,
) -> Result<ImportReport> {
    import_jsonl(Provider::Codex, input, out, key)
}

pub fn import_claude_jsonl(
    input: &Path,
    out: &Path,
    key: Option<&LocalKeyFile>,
) -> Result<ImportReport> {
    import_jsonl(Provider::Claude, input, out, key)
}

fn import_jsonl(
    provider: Provider,
    input: &Path,
    out: &Path,
    key: Option<&LocalKeyFile>,
) -> Result<ImportReport> {
    let source_events = read_jsonl_source(input)?;
    if source_events.is_empty() {
        bail!("source JSONL contains no events");
    }
    let context = import_context(provider, &source_events);
    let mut events = Vec::new();
    let mut previous_hash: Option<String> = None;

    for source_event in &source_events {
        for draft in map_source_event(&context, source_event)? {
            let event = build_imported_event(
                &context.run_id,
                events.len() as u64 + 1,
                previous_hash.clone(),
                draft,
                source_event,
                key,
            )?;
            previous_hash = event
                .get("event_hash")
                .and_then(Value::as_str)
                .map(str::to_owned);
            events.push(event);
        }
    }

    if events.is_empty() {
        bail!("source JSONL contained no importable events");
    }

    write_jsonl(out, &events)?;
    Ok(ImportReport {
        provider: provider.name(),
        run_id: context.run_id,
        source_events: source_events.len(),
        events: events.len(),
    })
}

fn read_jsonl_source(path: &Path) -> Result<Vec<Value>> {
    if path.as_os_str() == "-" {
        let stdin = io::stdin();
        return read_jsonl_lines(stdin.lock(), "stdin");
    }
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    read_jsonl_lines(BufReader::new(file), &path.display().to_string())
}

fn read_jsonl_lines<R: BufRead>(reader: R, label: &str) -> Result<Vec<Value>> {
    let mut values = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line = line.with_context(|| format!("read line {}", index + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        values.push(
            serde_json::from_str(&line)
                .with_context(|| format!("parse JSON line {} in {label}", index + 1))?,
        );
    }
    Ok(values)
}

fn import_context(provider: Provider, source_events: &[Value]) -> ImportContext {
    let external_id = match provider {
        Provider::Codex => source_events
            .iter()
            .find_map(|event| event.get("thread_id").and_then(Value::as_str)),
        Provider::Claude => source_events
            .iter()
            .find_map(|event| event.get("session_id").and_then(Value::as_str)),
    }
    .map(str::to_owned)
    .unwrap_or_else(|| Uuid::new_v4().simple().to_string());

    let sanitized = sanitize_id(&external_id);
    let run_id = format!("run_{}_{}", provider.name(), sanitized);
    let session_uri = match provider {
        Provider::Codex => format!("codex://thread/{external_id}"),
        Provider::Claude => format!("claude://session/{external_id}"),
    };
    ImportContext {
        provider,
        run_id,
        session_uri,
    }
}

fn sanitize_id(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();
    if sanitized.is_empty() {
        Uuid::new_v4().simple().to_string()
    } else {
        sanitized
    }
}

fn map_source_event(context: &ImportContext, source_event: &Value) -> Result<Vec<EventDraft>> {
    match context.provider {
        Provider::Codex => Ok(map_codex_event(context, source_event)),
        Provider::Claude => Ok(map_claude_event(context, source_event)),
    }
}

fn map_codex_event(context: &ImportContext, source_event: &Value) -> Vec<EventDraft> {
    let source_type = source_event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("event");
    let mut metadata = base_metadata(context.provider, source_event);

    match source_type {
        "thread.started" => {
            insert_string(
                &mut metadata,
                "thread_id",
                source_event.get("thread_id").and_then(Value::as_str),
            );
            vec![EventDraft {
                event_type: "run.start".to_owned(),
                action: Some("codex.thread.started".to_owned()),
                resource: Some(context.session_uri.clone()),
                subject: "codex-cli".to_owned(),
                metadata,
            }]
        }
        "turn.started" => vec![EventDraft {
            event_type: "agent.invoke".to_owned(),
            action: Some("codex.turn.started".to_owned()),
            resource: Some(context.session_uri.clone()),
            subject: "codex-cli".to_owned(),
            metadata,
        }],
        "turn.completed" => {
            insert_value(&mut metadata, "usage", source_event.get("usage").cloned());
            vec![EventDraft {
                event_type: "run.end".to_owned(),
                action: Some("codex.turn.completed".to_owned()),
                resource: Some(context.session_uri.clone()),
                subject: "codex-cli".to_owned(),
                metadata,
            }]
        }
        "turn.failed" | "error" => vec![EventDraft {
            event_type: "run.error".to_owned(),
            action: Some(format!("codex.{source_type}")),
            resource: Some(context.session_uri.clone()),
            subject: "codex-cli".to_owned(),
            metadata,
        }],
        "item.started" | "item.completed" => {
            let item = source_event.get("item").unwrap_or(&Value::Null);
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("item");
            insert_string(
                &mut metadata,
                "item_id",
                item.get("id").and_then(Value::as_str),
            );
            insert_string(&mut metadata, "item_type", Some(item_type));
            insert_string(
                &mut metadata,
                "item_status",
                item.get("status").and_then(Value::as_str),
            );
            insert_value(&mut metadata, "exit_code", item.get("exit_code").cloned());

            let (event_type, action, resource) = match item_type {
                "command_execution" => (
                    "tool.execute",
                    "shell.command".to_owned(),
                    item_uri(context, item),
                ),
                "file_change" | "file_diff" => (
                    "artifact.update",
                    format!("codex.{item_type}"),
                    item.get("path").and_then(Value::as_str).map(str::to_owned),
                ),
                "agent_message" => (
                    "artifact.create",
                    "codex.agent_message".to_owned(),
                    item_uri(context, item),
                ),
                "reasoning" | "plan_update" => (
                    "agent.plan",
                    format!("codex.{item_type}"),
                    item_uri(context, item),
                ),
                "mcp_tool_call" | "web_search" => (
                    "tool.execute",
                    format!("codex.{item_type}"),
                    item_uri(context, item),
                ),
                _ => (
                    "agent.event",
                    format!("codex.{item_type}"),
                    item_uri(context, item),
                ),
            };

            vec![EventDraft {
                event_type: event_type.to_owned(),
                action: Some(action),
                resource,
                subject: "codex-cli".to_owned(),
                metadata,
            }]
        }
        _ => vec![EventDraft {
            event_type: "agent.event".to_owned(),
            action: Some(format!("codex.{source_type}")),
            resource: Some(context.session_uri.clone()),
            subject: "codex-cli".to_owned(),
            metadata,
        }],
    }
}

fn map_claude_event(context: &ImportContext, source_event: &Value) -> Vec<EventDraft> {
    let source_type = source_event
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("event");
    let mut metadata = base_metadata(context.provider, source_event);

    match source_type {
        "system" => {
            let subtype = source_event
                .get("subtype")
                .and_then(Value::as_str)
                .unwrap_or("system");
            insert_string(&mut metadata, "subtype", Some(subtype));
            match subtype {
                "init" => {
                    insert_string(
                        &mut metadata,
                        "cwd",
                        source_event.get("cwd").and_then(Value::as_str),
                    );
                    insert_string(
                        &mut metadata,
                        "model",
                        source_event.get("model").and_then(Value::as_str),
                    );
                    insert_string(
                        &mut metadata,
                        "claude_code_version",
                        source_event
                            .get("claude_code_version")
                            .and_then(Value::as_str),
                    );
                    insert_string(
                        &mut metadata,
                        "permission_mode",
                        source_event.get("permissionMode").and_then(Value::as_str),
                    );
                    insert_value(&mut metadata, "tools", source_event.get("tools").cloned());
                    vec![EventDraft {
                        event_type: "run.start".to_owned(),
                        action: Some("claude.session.started".to_owned()),
                        resource: Some(context.session_uri.clone()),
                        subject: "claude-code".to_owned(),
                        metadata,
                    }]
                }
                "thinking_tokens" => {
                    insert_value(
                        &mut metadata,
                        "estimated_tokens",
                        source_event.get("estimated_tokens").cloned(),
                    );
                    insert_value(
                        &mut metadata,
                        "estimated_tokens_delta",
                        source_event.get("estimated_tokens_delta").cloned(),
                    );
                    vec![EventDraft {
                        event_type: "agent.plan".to_owned(),
                        action: Some("claude.thinking_tokens".to_owned()),
                        resource: Some(context.session_uri.clone()),
                        subject: "claude-code".to_owned(),
                        metadata,
                    }]
                }
                _ => vec![EventDraft {
                    event_type: "agent.event".to_owned(),
                    action: Some(format!("claude.system.{subtype}")),
                    resource: Some(context.session_uri.clone()),
                    subject: "claude-code".to_owned(),
                    metadata,
                }],
            }
        }
        "rate_limit_event" => {
            let info = source_event.get("rate_limit_info").unwrap_or(&Value::Null);
            insert_string(
                &mut metadata,
                "status",
                info.get("status").and_then(Value::as_str),
            );
            insert_string(
                &mut metadata,
                "rate_limit_type",
                info.get("rateLimitType").and_then(Value::as_str),
            );
            vec![EventDraft {
                event_type: "permission.check".to_owned(),
                action: Some("claude.rate_limit".to_owned()),
                resource: info
                    .get("rateLimitType")
                    .and_then(Value::as_str)
                    .map(|kind| format!("claude://rate-limit/{kind}"))
                    .or_else(|| Some(context.session_uri.clone())),
                subject: "claude-code".to_owned(),
                metadata,
            }]
        }
        "assistant" => map_claude_assistant_event(context, source_event, metadata),
        "user" => map_claude_user_event(context, source_event, metadata),
        "result" => {
            insert_string(
                &mut metadata,
                "subtype",
                source_event.get("subtype").and_then(Value::as_str),
            );
            insert_value(
                &mut metadata,
                "is_error",
                source_event.get("is_error").cloned(),
            );
            insert_value(
                &mut metadata,
                "duration_ms",
                source_event.get("duration_ms").cloned(),
            );
            insert_value(
                &mut metadata,
                "num_turns",
                source_event.get("num_turns").cloned(),
            );
            insert_value(&mut metadata, "usage", source_event.get("usage").cloned());
            insert_value(
                &mut metadata,
                "total_cost_usd",
                source_event.get("total_cost_usd").cloned(),
            );
            let is_error = source_event
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            vec![EventDraft {
                event_type: if is_error { "run.error" } else { "run.end" }.to_owned(),
                action: Some(format!(
                    "claude.result.{}",
                    source_event
                        .get("subtype")
                        .and_then(Value::as_str)
                        .unwrap_or("complete")
                )),
                resource: Some(context.session_uri.clone()),
                subject: "claude-code".to_owned(),
                metadata,
            }]
        }
        value if value.contains("hook") => vec![EventDraft {
            event_type: "agent.event".to_owned(),
            action: Some(format!("claude.{value}")),
            resource: Some(context.session_uri.clone()),
            subject: "claude-code".to_owned(),
            metadata,
        }],
        _ => vec![EventDraft {
            event_type: "agent.event".to_owned(),
            action: Some(format!("claude.{source_type}")),
            resource: Some(context.session_uri.clone()),
            subject: "claude-code".to_owned(),
            metadata,
        }],
    }
}

fn map_claude_assistant_event(
    _context: &ImportContext,
    source_event: &Value,
    metadata: Value,
) -> Vec<EventDraft> {
    let message = source_event.get("message").unwrap_or(&Value::Null);
    let message_id = message.get("id").and_then(Value::as_str);
    let model = message.get("model").and_then(Value::as_str);
    let mut drafts = Vec::new();

    for content in message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let content_type = content
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("content");
        let mut item_metadata = metadata.clone();
        insert_string(&mut item_metadata, "message_id", message_id);
        insert_string(&mut item_metadata, "model", model);
        insert_string(&mut item_metadata, "content_type", Some(content_type));

        let (event_type, action, resource) = match content_type {
            "tool_use" => {
                let tool_name = content
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool");
                insert_string(
                    &mut item_metadata,
                    "tool_use_id",
                    content.get("id").and_then(Value::as_str),
                );
                (
                    "tool.execute",
                    tool_name.to_owned(),
                    claude_tool_resource(tool_name, content.get("input")),
                )
            }
            "text" => (
                "artifact.create",
                "claude.assistant_message".to_owned(),
                message_id.map(|id| format!("claude://message/{id}")),
            ),
            "thinking" => (
                "agent.plan",
                "claude.thinking".to_owned(),
                message_id.map(|id| format!("claude://message/{id}")),
            ),
            _ => (
                "agent.event",
                format!("claude.assistant.{content_type}"),
                message_id.map(|id| format!("claude://message/{id}")),
            ),
        };

        drafts.push(EventDraft {
            event_type: event_type.to_owned(),
            action: Some(action),
            resource,
            subject: "claude-code".to_owned(),
            metadata: item_metadata,
        });
    }

    if drafts.is_empty() {
        drafts.push(EventDraft {
            event_type: "agent.event".to_owned(),
            action: Some("claude.assistant".to_owned()),
            resource: message_id.map(|id| format!("claude://message/{id}")),
            subject: "claude-code".to_owned(),
            metadata,
        });
    }

    drafts
}

fn map_claude_user_event(
    context: &ImportContext,
    source_event: &Value,
    metadata: Value,
) -> Vec<EventDraft> {
    let message = source_event.get("message").unwrap_or(&Value::Null);
    let mut drafts = Vec::new();
    for content in message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let content_type = content
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("content");
        let mut item_metadata = metadata.clone();
        insert_string(&mut item_metadata, "content_type", Some(content_type));
        insert_string(
            &mut item_metadata,
            "tool_use_id",
            content.get("tool_use_id").and_then(Value::as_str),
        );
        insert_value(
            &mut item_metadata,
            "is_error",
            content.get("is_error").cloned(),
        );
        if let Some(result) = source_event.get("tool_use_result") {
            insert_string(
                &mut item_metadata,
                "tool_result_type",
                result.get("type").and_then(Value::as_str),
            );
            insert_string(
                &mut item_metadata,
                "file_path",
                result.pointer("/file/filePath").and_then(Value::as_str),
            );
        }
        drafts.push(EventDraft {
            event_type: if content_type == "tool_result" {
                "tool.execute".to_owned()
            } else {
                "agent.event".to_owned()
            },
            action: Some(format!("claude.{content_type}")),
            resource: content
                .get("tool_use_id")
                .and_then(Value::as_str)
                .map(|id| format!("claude://tool/{id}/result"))
                .or_else(|| Some(context.session_uri.clone())),
            subject: "claude-code".to_owned(),
            metadata: item_metadata,
        });
    }

    if drafts.is_empty() {
        drafts.push(EventDraft {
            event_type: "agent.event".to_owned(),
            action: Some("claude.user".to_owned()),
            resource: Some(context.session_uri.clone()),
            subject: "claude-code".to_owned(),
            metadata,
        });
    }

    drafts
}

fn build_imported_event(
    run_id: &str,
    sequence: u64,
    previous_hash: Option<String>,
    draft: EventDraft,
    source_event: &Value,
    key: Option<&LocalKeyFile>,
) -> Result<Value> {
    let payload_digest = canonical_hash(source_event)?;
    let mut metadata = draft.metadata;
    insert_string(&mut metadata, "source_event_digest", Some(&payload_digest));
    let mut event = build_event(
        run_id.to_owned(),
        sequence,
        draft.event_type,
        draft.action,
        draft.resource,
        previous_hash,
        Some(draft.subject),
        Some(metadata),
    )?;
    event["payload_digest"] = Value::String(payload_digest);
    event["event_hash"] = Value::String(event_hash(&event)?);
    if let Some(key) = key {
        sign_value(&mut event, key)?;
    }
    Ok(event)
}

fn base_metadata(provider: Provider, source_event: &Value) -> Value {
    let mut map = Map::new();
    map.insert(
        "provider".to_owned(),
        Value::String(provider.name().to_owned()),
    );
    if let Some(source_type) = source_event.get("type").and_then(Value::as_str) {
        map.insert(
            "source_event_type".to_owned(),
            Value::String(source_type.to_owned()),
        );
    }
    if let Some(subtype) = source_event.get("subtype").and_then(Value::as_str) {
        map.insert(
            "source_subtype".to_owned(),
            Value::String(subtype.to_owned()),
        );
    }
    Value::Object(map)
}

fn insert_string(metadata: &mut Value, key: &str, value: Option<&str>) {
    if let (Value::Object(map), Some(value)) = (metadata, value) {
        map.insert(key.to_owned(), Value::String(value.to_owned()));
    }
}

fn insert_value(metadata: &mut Value, key: &str, value: Option<Value>) {
    if let (Value::Object(map), Some(value)) = (metadata, value)
        && !value.is_null()
    {
        map.insert(key.to_owned(), value);
    }
}

fn item_uri(context: &ImportContext, item: &Value) -> Option<String> {
    item.get("id")
        .and_then(Value::as_str)
        .map(|id| format!("{}/item/{id}", context.session_uri))
}

fn claude_tool_resource(tool_name: &str, input: Option<&Value>) -> Option<String> {
    let input = input?;
    input
        .get("file_path")
        .or_else(|| input.get("path"))
        .or_else(|| input.get("notebook_path"))
        .and_then(Value::as_str)
        .map(|path| format!("file://{path}"))
        .or_else(|| {
            input
                .get("command")
                .map(|_| format!("claude://tool/{tool_name}/command"))
        })
        .or_else(|| Some(format!("claude://tool/{tool_name}")))
}

impl Provider {
    fn name(self) -> &'static str {
        match self {
            Provider::Codex => "codex",
            Provider::Claude => "claude",
        }
    }
}
