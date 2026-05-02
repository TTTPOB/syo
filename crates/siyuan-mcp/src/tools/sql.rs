use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_like};
use siyuan_model::sql_guard;

use super::util::{ensure_object, optional_u64, required_string, siyuan_to_mcp, with_hint};

pub async fn raw_sql(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let stmt = required_string(&map, "stmt")?;

    // AST-level read-only guard. The kernel does NOT enforce read-only at
    // the SQL level (see security advisories GHSA-jqwg-75qf-vmf9 and
    // GHSA-j7wh-x834-p3r7), so this check is the actual gate, not just a
    // UX nicety. Reject anything that is not a single Query / Explain-of-
    // Query node before any kernel round trip.
    if let Err(e) = sql_guard::validate_read_only(&stmt) {
        return Err(McpError::invalid_params(format!("`stmt`: {e}"), None));
    }

    let rows = client.sql(&stmt).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "rows": rows }),
        "Power tool: results are raw rows from the SiYuan SQLite database. Some columns may be \
         unstable internal fields. Results reflect the SQL index which may lag mutations by \
         ~100–500 ms. The kernel does not enforce SQL-level read-only — this server validates \
         the statement AST locally and rejects writes before any round trip.",
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

    // Escape single quotes for SQL string-literal safety. The SiYuan
    // kernel's SQL engine does not support ESCAPE '\' in LIKE patterns,
    // so % and _ in user input behave as LIKE wildcards.
    let escaped = escape_sql_like(&query);
    let stmt = format!(
        "SELECT id, root_id, markdown FROM blocks \
         WHERE markdown LIKE '%{escaped}%' LIMIT {limit}"
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

    #[tokio::test]
    async fn raw_sql_rejects_non_select_stmt() {
        // The dummy client points at an unreachable port: if the AST guard
        // regresses, this test would surface a network error instead of
        // `invalid_params`. Pinning the assertion to the message keeps that
        // contract obvious.
        let client = dummy_client();
        let args = json!({ "stmt": "DROP TABLE blocks" });
        let err = raw_sql(&client, args)
            .await
            .expect_err("non-SELECT stmt must be rejected client-side");
        assert!(
            err.message.contains("read-only"),
            "error message should mention the read-only requirement; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn raw_sql_accepts_leading_whitespace_select() {
        // `"\n\n  SELECT 1"` should pass the AST guard and proceed to the
        // HTTP layer (which then fails because the dummy client cannot
        // connect). This pins the parser's whitespace tolerance so a future
        // regression cannot accidentally tighten the validator.
        let client = dummy_client();
        let args = json!({ "stmt": "\n\n  SELECT 1" });
        let err = raw_sql(&client, args)
            .await
            .expect_err("dummy client cannot reach the kernel");
        assert!(
            !err.message.contains("read-only"),
            "leading-whitespace SELECT must clear the read-only guard; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn raw_sql_rejects_with_tail_delete() {
        // The whole reason we upgraded from a leading-keyword check to AST
        // validation: this statement starts with `WITH` but executes a
        // DELETE. The AST guard sees the SetExpr::Delete under the WITH and
        // rejects it before any kernel round trip.
        let client = dummy_client();
        let args = json!({
            "stmt": "WITH x AS (SELECT id FROM blocks) DELETE FROM blocks WHERE id IN (SELECT id FROM x)"
        });
        let err = raw_sql(&client, args)
            .await
            .expect_err("WITH-tail DELETE must be rejected by the AST guard");
        assert!(
            err.message.contains("DELETE"),
            "error should name the rejected operation; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn raw_sql_rejects_multi_statement() {
        let client = dummy_client();
        let args = json!({ "stmt": "SELECT 1; DROP TABLE blocks" });
        let err = raw_sql(&client, args)
            .await
            .expect_err("multi-statement input must be rejected");
        assert!(
            err.message.contains("single statement"),
            "error should name the constraint; got: {}",
            err.message
        );
    }
}
