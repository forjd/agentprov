use agentprov::collector::{CollectorStore, EventListOptions};
use agentprov::event::{EventInput, build_event_from_input, event_hash};
use agentprov::run_log::{AppendEventInput, append_event_to_run, write_jsonl};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn collector_ingests_lists_reads_and_verifies_run() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");
    let db = dir.path().join("collector.sqlite");

    let mut start = EventInput::new("run_collector_test", 1, "run.start");
    start.action = Some("trigger.manual".to_owned());
    start.metadata = Some(json!({"agent": "collector-test"}));
    write_jsonl(&run, &[build_event_from_input(start).unwrap()]).unwrap();

    let mut append = AppendEventInput::new("tool.execute");
    append.action = Some("http.get".to_owned());
    append.resource = Some("https://example.com".to_owned());
    append.metadata = Some(json!({"capture": "digest-only"}));
    append_event_to_run(&run, append).unwrap();

    let mut store = CollectorStore::open(&db).unwrap();
    let run_id = store.ingest_jsonl_file(&run).unwrap();
    assert_eq!(run_id, "run_collector_test");

    let runs = store.list_runs().unwrap();
    assert_eq!(runs["runs"][0]["run_id"], "run_collector_test");

    let events = store.run_events("run_collector_test").unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["event_type"], "tool.execute");

    let report = store.verify_run("run_collector_test", false).unwrap();
    assert_eq!(report["verifies"], true);
    assert_eq!(report["events"], 2);

    let html = store.dashboard_html().unwrap();
    assert!(html.contains("AgentProv Collector"));
    assert!(html.contains("run_collector_test"));
    assert!(html.contains("tool.execute"));
    assert!(html.contains("http.get"));
}

#[test]
fn collector_appends_streamed_events_and_rejects_invalid_links() {
    let mut store = CollectorStore::open_memory().unwrap();

    let mut start = EventInput::new("run_streamed_test", 1, "run.start");
    start.action = Some("trigger.manual".to_owned());
    let start = build_event_from_input(start).unwrap();

    let report = store
        .append_event("test-stream", "run_streamed_test", start.clone())
        .unwrap();
    assert_eq!(report["run_id"], "run_streamed_test");
    assert_eq!(report["sequence"], 1);

    let mut next = EventInput::new("run_streamed_test", 2, "tool.execute");
    next.action = Some("github.issue.read".to_owned());
    next.resource = Some("github://forjd/agentprov/issues/1".to_owned());
    next.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
    let next = build_event_from_input(next).unwrap();

    store
        .append_event("test-stream", "run_streamed_test", next)
        .unwrap();

    let events = store.run_events("run_streamed_test").unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[1]["event_type"], "tool.execute");

    let mut invalid = EventInput::new("run_streamed_test", 3, "tool.execute");
    invalid.action = Some("github.issue.comment".to_owned());
    invalid.previous_event_hash = Some("blake3:not-the-previous-event".to_owned());
    let invalid = build_event_from_input(invalid).unwrap();

    let error = store
        .append_event("test-stream", "run_streamed_test", invalid)
        .unwrap_err()
        .to_string();
    assert!(error.contains("previous_event_hash mismatch"));
    assert_eq!(store.run_events("run_streamed_test").unwrap().len(), 2);

    let mut wrong_run = events[0].clone();
    wrong_run["run_id"] = json!("run_other");
    wrong_run["event_hash"] = json!(event_hash(&wrong_run).unwrap());
    let error = store
        .append_event("test-stream", "run_streamed_test", wrong_run)
        .unwrap_err()
        .to_string();
    assert!(error.contains("does not match target run"));
}

#[test]
fn collector_lists_bounded_event_pages() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");
    let db = dir.path().join("collector.sqlite");

    let mut start = EventInput::new("run_page_test", 1, "run.start");
    start.action = Some("trigger.manual".to_owned());
    write_jsonl(&run, &[build_event_from_input(start).unwrap()]).unwrap();

    for action in ["tool.first", "tool.second", "tool.third"] {
        let mut append = AppendEventInput::new("tool.execute");
        append.action = Some(action.to_owned());
        append_event_to_run(&run, append).unwrap();
    }

    let mut store = CollectorStore::open(&db).unwrap();
    store.ingest_jsonl_file(&run).unwrap();

    let page = store
        .run_events_json(
            "run_page_test",
            EventListOptions {
                after_sequence: Some(1),
                limit: Some(2),
            },
        )
        .unwrap();

    assert_eq!(page["count"], 2);
    assert_eq!(page["after_sequence"], 1);
    assert_eq!(page["limit"], 2);
    assert_eq!(page["next_after_sequence"], 3);
    assert_eq!(page["events"][0]["sequence"], 2);
    assert_eq!(page["events"][0]["action"], "tool.first");
    assert_eq!(page["events"][1]["sequence"], 3);
    assert_eq!(page["events"][1]["action"], "tool.second");

    let empty_page = store
        .run_events_json(
            "run_page_test",
            EventListOptions {
                after_sequence: Some(4),
                limit: Some(2),
            },
        )
        .unwrap();
    assert_eq!(empty_page["count"], 0);
    assert!(empty_page["events"].as_array().unwrap().is_empty());
    assert!(empty_page["next_after_sequence"].is_null());
}
