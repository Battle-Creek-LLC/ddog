use clap::Args;

use crate::cli::Ctx;
use crate::output::{emit_json, emit_text_event, OutputMode};

use super::{parse_fields, parse_indexes};

#[derive(Debug, Args)]
pub struct GetArgs {
    /// Log event ID.
    pub event_id: String,

    /// Comma-separated index names to search within.
    #[arg(long)]
    pub index: Option<String>,

    /// Comma-separated attribute paths for text output.
    #[arg(long)]
    pub fields: Option<String>,
}

pub async fn run(ctx: Ctx, args: GetArgs) -> anyhow::Result<()> {
    let ev = ctx
        .client
        .logs_get(&args.event_id, parse_indexes(args.index))
        .await?;

    match ctx.output {
        OutputMode::Json | OutputMode::Table | OutputMode::Ndjson => emit_json(&ev)?,
        OutputMode::Text => emit_text_event(&ev, &parse_fields(args.fields)),
    }
    Ok(())
}
