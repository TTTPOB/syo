use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient};

use super::util::{anyhow_to_mcp, ensure_object, optional_u64, required_string, with_hint};

pub async fn ls_tags(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let _ = ensure_object(args)?;
    let tags = siyuan_model::tag::list_tags(client)
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "tags": tags }),
        "Tag list is derived from the SQL index and is eventually consistent. Freshly-tagged \
         blocks may take ~100–500 ms to appear here. Pass each tag to siyuan_tag_search \
         (without the surrounding # characters) to find tagged blocks.",
    ))
}

pub async fn search_by_tag(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let tag = required_string(&map, "tag")?;
    // Cap user-supplied limit to MAX_SEARCH_LIMIT (mirrors siyuan_search_text)
    // so a pathological caller cannot ask the kernel for an unbounded result
    // set. `limit == 0` is rejected up front as invalid_params: the model
    // layer's bail! reaches the user as a clear validation error rather than
    // being silently promoted to 1.
    let raw_limit = optional_u64(&map, "limit").unwrap_or(50);
    if raw_limit == 0 {
        return Err(McpError::invalid_params(
            siyuan_model::tag::ZERO_LIMIT_ERR.to_string(),
            None,
        ));
    }
    let limit = raw_limit.min(MAX_SEARCH_LIMIT) as usize;

    let hits = siyuan_model::tag::search_by_tag(client, &tag, limit)
        .await
        .map_err(anyhow_to_mcp)?;
    let hits_json =
        serde_json::to_value(hits).map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(with_hint(
        json!({ "hits": hits_json }),
        "Results are eventually consistent with the SQL index. Blocks tagged very recently may \
         not appear yet. The `limit` argument is capped server-side at 1000. Use siyuan_doc_get \
         or siyuan_sql to verify freshly-tagged blocks.",
    ))
}
