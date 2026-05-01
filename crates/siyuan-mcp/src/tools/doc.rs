use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_model::pagination::PageRequest;
use siyuan_types::BlockId;

use super::util::{
    anyhow_to_mcp, ensure_object, optional_u64, required_string, siyuan_to_mcp, with_hint,
};

pub async fn get_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let page = optional_u64(&map, "page").unwrap_or(1) as usize;
    let page_size = optional_u64(&map, "page_size").unwrap_or(50) as usize;
    let format = map
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("agent-md");

    let bundle = siyuan_model::load::load_doc(client, &id, PageRequest { page, page_size })
        .await
        .map_err(anyhow_to_mcp)?;

    let total_pages = bundle.page.total_pages;
    let current_page = bundle.page.page;

    let content = match format {
        "json" => siyuan_render::json_bundle::render_bundle(&bundle, false)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?,
        "json-pretty" => siyuan_render::json_bundle::render_bundle(&bundle, true)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?,
        _ => siyuan_render::agent_md::render_doc(&bundle),
    };

    let payload = json!({ "format": format, "content": content });

    // Only add pagination hint when there are more pages to fetch.
    if total_pages > 1 {
        Ok(with_hint(
            payload,
            &format!(
                "Pagination: this is page {current_page} of {total_pages}. \
                 Call again with page={} to fetch the next page. \
                 Use format=json for structured access to block metadata.",
                current_page + 1
            ),
        ))
    } else {
        Ok(payload)
    }
}

pub async fn get_block(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let bk = client
        .get_block_kramdown(&id)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(json!({ "id": bk.id, "kramdown": bk.kramdown }))
}

pub async fn create_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let notebook_str = required_string(&map, "notebook")?;
    let hpath = required_string(&map, "hpath")?;
    let markdown = required_string(&map, "markdown")?;

    let notebook = siyuan_types::NotebookId::parse(&notebook_str)
        .map_err(|e| McpError::invalid_params(format!("invalid notebook id: {e}"), None))?;

    let new_id = client
        .create_doc_with_md(&notebook, &hpath, &markdown)
        .await
        .map_err(siyuan_to_mcp)?;

    Ok(with_hint(
        json!({ "id": new_id }),
        "Mutation completed at the kernel. SQL-indexed reads (siyuan_get_doc, siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once. The returned id is the new document's root block id.",
    ))
}
