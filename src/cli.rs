use crate::collector::{CollectorStore, serve};
use crate::event::{build_event, event_hash, verify_event_hash};
use crate::export::{to_openinference_span, to_otel_span};
use crate::integrations::{import_claude_jsonl, import_codex_jsonl};
use crate::policy::policy_decision;
use crate::run_log::{
    AppendEventInput, append_jsonl, next_event_for_run as build_next_event_for_run, read_jsonl,
    verify_run_log, write_jsonl,
};
use crate::schema::{SchemaKind, validate_value};
use crate::signing::{
    generate_key, inspect_key_view, public_key_view, read_key, sign_value, signed_payload_hash,
    verify_signature,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "agentprov")]
#[command(version)]
#[command(about = "MVP identity and provenance primitives for AI agent runs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Manifest {
        #[command(subcommand)]
        command: ManifestCommand,
    },
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
    Event {
        #[command(subcommand)]
        command: EventCommand,
    },
    Key {
        #[command(subcommand)]
        command: KeyCommand,
    },
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
    },
    Demo {
        #[command(subcommand)]
        command: DemoCommand,
    },
    Export {
        #[command(subcommand)]
        command: ExportCommand,
    },
    Import {
        #[command(subcommand)]
        command: ImportCommand,
    },
    Validate {
        #[arg(value_enum)]
        kind: ValidateKind,
        file: PathBuf,
    },
    Collector {
        #[command(subcommand)]
        command: CollectorCommand,
    },
}

#[derive(Subcommand, Debug)]
enum ManifestCommand {
    Example,
    Hash {
        file: PathBuf,
    },
    Sign {
        file: PathBuf,
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    VerifySignature {
        file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum RunCommand {
    Example,
    Init {
        #[arg(long)]
        agent: PathBuf,
        #[arg(long, value_enum)]
        trigger: TriggerType,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        key: Option<PathBuf>,
    },
    Verify {
        file: PathBuf,
        #[arg(long)]
        require_signatures: bool,
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum EventCommand {
    Hash {
        file: PathBuf,
    },
    Verify {
        file: PathBuf,
    },
    Append {
        #[arg(long)]
        run: PathBuf,
        #[arg(long = "type")]
        event_type: String,
        #[arg(long)]
        action: Option<String>,
        #[arg(long)]
        resource: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        key: Option<PathBuf>,
    },
    Sign {
        file: PathBuf,
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    VerifySignature {
        file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum KeyCommand {
    Generate {
        #[arg(long)]
        out: PathBuf,
    },
    Public {
        #[arg(long)]
        key: PathBuf,
    },
    Inspect {
        #[arg(long)]
        key: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum PolicyCommand {
    Check {
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        agent: String,
        #[arg(long)]
        action: String,
        #[arg(long)]
        resource: String,
        #[arg(long)]
        emit_event: bool,
        #[arg(long)]
        run: Option<PathBuf>,
        #[arg(long)]
        key: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum DemoCommand {
    ManualToolRun {
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum ExportCommand {
    Otel {
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Openinference {
        file: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum ImportCommand {
    #[command(about = "Import Codex `codex exec --json` JSONL into an AgentProv run log")]
    Codex {
        #[arg(value_name = "FILE", help = "Codex JSONL file, or '-' for stdin")]
        file: PathBuf,
        #[arg(long, help = "Output AgentProv run log path")]
        out: PathBuf,
        #[arg(
            long,
            help = "Optional local AgentProv key used to sign imported events"
        )]
        key: Option<PathBuf>,
    },
    #[command(
        about = "Import Claude Code `--output-format stream-json` JSONL into an AgentProv run log"
    )]
    Claude {
        #[arg(value_name = "FILE", help = "Claude Code JSONL file, or '-' for stdin")]
        file: PathBuf,
        #[arg(long, help = "Output AgentProv run log path")]
        out: PathBuf,
        #[arg(
            long,
            help = "Optional local AgentProv key used to sign imported events"
        )]
        key: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum CollectorCommand {
    Ingest {
        file: PathBuf,
        #[arg(long)]
        db: PathBuf,
    },
    Runs {
        #[arg(long)]
        db: PathBuf,
    },
    Events {
        run_id: String,
        #[arg(long)]
        db: PathBuf,
    },
    Verify {
        run_id: String,
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        require_signatures: bool,
    },
    Ui {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Serve {
        #[arg(long, default_value = "127.0.0.1:8787")]
        addr: String,
        #[arg(long)]
        db: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ValidateKind {
    Manifest,
    RunEnvelope,
    Event,
    Policy,
}

impl From<ValidateKind> for SchemaKind {
    fn from(value: ValidateKind) -> Self {
        match value {
            ValidateKind::Manifest => SchemaKind::Manifest,
            ValidateKind::RunEnvelope => SchemaKind::RunEnvelope,
            ValidateKind::Event => SchemaKind::Event,
            ValidateKind::Policy => SchemaKind::Policy,
        }
    }
}

#[derive(Clone, Debug, ValueEnum)]
enum TriggerType {
    Manual,
    Scheduled,
    Webhook,
    Api,
    Ci,
    Delegated,
}
impl std::fmt::Display for TriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            TriggerType::Manual => "manual",
            TriggerType::Scheduled => "scheduled",
            TriggerType::Webhook => "webhook",
            TriggerType::Api => "api",
            TriggerType::Ci => "ci",
            TriggerType::Delegated => "delegated",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Owner {
    #[serde(rename = "type")]
    owner_type: String,
    id: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct Source {
    repo: String,
    commit: String,
    image_digest: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct RuntimeManifest {
    #[serde(rename = "type")]
    runtime_type: String,
    environment: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct PolicyRef {
    id: String,
    version: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct AgentManifest {
    schema: String,
    agent_id: String,
    name: String,
    description: String,
    version: String,
    owner: Owner,
    source: Source,
    runtime: RuntimeManifest,
    capabilities: Vec<String>,
    policy: PolicyRef,
    public_key: Option<Value>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Trigger {
    #[serde(rename = "type")]
    trigger_type: String,
    id: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct RunAgentRef {
    agent_id: String,
    version: String,
    manifest_digest: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct Actor {
    #[serde(rename = "type")]
    actor_type: String,
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_method: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct RuntimeRun {
    host: String,
    os: String,
    environment: String,
    container_image_digest: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Authority {
    capabilities: Vec<String>,
    policy_id: String,
    policy_version: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct RunEnvelope {
    schema: String,
    run_id: String,
    trace_id: String,
    parent_run_id: Option<String>,
    trigger: Trigger,
    agent: RunAgentRef,
    actor_chain: Vec<Actor>,
    runtime: RuntimeRun,
    authority: Authority,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    status: String,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Manifest { command } => handle_manifest(command),
        Commands::Run { command } => handle_run(command),
        Commands::Event { command } => handle_event(command),
        Commands::Key { command } => handle_key(command),
        Commands::Policy { command } => handle_policy(command),
        Commands::Demo { command } => handle_demo(command),
        Commands::Export { command } => handle_export(command),
        Commands::Import { command } => handle_import(command),
        Commands::Validate { kind, file } => handle_validate(kind, file),
        Commands::Collector { command } => handle_collector(command),
    }
}

fn handle_manifest(command: ManifestCommand) -> Result<()> {
    match command {
        ManifestCommand::Example => print_json(&example_manifest()?),
        ManifestCommand::Hash { file } => {
            println!("{}", signed_payload_hash(&read_json_file(&file)?)?);
            Ok(())
        }
        ManifestCommand::Sign { file, key, out } => {
            let mut value = read_json_file(&file)?;
            validate_value(SchemaKind::Manifest, &value)?;
            sign_value(&mut value, &read_key(&key)?)?;
            write_pretty_json(&out, &value)?;
            println!("signed manifest written to {}", out.display());
            Ok(())
        }
        ManifestCommand::VerifySignature { file } => {
            let value = read_json_file(&file)?;
            validate_value(SchemaKind::Manifest, &value)?;
            verify_signature(&value)?;
            println!("ok: manifest signature verifies");
            Ok(())
        }
    }
}

fn handle_run(command: RunCommand) -> Result<()> {
    match command {
        RunCommand::Example => print_json(&example_run()?),
        RunCommand::Init {
            agent,
            trigger,
            out,
            key,
        } => {
            let manifest = read_json_file(&agent)?;
            validate_value(SchemaKind::Manifest, &manifest)?;
            if manifest_signature(&manifest).is_some() {
                verify_signature(&manifest)?;
            }
            let mut metadata = json!({
                "agent_manifest_digest": signed_payload_hash(&manifest)?,
                "trigger_type": trigger.to_string(),
            });
            if let Some(key_id) = manifest_key_id(&manifest) {
                metadata["agent_manifest_key_id"] = Value::String(key_id.to_owned());
            }
            let run_id = format!("run_{}", Uuid::new_v4().simple());
            let mut event = build_event(
                run_id,
                1,
                "run.start".to_owned(),
                Some(format!("trigger.{trigger}")),
                Some(agent.display().to_string()),
                None,
                None,
                Some(metadata),
            )?;
            if let Some(key_path) = key {
                sign_value(&mut event, &read_key(&key_path)?)?;
            }
            write_jsonl(&out, &[event])?;
            println!("run log initialised at {}", out.display());
            Ok(())
        }
        RunCommand::Verify {
            file,
            require_signatures,
            manifest,
        } => {
            let report = verify_run_log(&file, require_signatures)?;
            if let Some(manifest_path) = &manifest {
                verify_run_manifest_binding(&file, manifest_path)?;
            }
            println!("Run verifies");
            println!("Events: {}", report.events);
            println!("Event chain: valid");
            if manifest.is_some() {
                println!("Manifest: verified");
            }
            println!(
                "Signatures: {}",
                if report.signatures_present {
                    "valid"
                } else {
                    "not present"
                }
            );
            Ok(())
        }
    }
}

fn handle_event(command: EventCommand) -> Result<()> {
    match command {
        EventCommand::Hash { file } => {
            println!("{}", event_hash(&read_json_file(&file)?)?);
            Ok(())
        }
        EventCommand::Verify { file } => {
            verify_event_hash(&read_json_file(&file)?)?;
            println!("ok: event hash verifies");
            Ok(())
        }
        EventCommand::Append {
            run,
            event_type,
            action,
            resource,
            subject,
            key,
        } => {
            let mut event = next_event_for_run(&run, event_type, action, resource, subject, None)?;
            if let Some(key_path) = key {
                sign_value(&mut event, &read_key(&key_path)?)?;
            }
            append_jsonl(&run, &event)?;
            println!("event appended to {}", run.display());
            Ok(())
        }
        EventCommand::Sign { file, key, out } => {
            let mut value = read_json_file(&file)?;
            sign_value(&mut value, &read_key(&key)?)?;
            write_pretty_json(&out, &value)?;
            println!("signed event written to {}", out.display());
            Ok(())
        }
        EventCommand::VerifySignature { file } => {
            verify_signature(&read_json_file(&file)?)?;
            println!("ok: event signature verifies");
            Ok(())
        }
    }
}

fn handle_key(command: KeyCommand) -> Result<()> {
    match command {
        KeyCommand::Generate { out } => {
            let value = serde_json::to_value(generate_key())?;
            write_pretty_json(&out, &value)?;
            println!("key written to {}", out.display());
            Ok(())
        }
        KeyCommand::Public { key } => print_json(&public_key_view(&read_key(&key)?)),
        KeyCommand::Inspect { key } => print_json(&inspect_key_view(&read_key(&key)?)),
    }
}

fn handle_policy(command: PolicyCommand) -> Result<()> {
    match command {
        PolicyCommand::Check {
            policy,
            agent,
            action,
            resource,
            emit_event,
            run,
            key,
        } => {
            let policy = read_json_file(&policy)?;
            validate_value(SchemaKind::Policy, &policy)?;
            let decision = policy_decision(&policy, &agent, &action, &resource);
            if emit_event {
                let run = run.context("--run is required when --emit-event is used")?;
                let signing_key = key.as_deref().map(read_key).transpose()?;
                let mut event = next_event_for_run(
                    &run,
                    "permission.check".to_owned(),
                    Some(action.clone()),
                    Some(resource.clone()),
                    Some(agent.clone()),
                    Some(decision.clone()),
                )?;
                if let Some(key) = &signing_key {
                    sign_value(&mut event, key)?;
                }
                append_jsonl(&run, &event)?;
                if decision.get("decision").and_then(Value::as_str) == Some("require_approval") {
                    let mut approval_event = next_event_for_run(
                        &run,
                        "human.approval.request".to_owned(),
                        Some(action),
                        Some(resource),
                        Some(agent),
                        Some(json!({
                            "permission_decision": decision,
                            "approval_status": "requested",
                        })),
                    )?;
                    if let Some(key) = &signing_key {
                        sign_value(&mut approval_event, key)?;
                    }
                    append_jsonl(&run, &approval_event)?;
                }
                println!("permission decision event appended to {}", run.display());
            } else {
                print_json(&decision)?;
            }
            Ok(())
        }
    }
}

fn handle_demo(command: DemoCommand) -> Result<()> {
    match command {
        DemoCommand::ManualToolRun { out } => {
            fs::create_dir_all(&out).with_context(|| format!("create {}", out.display()))?;
            let run = out.join("run.jsonl");
            let run_id = "run_demo_manual_tool".to_owned();
            let mut events = Vec::new();
            events.push(build_event(run_id.clone(), 1, "run.start".to_owned(), Some("trigger.manual".to_owned()), Some("demo".to_owned()), None, Some("research-agent".to_owned()), Some(json!({"agent":"research-agent","agent_version":"0.1.0","actor_chain":["danjdewhurst","hermes","research-agent"]})))?);
            let previous = event_hash(events.last().unwrap())?;
            events.push(build_event(
                run_id.clone(),
                2,
                "llm.call".to_owned(),
                Some("model.invoke".to_owned()),
                Some("openai:gpt-example".to_owned()),
                Some(previous),
                Some("research-agent".to_owned()),
                Some(json!({"prompt_digest":"blake3:demo","capture":"digest-only"})),
            )?);
            let previous = event_hash(events.last().unwrap())?;
            events.push(build_event(
                run_id.clone(),
                3,
                "permission.check".to_owned(),
                Some("discord.message.create".to_owned()),
                Some("discord://guild/demo/channel/demo".to_owned()),
                Some(previous),
                Some("research-agent".to_owned()),
                Some(json!({"decision":"allow","policy_id":"policy_demo"})),
            )?);
            let previous = event_hash(events.last().unwrap())?;
            events.push(build_event(
                run_id,
                4,
                "tool.execute".to_owned(),
                Some("discord.message.create".to_owned()),
                Some("discord://guild/demo/channel/demo".to_owned()),
                Some(previous),
                Some("research-agent".to_owned()),
                Some(json!({"tool":"discord.send_message","output_digest":"blake3:demo-output"})),
            )?);
            write_jsonl(&run, &events)?;
            println!("demo written to {}", out.display());
            println!("Run verifies");
            println!("Agent: research-agent v0.1.0");
            println!("Trigger: manual");
            println!("Actor chain: danjdewhurst -> hermes -> research-agent");
            println!("Events: 4");
            println!("Permission checks: 1 allowed");
            println!("Tool calls: 1");
            println!("Event chain: valid");
            println!("Signatures: not present");
            Ok(())
        }
    }
}

fn handle_export(command: ExportCommand) -> Result<()> {
    match command {
        ExportCommand::Otel { file, out } => {
            let events = read_jsonl(&file)?;
            let spans: Vec<Value> = events.iter().map(to_otel_span).collect();
            write_pretty_json(
                &out,
                &json!({"resourceSpans":[{"scopeSpans":[{"spans":spans}]}]}),
            )?;
            println!("OTel-shaped export written to {}", out.display());
            Ok(())
        }
        ExportCommand::Openinference { file, out } => {
            let events = read_jsonl(&file)?;
            let spans: Vec<Value> = events.iter().map(to_openinference_span).collect();
            write_pretty_json(&out, &json!({"spans":spans}))?;
            println!("OpenInference-shaped export written to {}", out.display());
            Ok(())
        }
    }
}

fn handle_import(command: ImportCommand) -> Result<()> {
    match command {
        ImportCommand::Codex { file, out, key } => {
            let key = key.as_deref().map(read_key).transpose()?;
            let report = import_codex_jsonl(&file, &out, key.as_ref())?;
            print_import_report(&out, &report);
            Ok(())
        }
        ImportCommand::Claude { file, out, key } => {
            let key = key.as_deref().map(read_key).transpose()?;
            let report = import_claude_jsonl(&file, &out, key.as_ref())?;
            print_import_report(&out, &report);
            Ok(())
        }
    }
}

fn handle_validate(kind: ValidateKind, file: PathBuf) -> Result<()> {
    let schema_kind = SchemaKind::from(kind);
    validate_value(schema_kind, &read_json_file(&file)?)?;
    println!("ok: {} schema validates", schema_kind.name());
    Ok(())
}

fn handle_collector(command: CollectorCommand) -> Result<()> {
    match command {
        CollectorCommand::Ingest { file, db } => {
            let mut store = CollectorStore::open(&db)?;
            let run_id = store.ingest_jsonl_file(&file)?;
            println!("ingested run {run_id}");
            Ok(())
        }
        CollectorCommand::Runs { db } => {
            let store = CollectorStore::open(&db)?;
            print_json(&store.list_runs()?)
        }
        CollectorCommand::Events { run_id, db } => {
            let store = CollectorStore::open(&db)?;
            print_json(&store.run_events_json(&run_id)?)
        }
        CollectorCommand::Verify {
            run_id,
            db,
            require_signatures,
        } => {
            let store = CollectorStore::open(&db)?;
            print_json(&store.verify_run(&run_id, require_signatures)?)
        }
        CollectorCommand::Ui { db, out } => {
            let store = CollectorStore::open(&db)?;
            fs::write(&out, store.dashboard_html()?)
                .with_context(|| format!("write {}", out.display()))?;
            println!("collector UI written to {}", out.display());
            Ok(())
        }
        CollectorCommand::Serve { addr, db } => serve(&addr, &db),
    }
}

fn verify_run_manifest_binding(run: &Path, manifest_path: &Path) -> Result<()> {
    let manifest = read_json_file(manifest_path)?;
    validate_value(SchemaKind::Manifest, &manifest)?;
    if manifest_signature(&manifest).is_some() {
        verify_signature(&manifest)?;
    }

    let expected_digest = signed_payload_hash(&manifest)?;
    let events = read_jsonl(run)?;
    let first = events
        .first()
        .with_context(|| format!("run log {} has no events", run.display()))?;
    let actual_digest = first
        .pointer("/metadata/agent_manifest_digest")
        .and_then(Value::as_str)
        .context("run log does not record metadata.agent_manifest_digest")?;
    if actual_digest != expected_digest {
        anyhow::bail!(
            "manifest digest mismatch: expected {expected_digest}, actual {actual_digest}"
        );
    }

    if let Some(expected_key_id) = manifest_key_id(&manifest)
        && let Some(actual_key_id) = first
            .pointer("/metadata/agent_manifest_key_id")
            .and_then(Value::as_str)
        && actual_key_id != expected_key_id
    {
        anyhow::bail!(
            "manifest key_id mismatch: expected {expected_key_id}, actual {actual_key_id}"
        );
    }

    Ok(())
}

fn manifest_signature(value: &Value) -> Option<&Value> {
    value.get("signature").filter(|value| !value.is_null())
}

fn manifest_key_id(value: &Value) -> Option<&str> {
    manifest_signature(value)
        .and_then(|signature| signature.get("key_id").and_then(Value::as_str))
        .or_else(|| value.get("key_id").and_then(Value::as_str))
}

fn print_import_report(out: &Path, report: &crate::integrations::ImportReport) {
    println!("{} import written to {}", report.provider, out.display());
    println!("Run ID: {}", report.run_id);
    println!("Source events: {}", report.source_events);
    println!("AgentProv events: {}", report.events);
}

fn next_event_for_run(
    run: &Path,
    event_type: String,
    action: Option<String>,
    resource: Option<String>,
    subject: Option<String>,
    metadata: Option<Value>,
) -> Result<Value> {
    build_next_event_for_run(
        run,
        AppendEventInput {
            event_type,
            action,
            resource,
            subject,
            metadata,
            payload_digest: None,
        },
    )
}

fn example_manifest() -> Result<Value> {
    serde_json::to_value(AgentManifest {
        schema: "agentprov.dev/manifest/v1".to_owned(),
        agent_id: format!("agent_{}", Uuid::new_v4().simple()),
        name: "research-agent".to_owned(),
        description: "Researches a topic and drafts a response".to_owned(),
        version: "0.1.0".to_owned(),
        owner: Owner {
            owner_type: "github_user".to_owned(),
            id: "danjdewhurst".to_owned(),
        },
        source: Source {
            repo: "https://github.com/example/research-agent".to_owned(),
            commit: "abc123".to_owned(),
            image_digest: None,
        },
        runtime: RuntimeManifest {
            runtime_type: "cli".to_owned(),
            environment: "local".to_owned(),
        },
        capabilities: vec![
            "web.search".to_owned(),
            "http.get".to_owned(),
            "discord.message.create".to_owned(),
        ],
        policy: PolicyRef {
            id: "policy_research_agent".to_owned(),
            version: "v1".to_owned(),
        },
        public_key: None,
    })
    .context("serialize example manifest")
}
fn example_run() -> Result<Value> {
    serde_json::to_value(RunEnvelope {
        schema: "agentprov.dev/run-envelope/v1".to_owned(),
        run_id: format!("run_{}", Uuid::new_v4().simple()),
        trace_id: format!("trace_{}", Uuid::new_v4().simple()),
        parent_run_id: None,
        trigger: Trigger {
            trigger_type: "manual".to_owned(),
            id: "discord_message_123".to_owned(),
        },
        agent: RunAgentRef {
            agent_id: "agent_01hxexample".to_owned(),
            version: "0.1.0".to_owned(),
            manifest_digest: "blake3:example".to_owned(),
        },
        actor_chain: vec![
            Actor {
                actor_type: "user".to_owned(),
                id: "danjdewhurst".to_owned(),
                auth_method: Some("discord".to_owned()),
            },
            Actor {
                actor_type: "service".to_owned(),
                id: "hermes".to_owned(),
                auth_method: None,
            },
            Actor {
                actor_type: "agent".to_owned(),
                id: "agent_01hxexample".to_owned(),
                auth_method: None,
            },
        ],
        runtime: RuntimeRun {
            host: "example-host".to_owned(),
            os: "linux".to_owned(),
            environment: "local".to_owned(),
            container_image_digest: None,
        },
        authority: Authority {
            capabilities: vec!["web.search".to_owned(), "discord.message.create".to_owned()],
            policy_id: "policy_research_agent".to_owned(),
            policy_version: "v1".to_owned(),
        },
        started_at: Utc::now(),
        ended_at: None,
        status: "running".to_owned(),
    })
    .context("serialize example run")
}

fn read_json_file(path: &Path) -> Result<Value> {
    let content =
        fs::read_to_string(path).with_context(|| format!("read JSON file {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parse JSON file {}", path.display()))
}
fn write_pretty_json(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
        .with_context(|| format!("write {}", path.display()))
}
fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
