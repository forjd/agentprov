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
    assert!(otel_json.get("resourceSpans").is_some());
    assert!(oi_json.get("spans").is_some());
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
            json!({"type":"item.completed","item":{"id":"item_1","type":"command_execution","command":"bash -lc ls","aggregated_output":"SECRET_OUTPUT","exit_code":0,"status":"completed"}}),
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
            json!({"type":"assistant","message":{"id":"msg_1","model":"claude-opus-4-8","content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"/repo/Cargo.toml"}}]},"session_id":"23af37d5-430d-4742-8a9c-62a711435dfd"}),
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
