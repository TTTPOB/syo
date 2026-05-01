use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{ensure_object, optional_u64, required_string, siyuan_to_mcp, with_hint};

pub async fn raw_sql(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let stmt = required_string(&map, "stmt")?;

    let rows = client.sql(&stmt).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "rows": rows }),
        "Power tool: results are raw rows from the SiYuan SQLite database. Some columns may be \
         unstable internal fields. Results reflect the SQL index which may lag mutations by \
         ~100–500 ms. This is read-only; do not issue INSERT/UPDATE/DELETE.",
    ))
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
    Ok(with_hint(
        json!({ "hits": rows }),
        "Results are SQL LIKE substring matches (case-insensitive on most SQLite builds). \
         The query searches block markdown content. Results may lag recent mutations by \
         ~100–500 ms. If too many results are returned, narrow the query or lower the limit.",
    ))
}
