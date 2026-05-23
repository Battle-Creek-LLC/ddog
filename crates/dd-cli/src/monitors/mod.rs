mod get;
mod list;

use clap::{Args, Subcommand};

use crate::cli::Ctx;

#[derive(Debug, Args)]
pub struct MonitorsArgs {
    #[command(subcommand)]
    pub command: MonitorsCommand,
}

#[derive(Debug, Subcommand)]
pub enum MonitorsCommand {
    /// List monitors with their overall state.
    List(list::ListArgs),
    /// Fetch a single monitor (adds query + message).
    Get(get::GetArgs),
}

pub async fn dispatch(ctx: Ctx, args: MonitorsArgs) -> anyhow::Result<()> {
    match args.command {
        MonitorsCommand::List(a) => list::run(ctx, a).await,
        MonitorsCommand::Get(a) => get::run(ctx, a).await,
    }
}
