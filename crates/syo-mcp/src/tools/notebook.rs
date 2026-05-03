use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_client::api::notebook::Notebook;
use siyuan_types::NotebookId;

use super::util::{anyhow_to_mcp, ensure_object, required_string, with_hint};

fn parse_notebook_id(s: &str) -> Result<NotebookId, McpError> {
    NotebookId::parse(s)
        .map_err(|e| McpError::invalid_params(format!("invalid notebook id: {e}"), None))
}

// Notebook doesn't impl Serialize in siyuan-client, so convert manually.
fn notebook_to_json(nb: &Notebook) -> Value {
    json!({
        "id":     nb.id.as_str(),
        "name":   nb.name,
        "icon":   nb.icon,
        "sort":   nb.sort,
        "closed": nb.closed,
    })
}

pub async fn ls(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let _ = ensure_object(args)?;
    let output = syo_core::notebook::ls(client)
        .await
        .map_err(anyhow_to_mcp)?;
    let json_notebooks: Vec<Value> = output.notebooks.iter().map(notebook_to_json).collect();
    Ok(json!({ "notebooks": json_notebooks }))
}

pub async fn create(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let name = required_string(&map, "name")?;
    let output = syo_core::notebook::create(client, syo_core::notebook::CreateInput { name })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        notebook_to_json(&output.notebook),
        "Notebook created and opened. The returned id can be used in subsequent calls \
         (syo_siyuan_doc_create, syo_siyuan_notebook_rename, etc.). It also appears in \
         syo_siyuan_notebook_ls.",
    ))
}

pub async fn rename(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    let name = required_string(&map, "name")?;
    syo_core::notebook::rename(client, syo_core::notebook::RenameInput { id, name })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Notebook renamed at the kernel. The change is reflected immediately in syo_siyuan_notebook_ls. \
         SQL-indexed reads may briefly show the old name for ~100–500 ms.",
    ))
}

pub async fn remove(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    syo_core::notebook::remove(client, syo_core::notebook::RemoveInput { id })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Notebook permanently removed, including all its documents. This action is irreversible. \
         The notebook will no longer appear in syo_siyuan_notebook_ls.",
    ))
}
