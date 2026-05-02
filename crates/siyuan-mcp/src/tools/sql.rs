use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_like};

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
    // Cap user-supplied limit to MAX_SEARCH_LIMIT so a pathological caller
    // can't ask the kernel for an unbounded result set.
    let limit = optional_u64(&map, "limit")
        .unwrap_or(50)
        .min(MAX_SEARCH_LIMIT);

    // Escape both single quotes (for the SQL string literal) and LIKE meta-
    // characters %, _, \ (so a substring search behaves as a substring
    // search even when the query contains those characters). The matching
    // ESCAPE '\\' clause makes our backslash-escapes effective.
    let escaped = escape_sql_like(&query);
    let stmt = format!(
        "SELECT id, root_id, markdown FROM blocks \
         WHERE markdown LIKE '%{escaped}%' ESCAPE '\\' LIMIT {limit}"
    );

    let rows = client.sql(&stmt).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "hits": rows }),
        "Results are SQL LIKE substring matches (case-insensitive on most SQLite builds). \
         The query searches block markdown content. Results may lag recent mutations by \
         ~100–500 ms. If too many results are returned, narrow the query or lower the limit. \
         The `limit` argument is capped server-side at 1000.",
    ))
}
