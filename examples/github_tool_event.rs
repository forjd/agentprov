use agentprov::event::{EventInput, build_event_from_input};
use agentprov::run_log::{AppendEventInput, append_event_to_run, verify_run_log, write_jsonl};
use serde_json::json;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("runs/github-tool-event.jsonl"));

    let mut start = EventInput::new("run_github_tool_example", 1, "run.start");
    start.action = Some("trigger.manual".to_owned());
    start.resource = Some("example://github-tool".to_owned());
    start.subject = Some("agent_01hxexample".to_owned());
    start.metadata = Some(json!({
        "agent": "research-agent",
        "tool": "github"
    }));
    write_jsonl(&out, &[build_event_from_input(start)?])?;

    let mut permission = AppendEventInput::new("permission.check");
    permission.action = Some("github.issue.comment.create".to_owned());
    permission.resource = Some("repo://forjd/agentprov/issues/1".to_owned());
    permission.subject = Some("agent_01hxexample".to_owned());
    permission.metadata = Some(json!({
        "decision": "allow",
        "policy_id": "policy_research_agent",
        "policy_version": "v1"
    }));
    append_event_to_run(&out, permission)?;

    let mut tool = AppendEventInput::new("tool.execute");
    tool.action = Some("github.issue.comment.create".to_owned());
    tool.resource = Some("repo://forjd/agentprov/issues/1".to_owned());
    tool.subject = Some("agent_01hxexample".to_owned());
    tool.metadata = Some(json!({
        "tool": "github",
        "output_digest": "blake3:example-output"
    }));
    append_event_to_run(&out, tool)?;

    verify_run_log(&out, false)?;
    println!("GitHub tool run log written to {}", out.display());
    Ok(())
}
