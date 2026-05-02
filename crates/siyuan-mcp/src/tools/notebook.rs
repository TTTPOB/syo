use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

use super::util::{ensure_object, required_string, siyuan_to_mcp, with_hint};

fn parse_notebook_id(s: &str) -> Result<NotebookId, McpError> {
    NotebookId::parse(s)
        .map_err(|e| McpError::invalid_params(format!("invalid notebook id: {e}"), None))
}

// Notebook doesn't impl Serialize in siyuan-client, so convert manually.
fn notebook_to_json(nb: &siyuan_client::api::notebook::Notebook) -> Value {
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
    let notebooks = client.ls_notebooks().await.map_err(siyuan_to_mcp)?;
    let json_notebooks: Vec<Value> = notebooks.iter().map(notebook_to_json).collect();
    Ok(json!({ "notebooks": json_notebooks }))
}

pub async fn open(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    client.open_notebook(&id).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Notebook is now open. Documents inside it are visible to siyuan_doc_resolve and \
         SQL-indexed reads. SQL-indexed reads may briefly show stale state for ~100–500 ms.",
    ))
}

pub async fn close(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    client.close_notebook(&id).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Notebook is now closed. Its documents are no longer visible to siyuan_doc_resolve or \
         SQL queries. Reopen with siyuan_notebook_open.",
    ))
}

pub async fn create(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let name = required_string(&map, "name")?;
    let nb = client.create_notebook(&name).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        notebook_to_json(&nb),
        "Notebook created and opened. The returned id can be used in subsequent calls \
         (siyuan_doc_create, siyuan_notebook_rename, etc.). It also appears in \
         siyuan_notebook_ls.",
    ))
}

pub async fn rename(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    let name = required_string(&map, "name")?;
    client
        .rename_notebook(&id, &name)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Notebook renamed at the kernel. The change is reflected immediately in siyuan_notebook_ls. \
         SQL-indexed reads may briefly show the old name for ~100–500 ms.",
    ))
}

pub async fn remove(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    client.remove_notebook(&id).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Notebook permanently removed, including all its documents. This action is irreversible. \
         The notebook will no longer appear in siyuan_notebook_ls.",
    ))
}
