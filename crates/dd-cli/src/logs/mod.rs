mod aggregate;
mod facets;
mod get;
mod search;
mod tail;

use clap::{Args, Subcommand};

use crate::cli::Ctx;

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[command(subcommand)]
    pub command: LogsCommand,
}

#[derive(Debug, Subcommand)]
pub enum LogsCommand {
    /// One-shot query over a time range.
    Search(search::SearchArgs),
    /// Stream new matching logs (polling).
    Tail(tail::TailArgs),
    /// Fetch a single log event by ID.
    Get(get::GetArgs),
    /// Run an aggregation query.
    Aggregate(aggregate::AggregateArgs),
    /// Discover facets (not yet implemented).
    Facets(facets::FacetsArgs),
}

pub async fn dispatch(ctx: Ctx, args: LogsArgs) -> anyhow::Result<()> {
    match args.command {
        LogsCommand::Search(a) => search::run(ctx, a).await,
        LogsCommand::Tail(a) => tail::run(ctx, a).await,
        LogsCommand::Get(a) => get::run(ctx, a).await,
        LogsCommand::Aggregate(a) => aggregate::run(ctx, a).await,
        LogsCommand::Facets(a) => facets::run(ctx, a).await,
    }
}

pub fn parse_fields(raw: Option<String>) -> Vec<String> {
    raw.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

pub fn parse_indexes(raw: Option<String>) -> Option<Vec<String>> {
    raw.map(|s| {
        s.split(',')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect()
    })
}
