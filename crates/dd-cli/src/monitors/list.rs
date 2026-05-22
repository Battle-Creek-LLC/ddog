use clap::Args;
use dd_api::monitors::MonitorListFilter;

use crate::cli::Ctx;
use crate::output::{emit_json, emit_ndjson_each, emit_table_rows, OutputMode};

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Filter by monitor name substring.
    #[arg(long)]
    pub name: Option<String>,

    /// Filter by scope tag, e.g. `service:nodejs-worker`.
    #[arg(long)]
    pub tag: Option<String>,

    /// Filter by a tag set on the monitor object itself.
    #[arg(long)]
    pub monitor_tag: Option<String>,
}

pub async fn run(ctx: Ctx, args: ListArgs) -> anyhow::Result<()> {
    let filter = MonitorListFilter {
        name: args.name,
        tags: args.tag,
        monitor_tags: args.monitor_tag,
    };
    let monitors = ctx.client.monitors_list(&filter).await?;

    match ctx.output {
        OutputMode::Json => emit_json(&monitors)?,
        OutputMode::Ndjson => emit_ndjson_each(&monitors)?,
        OutputMode::Text | OutputMode::Table => {
            let rows = monitors
                .iter()
                .map(|m| {
                    vec![
                        m.id.to_string(),
                        m.name.clone().unwrap_or_else(|| "-".into()),
                        m.overall_state.clone().unwrap_or_else(|| "-".into()),
                        m.tags.join(","),
                    ]
                })
                .collect();
            emit_table_rows(&["id", "name", "overall_state", "tags"], rows);
        }
    }
    Ok(())
}
