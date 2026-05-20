use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{
    anyhow_to_mcp, ensure_object, optional_string, optional_u64, required_string, with_hint,
};

pub async fn raw_sql(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let stmt = required_string(&map, "stmt")?;

    let output = syo_core::sql::raw(client, syo_core::sql::SqlInput { stmt })
        .await
        .map_err(anyhow_to_mcp)?;
    let hint = if output.has_more {
        "Power tool: results are raw rows from the SiYuan SQLite database. This response is \
         truncated: more SQL rows exist. Add LIMIT/OFFSET to the original query and call again \
         to continue. Some columns may be unstable internal fields. Results reflect the SQL \
         index which may lag mutations by ~100–500 ms. The statement AST is validated locally \
         and writes are rejected before any round trip."
    } else if output.probe_applied {
        "Power tool: results are raw rows from the SiYuan SQLite database. No additional rows \
         were detected beyond this response. Some columns may be unstable internal fields. \
         Results reflect the SQL index which may lag mutations by ~100–500 ms. The statement \
         AST is validated locally and writes are rejected before any round trip."
    } else {
        "Power tool: results are raw rows from the SiYuan SQLite database. Some columns may be \
         unstable internal fields. Results reflect the SQL index which may lag mutations by \
         ~100–500 ms. The kernel does not enforce SQL-level read-only — this server validates \
         the statement AST locally and rejects writes before any round trip. Explicit LIMIT/FETCH \
         was present, so no extra more-results probe was applied."
    };
    Ok(with_hint(
        json!({
            "rows": output.rows,
            "limit": output.limit,
            "has_more": output.has_more,
            "more_results_probe_applied": output.probe_applied,
        }),
        hint,
    ))
}

pub async fn search(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let block_type = optional_string(&map, "type").unwrap_or_default();
    let contains = optional_string(&map, "contains").unwrap_or_default();
    let limit = optional_u64(&map, "limit").unwrap_or(50) as usize;

    let output = syo_core::search::search(
        client,
        syo_core::search::SearchInput {
            block_type,
            contains,
            limit,
        },
    )
    .await
    .map_err(anyhow_to_mcp)?;
    let hint = if output.has_more {
        "Results are SQL-filtered by block type (=) and/or content (LIKE). This response hit \
         the requested limit and more hits exist; call again with a larger `limit` to retrieve \
         more. Results may lag recent mutations by ~100–500 ms."
    } else {
        "Results are SQL-filtered by block type (=) and/or content (LIKE). No additional hits \
         were detected beyond this response. Results may lag recent mutations by ~100–500 ms."
    };
    Ok(with_hint(
        json!({
            "hits": output.hits,
            "limit": output.limit,
            "has_more": output.has_more,
        }),
        hint,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_client() -> SiyuanClient {
        SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds")
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
