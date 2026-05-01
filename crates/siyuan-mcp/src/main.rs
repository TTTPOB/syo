use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use tracing::warn;

mod registry;
mod server;
mod tools;

/// Args parsed from the command line / environment.
#[derive(Parser, Debug)]
#[command(name = "siyuan-mcp", version, about = "MCP server for SiYuan")]
struct Args {
    /// Base URL of the SiYuan kernel HTTP API.
    #[arg(long, env = "SIYUAN_BASE_URL", default_value = "http://127.0.0.1:6806")]
    base_url: String,

    /// API token (Authorization: Token <value>). Warn at startup if absent.
    #[arg(long, env = "SIYUAN_TOKEN")]
    token: Option<String>,

    /// HTTP request timeout in milliseconds (reserved for future use).
    #[arg(long, env = "SIYUAN_TIMEOUT_MS", default_value_t = 30000)]
    timeout_ms: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing must go to stderr so it doesn't pollute the stdio JSON-RPC channel.
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();

    let args = Args::parse();
    let _ = args.timeout_ms; // reserved; reqwest uses its own default for now

    if args.token.is_none() {
        warn!("--token / SIYUAN_TOKEN not set; API calls requiring auth will fail");
    }

    let client =
        siyuan_client::SiyuanClient::new(&args.base_url, args.token.as_deref().unwrap_or(""))
            .with_context(|| format!("failed to build SiyuanClient for {}", args.base_url))?;

    let client = Arc::new(client);
    let (tools, handlers) = registry::build(Arc::clone(&client));
    let srv = server::SiyuanMcpServer::new(client, tools, handlers);

    srv.run().await
}
