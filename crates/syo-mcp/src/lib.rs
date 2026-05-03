//! MCP server library exposing the SiYuan harness over rmcp's stdio transport.
//!
//! The single public entry point is [`serve`], which builds the tool registry
//! and runs the JSON-RPC server until the client disconnects. The caller owns
//! the [`siyuan_client::SiyuanClient`] (and thus its timeout configuration);
//! this library only consumes it.

use std::sync::Arc;

use siyuan_client::SiyuanClient;

mod registry;
mod server;
mod tools;

/// Run the SiYuan MCP server on stdio until the client disconnects.
///
/// The provided client carries its own timeout configuration; configure it
/// before calling this function. Tracing output should be directed to stderr
/// by the caller — stdout is reserved for JSON-RPC framing.
pub async fn serve(client: Arc<SiyuanClient>) -> anyhow::Result<()> {
    let (tools, handlers) = registry::build(Arc::clone(&client));
    let srv = server::SyoMcpServer::new(client, tools, handlers);
    srv.run().await
}
