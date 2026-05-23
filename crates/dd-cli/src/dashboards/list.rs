use clap::Args;

use crate::cli::Ctx;
use crate::output::{emit_json, emit_ndjson_each, emit_table_rows, OutputMode};

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Filter by title substring (client-side; the API has no title search).
    #[arg(long)]
    pub query: Option<String>,
}

pub async fn run(ctx: Ctx, args: ListArgs) -> anyhow::Result<()> {
    let mut dashboards = ctx.client.dashboards_list().await?.dashboards;

    if let Some(q) = args.query.as_deref() {
        let needle = q.to_lowercase();
        dashboards.retain(|d| {
            d.title
                .as_deref()
                .map(|t| t.to_lowercase().contains(&needle))
                .unwrap_or(false)
        });
    }

    match ctx.output {
        OutputMode::Json => emit_json(&dashboards)?,
        OutputMode::Ndjson => emit_ndjson_each(&dashboards)?,
        OutputMode::Text | OutputMode::Table => {
            let rows = dashboards
                .iter()
                .map(|d| {
                    vec![
                        d.id.clone(),
                        d.title.clone().unwrap_or_else(|| "-".into()),
                        d.url.clone().unwrap_or_else(|| "-".into()),
                    ]
                })
                .collect();
            emit_table_rows(&["id", "title", "url"], rows);
        }
    }
    Ok(())
}
