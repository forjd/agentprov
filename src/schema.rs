use anyhow::{Context, Result, bail};
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub enum SchemaKind {
    Manifest,
    RunEnvelope,
    Event,
    Policy,
}

impl SchemaKind {
    pub fn name(self) -> &'static str {
        match self {
            SchemaKind::Manifest => "manifest",
            SchemaKind::RunEnvelope => "run-envelope",
            SchemaKind::Event => "event",
            SchemaKind::Policy => "policy",
        }
    }

    fn schema_json(self) -> &'static str {
        match self {
            SchemaKind::Manifest => include_str!("../schemas/manifest-v1.schema.json"),
            SchemaKind::RunEnvelope => include_str!("../schemas/run-envelope-v1.schema.json"),
            SchemaKind::Event => include_str!("../schemas/event-v1.schema.json"),
            SchemaKind::Policy => include_str!("../schemas/policy-v1.schema.json"),
        }
    }
}

pub fn validate_value(kind: SchemaKind, value: &Value) -> Result<()> {
    let schema: Value =
        serde_json::from_str(kind.schema_json()).context("parse embedded JSON Schema")?;
    let validator = jsonschema::validator_for(&schema).context("compile embedded JSON Schema")?;
    if let Err(error) = validator.validate(value) {
        bail!("{} schema validation failed: {error}", kind.name());
    }
    Ok(())
}
