use agentprov::collector::CollectorStore;
use agentprov::event::{EventInput, build_event_from_input};
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
