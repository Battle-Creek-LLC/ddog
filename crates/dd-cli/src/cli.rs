use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::dashboards::DashboardsArgs;
use crate::logs::LogsArgs;
use crate::monitors::MonitorsArgs;
use crate::output::OutputMode;

#[derive(Debug, Parser)]
#[command(
    name = "ddog",
    version,
    about = "Datadog CLI for humans and agents",
    long_about = "Query Datadog logs (and more, later) from the terminal.\n\nGlobal auth is resolved from CLI flags > environment > profile > defaults."
)]
pub struct Cli {
    /// Datadog API key.
    #[arg(long, env = "DD_API_KEY", global = true, hide_env_values = true)]
    pub api_key: Option<String>,

    /// Datadog Application key.
    #[arg(long, env = "DD_APP_KEY", global = true, hide_env_values = true)]
    pub app_key: Option<String>,

    /// Datadog site (e.g. datadoghq.com, datadoghq.eu, us3.datadoghq.com).
    #[arg(long, env = "DD_SITE", global = true)]
    pub site: Option<String>,

    /// Named profile from the config file.
    #[arg(long, env = "DD_PROFILE", global = true)]
    pub profile: Option<String>,

    /// Path to the config file (default: XDG config dir).
    #[arg(long, env = "DD_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    /// Output mode: text | json | ndjson | table (auto-selected when unset).
    #[arg(short, long, env = "DD_OUTPUT", global = true)]
    pub output: Option<String>,

    /// Increase verbosity (repeatable: -v info, -vv debug, -vvv trace).
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Query Datadog logs.
    Logs(LogsArgs),
    /// Read Datadog dashboards.
    Dashboards(DashboardsArgs),
    /// Read Datadog monitors.
    Monitors(MonitorsArgs),
}

pub struct Ctx {
    pub client: dd_api::Client,
    pub output: OutputMode,
}
