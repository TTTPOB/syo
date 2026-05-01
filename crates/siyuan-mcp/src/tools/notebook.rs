use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

use super::util::{ensure_object, required_string, siyuan_to_mcp};

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
    Ok(json!({ "ok": true }))
}

pub async fn close(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    client.close_notebook(&id).await.map_err(siyuan_to_mcp)?;
    Ok(json!({ "ok": true }))
}

pub async fn create(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let name = required_string(&map, "name")?;
    let nb = client.create_notebook(&name).await.map_err(siyuan_to_mcp)?;
    Ok(notebook_to_json(&nb))
}

pub async fn rename(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    let name = required_string(&map, "name")?;
    client
        .rename_notebook(&id, &name)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(json!({ "ok": true }))
}

pub async fn remove(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
    client.remove_notebook(&id).await.map_err(siyuan_to_mcp)?;
    Ok(json!({ "ok": true }))
}
