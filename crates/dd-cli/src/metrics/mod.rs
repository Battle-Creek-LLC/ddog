mod query;

use clap::{Args, Subcommand};

use crate::cli::Ctx;

#[derive(Debug, Args)]
pub struct MetricsArgs {
    #[command(subcommand)]
    pub command: MetricsCommand,
}

#[derive(Debug, Subcommand)]
pub enum MetricsCommand {
    /// Query timeseries points for a metric query.
    Query(query::QueryArgs),
}

pub async fn dispatch(ctx: Ctx, args: MetricsArgs) -> anyhow::Result<()> {
    match args.command {
        MetricsCommand::Query(a) => query::run(ctx, a).await,
    }
}
