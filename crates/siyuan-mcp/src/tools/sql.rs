use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{ensure_object, optional_u64, required_string, siyuan_to_mcp};

pub async fn raw_sql(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let stmt = required_string(&map, "stmt")?;

    let rows = client.sql(&stmt).await.map_err(siyuan_to_mcp)?;
    Ok(json!({ "rows": rows }))
}

pub async fn search_text(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let query = required_string(&map, "query")?;
    let limit = optional_u64(&map, "limit").unwrap_or(50);

    // Escape single quotes to prevent SQL injection within the LIKE pattern.
    let escaped = query.replace('\'', "''");
    let stmt = format!(
        "SELECT id, root_id, markdown FROM blocks WHERE markdown LIKE '%{escaped}%' LIMIT {limit}"
    );

    let rows = client.sql(&stmt).await.map_err(siyuan_to_mcp)?;
    Ok(json!({ "hits": rows }))
}
