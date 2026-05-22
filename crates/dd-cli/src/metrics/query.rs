use std::io::{self, Read, Write};

use clap::Args;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use dd_api::metrics::TimeseriesResponse;
use is_terminal::IsTerminal;
use serde::Serialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::cli::Ctx;
use crate::output::{emit_json, OutputMode};
use crate::time_spec;

#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Metric query, e.g.
    /// `sum:bridgeft.import.records{*} by {feed}.rollup(sum, 86400)`.
    /// Reads from stdin if omitted.
    pub query: Option<String>,

    /// Start of the window: `now`, `now-<N><unit>` (s|m|h|d|w), or RFC-3339.
    #[arg(long, default_value = "now-1d")]
    pub from: String,

    /// End of the window. Same format as `--from`.
    #[arg(long, default_value = "now")]
    pub to: String,

    /// Bucket interval as a duration (`1d`, `1h`, `15m`, …). When omitted,
    /// Datadog rolls up automatically (or the query's own `.rollup(...)` wins).
    #[arg(long)]
    pub interval: Option<String>,
}

/// One flattened timeseries point: which series (`scope`), when, and the value.
#[derive(Debug, Serialize)]
struct Point {
    scope: String,
    timestamp: String,
    timestamp_ms: i64,
    /// `null` where the series has no data for that bucket (≠ a real zero).
    value: Option<f64>,
}

pub async fn run(ctx: Ctx, args: QueryArgs) -> anyhow::Result<()> {
    let query = resolve_query(args.query)?;
    let from_ms = time_spec::to_epoch_secs(&args.from)? * 1000;
    let to_ms = time_spec::to_epoch_secs(&args.to)? * 1000;
    let interval_ms = match args.interval.as_deref() {
        Some(d) => Some(time_spec::duration_secs(d)? * 1000),
        None => None,
    };

    let resp = ctx
        .client
        .metrics_timeseries(from_ms, to_ms, interval_ms, &query)
        .await?;
    let points = flatten(&resp);

    match ctx.output {
        OutputMode::Ndjson => emit_ndjson(&points)?,
        OutputMode::Json => emit_json(&points)?,
        OutputMode::Table | OutputMode::Text => render_table(&points),
    }
    Ok(())
}

/// Expand the column-oriented v2 response (series metadata + shared `times` +
/// `values[series][time]`) into one row per (series, bucket).
fn flatten(resp: &TimeseriesResponse) -> Vec<Point> {
    let Some(attrs) = resp.data.as_ref().map(|d| &d.attributes) else {
        return Vec::new();
    };
    let mut points = Vec::new();
    for (i, meta) in attrs.series.iter().enumerate() {
        let scope = if meta.group_tags.is_empty() {
            "*".to_string()
        } else {
            meta.group_tags.join(",")
        };
        let Some(row) = attrs.values.get(i) else {
            continue;
        };
        for (j, &ts_ms) in attrs.times.iter().enumerate() {
            points.push(Point {
                scope: scope.clone(),
                timestamp: format_ts(ts_ms),
                timestamp_ms: ts_ms,
                value: row.get(j).copied().flatten(),
            });
        }
    }
    points
}

fn emit_ndjson(points: &[Point]) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    for p in points {
        serde_json::to_writer(&mut handle, p)?;
        writeln!(handle)?;
    }
    Ok(())
}

fn render_table(points: &[Point]) {
    if points.is_empty() {
        println!("(no results)");
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["scope", "timestamp", "value"]);
    for p in points {
        let value = match p.value {
            Some(v) => format_value(v),
            None => "—".to_string(),
        };
        table.add_row(vec![p.scope.clone(), p.timestamp.clone(), value]);
    }
    println!("{table}");
}

fn format_value(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        (v as i64).to_string()
    } else {
        v.to_string()
    }
}

fn format_ts(ms: i64) -> String {
    OffsetDateTime::from_unix_timestamp(ms / 1000)
        .ok()
        .and_then(|dt| dt.format(&Rfc3339).ok())
        .unwrap_or_else(|| ms.to_string())
}

fn resolve_query(cli_query: Option<String>) -> anyhow::Result<String> {
    if let Some(q) = cli_query {
        if !q.trim().is_empty() {
            return Ok(q);
        }
    }
    if io::stdin().is_terminal() {
        anyhow::bail!("a metric query is required (pass it as an argument or on stdin)");
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    let trimmed = buf.trim();
    if trimmed.is_empty() {
        anyhow::bail!("a metric query is required (pass it as an argument or on stdin)");
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response_from(json: &str) -> TimeseriesResponse {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn flatten_zips_series_with_shared_times() {
        let resp = response_from(
            r#"{"data":{"attributes":{
                "series":[
                    {"group_tags":["feed:positions"]},
                    {"group_tags":["feed:account_balances"]}
                ],
                "times":[1747000000000,1747086400000],
                "values":[[2901,null],[1483,1490]]
            }}}"#,
        );
        let pts = flatten(&resp);
        assert_eq!(pts.len(), 4);
        assert_eq!(pts[0].scope, "feed:positions");
        assert_eq!(pts[0].value, Some(2901.0));
        assert_eq!(pts[0].timestamp_ms, 1747000000000);
        assert!(pts[0].timestamp.contains('T'));
        assert_eq!(pts[1].value, None); // null bucket stays null
        assert_eq!(pts[2].scope, "feed:account_balances");
        assert_eq!(pts[3].value, Some(1490.0));
    }

    #[test]
    fn flatten_defaults_scope_when_ungrouped() {
        let resp = response_from(
            r#"{"data":{"attributes":{
                "series":[{"group_tags":[]}],
                "times":[1747000000000],
                "values":[[22]]
            }}}"#,
        );
        let pts = flatten(&resp);
        assert_eq!(pts[0].scope, "*");
        assert_eq!(pts[0].value, Some(22.0));
    }

    #[test]
    fn flatten_handles_empty_response() {
        assert!(flatten(&TimeseriesResponse::default()).is_empty());
    }

    #[test]
    fn format_value_drops_trailing_zero() {
        assert_eq!(format_value(2901.0), "2901");
        assert_eq!(format_value(2.5), "2.5");
    }
}
