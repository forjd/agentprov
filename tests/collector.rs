use agentprov::collector::{
    AppendOptions, CollectorStore, EventListOptions, IngestOptions, RunListOptions,
};
use agentprov::event::{EventInput, build_event_from_input, event_hash};
use agentprov::run_log::{AppendEventInput, append_event_to_run, read_jsonl, write_jsonl};
use agentprov::signing::{generate_key, sign_value};
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

    let exported = dir.path().join("exported.jsonl");
    let exported_count = store
        .export_jsonl_file("run_collector_test", &exported)
        .unwrap();
    assert_eq!(exported_count, 2);
    assert_eq!(read_jsonl(&exported).unwrap(), read_jsonl(&run).unwrap());

    let exported_jsonl = store.export_jsonl_string("run_collector_test").unwrap();
    let exported_events = exported_jsonl
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(exported_events, read_jsonl(&run).unwrap());

    let html = store.dashboard_html().unwrap();
    assert!(html.contains("AgentProv Collector"));
    assert!(html.contains("run_collector_test"));
    assert!(html.contains("tool.execute"));
    assert!(html.contains("http.get"));
}

#[test]
fn collector_rejects_invalid_ingest_chains() {
    let mut store = CollectorStore::open_memory().unwrap();

    let start =
        build_event_from_input(EventInput::new("run_invalid_ingest", 1, "run.start")).unwrap();
    let mut next = EventInput::new("run_invalid_ingest", 2, "tool.execute");
    next.previous_event_hash = Some("blake3:not-the-previous-event".to_owned());
    let next = build_event_from_input(next).unwrap();

    let error = store
        .ingest_events("test", &[start, next])
        .unwrap_err()
        .to_string();
    assert!(error.contains("previous_event_hash mismatch"));

    let missing = store
        .run_events("run_invalid_ingest")
        .unwrap_err()
        .to_string();
    assert!(missing.contains("run not found: run_invalid_ingest"));
}

#[test]
fn collector_ingest_can_require_signatures() {
    let mut store = CollectorStore::open_memory().unwrap();

    let unsigned =
        build_event_from_input(EventInput::new("run_unsigned_ingest", 1, "run.start")).unwrap();
    let error = store
        .ingest_events_with_options(
            "test",
            &[unsigned],
            IngestOptions {
                require_signatures: true,
            },
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("missing signature at sequence 1"));

    let key = generate_key();
    let mut start =
        build_event_from_input(EventInput::new("run_signed_ingest", 1, "run.start")).unwrap();
    sign_value(&mut start, &key).unwrap();
    let mut next = EventInput::new("run_signed_ingest", 2, "tool.execute");
    next.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
    let mut next = build_event_from_input(next).unwrap();
    sign_value(&mut next, &key).unwrap();

    let run_id = store
        .ingest_events_with_options(
            "test",
            &[start, next],
            IngestOptions {
                require_signatures: true,
            },
        )
        .unwrap();
    assert_eq!(run_id, "run_signed_ingest");
    assert_eq!(
        store.verify_run("run_signed_ingest", true).unwrap()["events"],
        2
    );
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
fn collector_append_can_require_signatures() {
    let mut store = CollectorStore::open_memory().unwrap();
    let key = generate_key();

    let mut start =
        build_event_from_input(EventInput::new("run_signed_stream", 1, "run.start")).unwrap();
    sign_value(&mut start, &key).unwrap();
    store
        .append_event_with_options(
            "test-stream",
            "run_signed_stream",
            start.clone(),
            AppendOptions {
                require_signatures: true,
            },
        )
        .unwrap();

    let mut unsigned = EventInput::new("run_signed_stream", 2, "tool.execute");
    unsigned.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
    let unsigned = build_event_from_input(unsigned).unwrap();
    let error = store
        .append_event_with_options(
            "test-stream",
            "run_signed_stream",
            unsigned,
            AppendOptions {
                require_signatures: true,
            },
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("missing signature at sequence 2"));
    assert_eq!(store.run_events("run_signed_stream").unwrap().len(), 1);

    let mut signed = EventInput::new("run_signed_stream", 2, "tool.execute");
    signed.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
    let mut signed = build_event_from_input(signed).unwrap();
    sign_value(&mut signed, &key).unwrap();
    store
        .append_event_with_options(
            "test-stream",
            "run_signed_stream",
            signed,
            AppendOptions {
                require_signatures: true,
            },
        )
        .unwrap();
    assert_eq!(
        store.verify_run("run_signed_stream", true).unwrap()["events"],
        2
    );
}

#[test]
fn collector_lists_bounded_run_pages() {
    let mut store = CollectorStore::open_memory().unwrap();

    for run_id in ["run_page_one", "run_page_two", "run_page_three"] {
        let start = build_event_from_input(EventInput::new(run_id, 1, "run.start")).unwrap();
        store.ingest_events("test", &[start]).unwrap();
    }

    let page = store
        .list_runs_json(RunListOptions { limit: Some(2) })
        .unwrap();
    assert_eq!(page["count"], 2);
    assert_eq!(page["limit"], 2);
    assert_eq!(page["has_more"], true);
    assert_eq!(page["runs"].as_array().unwrap().len(), 2);

    let all = store.list_runs().unwrap();
    assert_eq!(all["count"], 3);
    assert_eq!(all["has_more"], false);
}

#[test]
fn collector_lists_bounded_event_pages() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");
    let db = dir.path().join("collector.sqlite");

    let mut start = EventInput::new("run_page_test", 1, "run.start");
    start.action = Some("trigger.manual".to_owned());
    write_jsonl(&run, &[build_event_from_input(start).unwrap()]).unwrap();

    for (event_type, action) in [
        ("tool.execute", "tool.first"),
        ("permission.check", "policy.check"),
        ("tool.execute", "tool.second"),
    ] {
        let mut append = AppendEventInput::new(event_type);
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
                event_type: None,
            },
        )
        .unwrap();

    assert_eq!(page["count"], 2);
    assert_eq!(page["after_sequence"], 1);
    assert_eq!(page["limit"], 2);
    assert_eq!(page["has_more"], true);
    assert_eq!(page["next_after_sequence"], 3);
    assert_eq!(page["events"][0]["sequence"], 2);
    assert_eq!(page["events"][0]["action"], "tool.first");
    assert_eq!(page["events"][1]["sequence"], 3);
    assert_eq!(page["events"][1]["action"], "policy.check");

    let filtered_page = store
        .run_events_json(
            "run_page_test",
            EventListOptions {
                after_sequence: Some(1),
                limit: Some(2),
                event_type: Some("tool.execute".to_owned()),
            },
        )
        .unwrap();

    assert_eq!(filtered_page["count"], 2);
    assert_eq!(filtered_page["event_type"], "tool.execute");
    assert_eq!(filtered_page["has_more"], false);
    assert_eq!(filtered_page["next_after_sequence"], 4);
    assert_eq!(filtered_page["events"][0]["sequence"], 2);
    assert_eq!(filtered_page["events"][1]["sequence"], 4);

    let empty_page = store
        .run_events_json(
            "run_page_test",
            EventListOptions {
                after_sequence: Some(4),
                limit: Some(2),
                event_type: None,
            },
        )
        .unwrap();
    assert_eq!(empty_page["count"], 0);
    assert_eq!(empty_page["has_more"], false);
    assert!(empty_page["events"].as_array().unwrap().is_empty());
    assert!(empty_page["next_after_sequence"].is_null());
}
