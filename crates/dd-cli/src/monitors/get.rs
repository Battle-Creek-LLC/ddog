use clap::Args;

use crate::cli::Ctx;
use crate::output::{emit_json, OutputMode};

#[derive(Debug, Args)]
pub struct GetArgs {
    /// Monitor ID.
    pub monitor_id: i64,
}

pub async fn run(ctx: Ctx, args: GetArgs) -> anyhow::Result<()> {
    let m = ctx.client.monitor_get(args.monitor_id).await?;

    match ctx.output {
        OutputMode::Json | OutputMode::Ndjson | OutputMode::Table => emit_json(&m)?,
        OutputMode::Text => {
            let name = m.name.as_deref().unwrap_or("-");
            let state = m.overall_state.as_deref().unwrap_or("-");
            println!("{}  {state}  {name}", m.id);
            if !m.tags.is_empty() {
                println!("tags: {}", m.tags.join(","));
            }
            if let Some(q) = m.query.as_deref() {
                println!("query: {q}");
            }
            if let Some(msg) = m.message.as_deref() {
                if !msg.is_empty() {
                    println!("message: {msg}");
                }
            }
        }
    }
    Ok(())
}
