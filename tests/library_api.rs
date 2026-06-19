use agentprov::event::{EventInput, build_event_from_input, verify_event_hash};
use agentprov::run_log::{AppendEventInput, append_event_to_run, verify_run_log, write_jsonl};
use serde_json::{Value, json};
use tempfile::tempdir;

#[test]
fn event_input_builds_verifiable_event() {
    let mut input = EventInput::new("run_library_api", 1, "tool.execute");
    input.action = Some("http.get".to_owned());
    input.resource = Some("https://example.com".to_owned());
    input.subject = Some("agent_library_api".to_owned());
    input.metadata = Some(json!({"capture": "digest-only"}));

    let event = build_event_from_input(input).unwrap();

    assert_eq!(event["schema"], "agentprov.dev/event/v1");
    assert_eq!(event["run_id"], "run_library_api");
    assert_eq!(event["sequence"], 1);
    assert_eq!(event["subject"]["id"], "agent_library_api");
    verify_event_hash(&event).unwrap();
}

#[test]
fn append_event_input_appends_and_preserves_chain() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");
    let start = build_event_from_input(EventInput::new("run_library_api", 1, "run.start")).unwrap();
    let start_hash = start["event_hash"].as_str().unwrap().to_owned();
    write_jsonl(&run, &[start]).unwrap();

    let mut input = AppendEventInput::new("permission.check");
    input.action = Some("discord.message.create".to_owned());
    input.resource = Some("discord://guild/123/channel/456".to_owned());
    input.metadata = Some(json!({"decision": "allow"}));

    let appended = append_event_to_run(&run, input).unwrap();

    assert_eq!(appended["sequence"], Value::from(2));
    assert_eq!(appended["previous_event_hash"], start_hash);
    verify_run_log(&run, false).unwrap();
}
