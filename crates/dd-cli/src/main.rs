mod cli;
mod logs;
mod output;
mod time_spec;

use std::process::ExitCode;

use clap::Parser;
use cli::{Cli, Command};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let exit = match run(cli).await {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err:#}");
            err.downcast_ref::<dd_api::ApiError>()
                .map(|e| e.exit_code())
                .unwrap_or(1)
        }
    };
    ExitCode::from(exit as u8)
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("ddog={level},dd_api={level},dd_cli={level}")));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let cfg = dd_config::resolve(dd_config::Overrides {
        api_key: cli.api_key.clone(),
        app_key: cli.app_key.clone(),
        site: cli.site.clone(),
        profile: cli.profile.clone(),
        config_path: cli.config.clone(),
    })?;

    let client = dd_api::ClientBuilder::new(&cfg.api_key, &cfg.app_key)
        .site(&cfg.site)
        .build()?;

    let ctx = cli::Ctx {
        client,
        output: output::resolve_mode(cli.output.as_deref()),
    };

    match cli.command {
        Command::Logs(logs) => logs::dispatch(ctx, logs).await,
    }
}
