use clap::Args;
use serde_json::Value;

use crate::cli::Ctx;
use crate::output::{emit_json, OutputMode};

#[derive(Debug, Args)]
pub struct GetArgs {
    /// Dashboard ID (e.g. `abc-def-ghi`).
    pub dashboard_id: String,
}

pub async fn run(ctx: Ctx, args: GetArgs) -> anyhow::Result<()> {
    let dash = ctx.client.dashboard_get(&args.dashboard_id).await?;

    match ctx.output {
        // The raw definition is the point of `get` — emit it whole.
        OutputMode::Json | OutputMode::Ndjson => emit_json(&dash)?,
        OutputMode::Text | OutputMode::Table => print_summary(&dash),
    }
    Ok(())
}

/// A one-line orientation summary; the full tree only makes sense as JSON.
fn print_summary(dash: &Value) {
    let id = dash.get("id").and_then(Value::as_str).unwrap_or("-");
    let title = dash.get("title").and_then(Value::as_str).unwrap_or("-");
    let url = dash.get("url").and_then(Value::as_str).unwrap_or("-");
    let widgets = dash
        .get("widgets")
        .and_then(Value::as_array)
        .map(|w| w.len())
        .unwrap_or(0);
    println!("{id}  {title}  ({widgets} widgets)  {url}");
    eprintln!("(use -o json to read widget queries)");
}
