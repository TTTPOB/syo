use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use clap::Args;
use tracing::warn;

use crate::config::Config;

/// Arguments for the `serve-mcp` subcommand.
///
/// `--base-url` and `--token` are inherited as global flags on the top-level
/// `siyuan` CLI; this struct only adds MCP-specific options.
#[derive(Args, Debug)]
pub struct ServeMcpArgs {
    /// HTTP request timeout in milliseconds. Pass 0 to disable the timeout
    /// entirely (useful when the caller imposes its own deadline).
    #[arg(long, env = "SIYUAN_TIMEOUT_MS", default_value_t = 30000)]
    pub timeout_ms: u64,
}

/// Run the SiYuan MCP server on stdio until the client disconnects.
///
/// This subcommand bypasses [`Config::into_client`] because it needs to apply
/// the `--timeout-ms` override directly on the client builder. All tracing
/// must already be routed to stderr by `init_tracing` so the stdio JSON-RPC
/// channel stays clean.
pub async fn run(cfg: Config, args: ServeMcpArgs) -> Result<()> {
    if cfg.token.is_empty() {
        warn!("--token / SIYUAN_TOKEN not set; API calls requiring auth will fail");
    }

    // 0 -> Duration::ZERO, which SiyuanClient interprets as "no timeout".
    let timeout = Duration::from_millis(args.timeout_ms);
    let client = siyuan_client::SiyuanClient::new_with_timeout(&cfg.base_url, &cfg.token, timeout)
        .with_context(|| format!("failed to build SiyuanClient for {}", cfg.base_url))?;

    siyuan_mcp::serve(Arc::new(client)).await
}
