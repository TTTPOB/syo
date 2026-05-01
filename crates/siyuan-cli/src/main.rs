mod commands;
mod config;
mod output;

use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "siyuan", version, about = "Agent harness for SiYuan")]
struct Cli {
    /// Base URL of the SiYuan kernel HTTP API.
    #[arg(long, env = "SIYUAN_BASE_URL", global = true)]
    base_url: Option<String>,

    /// API token (Authorization: Token <value>).
    #[arg(long, env = "SIYUAN_TOKEN", global = true)]
    token: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print kernel version (smoke test).
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let cfg = config::Config::resolve(cli.base_url, cli.token)?;
    let client = cfg.into_client()?;

    match cli.cmd {
        Cmd::Status => {
            let v = client.system_version().await?;
            info!(%v, "siyuan ok");
            println!("{v}");
        }
    }
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
}
