use crate::run_log::{read_jsonl, verify_events, write_jsonl};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct EventListOptions {
    pub after_sequence: Option<u64>,
    pub limit: Option<u64>,
    pub event_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AppendOptions {
    pub require_signatures: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct IngestOptions {
    pub require_signatures: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RunListOptions {
    pub limit: Option<u64>,
}

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
        self.ingest_events_with_options(source, events, IngestOptions::default())
    }

    pub fn ingest_events_with_options(
        &mut self,
        source: &str,
        events: &[Value],
        options: IngestOptions,
    ) -> Result<String> {
        if events.is_empty() {
            bail!("cannot ingest empty run");
        }
        let run_id = events[0]
            .get("run_id")
            .and_then(Value::as_str)
            .context("first event has no run_id")?
            .to_owned();
        verify_events(events, options.require_signatures)?;
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

    pub fn append_event(&mut self, source: &str, run_id: &str, event: Value) -> Result<Value> {
        self.append_event_with_options(source, run_id, event, AppendOptions::default())
    }

    pub fn append_event_with_options(
        &mut self,
        source: &str,
        run_id: &str,
        event: Value,
        options: AppendOptions,
    ) -> Result<Value> {
        let event_run_id = event
            .get("run_id")
            .and_then(Value::as_str)
            .context("event has no run_id")?;
        if event_run_id != run_id {
            bail!("event run_id {event_run_id} does not match target run {run_id}");
        }
        let sequence = event
            .get("sequence")
            .and_then(Value::as_u64)
            .context("event has no sequence")?;
        let event_type = event
            .get("event_type")
            .and_then(Value::as_str)
            .context("event has no event_type")?
            .to_owned();
        let event_hash = event
            .get("event_hash")
            .and_then(Value::as_str)
            .context("event has no event_hash")?
            .to_owned();

        let mut events = self.run_events_or_empty(run_id)?;
        events.push(event.clone());
        verify_events(&events, options.require_signatures)?;

        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO runs (run_id, source, created_at) VALUES (?1, ?2, ?3)",
            params![run_id, source, Utc::now().to_rfc3339()],
        )?;
        tx.execute(
            "INSERT INTO events (run_id, sequence, event_type, event_hash, event_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                run_id,
                sequence as i64,
                event_type,
                event_hash,
                serde_json::to_string(&event)?
            ],
        )?;
        tx.commit()?;

        Ok(json!({
            "run_id": run_id,
            "sequence": sequence,
            "event_hash": event_hash,
        }))
    }

    pub fn ingest_jsonl_file(&mut self, path: &Path) -> Result<String> {
        self.ingest_jsonl_file_with_options(path, IngestOptions::default())
    }

    pub fn ingest_jsonl_file_with_options(
        &mut self,
        path: &Path,
        options: IngestOptions,
    ) -> Result<String> {
        let events = read_jsonl(path)?;
        self.ingest_events_with_options(&path.display().to_string(), &events, options)
    }

    pub fn export_jsonl_file(&self, run_id: &str, path: &Path) -> Result<usize> {
        let events = self.run_events(run_id)?;
        let count = events.len();
        write_jsonl(path, &events)?;
        Ok(count)
    }

    pub fn export_jsonl_string(&self, run_id: &str) -> Result<String> {
        let events = self.run_events(run_id)?;
        let mut jsonl = String::new();
        for event in events {
            jsonl.push_str(&serde_json::to_string(&event)?);
            jsonl.push('\n');
        }
        Ok(jsonl)
    }

    pub fn list_runs(&self) -> Result<Value> {
        self.list_runs_json(RunListOptions::default())
    }

    pub fn list_runs_json(&self, options: RunListOptions) -> Result<Value> {
        let mut runs = self.run_rows(options.limit.map(limit_plus_one).transpose()?)?;
        let has_more = if let Some(limit) = options.limit {
            let limit_usize = usize::try_from(limit).context("limit is too large for usize")?;
            if runs.len() > limit_usize {
                runs.truncate(limit_usize);
                true
            } else {
                false
            }
        } else {
            false
        };
        let count = runs.len();
        Ok(json!({
            "runs": runs,
            "count": count,
            "limit": options.limit,
            "has_more": has_more,
        }))
    }

    fn run_rows(&self, limit: Option<i64>) -> Result<Vec<Value>> {
        if let Some(limit) = limit {
            let mut statement = self.connection.prepare(
                "SELECT run_id, source, created_at FROM runs ORDER BY created_at DESC LIMIT ?1",
            )?;
            let rows = statement.query_map(params![limit], |row| {
                Ok(json!({
                    "run_id": row.get::<_, String>(0)?,
                    "source": row.get::<_, Option<String>>(1)?,
                    "created_at": row.get::<_, String>(2)?,
                }))
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        } else {
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
    }

    pub fn run_events(&self, run_id: &str) -> Result<Vec<Value>> {
        let events = self.run_events_or_empty(run_id)?;
        if events.is_empty() {
            bail!("run not found: {run_id}");
        }
        Ok(events)
    }

    pub fn run_events_page(&self, run_id: &str, options: EventListOptions) -> Result<Vec<Value>> {
        if !self.run_exists(run_id)? {
            bail!("run not found: {run_id}");
        }
        let after_sequence = options
            .after_sequence
            .map(i64::try_from)
            .transpose()
            .context("after_sequence is too large for SQLite")?
            .unwrap_or(0);
        let limit = options
            .limit
            .map(i64::try_from)
            .transpose()
            .context("limit is too large for SQLite")?;
        let event_json = match (options.event_type.as_deref(), limit) {
            (Some(event_type), Some(limit)) => {
                let mut statement = self.connection.prepare(
                    "SELECT event_json FROM events WHERE run_id = ?1 AND sequence > ?2 AND event_type = ?3 ORDER BY sequence ASC LIMIT ?4",
                )?;
                let rows = statement
                    .query_map(params![run_id, after_sequence, event_type, limit], |row| {
                        row.get::<_, String>(0)
                    })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            }
            (Some(event_type), None) => {
                let mut statement = self.connection.prepare(
                    "SELECT event_json FROM events WHERE run_id = ?1 AND sequence > ?2 AND event_type = ?3 ORDER BY sequence ASC",
                )?;
                let rows = statement
                    .query_map(params![run_id, after_sequence, event_type], |row| {
                        row.get::<_, String>(0)
                    })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            }
            (None, Some(limit)) => {
                let mut statement = self.connection.prepare(
                    "SELECT event_json FROM events WHERE run_id = ?1 AND sequence > ?2 ORDER BY sequence ASC LIMIT ?3",
                )?;
                let rows = statement.query_map(params![run_id, after_sequence, limit], |row| {
                    row.get::<_, String>(0)
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            }
            (None, None) => {
                let mut statement = self.connection.prepare(
                    "SELECT event_json FROM events WHERE run_id = ?1 AND sequence > ?2 ORDER BY sequence ASC",
                )?;
                let rows = statement.query_map(params![run_id, after_sequence], |row| {
                    row.get::<_, String>(0)
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            }
        };
        event_json
            .into_iter()
            .map(|event| serde_json::from_str(&event).context("parse stored event JSON"))
            .collect()
    }

    fn run_events_or_empty(&self, run_id: &str) -> Result<Vec<Value>> {
        let mut statement = self
            .connection
            .prepare("SELECT event_json FROM events WHERE run_id = ?1 ORDER BY sequence ASC")?;
        let rows = statement.query_map(params![run_id], |row| row.get::<_, String>(0))?;
        let event_json = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        event_json
            .into_iter()
            .map(|event| serde_json::from_str(&event).context("parse stored event JSON"))
            .collect()
    }

    fn run_exists(&self, run_id: &str) -> Result<bool> {
        let count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM runs WHERE run_id = ?1",
            params![run_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn has_more_events(
        &self,
        run_id: &str,
        after_sequence: u64,
        event_type: Option<&str>,
    ) -> Result<bool> {
        let after_sequence =
            i64::try_from(after_sequence).context("after_sequence is too large for SQLite")?;
        let exists: Option<i64> = if let Some(event_type) = event_type {
            self.connection
                .query_row(
                    "SELECT 1 FROM events WHERE run_id = ?1 AND sequence > ?2 AND event_type = ?3 LIMIT 1",
                    params![run_id, after_sequence, event_type],
                    |row| row.get(0),
                )
                .optional()?
        } else {
            self.connection
                .query_row(
                    "SELECT 1 FROM events WHERE run_id = ?1 AND sequence > ?2 LIMIT 1",
                    params![run_id, after_sequence],
                    |row| row.get(0),
                )
                .optional()?
        };
        Ok(exists.is_some())
    }

    pub fn run_events_json(&self, run_id: &str, options: EventListOptions) -> Result<Value> {
        let events = self.run_events_page(run_id, options.clone())?;
        let next_after_sequence = events
            .last()
            .and_then(|event| event.get("sequence"))
            .and_then(Value::as_u64);
        let has_more = if options.limit.is_some() {
            if let Some(next_after_sequence) = next_after_sequence {
                self.has_more_events(run_id, next_after_sequence, options.event_type.as_deref())?
            } else {
                false
            }
        } else {
            false
        };
        let count = events.len();
        Ok(json!({
            "run_id": run_id,
            "events": events,
            "count": count,
            "after_sequence": options.after_sequence,
            "limit": options.limit,
            "event_type": options.event_type,
            "has_more": has_more,
            "next_after_sequence": next_after_sequence,
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
        let runs = self.run_rows(None)?;
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
    let response = http_response_for_request(&request, db);
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn http_response_for_request(request: &str, db: &Path) -> String {
    match route_http_request(request, db) {
        Ok(response) => response,
        Err(error) => {
            let status = http_error_status(&error);
            json_response(status, &json!({"error": error.to_string()})).unwrap_or_else(|_| {
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: 33\r\nConnection: close\r\n\r\n{\"error\":\"internal server error\"}".to_owned()
            })
        }
    }
}

fn route_http_request(request: &str, db: &Path) -> Result<String> {
    let (head, body) = request.split_once("\r\n\r\n").unwrap_or((request, ""));
    let mut lines = head.lines();
    let request_line = lines.next().context("missing HTTP request line")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().context("missing HTTP method")?;
    let target = parts.next().context("missing HTTP path")?;
    let (path, query) = split_request_target(target);
    let mut store = CollectorStore::open(db)?;

    let response = match (method, path) {
        ("POST", "/ingest") => {
            let events = parse_jsonl_body(body)?;
            let run_id =
                store.ingest_events_with_options("http", &events, ingest_options(query)?)?;
            json_response(200, &json!({"run_id": run_id, "events": events.len()}))?
        }
        ("POST", path) if path.starts_with("/runs/") && path.ends_with("/events") => {
            let run_id = path
                .trim_start_matches("/runs/")
                .trim_end_matches("/events")
                .trim_end_matches('/');
            let report = store.append_event_with_options(
                "http-stream",
                run_id,
                parse_json_body(body)?,
                append_options(query)?,
            )?;
            json_response(200, &report)?
        }
        ("GET", "/runs") => json_response(200, &store.list_runs_json(run_list_options(query)?)?)?,
        ("GET", path) if path.starts_with("/runs/") && path.ends_with("/events") => {
            let run_id = path
                .trim_start_matches("/runs/")
                .trim_end_matches("/events")
                .trim_end_matches('/');
            json_response(
                200,
                &store.run_events_json(run_id, event_list_options(query)?)?,
            )?
        }
        ("GET", path) if path.starts_with("/runs/") && path.ends_with("/export") => {
            let run_id = path
                .trim_start_matches("/runs/")
                .trim_end_matches("/export")
                .trim_end_matches('/');
            jsonl_response(200, &store.export_jsonl_string(run_id)?)
        }
        ("GET", path) if path.starts_with("/runs/") && path.ends_with("/verify") => {
            let run_id = path
                .trim_start_matches("/runs/")
                .trim_end_matches("/verify")
                .trim_end_matches('/');
            json_response(
                200,
                &store.verify_run(run_id, verify_require_signatures(query)?)?,
            )?
        }
        _ => json_response(404, &json!({"error": "not found"}))?,
    };

    Ok(response)
}

fn http_error_status(error: &anyhow::Error) -> u16 {
    let message = error.to_string();
    if message.starts_with("run not found:") {
        404
    } else if is_bad_request_error(&message) {
        400
    } else {
        500
    }
}

fn is_bad_request_error(message: &str) -> bool {
    message.starts_with("parse ")
        || message.starts_with("missing HTTP ")
        || message.starts_with("request body ")
        || message.contains(" must ")
        || message.contains(" mismatch")
        || message.contains(" does not match ")
        || message.contains(" has no ")
        || message.starts_with("missing signature")
        || message.contains("schema validation failed")
        || message.contains("event hash mismatch")
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

fn split_request_target(target: &str) -> (&str, Option<&str>) {
    if let Some((path, query)) = target.split_once('?') {
        (path, Some(query))
    } else {
        (target, None)
    }
}

fn event_list_options(query: Option<&str>) -> Result<EventListOptions> {
    Ok(EventListOptions {
        after_sequence: query_param_u64(query, "after_sequence")?,
        limit: query_param_u64(query, "limit")?,
        event_type: query_param_string(query, "event_type"),
    })
}

fn append_options(query: Option<&str>) -> Result<AppendOptions> {
    Ok(AppendOptions {
        require_signatures: query_param_bool(query, "require_signatures")?.unwrap_or(false),
    })
}

fn ingest_options(query: Option<&str>) -> Result<IngestOptions> {
    Ok(IngestOptions {
        require_signatures: query_param_bool(query, "require_signatures")?.unwrap_or(false),
    })
}

fn run_list_options(query: Option<&str>) -> Result<RunListOptions> {
    Ok(RunListOptions {
        limit: query_param_u64(query, "limit")?,
    })
}

fn verify_require_signatures(query: Option<&str>) -> Result<bool> {
    Ok(query_param_bool(query, "require_signatures")?.unwrap_or(false))
}

fn limit_plus_one(limit: u64) -> Result<i64> {
    let limit = limit.checked_add(1).context("limit is too large")?;
    i64::try_from(limit).context("limit is too large for SQLite")
}

fn query_param_u64(query: Option<&str>, name: &str) -> Result<Option<u64>> {
    let Some(query) = query else {
        return Ok(None);
    };
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == name {
            let parsed = value
                .parse::<u64>()
                .with_context(|| format!("{name} must be an unsigned integer"))?;
            if name == "limit" && parsed == 0 {
                bail!("limit must be greater than 0");
            }
            return Ok(Some(parsed));
        }
    }
    Ok(None)
}

fn query_param_bool(query: Option<&str>, name: &str) -> Result<Option<bool>> {
    let Some(query) = query else {
        return Ok(None);
    };
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == name {
            return match value {
                "true" => Ok(Some(true)),
                "false" => Ok(Some(false)),
                _ => bail!("{name} must be true or false"),
            };
        }
    }
    Ok(None)
}

fn query_param_string(query: Option<&str>, name: &str) -> Option<String> {
    let query = query?;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == name {
            return Some(value.to_owned());
        }
    }
    None
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

fn parse_json_body(body: &str) -> Result<Value> {
    serde_json::from_str(body.trim()).context("parse JSON request body")
}

fn json_response(status: u16, value: &Value) -> Result<String> {
    let reason = http_reason(status);
    let body = serde_json::to_string(value)?;
    Ok(format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ))
}

fn jsonl_response(status: u16, body: &str) -> String {
    let reason = http_reason(status);
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventInput, build_event_from_input};
    use crate::signing::{generate_key, sign_value};
    use tempfile::tempdir;

    #[test]
    fn http_errors_are_returned_as_json_responses() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("collector.sqlite");

        let bad_query = http_response_for_request(
            "GET /runs/run_missing/events?limit=0 HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &db,
        );
        assert!(bad_query.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(bad_query.contains("\"error\":\"limit must be greater than 0\""));

        let missing_run = http_response_for_request(
            "GET /runs/run_missing/events HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &db,
        );
        assert!(missing_run.starts_with("HTTP/1.1 404 Not Found"));
        assert!(missing_run.contains("\"error\":\"run not found: run_missing\""));

        let bad_json = http_response_for_request(
            "POST /ingest HTTP/1.1\r\nHost: localhost\r\nContent-Length: 8\r\n\r\nnot-json",
            &db,
        );
        assert!(bad_json.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(bad_json.contains("\"error\":\"parse body line 1\""));

        let first =
            build_event_from_input(EventInput::new("run_bad_ingest", 1, "run.start")).unwrap();
        let mut second = EventInput::new("run_bad_ingest", 2, "tool.execute");
        second.previous_event_hash = Some("blake3:not-the-previous-event".to_owned());
        let second = build_event_from_input(second).unwrap();
        let invalid_chain = [first, second]
            .iter()
            .map(serde_json::to_string)
            .collect::<serde_json::Result<Vec<_>>>()
            .unwrap()
            .join("\n");
        let bad_ingest = http_response_for_request(
            &format!(
                "POST /ingest HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
                invalid_chain.len(),
                invalid_chain
            ),
            &db,
        );
        assert!(bad_ingest.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(bad_ingest.contains("previous_event_hash mismatch"));

        let unsigned = serde_json::to_string(
            &build_event_from_input(EventInput::new("run_unsigned_ingest", 1, "run.start"))
                .unwrap(),
        )
        .unwrap();
        let strict_ingest = http_response_for_request(
            &format!(
                "POST /ingest?require_signatures=true HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
                unsigned.len(),
                unsigned
            ),
            &db,
        );
        assert!(strict_ingest.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(strict_ingest.contains("\"error\":\"missing signature at sequence 1\""));

        let bad_boolean = http_response_for_request(
            "GET /runs/run_missing/verify?require_signatures=yes HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &db,
        );
        assert!(bad_boolean.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(bad_boolean.contains("\"error\":\"require_signatures must be true or false\""));
    }

    #[test]
    fn http_events_support_event_type_filter() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("collector.sqlite");
        let mut store = CollectorStore::open(&db).unwrap();

        let start =
            build_event_from_input(EventInput::new("run_http_filter", 1, "run.start")).unwrap();
        let mut tool = EventInput::new("run_http_filter", 2, "tool.execute");
        tool.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
        let tool = build_event_from_input(tool).unwrap();
        let mut permission = EventInput::new("run_http_filter", 3, "permission.check");
        permission.previous_event_hash = Some(tool["event_hash"].as_str().unwrap().to_owned());
        let permission = build_event_from_input(permission).unwrap();
        store
            .ingest_events("test", &[start, tool, permission])
            .unwrap();

        let response = http_response_for_request(
            "GET /runs/run_http_filter/events?event_type=tool.execute&limit=1 HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &db,
        );
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        let body = response.split("\r\n\r\n").nth(1).unwrap();
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["count"], 1);
        assert_eq!(value["event_type"], "tool.execute");
        assert_eq!(value["has_more"], false);
        assert_eq!(value["events"][0]["event_type"], "tool.execute");
        assert_eq!(value["next_after_sequence"], 2);
    }

    #[test]
    fn http_runs_support_limit() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("collector.sqlite");
        let mut store = CollectorStore::open(&db).unwrap();

        for run_id in ["run_http_one", "run_http_two"] {
            let start = build_event_from_input(EventInput::new(run_id, 1, "run.start")).unwrap();
            store.ingest_events("test", &[start]).unwrap();
        }

        let response =
            http_response_for_request("GET /runs?limit=1 HTTP/1.1\r\nHost: localhost\r\n\r\n", &db);
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        let body = response.split("\r\n\r\n").nth(1).unwrap();
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["count"], 1);
        assert_eq!(value["limit"], 1);
        assert_eq!(value["has_more"], true);
        assert_eq!(value["runs"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn http_runs_export_jsonl() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("collector.sqlite");
        let mut store = CollectorStore::open(&db).unwrap();

        let start =
            build_event_from_input(EventInput::new("run_http_export", 1, "run.start")).unwrap();
        let mut tool = EventInput::new("run_http_export", 2, "tool.execute");
        tool.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
        let tool = build_event_from_input(tool).unwrap();
        store
            .ingest_events("test", &[start.clone(), tool.clone()])
            .unwrap();

        let response = http_response_for_request(
            "GET /runs/run_http_export/export HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &db,
        );
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("Content-Type: application/x-ndjson"));

        let body = response.split("\r\n\r\n").nth(1).unwrap();
        let exported = body
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(exported, vec![start, tool]);
    }

    #[test]
    fn http_append_can_require_signatures() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("collector.sqlite");
        let mut store = CollectorStore::open(&db).unwrap();
        let key = generate_key();

        let mut start =
            build_event_from_input(EventInput::new("run_http_signed_stream", 1, "run.start"))
                .unwrap();
        sign_value(&mut start, &key).unwrap();
        store.ingest_events("test", &[start.clone()]).unwrap();

        let mut unsigned = EventInput::new("run_http_signed_stream", 2, "tool.execute");
        unsigned.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
        let unsigned = serde_json::to_string(&build_event_from_input(unsigned).unwrap()).unwrap();
        let strict_response = http_response_for_request(
            &format!(
                "POST /runs/run_http_signed_stream/events?require_signatures=true HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
                unsigned.len(),
                unsigned
            ),
            &db,
        );
        assert!(strict_response.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(strict_response.contains("\"error\":\"missing signature at sequence 2\""));
        assert_eq!(store.run_events("run_http_signed_stream").unwrap().len(), 1);

        let mut signed = EventInput::new("run_http_signed_stream", 2, "tool.execute");
        signed.previous_event_hash = Some(start["event_hash"].as_str().unwrap().to_owned());
        let mut signed = build_event_from_input(signed).unwrap();
        sign_value(&mut signed, &key).unwrap();
        let signed = serde_json::to_string(&signed).unwrap();
        let ok_response = http_response_for_request(
            &format!(
                "POST /runs/run_http_signed_stream/events?require_signatures=true HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
                signed.len(),
                signed
            ),
            &db,
        );
        assert!(ok_response.starts_with("HTTP/1.1 200 OK"));
        let body = ok_response.split("\r\n\r\n").nth(1).unwrap();
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["sequence"], 2);

        let strict_verify = CollectorStore::open(&db)
            .unwrap()
            .verify_run("run_http_signed_stream", true)
            .unwrap();
        assert_eq!(strict_verify["events"], 2);
    }

    #[test]
    fn http_verify_can_require_signatures() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("collector.sqlite");
        let mut store = CollectorStore::open(&db).unwrap();

        let start =
            build_event_from_input(EventInput::new("run_http_verify", 1, "run.start")).unwrap();
        store.ingest_events("test", &[start]).unwrap();

        let lenient = http_response_for_request(
            "GET /runs/run_http_verify/verify HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &db,
        );
        assert!(lenient.starts_with("HTTP/1.1 200 OK"));
        let body = lenient.split("\r\n\r\n").nth(1).unwrap();
        let value: Value = serde_json::from_str(body).unwrap();
        assert_eq!(value["verifies"], true);
        assert_eq!(value["signatures"], "not present");

        let strict = http_response_for_request(
            "GET /runs/run_http_verify/verify?require_signatures=true HTTP/1.1\r\nHost: localhost\r\n\r\n",
            &db,
        );
        assert!(strict.starts_with("HTTP/1.1 400 Bad Request"));
        assert!(strict.contains("\"error\":\"missing signature at sequence 1\""));
    }
}
