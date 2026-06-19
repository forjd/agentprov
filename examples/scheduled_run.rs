use agentprov::event::{EventInput, build_event_from_input};
use agentprov::run_log::{AppendEventInput, append_event_to_run, verify_run_log, write_jsonl};
use serde_json::json;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("runs/scheduled-run.jsonl"));

    let mut start = EventInput::new("run_scheduled_example", 1, "run.start");
    start.action = Some("trigger.scheduled".to_owned());
    start.resource = Some("schedule://nightly/research-summary".to_owned());
    start.subject = Some("agent_01hxexample".to_owned());
    start.metadata = Some(json!({
        "agent": "research-agent",
        "trigger_type": "scheduled",
        "schedule_id": "nightly-research-summary"
    }));
    write_jsonl(&out, &[build_event_from_input(start)?])?;

    let mut invoke = AppendEventInput::new("agent.invoke");
    invoke.action = Some("schedule.run".to_owned());
    invoke.resource = Some("schedule://nightly/research-summary".to_owned());
    invoke.subject = Some("agent_01hxexample".to_owned());
    invoke.metadata = Some(json!({
        "prompt_digest": "blake3:example-scheduled-prompt",
        "capture": "digest-only"
    }));
    append_event_to_run(&out, invoke)?;

    verify_run_log(&out, false)?;
    println!("Scheduled run log written to {}", out.display());
    Ok(())
}
