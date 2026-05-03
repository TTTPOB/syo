use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{anyhow_to_mcp, ensure_object, optional_u64, required_string, with_hint};

pub async fn ls_tags(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let _ = ensure_object(args)?;
    let output = syo_core::tag::list_tags(client)
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "tags": output.tags }),
        "Tag list is derived from the SQL index and is eventually consistent. Freshly-tagged \
         blocks may take ~100–500 ms to appear here. Pass each tag to syo_siyuan_tag_search \
         (without the surrounding # characters) to find tagged blocks.",
    ))
}

pub async fn search_by_tag(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let tag = required_string(&map, "tag")?;
    // Cap user-supplied limit to MAX_SEARCH_LIMIT (mirrors syo_siyuan_search)
    // so a pathological caller cannot ask the kernel for an unbounded result
    // set. `limit == 0` is rejected up front as invalid_params: the model
    // layer's bail! reaches the user as a clear validation error rather than
    // being silently promoted to 1.
    let raw_limit = optional_u64(&map, "limit").unwrap_or(50);
    if raw_limit == 0 {
        return Err(McpError::invalid_params("limit must not be zero", None));
    }
    let limit = raw_limit as usize;

    let output =
        syo_core::tag::search_by_tag(client, syo_core::tag::SearchByTagInput { tag, limit })
            .await
            .map_err(anyhow_to_mcp)?;
    let hits_json = serde_json::to_value(output.hits)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(with_hint(
        json!({ "hits": hits_json }),
        "Results are eventually consistent with the SQL index. Blocks tagged very recently may \
         not appear yet. The `limit` argument is capped server-side at 1000. Use syo_siyuan_doc_get \
         or syo_siyuan_sql to verify freshly-tagged blocks.",
    ))
}
