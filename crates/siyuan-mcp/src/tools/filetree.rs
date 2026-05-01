use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::{BlockId, NotebookId};

use super::util::{ensure_object, required_string, siyuan_to_mcp, string_array, with_hint};

fn parse_notebook_id(s: &str) -> Result<NotebookId, McpError> {
    NotebookId::parse(s)
        .map_err(|e| McpError::invalid_params(format!("invalid notebook id: {e}"), None))
}

fn parse_block_id(s: &str) -> Result<BlockId, McpError> {
    BlockId::parse(s).map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))
}

pub async fn resolve(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let notebook = parse_notebook_id(&required_string(&map, "notebook")?)?;
    let hpath = required_string(&map, "hpath")?;

    let ids = client
        .get_ids_by_hpath(&notebook, &hpath)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(json!({ "ids": ids }))
}

pub async fn hpath_by_id(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;

    let hpath = client.get_hpath_by_id(&id).await.map_err(siyuan_to_mcp)?;
    Ok(json!({ "hpath": hpath }))
}

pub async fn rename_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let notebook = parse_notebook_id(&required_string(&map, "notebook")?)?;
    // path is the storage .sy path (kernel quirk), not the human-readable hpath.
    let path = required_string(&map, "path")?;
    let title = required_string(&map, "title")?;

    client
        .rename_doc(&notebook, &path, &title)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Filesystem-level mutation: document title updated. siyuan_doc_hpath_by_id reflects the \
         new title immediately. Use the storage .sy path for any follow-up calls requiring path.",
    ))
}

pub async fn move_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let from_paths = string_array(&map, "from_paths")?;
    let to_notebook = parse_notebook_id(&required_string(&map, "to_notebook")?)?;
    let to_path = required_string(&map, "to_path")?;

    client
        .move_docs(&from_paths, &to_notebook, &to_path)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Filesystem-level mutation: documents moved to the target notebook/path. \
         siyuan_doc_resolve and siyuan_doc_hpath_by_id reflect the change immediately. \
         Use the storage .sy path for follow-up calls.",
    ))
}

pub async fn remove_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let notebook = parse_notebook_id(&required_string(&map, "notebook")?)?;
    let path = required_string(&map, "path")?;

    client
        .remove_doc(&notebook, &path)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Filesystem-level mutation: document permanently removed including all child blocks. \
         This action is irreversible. siyuan_doc_resolve will no longer find this path.",
    ))
}
