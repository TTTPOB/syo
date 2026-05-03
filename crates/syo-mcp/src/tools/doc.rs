use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

use super::util::{
    MAX_PAGE_SIZE, anyhow_to_mcp, ensure_object, optional_u64, required_string, with_hint,
};

pub async fn get_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let page = optional_u64(&map, "page").unwrap_or(1) as usize;
    let page_size = optional_u64(&map, "page_size")
        .unwrap_or(50)
        .min(MAX_PAGE_SIZE) as usize;
    let format_str = map
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("agent-md");

    let format = match format_str {
        "json" => syo_core::doc::DocFormat::Json,
        "json-pretty" => syo_core::doc::DocFormat::JsonPretty,
        _ => syo_core::doc::DocFormat::AgentMd,
    };

    let output = syo_core::doc::get(
        client,
        syo_core::doc::GetDocInput {
            id,
            page,
            page_size,
            format,
        },
    )
    .await
    .map_err(anyhow_to_mcp)?;

    let total_pages = output.total_pages;
    let current_page = output.page;

    let payload = json!({ "format": format_str, "content": output.content });

    match next_page_hint(current_page, total_pages, format_str) {
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

pub async fn create_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let notebook_str = required_string(&map, "notebook")?;
    let hpath = required_string(&map, "hpath")?;
    let markdown = required_string(&map, "markdown")?;

    let notebook = siyuan_types::NotebookId::parse(&notebook_str)
        .map_err(|e| McpError::invalid_params(format!("invalid notebook id: {e}"), None))?;

    let output = syo_core::doc::create(
        client,
        syo_core::doc::CreateDocInput {
            notebook,
            hpath,
            markdown,
            force: true,
        },
    )
    .await
    .map_err(anyhow_to_mcp)?;

    Ok(with_hint(
        json!({ "id": output.id }),
        "Mutation completed at the kernel. SQL-indexed reads (syo_siyuan_doc_get, syo_siyuan_sql) may \
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
