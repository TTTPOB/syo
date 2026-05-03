use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context as _, Result};
use clap::Args;
use tracing::warn;

use crate::config::Config;

/// Run the Model Context Protocol server on stdio until the client disconnects.
///
/// Sibling commands: every other subcommand is a one-shot CLI call against
/// the kernel; serve-mcp instead exposes those operations as MCP tools to
/// an LLM/agent over a stdio JSON-RPC channel. Use this when the caller is
/// a Claude Code-style agent harness.
///
/// Inputs:
///   --base-url, --token (global, inherited): see top-level `syo --help`.
///     Unlike other subcommands, serve-mcp tolerates a missing token — it
///     boots, logs a warning, and defers auth failures to per-tool kernel
///     calls.
///   --timeout-ms (optional, default 30000, env `SIYUAN_TIMEOUT_MS`):
///     per-HTTP-request timeout in milliseconds. Pass `0` to disable the
///     timeout entirely (useful when the caller imposes its own deadline).
///
/// Behaviour:
///   - Reads JSON-RPC frames on stdin, writes them on stdout. Tracing logs
///     are routed to stderr by `init_tracing` so they do not interfere
///     with the JSON-RPC channel — do NOT pipe stderr into stdin/stdout.
///   - The process runs until the client closes the channel.
///
/// Example:
///   in:  serve-mcp --timeout-ms 60000   (env: SIYUAN_TOKEN=xxx)
///   out: <stdio JSON-RPC traffic; LLM client drives requests>
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
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
