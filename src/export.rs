use serde_json::{Value, json};

pub fn to_otel_span(event: &Value) -> Value {
    let event_type = event
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or("event");
    json!({
        "traceId": event.get("run_id"),
        "spanId": event.get("event_id"),
        "name": event_type,
        "attributes": {
            "agentprov.event.type": event_type,
            "agentprov.event.hash": event.get("event_hash"),
            "agentprov.event.previous_hash": event.get("previous_event_hash"),
            "agentprov.event.sequence": event.get("sequence"),
            "agentprov.action": event.get("action"),
            "agentprov.resource": event.get("resource"),
            "gen_ai.operation.name": otel_operation_name(event_type),
            "gen_ai.agent.id": event.pointer("/subject/id"),
            "gen_ai.tool.name": event.get("resource"),
        }
    })
}

pub fn to_openinference_span(event: &Value) -> Value {
    let event_type = event
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or("event");
    json!({
        "trace_id": event.get("run_id"),
        "span_id": event.get("event_id"),
        "name": event_type,
        "attributes": {
            "openinference.span.kind": openinference_kind(event_type),
            "input.value": event.get("action"),
            "metadata": event.get("metadata"),
            "agent.name": event.pointer("/subject/id"),
            "tool.name": event.get("resource"),
            "agentprov.event.hash": event.get("event_hash"),
            "agentprov.event.type": event_type,
            "agentprov.action": event.get("action"),
            "agentprov.resource": event.get("resource"),
        }
    })
}

fn otel_operation_name(event_type: &str) -> &str {
    match event_type {
        "run.start" | "agent.invoke" => "invoke_agent",
        "agent.plan" => "plan",
        "tool.execute" => "execute_tool",
        "memory.read" => "search_memory",
        "memory.write" => "update_memory",
        _ => event_type,
    }
}

fn openinference_kind(event_type: &str) -> &str {
    match event_type {
        "llm.call" => "LLM",
        "tool.execute" => "TOOL",
        "prompt.render" => "PROMPT",
        "agent.invoke" | "agent.plan" | "run.start" => "AGENT",
        "retrieval.search" => "RETRIEVER",
        "guardrail.check" => "GUARDRAIL",
        "eval.run" => "EVALUATOR",
        _ => "CHAIN",
    }
}
