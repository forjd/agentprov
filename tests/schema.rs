use serde_json::Value;
use std::fs;

fn validate(schema_path: &str, instance_path: &str) {
    let schema: Value = serde_json::from_str(&fs::read_to_string(schema_path).unwrap()).unwrap();
    let instance: Value =
        serde_json::from_str(&fs::read_to_string(instance_path).unwrap()).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    if let Err(error) = validator.validate(&instance) {
        panic!("{instance_path} failed {schema_path}: {error}");
    }
}

#[test]
fn examples_validate_against_json_schemas() {
    validate("schemas/manifest-v1.schema.json", "examples/manifest.json");
    validate("schemas/run-envelope-v1.schema.json", "examples/run.json");
    validate("schemas/event-v1.schema.json", "examples/event.json");
    validate(
        "schemas/event-v1.schema.json",
        "examples/approval-request-event.json",
    );
    validate(
        "schemas/event-v1.schema.json",
        "examples/approval-grant-event.json",
    );
    validate(
        "schemas/event-v1.schema.json",
        "examples/approval-deny-event.json",
    );
    validate("schemas/policy-v1.schema.json", "examples/policy.json");
}
