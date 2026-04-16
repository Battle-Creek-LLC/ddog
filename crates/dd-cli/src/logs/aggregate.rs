use std::str::FromStr;

use clap::Args;
use comfy_table::{presets::UTF8_FULL, ContentArrangement, Table};
use dd_api::logs::{AggregateRequest, Compute, GroupBy, SearchFilter, StorageTier};

use crate::cli::Ctx;
use crate::output::{emit_json, OutputMode};
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
}

pub async fn run(ctx: Ctx, args: AggregateArgs) -> anyhow::Result<()> {
    let storage = StorageTier::from_str(&args.storage).map_err(anyhow::Error::msg)?;
    let from = time_spec::normalize(&args.from)?;
    let to = time_spec::normalize(&args.to)?;

    let computes = if args.measure.is_empty() {
        vec![Compute { aggregation: "count".into(), metric: None, r#type: None }]
    } else {
        args.measure.iter().map(|m| parse_measure(m)).collect::<Result<_, _>>()?
    };

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

    match ctx.output {
        OutputMode::Json | OutputMode::Ndjson => emit_json(&buckets)?,
        OutputMode::Table | OutputMode::Text => render_table(&buckets),
    }
    Ok(())
}

fn parse_measure(raw: &str) -> anyhow::Result<Compute> {
    if raw == "count" {
        return Ok(Compute { aggregation: "count".into(), metric: None, r#type: None });
    }
    let (agg, metric) = raw
        .split_once(':')
        .ok_or_else(|| anyhow::anyhow!("invalid measure '{raw}' (use 'count' or '<agg>:<facet>')"))?;
    Ok(Compute {
        aggregation: agg.to_string(),
        metric: Some(metric.to_string()),
        r#type: None,
    })
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
