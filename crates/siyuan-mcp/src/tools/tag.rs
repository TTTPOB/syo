use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{anyhow_to_mcp, ensure_object, required_string, with_hint};

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

    let hits = siyuan_model::tag::search_by_tag(client, &tag)
        .await
        .map_err(anyhow_to_mcp)?;
    let hits_json =
        serde_json::to_value(hits).map_err(|e| McpError::internal_error(e.to_string(), None))?;
    Ok(with_hint(
        json!({ "hits": hits_json }),
        "Results are eventually consistent with the SQL index. Blocks tagged very recently may \
         not appear yet. Use siyuan_get_doc or siyuan_sql to verify freshly-tagged blocks.",
    ))
}
