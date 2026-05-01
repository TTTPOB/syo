use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
    transport::stdio,
};
use tracing::warn;

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

    /// HTTP request timeout in milliseconds.
    #[arg(long, env = "SIYUAN_TIMEOUT_MS", default_value_t = 30000)]
    timeout_ms: u64,
}

// ---------------------------------------------------------------------------
// Server implementation
// ---------------------------------------------------------------------------

/// MCP server that wraps the SiYuan HTTP client.
/// The tool list is empty in this skeleton; tools are added in Task 9.
struct SiyuanMcpServer {
    _client: Arc<siyuan_client::SiyuanClient>,
    tools: Vec<Tool>,
}

impl SiyuanMcpServer {
    fn new(client: siyuan_client::SiyuanClient) -> Self {
        Self {
            _client: Arc::new(client),
            tools: vec![],
        }
    }
}

impl ServerHandler for SiyuanMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("siyuan-mcp", env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tools.clone()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        // No tools are registered yet; reject every call.
        Err(McpError::invalid_params(
            format!("unknown tool: {}", request.name),
            None,
        ))
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

    if args.token.is_none() {
        warn!("--token / SIYUAN_TOKEN not set; API calls requiring auth will fail");
    }

    let client =
        siyuan_client::SiyuanClient::new(&args.base_url, args.token.as_deref().unwrap_or(""))
            .with_context(|| format!("failed to build SiyuanClient for {}", args.base_url))?;

    let server = SiyuanMcpServer::new(client);

    let ct = server
        .serve(stdio())
        .await
        .context("failed to start MCP stdio transport")?;
    ct.waiting().await?;

    Ok(())
}
