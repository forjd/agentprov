use crate::run_log::{read_jsonl, verify_events};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

pub struct CollectorStore {
    connection: Connection,
}

impl CollectorStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let connection =
            Connection::open(path).with_context(|| format!("open SQLite DB {}", path.display()))?;
        let store = Self { connection };
        store.init()?;
        Ok(store)
    }

    pub fn open_memory() -> Result<Self> {
        let store = Self {
            connection: Connection::open_in_memory().context("open in-memory SQLite DB")?,
        };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<()> {
        self.connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS runs (
                run_id TEXT PRIMARY KEY,
                source TEXT,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS events (
                run_id TEXT NOT NULL,
                sequence INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_hash TEXT,
                event_json TEXT NOT NULL,
                PRIMARY KEY (run_id, sequence),
                FOREIGN KEY (run_id) REFERENCES runs(run_id)
            );
            "#,
        )?;
        Ok(())
    }

    pub fn ingest_events(&mut self, source: &str, events: &[Value]) -> Result<String> {
        if events.is_empty() {
            bail!("cannot ingest empty run");
        }
        let run_id = events[0]
            .get("run_id")
            .and_then(Value::as_str)
            .context("first event has no run_id")?
            .to_owned();
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO runs (run_id, source, created_at) VALUES (?1, ?2, ?3)",
            params![run_id, source, Utc::now().to_rfc3339()],
        )?;
        tx.execute("DELETE FROM events WHERE run_id = ?1", params![run_id])?;
        for event in events {
            let event_run_id = event
                .get("run_id")
                .and_then(Value::as_str)
                .context("event has no run_id")?;
            if event_run_id != run_id {
                bail!("run_id mismatch while ingesting collector run");
            }
            let sequence = event
                .get("sequence")
                .and_then(Value::as_u64)
                .context("event has no sequence")?;
            let event_type = event
                .get("event_type")
                .and_then(Value::as_str)
                .context("event has no event_type")?;
            let event_hash = event.get("event_hash").and_then(Value::as_str);
            tx.execute(
                "INSERT INTO events (run_id, sequence, event_type, event_hash, event_json) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    run_id,
                    sequence as i64,
                    event_type,
                    event_hash,
                    serde_json::to_string(event)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(run_id)
    }

    pub fn ingest_jsonl_file(&mut self, path: &Path) -> Result<String> {
        let events = read_jsonl(path)?;
        self.ingest_events(&path.display().to_string(), &events)
    }

    pub fn list_runs(&self) -> Result<Value> {
        let runs = self.run_rows()?;
        Ok(json!({ "runs": runs }))
    }

    fn run_rows(&self) -> Result<Vec<Value>> {
        let mut statement = self
            .connection
            .prepare("SELECT run_id, source, created_at FROM runs ORDER BY created_at DESC")?;
        let rows = statement.query_map([], |row| {
            Ok(json!({
                "run_id": row.get::<_, String>(0)?,
                "source": row.get::<_, Option<String>>(1)?,
                "created_at": row.get::<_, String>(2)?,
            }))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn run_events(&self, run_id: &str) -> Result<Vec<Value>> {
        let mut statement = self
            .connection
            .prepare("SELECT event_json FROM events WHERE run_id = ?1 ORDER BY sequence ASC")?;
        let rows = statement.query_map(params![run_id], |row| row.get::<_, String>(0))?;
        let event_json = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        if event_json.is_empty() {
            bail!("run not found: {run_id}");
        }
        event_json
            .into_iter()
            .map(|event| serde_json::from_str(&event).context("parse stored event JSON"))
            .collect()
    }

    pub fn run_events_json(&self, run_id: &str) -> Result<Value> {
        Ok(json!({
            "run_id": run_id,
            "events": self.run_events(run_id)?,
        }))
    }

    pub fn verify_run(&self, run_id: &str, require_signatures: bool) -> Result<Value> {
        let events = self.run_events(run_id)?;
        let report = verify_events(&events, require_signatures)?;
        Ok(json!({
            "run_id": run_id,
            "verifies": true,
            "events": report.events,
            "signatures": if report.signatures_present { "valid" } else { "not present" },
        }))
    }

    pub fn dashboard_html(&self) -> Result<String> {
        let runs = self.run_rows()?;
        let mut html = String::from(
            r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>AgentProv Collector</title>
<style>
body { font-family: system-ui, sans-serif; margin: 2rem; color: #17202a; }
h1, h2 { margin-bottom: 0.35rem; }
.run { border: 1px solid #d5dde5; border-radius: 8px; padding: 1rem; margin: 1rem 0; }
.meta { color: #536271; font-size: 0.9rem; }
.ok { color: #0b6b3a; font-weight: 600; }
.bad { color: #a12828; font-weight: 600; }
table { border-collapse: collapse; width: 100%; margin-top: 0.75rem; }
th, td { border-bottom: 1px solid #e6ebf0; padding: 0.45rem; text-align: left; vertical-align: top; }
th { font-size: 0.8rem; color: #536271; text-transform: uppercase; }
code { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.9em; }
</style>
</head>
<body>
<h1>AgentProv Collector</h1>
"#,
        );

        if runs.is_empty() {
            html.push_str("<p>No runs stored.</p>");
        }

        for run in runs {
            let run_id = run["run_id"].as_str().unwrap_or_default();
            html.push_str("<section class=\"run\">");
            html.push_str(&format!("<h2>{}</h2>", escape_html(run_id)));
            html.push_str(&format!(
                "<div class=\"meta\">Source: {} | Created: {}</div>",
                escape_html(run["source"].as_str().unwrap_or("unknown")),
                escape_html(run["created_at"].as_str().unwrap_or("unknown"))
            ));
            match self.verify_run(run_id, false) {
                Ok(report) => html.push_str(&format!(
                    "<p class=\"ok\">Verifies: {} events, signatures {}</p>",
                    report["events"],
                    escape_html(report["signatures"].as_str().unwrap_or("unknown"))
                )),
                Err(error) => html.push_str(&format!(
                    "<p class=\"bad\">Verification failed: {}</p>",
                    escape_html(&error.to_string())
                )),
            }
            html.push_str("<table><thead><tr><th>Seq</th><th>Type</th><th>Subject</th><th>Action</th><th>Resource</th><th>Decision</th></tr></thead><tbody>");
            for event in self.run_events(run_id)? {
                html.push_str("<tr>");
                html.push_str(&format!("<td>{}</td>", event["sequence"]));
                html.push_str(&format!(
                    "<td><code>{}</code></td>",
                    escape_html(event["event_type"].as_str().unwrap_or(""))
                ));
                html.push_str(&format!(
                    "<td>{}</td>",
                    escape_html(
                        event
                            .pointer("/subject/id")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    )
                ));
                html.push_str(&format!(
                    "<td><code>{}</code></td>",
                    escape_html(event["action"].as_str().unwrap_or(""))
                ));
                html.push_str(&format!(
                    "<td><code>{}</code></td>",
                    escape_html(event["resource"].as_str().unwrap_or(""))
                ));
                html.push_str(&format!(
                    "<td>{}</td>",
                    escape_html(
                        event
                            .pointer("/metadata/decision")
                            .or_else(|| event.pointer("/metadata/permission_decision/decision"))
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    )
                ));
                html.push_str("</tr>");
            }
            html.push_str("</tbody></table></section>");
        }

        html.push_str("</body></html>\n");
        Ok(html)
    }
}

pub fn serve(addr: &str, db: &Path) -> Result<()> {
    let listener = TcpListener::bind(addr).with_context(|| format!("bind collector at {addr}"))?;
    println!("AgentProv collector listening on http://{addr}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, db) {
                    eprintln!("collector request failed: {error:#}");
                }
            }
            Err(error) => eprintln!("collector connection failed: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, db: &Path) -> Result<()> {
    let request = read_http_request(&mut stream)?;
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((&request, ""));
    let mut lines = head.lines();
    let request_line = lines.next().context("missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().context("missing HTTP method")?;
    let path = parts.next().context("missing HTTP path")?;
    let mut store = CollectorStore::open(db)?;

    let response = match (method, path) {
        ("POST", "/ingest") => {
            let events = parse_jsonl_body(body)?;
            let run_id = store.ingest_events("http", &events)?;
            json_response(200, &json!({"run_id": run_id, "events": events.len()}))?
        }
        ("GET", "/runs") => json_response(200, &store.list_runs()?)?,
        ("GET", path) if path.starts_with("/runs/") && path.ends_with("/events") => {
            let run_id = path
                .trim_start_matches("/runs/")
                .trim_end_matches("/events")
                .trim_end_matches('/');
            json_response(200, &store.run_events_json(run_id)?)?
        }
        ("GET", path) if path.starts_with("/runs/") && path.ends_with("/verify") => {
            let run_id = path
                .trim_start_matches("/runs/")
                .trim_end_matches("/verify")
                .trim_end_matches('/');
            json_response(200, &store.verify_run(run_id, false)?)?
        }
        _ => json_response(404, &json!({"error": "not found"}))?,
    };

    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(header_end) = header_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..header_end]);
            let content_length = content_length(&headers).unwrap_or(0);
            if buffer.len() >= header_end + 4 + content_length {
                break;
            }
        }
    }
    String::from_utf8(buffer).context("HTTP request was not valid UTF-8")
}

fn header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn content_length(headers: &str) -> Option<usize> {
    headers.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse().ok())
            .flatten()
    })
}

fn parse_jsonl_body(body: &str) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    for (index, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        events.push(
            serde_json::from_str(line).with_context(|| format!("parse body line {}", index + 1))?,
        );
    }
    if events.is_empty() {
        bail!("request body contains no events");
    }
    Ok(events)
}

fn json_response(status: u16, value: &Value) -> Result<String> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "OK",
    };
    let body = serde_json::to_string(value)?;
    Ok(format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
