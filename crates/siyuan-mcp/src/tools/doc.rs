use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_model::pagination::PageRequest;
use siyuan_types::BlockId;

use super::util::{
    MAX_PAGE_SIZE, anyhow_to_mcp, ensure_object, optional_u64, required_string, siyuan_to_mcp,
    with_hint,
};

pub async fn get_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    // Do NOT cap `page` here: paginate() clamps an over-large `page` to
    // total_pages (see R1 BUG-1 fix), and capping here would prevent
    // legitimate access to high page numbers in long documents. We only
    // cap `page_size` so a pathological caller cannot defeat pagination.
    let page = optional_u64(&map, "page").unwrap_or(1) as usize;
    let page_size = optional_u64(&map, "page_size")
        .unwrap_or(50)
        .min(MAX_PAGE_SIZE) as usize;
    let format = map
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("agent-md");

    // load_doc returns anyhow::Error but may wrap a typed SiyuanError (e.g.
    // NotFound when the doc id is unknown). Downcast first so siyuan_to_mcp's
    // existing typed-error mapping reaches the wire — otherwise NotFound gets
    // flattened to a generic internal_error by anyhow_to_mcp.
    let bundle = siyuan_model::load::load_doc(client, &id, PageRequest { page, page_size })
        .await
        .map_err(|e| match e.downcast::<siyuan_types::SiyuanError>() {
            Ok(typed) => siyuan_to_mcp(typed),
            Err(other) => anyhow_to_mcp(other),
        })?;

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

    // Only emit a "next page" hint when there is actually a next page to fetch;
    // on the last page paginate() clamps `page` to `total_pages`, so suggesting
    // page=current+1 would loop forever.
    match next_page_hint(current_page, total_pages, format) {
        Some(hint) => Ok(with_hint(payload, &hint)),
        None => Ok(payload),
    }
}

// Decide whether to attach a pagination hint and, if so, what to say.
// Returns Some(hint) only when a strictly-later page exists. The `format`
// argument is kept in the signature so callers don't have to thread it
// separately if the hint copy ever differs by render format.
fn next_page_hint(current: usize, total: usize, _format: &str) -> Option<String> {
    if current < total {
        Some(format!(
            "Pagination: this is page {current} of {total}. \
             Call again with page={} to fetch the next page. \
             Use format=json for structured access to block metadata.",
            current + 1
        ))
    } else {
        None
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

#[cfg(test)]
mod tests {
    use super::next_page_hint;

    #[test]
    fn single_page_emits_no_hint() {
        assert!(next_page_hint(1, 1, "agent-md").is_none());
    }

    #[test]
    fn middle_page_points_to_next() {
        let hint = next_page_hint(2, 5, "agent-md").expect("middle page should yield a hint");
        assert!(hint.contains("page 2 of 5"));
        assert!(hint.contains("page=3"));
    }

    #[test]
    fn last_page_emits_no_hint() {
        assert!(next_page_hint(5, 5, "agent-md").is_none());
    }

    #[test]
    fn first_of_two_pages_points_to_two() {
        let hint = next_page_hint(1, 2, "json").expect("first of two pages should yield a hint");
        assert!(hint.contains("page=2"));
    }
}
