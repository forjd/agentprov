use agentprov::event::event_hash;
use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{Value, json};
use std::fs;
use tempfile::tempdir;

#[test]
fn manifest_example_prints_valid_json() {
    let output = Command::cargo_bin("agentprov")
        .unwrap()
        .args(["manifest", "example"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["schema"], "agentprov.dev/manifest/v1");
    assert_eq!(value["owner"]["id"], "danjdewhurst");
}

#[test]
fn version_flag_works() {
    Command::cargo_bin("agentprov")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("agentprov"));
}

#[test]
fn event_hash_outputs_blake3_digest() {
    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["event", "hash", "examples/event.json"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("blake3:"));
}

#[test]
fn event_verify_accepts_matching_hash() {
    let hash_output = Command::cargo_bin("agentprov")
        .unwrap()
        .args(["event", "hash", "examples/event.json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let hash = String::from_utf8(hash_output).unwrap().trim().to_owned();

    let mut value: Value =
        serde_json::from_str(&fs::read_to_string("examples/event.json").unwrap()).unwrap();
    value["event_hash"] = Value::String(hash);

    let dir = tempdir().unwrap();
    let path = dir.path().join("event.json");
    fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["event", "verify", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok: event hash verifies"));
}

#[test]
fn validate_command_accepts_examples_and_rejects_invalid_records() {
    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["validate", "manifest", "examples/manifest.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok: manifest schema validates"));

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["validate", "run-envelope", "examples/run.json"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "ok: run-envelope schema validates",
        ));

    let dir = tempdir().unwrap();
    let invalid_manifest = dir.path().join("manifest.json");
    let mut value: Value =
        serde_json::from_str(&fs::read_to_string("examples/manifest.json").unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("agent_id");
    fs::write(
        &invalid_manifest,
        serde_json::to_string_pretty(&value).unwrap(),
    )
    .unwrap();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["validate", "manifest", invalid_manifest.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "manifest schema validation failed",
        ));
}

#[test]
fn run_log_can_be_initialised_appended_and_verified() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "event",
            "append",
            "--run",
            run.to_str().unwrap(),
            "--type",
            "permission.check",
            "--action",
            "discord.message.create",
            "--resource",
            "discord://guild/123/channel/456",
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Run verifies"))
        .stdout(predicate::str::contains("Events: 2"));
}

#[test]
fn collector_events_supports_sequence_bounds() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");
    let db = dir.path().join("collector.sqlite");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    for action in ["tool.first", "tool.second"] {
        Command::cargo_bin("agentprov")
            .unwrap()
            .args([
                "event",
                "append",
                "--run",
                run.to_str().unwrap(),
                "--type",
                "tool.execute",
                "--action",
                action,
            ])
            .assert()
            .success();
    }

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "collector",
            "ingest",
            run.to_str().unwrap(),
            "--db",
            db.to_str().unwrap(),
        ])
        .assert()
        .success();

    let run_id = read_jsonl_fixture(&run)[0]["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let output = Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "collector",
            "events",
            &run_id,
            "--db",
            db.to_str().unwrap(),
            "--after-sequence",
            "1",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["count"], 1);
    assert_eq!(value["after_sequence"], 1);
    assert_eq!(value["limit"], 1);
    assert_eq!(value["next_after_sequence"], 2);
    assert_eq!(value["events"][0]["sequence"], 2);
    assert_eq!(value["events"][0]["action"], "tool.first");
}

#[test]
fn run_verify_rejects_tampered_log() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    let content = fs::read_to_string(&run)
        .unwrap()
        .replace("run.start", "run.tampered");
    fs::write(&run, content).unwrap();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("event hash mismatch"));
}

#[test]
fn run_verify_rejects_wrong_event_schema() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut events = read_jsonl_fixture(&run);
    events[0]["schema"] = Value::String("agentprov.dev/event/v0".to_owned());
    events[0]["event_hash"] = Value::String(event_hash(&events[0]).unwrap());
    write_jsonl_fixture(&run, &events);

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("event schema validation failed"));
}

#[test]
fn run_verify_rejects_mixed_run_ids() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "event",
            "append",
            "--run",
            run.to_str().unwrap(),
            "--type",
            "tool.execute",
        ])
        .assert()
        .success();

    let mut events = read_jsonl_fixture(&run);
    events[1]["run_id"] = Value::String("run_different".to_owned());
    events[1]["event_hash"] = Value::String(event_hash(&events[1]).unwrap());
    write_jsonl_fixture(&run, &events);

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("run_id mismatch"));
}

#[test]
fn run_verify_require_signatures_rejects_unsigned_logs() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "verify",
            run.to_str().unwrap(),
            "--require-signatures",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("missing signature"));
}

#[test]
fn key_generation_inspection_public_and_signature_verification_work() {
    let dir = tempdir().unwrap();
    let key = dir.path().join("agentprov.key");
    let signed = dir.path().join("event.signed.json");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["key", "generate", "--out", key.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["key", "public", "--key", key.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("public_key"))
        .stdout(predicate::str::contains("secret_key").not());

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["key", "inspect", "--key", key.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("has_secret_key"));

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "event",
            "sign",
            "examples/event.json",
            "--key",
            key.to_str().unwrap(),
            "--out",
            signed.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["event", "verify-signature", signed.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok: event signature verifies"));

    let tampered = fs::read_to_string(&signed)
        .unwrap()
        .replace("permission.check", "permission.tampered");
    fs::write(&signed, tampered).unwrap();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["event", "verify-signature", signed.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("signed hash mismatch"));
}

#[test]
fn manifest_signature_verification_work() {
    let dir = tempdir().unwrap();
    let key = dir.path().join("agentprov.key");
    let signed = dir.path().join("manifest.signed.json");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["key", "generate", "--out", key.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "manifest",
            "sign",
            "examples/manifest.json",
            "--key",
            key.to_str().unwrap(),
            "--out",
            signed.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["manifest", "verify-signature", signed.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok: manifest signature verifies"));

    let tampered = fs::read_to_string(&signed)
        .unwrap()
        .replace("Researches a topic", "Researches a tampered topic");
    fs::write(&signed, tampered).unwrap();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["manifest", "verify-signature", signed.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("signed hash mismatch"));
}

#[test]
fn run_verify_can_bind_to_manifest_digest() {
    let dir = tempdir().unwrap();
    let key = dir.path().join("agentprov.key");
    let signed_manifest = dir.path().join("manifest.signed.json");
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["key", "generate", "--out", key.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "manifest",
            "sign",
            "examples/manifest.json",
            "--key",
            key.to_str().unwrap(),
            "--out",
            signed_manifest.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            signed_manifest.to_str().unwrap(),
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "verify",
            run.to_str().unwrap(),
            "--manifest",
            signed_manifest.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Manifest: verified"));
}

#[test]
fn run_verify_rejects_manifest_digest_mismatch() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");
    let different_manifest = dir.path().join("manifest.json");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    let mut manifest: Value =
        serde_json::from_str(&fs::read_to_string("examples/manifest.json").unwrap()).unwrap();
    manifest["version"] = Value::String("0.2.0".to_owned());
    fs::write(
        &different_manifest,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "verify",
            run.to_str().unwrap(),
            "--manifest",
            different_manifest.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("manifest digest mismatch"));
}

#[test]
fn signed_append_supports_require_signatures() {
    let dir = tempdir().unwrap();
    let key = dir.path().join("agentprov.key");
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["key", "generate", "--out", key.to_str().unwrap()])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
            "--key",
            key.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "event",
            "append",
            "--run",
            run.to_str().unwrap(),
            "--type",
            "tool.execute",
            "--action",
            "demo",
            "--resource",
            "demo",
            "--key",
            key.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "verify",
            run.to_str().unwrap(),
            "--require-signatures",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Signatures: valid"));
}

#[test]
fn policy_check_returns_allow_decision_and_can_emit_event() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "policy",
            "check",
            "--policy",
            "examples/policy.json",
            "--agent",
            "agent_01hxexample",
            "--action",
            "discord.message.create",
            "--resource",
            "discord://guild/148756/channel/456",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"decision\": \"allow\""));

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "policy",
            "check",
            "--policy",
            "examples/policy.json",
            "--agent",
            "agent_01hxexample",
            "--action",
            "discord.message.create",
            "--resource",
            "discord://guild/148756/channel/456",
            "--emit-event",
            "--run",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Events: 2"));
}

#[test]
fn policy_check_emits_approval_request_when_required() {
    let dir = tempdir().unwrap();
    let run = dir.path().join("run.jsonl");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "init",
            "--agent",
            "examples/manifest.json",
            "--trigger",
            "manual",
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "policy",
            "check",
            "--policy",
            "examples/policy.json",
            "--agent",
            "agent_01hxexample",
            "--action",
            "github.pr.merge",
            "--resource",
            "repo://forjd/agentprov/pull/1",
            "--emit-event",
            "--run",
            run.to_str().unwrap(),
        ])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Events: 3"));

    let events = read_jsonl_fixture(&run);
    assert_eq!(events[1]["event_type"], "permission.check");
    assert_eq!(
        events[1]["metadata"]["decision"],
        Value::String("require_approval".to_owned())
    );
    assert_eq!(events[2]["event_type"], "human.approval.request");
    assert_eq!(
        events[2]["metadata"]["approval_status"],
        Value::String("requested".to_owned())
    );
}

#[test]
fn demo_manual_tool_run_generates_verifiable_run_log() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "demo",
            "manual-tool-run",
            "--out",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Event chain: valid"));

    let run = dir.path().join("run.jsonl");
    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Events: 4"));
}

#[test]
fn export_commands_write_json_files() {
    let dir = tempdir().unwrap();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "demo",
            "manual-tool-run",
            "--out",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let run = dir.path().join("run.jsonl");
    let otel = dir.path().join("otel.json");
    let openinference = dir.path().join("openinference.json");

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "export",
            "otel",
            run.to_str().unwrap(),
            "--out",
            otel.to_str().unwrap(),
        ])
        .assert()
        .success();
    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "export",
            "openinference",
            run.to_str().unwrap(),
            "--out",
            openinference.to_str().unwrap(),
        ])
        .assert()
        .success();

    let otel_json: Value = serde_json::from_str(&fs::read_to_string(otel).unwrap()).unwrap();
    let oi_json: Value = serde_json::from_str(&fs::read_to_string(openinference).unwrap()).unwrap();
    let otel_spans = otel_json["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .unwrap();
    let oi_spans = oi_json["spans"].as_array().unwrap();
    assert_eq!(otel_spans.len(), 4);
    assert_eq!(oi_spans.len(), 4);
    assert_eq!(otel_spans[0]["name"], Value::String("run.start".to_owned()));
    assert_eq!(
        otel_spans[3]["attributes"]["gen_ai.operation.name"],
        Value::String("execute_tool".to_owned())
    );
    assert_eq!(
        oi_spans[1]["attributes"]["openinference.span.kind"],
        Value::String("LLM".to_owned())
    );
    assert_eq!(
        oi_spans[3]["attributes"]["openinference.span.kind"],
        Value::String("TOOL".to_owned())
    );
}

#[test]
fn import_codex_jsonl_writes_verifiable_redacted_run_log() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("codex.jsonl");
    let run = dir.path().join("codex-run.jsonl");
    let key = dir.path().join("agentprov.key");
    write_jsonl_fixture(
        &source,
        &[
            json!({"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a53"}),
            json!({"type":"turn.started"}),
            json!({"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"SECRET_COMMAND","aggregated_output":"SECRET_OUTPUT","exit_code":0,"status":"completed"}}),
            json!({"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"SECRET_MESSAGE"}}),
            json!({"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":2}}),
        ],
    );

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["key", "generate", "--out", key.to_str().unwrap()])
        .assert()
        .success();

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "import",
            "codex",
            source.to_str().unwrap(),
            "--out",
            run.to_str().unwrap(),
            "--key",
            key.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("codex import written"))
        .stdout(predicate::str::contains("AgentProv events: 5"));

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "run",
            "verify",
            run.to_str().unwrap(),
            "--require-signatures",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Events: 5"))
        .stdout(predicate::str::contains("Signatures: valid"));

    let content = fs::read_to_string(run).unwrap();
    assert!(content.contains("codex.thread.started"));
    assert!(content.contains("payload_digest"));
    assert!(!content.contains("SECRET_COMMAND"));
    assert!(!content.contains("SECRET_OUTPUT"));
    assert!(!content.contains("SECRET_MESSAGE"));
}

#[test]
fn import_claude_jsonl_writes_verifiable_redacted_run_log() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("claude.jsonl");
    let run = dir.path().join("claude-run.jsonl");
    write_jsonl_fixture(
        &source,
        &[
            json!({"type":"system","subtype":"init","cwd":"/repo","session_id":"23af37d5-430d-4742-8a9c-62a711435dfd","tools":["Read"],"model":"claude-opus-4-8","permissionMode":"dontAsk","claude_code_version":"2.1.183"}),
            json!({"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour"},"session_id":"23af37d5-430d-4742-8a9c-62a711435dfd"}),
            json!({"type":"assistant","message":{"id":"msg_1","model":"claude-opus-4-8","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"SECRET_COMMAND"}}]},"session_id":"23af37d5-430d-4742-8a9c-62a711435dfd"}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","content":"SECRET_TOOL_RESULT"}]},"tool_use_result":{"type":"text","file":{"filePath":"/repo/Cargo.toml"}},"session_id":"23af37d5-430d-4742-8a9c-62a711435dfd"}),
            json!({"type":"assistant","message":{"id":"msg_2","model":"claude-opus-4-8","content":[{"type":"text","text":"SECRET_FINAL"}]},"session_id":"23af37d5-430d-4742-8a9c-62a711435dfd"}),
            json!({"type":"result","subtype":"success","is_error":false,"duration_ms":10,"num_turns":1,"result":"SECRET_RESULT","session_id":"23af37d5-430d-4742-8a9c-62a711435dfd","usage":{"input_tokens":4,"output_tokens":1}}),
        ],
    );

    Command::cargo_bin("agentprov")
        .unwrap()
        .args([
            "import",
            "claude",
            source.to_str().unwrap(),
            "--out",
            run.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("claude import written"))
        .stdout(predicate::str::contains("AgentProv events: 6"));

    Command::cargo_bin("agentprov")
        .unwrap()
        .args(["run", "verify", run.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Events: 6"));

    let content = fs::read_to_string(run).unwrap();
    assert!(content.contains("claude.session.started"));
    assert!(content.contains("payload_digest"));
    assert!(!content.contains("SECRET_COMMAND"));
    assert!(!content.contains("SECRET_TOOL_RESULT"));
    assert!(!content.contains("SECRET_FINAL"));
    assert!(!content.contains("SECRET_RESULT"));
}

fn write_jsonl_fixture(path: &std::path::Path, values: &[Value]) {
    let content = values
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    fs::write(path, format!("{content}\n")).unwrap();
}

fn read_jsonl_fixture(path: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
