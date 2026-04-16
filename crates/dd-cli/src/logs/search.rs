use std::io::{self, Read};
use std::str::FromStr;

use clap::Args;
use dd_api::logs::{Page, SearchFilter, SearchRequest, StorageTier};

use crate::cli::Ctx;
use crate::output::{emit_json, emit_ndjson_event, emit_table_events, emit_text_event, OutputMode};
use crate::time_spec;

use super::{parse_fields, parse_indexes};

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Datadog log query (e.g. `service:api status:error`). Reads from stdin if omitted.
    pub query: Option<String>,

    #[arg(long, default_value = "now-15m")]
    pub from: String,

    #[arg(long, default_value = "now")]
    pub to: String,

    /// Page size (max 1000).
    #[arg(short = 'n', long, default_value_t = 100)]
    pub limit: u32,

    /// Stop after N total events across pages. `0` = unlimited.
    #[arg(long, default_value_t = 1000)]
    pub max: u32,

    /// Sort order. Use `-timestamp` for newest first.
    #[arg(long, default_value = "-timestamp")]
    pub sort: String,

    /// Comma-separated index names.
    #[arg(long)]
    pub index: Option<String>,

    /// Comma-separated attribute paths to include in text/table output.
    #[arg(long)]
    pub fields: Option<String>,

    /// Storage tier: indexes | online-archives | flex.
    #[arg(long, default_value = "indexes")]
    pub storage: String,
}

pub async fn run(ctx: Ctx, args: SearchArgs) -> anyhow::Result<()> {
    let query = resolve_query(args.query)?;
    let storage = StorageTier::from_str(&args.storage).map_err(anyhow::Error::msg)?;
    let from = time_spec::normalize(&args.from)?;
    let to = time_spec::normalize(&args.to)?;
    let fields = parse_fields(args.fields);
    let indexes = parse_indexes(args.index);

    let limit = args.limit.clamp(1, 1000);

    let mut cursor: Option<String> = None;
    let mut total: u32 = 0;
    let mut collected = Vec::new();

    let started = std::time::Instant::now();
    loop {
        let req = SearchRequest {
            filter: SearchFilter {
                from: Some(from.clone()),
                to: Some(to.clone()),
                query: query.clone(),
                indexes: indexes.clone(),
                storage_tier: Some(storage),
            },
            page: Some(Page { cursor: cursor.clone(), limit: Some(limit) }),
            sort: Some(args.sort.clone()),
            ..Default::default()
        };

        let resp = ctx.client.logs_search(&req).await?;

        for ev in resp.data {
            total += 1;
            match ctx.output {
                OutputMode::Text => emit_text_event(&ev, &fields),
                OutputMode::Ndjson => emit_ndjson_event(&ev)?,
                OutputMode::Json | OutputMode::Table => collected.push(ev),
            }
            if args.max != 0 && total >= args.max {
                break;
            }
        }

        if args.max != 0 && total >= args.max {
            break;
        }
        match resp.meta.and_then(|m| m.page).and_then(|p| p.after) {
            Some(next) if !next.is_empty() => cursor = Some(next),
            _ => break,
        }
    }

    match ctx.output {
        OutputMode::Json => emit_json(&collected)?,
        OutputMode::Table => emit_table_events(&collected, &fields),
        OutputMode::Text => {
            let elapsed = started.elapsed();
            eprintln!("{total} events ({:.1}s)", elapsed.as_secs_f32());
        }
        OutputMode::Ndjson => {}
    }

    Ok(())
}

fn resolve_query(cli_query: Option<String>) -> anyhow::Result<Option<String>> {
    if let Some(q) = cli_query {
        if !q.is_empty() {
            return Ok(Some(q));
        }
    }
    if is_terminal::IsTerminal::is_terminal(&io::stdin()) {
        return Ok(None);
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    let trimmed = buf.trim();
    Ok(if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    })
}
