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
    // Reject empty / whitespace-only queries explicitly: a blank LIKE
    // pattern matches every block in the database and produces a result
    // set that is always trimmed by `limit`, masking the misuse.
    if query.trim().is_empty() {
        return Err(McpError::invalid_params("`query` must not be empty", None));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_client() -> SiyuanClient {
        SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds")
    }

    #[tokio::test]
    async fn search_text_rejects_whitespace_query() {
        let client = dummy_client();
        let args = json!({ "query": "   " });
        let err = search_text(&client, args)
            .await
            .expect_err("whitespace query must be rejected");
        assert!(
            err.message.contains("query"),
            "error message should reference `query`; got: {}",
            err.message
        );
    }
}
