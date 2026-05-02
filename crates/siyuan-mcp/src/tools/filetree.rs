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
    // Reject blank/whitespace-only hpaths but allow exactly "/" since the
    // kernel uses it as the canonical root hpath.
    if hpath != "/" && hpath.trim().is_empty() {
        return Err(McpError::invalid_params("`hpath` must not be empty", None));
    }

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
    // `string_array` only checks "is array of strings"; an empty array
    // would forward to the kernel as a no-op silently. Reject it here so
    // the caller learns about the misuse instead of seeing a fake success.
    if from_paths.is_empty() {
        return Err(McpError::invalid_params(
            "`from_paths` must contain at least one source path",
            None,
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    // `SiyuanClient::new` does not make any network call, so we can build a
    // throwaway client to exercise pre-flight validation paths that fail
    // before any HTTP I/O is initiated.
    fn dummy_client() -> SiyuanClient {
        SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds")
    }

    #[tokio::test]
    async fn move_doc_rejects_empty_from_paths() {
        let client = dummy_client();
        let args = json!({
            "from_paths": [],
            "to_notebook": "20260501000000-nb00001",
            "to_path": "/Target",
        });
        let err = move_doc(&client, args)
            .await
            .expect_err("empty from_paths must be rejected");
        assert!(
            err.message.contains("from_paths"),
            "error message should reference `from_paths`; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn resolve_rejects_whitespace_hpath() {
        let client = dummy_client();
        let args = json!({
            "notebook": "20260501000000-nb00001",
            "hpath": "   ",
        });
        let err = resolve(&client, args)
            .await
            .expect_err("whitespace hpath must be rejected");
        assert!(
            err.message.contains("hpath"),
            "error message should reference `hpath`; got: {}",
            err.message
        );
    }
}
