use std::io::{self, Write};

use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use dd_api::logs::LogEvent;
use is_terminal::IsTerminal;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
    Ndjson,
    Table,
}

pub fn resolve_mode(flag: Option<&str>) -> OutputMode {
    match flag {
        Some("text") => OutputMode::Text,
        Some("json") => OutputMode::Json,
        Some("ndjson") => OutputMode::Ndjson,
        Some("table") => OutputMode::Table,
        Some(other) => {
            eprintln!("warning: unknown output mode '{other}', using auto");
            auto_mode()
        }
        None => auto_mode(),
    }
}

fn auto_mode() -> OutputMode {
    if io::stdout().is_terminal() {
        OutputMode::Text
    } else {
        OutputMode::Json
    }
}

pub fn emit_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    if handle.is_terminal() {
        serde_json::to_writer_pretty(&mut handle, value)?;
    } else {
        serde_json::to_writer(&mut handle, value)?;
    }
    writeln!(handle)?;
    Ok(())
}

pub fn emit_ndjson_event(ev: &LogEvent) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer(&mut handle, ev)?;
    writeln!(handle)?;
    Ok(())
}

/// Emit each item as its own JSON line (newline-delimited JSON).
pub fn emit_ndjson_each<T: Serialize>(items: &[T]) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    for item in items {
        serde_json::to_writer(&mut handle, item)?;
        writeln!(handle)?;
    }
    Ok(())
}

/// Render a generic table from string headers and rows.
pub fn emit_table_rows(headers: &[&str], rows: Vec<Vec<String>>) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(headers.iter().map(|h| h.to_string()).collect::<Vec<_>>());
    for row in rows {
        table.add_row(row);
    }
    println!("{table}");
}

pub fn emit_text_event(ev: &LogEvent, fields: &[String]) {
    let ts = ev
        .attributes
        .timestamp
        .as_deref()
        .unwrap_or("-");
    let svc = ev.attributes.service.as_deref().unwrap_or("-");
    let status = ev
        .attributes
        .status
        .as_deref()
        .unwrap_or("info")
        .to_uppercase();
    let msg = ev.attributes.message.as_deref().unwrap_or("");

    if fields.is_empty() {
        println!("{ts}  {status:<5}  {svc:<20}  {msg}");
    } else {
        let extras: Vec<String> = fields
            .iter()
            .map(|f| format!("{f}={}", lookup_field(ev, f)))
            .collect();
        println!("{ts}  {status:<5}  {svc:<20}  {msg}  {}", extras.join(" "));
    }
}

pub fn emit_table_events(events: &[LogEvent], fields: &[String]) {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic);

    let mut header = vec![
        "timestamp".to_string(),
        "status".to_string(),
        "service".to_string(),
        "message".to_string(),
    ];
    header.extend(fields.iter().cloned());
    table.set_header(header);

    for ev in events {
        let mut row = vec![
            ev.attributes.timestamp.clone().unwrap_or_else(|| "-".into()),
            ev.attributes
                .status
                .clone()
                .unwrap_or_else(|| "info".into())
                .to_uppercase(),
            ev.attributes.service.clone().unwrap_or_else(|| "-".into()),
            truncate(ev.attributes.message.clone().unwrap_or_default(), 80),
        ];
        for f in fields {
            row.push(lookup_field(ev, f));
        }
        table.add_row(row);
    }

    println!("{table}");
}

fn lookup_field(ev: &LogEvent, path: &str) -> String {
    match path {
        "timestamp" => ev.attributes.timestamp.clone().unwrap_or_default(),
        "service" => ev.attributes.service.clone().unwrap_or_default(),
        "status" => ev.attributes.status.clone().unwrap_or_default(),
        "message" => ev.attributes.message.clone().unwrap_or_default(),
        "host" => ev.attributes.host.clone().unwrap_or_default(),
        "tags" => ev.attributes.tags.join(","),
        other => {
            let key = other.trim_start_matches('@');
            ev.attributes
                .attributes
                .pointer(&format!("/{}", key.replace('.', "/")))
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                })
                .unwrap_or_default()
        }
    }
}

fn truncate(mut s: String, n: usize) -> String {
    if s.chars().count() > n {
        s.truncate(
            s.char_indices()
                .nth(n)
                .map(|(i, _)| i)
                .unwrap_or(s.len()),
        );
        s.push('…');
    }
    s
}
