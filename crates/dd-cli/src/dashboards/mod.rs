mod get;
mod list;

use clap::{Args, Subcommand};

use crate::cli::Ctx;

#[derive(Debug, Args)]
pub struct DashboardsArgs {
    #[command(subcommand)]
    pub command: DashboardsCommand,
}

#[derive(Debug, Subcommand)]
pub enum DashboardsCommand {
    /// Fetch a dashboard definition (incl. widget queries).
    Get(get::GetArgs),
    /// List dashboards.
    List(list::ListArgs),
}

pub async fn dispatch(ctx: Ctx, args: DashboardsArgs) -> anyhow::Result<()> {
    match args.command {
        DashboardsCommand::Get(a) => get::run(ctx, a).await,
        DashboardsCommand::List(a) => list::run(ctx, a).await,
    }
}
