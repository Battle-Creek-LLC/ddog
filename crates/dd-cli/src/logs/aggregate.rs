use std::str::FromStr;

use clap::Args;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use dd_api::logs::{AggregateRequest, Bucket, Compute, GroupBy, SearchFilter, StorageTier};
use serde::Serialize;
use serde_json::Value;

use crate::cli::Ctx;
use crate::output::{emit_json, emit_ndjson_each, OutputMode};
use crate::time_spec;

use super::parse_indexes;

#[derive(Debug, Args)]
pub struct AggregateArgs {
    /// Datadog log query.
    pub query: Option<String>,

    #[arg(long, default_value = "now-1h")]
    pub from: String,

    #[arg(long, default_value = "now")]
    pub to: String,

    /// Comma-separated index names.
    #[arg(long)]
    pub index: Option<String>,

    /// Storage tier: indexes | online-archives | flex.
    #[arg(long, default_value = "indexes")]
    pub storage: String,

    /// Facet to group by (repeatable). e.g. `--group-by service`.
    #[arg(long = "group-by")]
    pub group_by: Vec<String>,

    /// Limit per group (default 10).
    #[arg(long, default_value_t = 10)]
    pub group_limit: u32,

    /// Measure to compute. `count` or `<agg>:<facet>` (e.g. `avg:@duration`).
    /// Repeatable. Defaults to `count` if none given.
    #[arg(long = "measure")]
    pub measure: Vec<String>,

    /// Bucket interval as a duration (`1d`, `1h`, `15m`, …). When set, each
    /// measure is computed as a timeseries and results are returned per
    /// bucket (one row per time bucket × measure × group). When omitted,
    /// a single total per group is returned (unchanged).
    #[arg(long)]
    pub interval: Option<String>,
}

pub async fn run(ctx: Ctx, args: AggregateArgs) -> anyhow::Result<()> {
    let storage = StorageTier::from_str(&args.storage).map_err(anyhow::Error::msg)?;
    let from = time_spec::normalize(&args.from)?;
    let to = time_spec::normalize(&args.to)?;

    // Validate the interval up front (rejects bad units like `1y`) and keep the
    // duration string — the aggregate API takes it verbatim, unlike the metrics
    // endpoint which wants milliseconds.
    let interval = match args.interval.as_deref() {
        Some(d) => {
            time_spec::duration_secs(d)?;
            Some(d.trim().to_string())
        }
        None => None,
    };

    // Labels in the same order as the computes, so c0/c1/… map back to a
    // human-readable measure name in the output.
    let measure_labels: Vec<String> = if args.measure.is_empty() {
        vec!["count".into()]
    } else {
        args.measure.clone()
    };

    let mut computes = if args.measure.is_empty() {
        vec![Compute { aggregation: "count".into(), metric: None, r#type: None, interval: None }]
    } else {
        args.measure.iter().map(|m| parse_measure(m)).collect::<Result<Vec<_>, _>>()?
    };

    if let Some(iv) = &interval {
        for c in &mut computes {
            c.r#type = Some("timeseries".into());
            c.interval = Some(iv.clone());
        }
    }

    let group_by = args
        .group_by
        .iter()
        .map(|f| GroupBy {
            facet: f.clone(),
            limit: Some(args.group_limit),
        })
        .collect();

    let req = AggregateRequest {
        filter: SearchFilter {
            from: Some(from),
            to: Some(to),
            query: args.query,
            indexes: parse_indexes(args.index),
            storage_tier: Some(storage),
        },
        compute: computes,
        group_by,
        options: None,
    };

    let resp = ctx.client.logs_aggregate(&req).await?;
    let buckets = resp.data.map(|d| d.buckets).unwrap_or_default();

    if interval.is_some() {
        let rows = flatten_timeseries(&buckets, &measure_labels);
        match ctx.output {
            OutputMode::Ndjson => emit_ndjson_each(&rows)?,
            OutputMode::Json => emit_json(&rows)?,
            OutputMode::Table | OutputMode::Text => render_ts_table(&rows),
        }
    } else {
        match ctx.output {
            OutputMode::Json | OutputMode::Ndjson => emit_json(&buckets)?,
            OutputMode::Table | OutputMode::Text => render_table(&buckets),
        }
    }
    Ok(())
}

fn parse_measure(raw: &str) -> anyhow::Result<Compute> {
    if raw == "count" {
        return Ok(Compute { aggregation: "count".into(), metric: None, r#type: None, interval: None });
    }
    let (agg, metric) = raw
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid measure '{raw}' (use 'count' or '<agg>:<facet>')"))?;
    Ok(Compute {
        aggregation: agg.to_string(),
        metric: Some(metric.to_string()),
        r#type: None,
        interval: None,
    })
}

/// One flattened timeseries row: which group (`by`), which measure, the bucket
/// start time, and the value for that bucket.
#[derive(Debug, Serialize)]
struct TsRow {
    /// Group-by tag values for this bucket, e.g. `{"feed": "positions"}`.
    /// Empty (`{}`) when no `--group-by` is used.
    by: Value,
    /// Measure label, mirroring the `--measure` string (`count`, `avg:@duration`).
    measure: String,
    /// Bucket start time, as returned by Datadog (RFC-3339 string).
    time: String,
    /// Aggregated value for the bucket; `null` where the bucket has no data.
    value: Value,
}

/// Expand each bucket's per-measure timeseries (`computes.c{i}` → an array of
/// `{time, value}`) into one row per (group, measure, bucket). Compute keys are
/// `c0`, `c1`, … in `--measure` order, so we index by position rather than rely
/// on map ordering.
fn flatten_timeseries(buckets: &[Bucket], measures: &[String]) -> Vec<TsRow> {
    let mut rows = Vec::new();
    for b in buckets {
        for (i, measure) in measures.iter().enumerate() {
            let key = format!("c{i}");
            let Some(points) = b.computes.get(key.as_str()).and_then(Value::as_array) else {
                continue;
            };
            for p in points {
                rows.push(TsRow {
                    by: b.by.clone(),
                    measure: measure.clone(),
                    time: p
                        .get("time")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    value: p.get("value").cloned().unwrap_or(Value::Null),
                });
            }
        }
    }
    rows
}

fn render_table(buckets: &[dd_api::logs::Bucket]) {
    if buckets.is_empty() {
        println!("(no results)");
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["group", "computes"]);

    for b in buckets {
        table.add_row(vec![
            serde_json::to_string(&b.by).unwrap_or_default(),
            serde_json::to_string(&b.computes).unwrap_or_default(),
        ]);
    }
    println!("{table}");
}

fn render_ts_table(rows: &[TsRow]) {
    if rows.is_empty() {
        println!("(no results)");
        return;
    }
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(vec!["group", "measure", "time", "value"]);

    for r in rows {
        table.add_row(vec![
            serde_json::to_string(&r.by).unwrap_or_default(),
            r.measure.clone(),
            r.time.clone(),
            value_to_string(&r.value),
        ]);
    }
    println!("{table}");
}

/// Render a JSON value for the table's value column. Numbers print as-is (no
/// forced `.0` on integers, since serde keeps the parsed form); a missing/`null`
/// bucket shows `—`, matching `metrics query`.
fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => "—".to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buckets_from(json: &str) -> Vec<Bucket> {
        let resp: dd_api::logs::AggregateResponse = serde_json::from_str(json).unwrap();
        resp.data.map(|d| d.buckets).unwrap_or_default()
    }

    #[test]
    fn flatten_expands_each_group_per_bucket() {
        let buckets = buckets_from(
            r#"{"data":{"buckets":[
                {"by":{"feed":"positions"},"computes":{"c0":[
                    {"value":100,"time":"2026-05-19T00:00:00.000Z"},
                    {"value":120,"time":"2026-05-20T00:00:00.000Z"}
                ]}},
                {"by":{"feed":"balances"},"computes":{"c0":[
                    {"value":5,"time":"2026-05-19T00:00:00.000Z"}
                ]}}
            ]}}"#,
        );
        let rows = flatten_timeseries(&buckets, &["count".to_string()]);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].by["feed"], "positions");
        assert_eq!(rows[0].measure, "count");
        assert_eq!(rows[0].time, "2026-05-19T00:00:00.000Z");
        assert_eq!(rows[0].value, 100);
        assert_eq!(rows[1].value, 120);
        assert_eq!(rows[2].by["feed"], "balances");
        assert_eq!(rows[2].value, 5);
    }

    #[test]
    fn flatten_maps_compute_keys_to_measures_by_position() {
        let buckets = buckets_from(
            r#"{"data":{"buckets":[
                {"by":{},"computes":{
                    "c0":[{"value":7,"time":"2026-05-19T00:00:00.000Z"}],
                    "c1":[{"value":2.5,"time":"2026-05-19T00:00:00.000Z"}]
                }}
            ]}}"#,
        );
        let rows = flatten_timeseries(&buckets, &["count".to_string(), "avg:@duration".to_string()]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].measure, "count");
        assert_eq!(rows[0].value, 7);
        assert_eq!(rows[1].measure, "avg:@duration");
        assert_eq!(rows[1].value, 2.5);
    }

    #[test]
    fn flatten_preserves_null_value() {
        let buckets = buckets_from(
            r#"{"data":{"buckets":[
                {"by":{},"computes":{"c0":[{"value":null,"time":"2026-05-19T00:00:00.000Z"}]}}
            ]}}"#,
        );
        let rows = flatten_timeseries(&buckets, &["count".to_string()]);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].value.is_null());
        assert_eq!(value_to_string(&rows[0].value), "—");
    }

    #[test]
    fn flatten_handles_empty_and_missing_keys() {
        assert!(flatten_timeseries(&[], &["count".to_string()]).is_empty());
        // A measure with no matching compute key yields no rows for it.
        let buckets = buckets_from(r#"{"data":{"buckets":[{"by":{},"computes":{}}]}}"#);
        assert!(flatten_timeseries(&buckets, &["count".to_string()]).is_empty());
    }

    #[test]
    fn value_to_string_keeps_integers_clean() {
        assert_eq!(value_to_string(&Value::from(2901)), "2901");
        assert_eq!(value_to_string(&Value::from(2.5)), "2.5");
        assert_eq!(value_to_string(&Value::Null), "—");
    }
}
