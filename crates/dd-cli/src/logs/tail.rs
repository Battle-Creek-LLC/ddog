use std::collections::HashSet;
use std::str::FromStr;
use std::time::Duration;

use clap::Args;
use dd_api::logs::{Page, SearchFilter, SearchRequest, StorageTier};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::cli::Ctx;
use crate::output::{emit_ndjson_event, emit_text_event, OutputMode};
use crate::time_spec;

use super::{parse_fields, parse_indexes};

#[derive(Debug, Args)]
pub struct TailArgs {
    /// Datadog log query.
    pub query: Option<String>,

    /// Poll interval (e.g. 2s, 5s, 30s). Minimum 2s.
    #[arg(long, default_value = "5s")]
    pub interval: String,

    /// First-poll start time (defaults to `now`).
    #[arg(long, default_value = "now")]
    pub since: String,

    /// Comma-separated index names.
    #[arg(long)]
    pub index: Option<String>,

    /// Comma-separated attribute paths to include in text output.
    #[arg(long)]
    pub fields: Option<String>,

    /// Storage tier: indexes | online-archives | flex.
    #[arg(long, default_value = "indexes")]
    pub storage: String,

    /// Max events to fetch per poll.
    #[arg(long, default_value_t = 200)]
    pub batch: u32,
}

pub async fn run(ctx: Ctx, args: TailArgs) -> anyhow::Result<()> {
    let interval = parse_duration(&args.interval)?.max(Duration::from_secs(2));
    let storage = StorageTier::from_str(&args.storage).map_err(anyhow::Error::msg)?;
    let fields = parse_fields(args.fields);
    let indexes = parse_indexes(args.index);

    // Track recently-seen event IDs to dedupe across overlapping polls.
    let mut seen: HashSet<String> = HashSet::new();
    const SEEN_CAP: usize = 10_000;

    let mut window_from = resolve_since(&args.since)?;
    let mut ctrl_c = Box::pin(tokio::signal::ctrl_c());

    loop {
        let tick = tokio::time::sleep(interval);
        tokio::select! {
            _ = &mut ctrl_c => {
                eprintln!("\ntail stopped");
                return Ok(());
            }
            _ = tick => {}
        }

        let now = OffsetDateTime::now_utc().format(&Rfc3339)?;
        let req = SearchRequest {
            filter: SearchFilter {
                from: Some(window_from.clone()),
                to: Some(now.clone()),
                query: args.query.clone(),
                indexes: indexes.clone(),
                storage_tier: Some(storage),
            },
            page: Some(Page { cursor: None, limit: Some(args.batch.clamp(1, 1000)) }),
            // Ascending so oldest prints first within a poll.
            sort: Some("timestamp".to_string()),
            ..Default::default()
        };

        match ctx.client.logs_search(&req).await {
            Ok(resp) => {
                for ev in resp.data {
                    if !seen.insert(ev.id.clone()) {
                        continue;
                    }
                    match ctx.output {
                        OutputMode::Ndjson | OutputMode::Json => emit_ndjson_event(&ev)?,
                        _ => emit_text_event(&ev, &fields),
                    }
                }
                if seen.len() > SEEN_CAP {
                    seen.clear();
                }
            }
            Err(e) => {
                eprintln!("tail: poll error: {e}");
            }
        }

        window_from = now;
    }
}

fn resolve_since(raw: &str) -> anyhow::Result<String> {
    let normalized = time_spec::normalize(raw)?;
    if normalized == "now" {
        Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
    } else {
        Ok(normalized)
    }
}

fn parse_duration(raw: &str) -> anyhow::Result<Duration> {
    let s = raw.trim();
    if s.is_empty() {
        anyhow::bail!("empty duration");
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: u64 = num
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid duration '{raw}' (expected e.g. 2s, 30s, 1m)"))?;
    match unit {
        "s" => Ok(Duration::from_secs(n)),
        "m" => Ok(Duration::from_secs(n * 60)),
        "h" => Ok(Duration::from_secs(n * 3600)),
        _ => anyhow::bail!("invalid duration unit '{unit}' (use s|m|h)"),
    }
}
