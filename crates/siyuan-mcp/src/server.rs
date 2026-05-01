use std::{collections::HashMap, sync::Arc};

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    service::RequestContext,
};
use siyuan_client::SiyuanClient;

use crate::registry::Handler;

pub struct SiyuanMcpServer {
    client: Arc<SiyuanClient>,
    tools: Vec<Tool>,
    handlers: HashMap<&'static str, Handler>,
}

impl SiyuanMcpServer {
    pub fn new(
        client: Arc<SiyuanClient>,
        tools: Vec<Tool>,
        handlers: HashMap<&'static str, Handler>,
    ) -> Self {
        Self {
            client,
            tools,
            handlers,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let ct = self
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| anyhow::anyhow!("MCP stdio transport failed: {e}"))?;
        ct.waiting().await?;
        Ok(())
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
        let name = request.name.as_ref();
        let handler = self
            .handlers
            .get(name)
            .ok_or_else(|| McpError::invalid_params(format!("unknown tool: {name}"), None))?;

        let args = request
            .arguments
            .map(serde_json::Value::Object)
            .unwrap_or(serde_json::Value::Null);

        let result = handler(Arc::clone(&self.client), args).await?;
        let content = Content::json(result)?;
        Ok(CallToolResult::success(vec![content]))
    }
}
