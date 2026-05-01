use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{anyhow_to_mcp, ensure_object, required_string};

pub async fn ls_tags(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let _ = ensure_object(args)?;
    let tags = siyuan_model::tag::list_tags(client)
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(json!({ "tags": tags }))
}

pub async fn search_by_tag(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let tag = required_string(&map, "tag")?;

    let hits = siyuan_model::tag::search_by_tag(client, &tag)
        .await
        .map_err(anyhow_to_mcp)?;
    let hits_json =
        serde_json::to_value(hits).map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(json!({ "hits": hits_json }))
}
