use agentprov::{
    AppendEventInput, EventInput, append_event_to_run, build_event_from_input, verify_run_log,
    write_jsonl,
};
use serde_json::json;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("runs/library-api.jsonl"));

    let mut start = EventInput::new("run_library_api_example", 1, "run.start");
    start.action = Some("trigger.api".to_owned());
    start.resource = Some("example://library-api".to_owned());
    start.subject = Some("agent_01hxexample".to_owned());
    start.metadata = Some(json!({
        "agent": "research-agent",
        "integration": "rust-library",
        "capture": "digest-only"
    }));
    write_jsonl(&out, &[build_event_from_input(start)?])?;

    let mut tool = AppendEventInput::new("tool.execute");
    tool.action = Some("example.lookup".to_owned());
    tool.resource = Some("example://dataset/customer-summary".to_owned());
    tool.subject = Some("agent_01hxexample".to_owned());
    tool.metadata = Some(json!({
        "result_digest": "blake3:example-result",
        "redaction": "payload omitted"
    }));
    append_event_to_run(&out, tool)?;

    verify_run_log(&out, false)?;
    println!("Library API run log written to {}", out.display());
    Ok(())
}
