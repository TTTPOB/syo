use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{anyhow_to_mcp, ensure_object};

pub async fn status(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let _ = ensure_object(args)?;
    let output = syo_core::system::status(client)
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(json!({ "version": output.version }))
}
