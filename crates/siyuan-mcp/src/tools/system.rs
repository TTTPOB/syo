use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{ensure_object, siyuan_to_mcp};

pub async fn status(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let _ = ensure_object(args)?;
    let version = client.system_version().await.map_err(siyuan_to_mcp)?;
    Ok(json!({ "version": version }))
}
