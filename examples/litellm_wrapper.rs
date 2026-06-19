use agentprov::event::{EventInput, build_event_from_input};
use agentprov::run_log::{AppendEventInput, append_event_to_run, verify_run_log, write_jsonl};
use serde_json::json;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("runs/litellm-wrapper.jsonl"));

    let mut start = EventInput::new("run_litellm_wrapper_example", 1, "run.start");
    start.action = Some("trigger.api".to_owned());
    start.resource = Some("example://litellm-wrapper".to_owned());
    start.subject = Some("agent_01hxexample".to_owned());
    start.metadata = Some(json!({
        "agent": "research-agent",
        "gateway": "litellm",
        "capture": "digest-only"
    }));
    write_jsonl(&out, &[build_event_from_input(start)?])?;

    let mut llm_call = AppendEventInput::new("llm.call");
    llm_call.action = Some("litellm.completion".to_owned());
    llm_call.resource = Some("litellm://model/example".to_owned());
    llm_call.subject = Some("agent_01hxexample".to_owned());
    llm_call.metadata = Some(json!({
        "gateway": "litellm",
        "provider": "example-provider",
        "model": "example",
        "prompt_digest": "blake3:example-prompt",
        "response_digest": "blake3:example-response"
    }));
    append_event_to_run(&out, llm_call)?;

    verify_run_log(&out, false)?;
    println!("LiteLLM-style run log written to {}", out.display());
    Ok(())
}
