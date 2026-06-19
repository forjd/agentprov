use crate::canonical::{canonical_hash, remove_field};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct EventInput {
    pub run_id: String,
    pub sequence: u64,
    pub event_type: String,
    pub action: Option<String>,
    pub resource: Option<String>,
    pub previous_event_hash: Option<String>,
    pub subject: Option<String>,
    pub metadata: Option<Value>,
    pub payload_digest: Option<String>,
}

impl EventInput {
    pub fn new(run_id: impl Into<String>, sequence: u64, event_type: impl Into<String>) -> Self {
        Self {
            run_id: run_id.into(),
            sequence,
            event_type: event_type.into(),
            action: None,
            resource: None,
            previous_event_hash: None,
            subject: None,
            metadata: None,
            payload_digest: None,
        }
    }
}

pub fn event_hash(value: &Value) -> Result<String> {
    let mut unsigned = value.clone();
    remove_field(&mut unsigned, "event_hash");
    remove_field(&mut unsigned, "signature");
    canonical_hash(&unsigned)
}

pub fn verify_event_hash(value: &Value) -> Result<()> {
    let expected = value
        .get("event_hash")
        .and_then(Value::as_str)
        .context("event_hash must be present and a string")?;
    let actual = event_hash(value)?;
    if expected == actual {
        Ok(())
    } else {
        bail!("event hash mismatch: expected {expected}, actual {actual}")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_event(
    run_id: String,
    sequence: u64,
    event_type: String,
    action: Option<String>,
    resource: Option<String>,
    previous_event_hash: Option<String>,
    subject: Option<String>,
    metadata: Option<Value>,
) -> Result<Value> {
    build_event_from_input(EventInput {
        run_id,
        sequence,
        event_type,
        action,
        resource,
        previous_event_hash,
        subject,
        metadata,
        payload_digest: None,
    })
}

pub fn build_event_from_input(input: EventInput) -> Result<Value> {
    let mut event = json!({
        "schema": "agentprov.dev/event/v1",
        "event_id": format!("evt_{}", Uuid::new_v4().simple()),
        "run_id": input.run_id,
        "sequence": input.sequence,
        "timestamp": Utc::now(),
        "event_type": input.event_type,
        "subject": {"type": "agent", "id": input.subject.unwrap_or_else(|| "agent_01hxexample".to_owned())},
        "action": input.action,
        "resource": input.resource,
        "payload_digest": input.payload_digest,
        "previous_event_hash": input.previous_event_hash,
        "event_hash": null,
        "signature": null,
        "key_id": null,
        "metadata": input.metadata,
    });
    let hash = event_hash(&event)?;
    event["event_hash"] = Value::String(hash);
    Ok(event)
}

#[cfg(test)]
mod tests {
    use crate::canonical::remove_field;
    use serde_json::json;

    #[test]
    fn event_hash_excludes_event_hash_and_signature_fields() {
        let mut event = json!({"event_hash": "ignored", "signature": "ignored", "sequence": 1});
        remove_field(&mut event, "event_hash");
        remove_field(&mut event, "signature");
        assert_eq!(event, json!({"sequence": 1}));
    }
}
