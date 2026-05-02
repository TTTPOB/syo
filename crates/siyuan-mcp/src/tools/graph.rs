use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_model::graph::Direction;
use siyuan_types::BlockId;

use super::util::{
    MAX_GRAPH_DEPTH, anyhow_to_mcp, ensure_object, optional_string, optional_u64, required_string,
    with_hint,
};

pub async fn neighborhood(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let center_str = required_string(&map, "center")?;
    let center = BlockId::parse(&center_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    // Cap depth so a pathological caller cannot ask the traversal to walk
    // millions of empty levels. The 500-node ceiling inside neighborhood()
    // bounds the working set, but iterating over depth itself is still O(depth).
    let depth = optional_u64(&map, "depth")
        .unwrap_or(1)
        .min(MAX_GRAPH_DEPTH) as usize;
    let direction_str = optional_string(&map, "direction");
    let direction = match direction_str.as_deref() {
        Some("outgoing") => Direction::Outgoing,
        Some("incoming") => Direction::Incoming,
        _ => Direction::Both,
    };

    let graph = siyuan_model::graph::neighborhood(client, &center, depth, direction)
        .await
        .map_err(anyhow_to_mcp)?;

    let truncated = graph.truncated;
    let graph_val =
        serde_json::to_value(graph).map_err(|e| McpError::internal_error(e.to_string(), None))?;

    let hint = if truncated {
        "Graph traversal hit the per-call node/edge limit (500 nodes / 1000 edges) — \
         `truncated` is true. The result is a partial view. Narrow the search by reducing \
         depth, switching to a single direction (outgoing or incoming), or querying a \
         more specific center block. Alternatively, use siyuan_sql to query the refs table \
         directly for unbounded results."
    } else {
        "Graph is complete within the requested depth and direction. `truncated` is false."
    };

    Ok(with_hint(json!(graph_val), hint))
}
