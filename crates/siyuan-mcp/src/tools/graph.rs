use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_model::graph::Direction;
use siyuan_types::BlockId;

use super::util::{anyhow_to_mcp, ensure_object, optional_string, optional_u64, required_string};

pub async fn neighborhood(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let center_str = required_string(&map, "center")?;
    let center = BlockId::parse(&center_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let depth = optional_u64(&map, "depth").unwrap_or(1) as usize;
    let direction_str = optional_string(&map, "direction");
    let direction = match direction_str.as_deref() {
        Some("outgoing") => Direction::Outgoing,
        Some("incoming") => Direction::Incoming,
        _ => Direction::Both,
    };

    let graph = siyuan_model::graph::neighborhood(client, &center, depth, direction)
        .await
        .map_err(anyhow_to_mcp)?;

    serde_json::to_value(graph)
        .map_err(|e| McpError::internal_error(e.to_string(), None))
        .map(|v| json!(v))
}
