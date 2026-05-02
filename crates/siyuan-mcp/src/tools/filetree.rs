use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, resolve as resolve_doc_meta};
use siyuan_types::{BlockId, NotebookId};

use super::util::{
    ensure_object, optional_string, required_string, siyuan_to_mcp, string_array, with_hint,
};

fn parse_notebook_id(s: &str) -> Result<NotebookId, McpError> {
    NotebookId::parse(s)
        .map_err(|e| McpError::invalid_params(format!("invalid notebook id: {e}"), None))
}

fn parse_block_id(s: &str) -> Result<BlockId, McpError> {
    BlockId::parse(s).map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))
}

/// Treat whitespace-only inputs as absent. Mirrors the rejection pattern used
/// elsewhere in this module so agents can't accidentally squeak past the
/// "exactly one input mode" rule by passing `"   "`.
fn is_present(s: Option<&str>) -> bool {
    s.is_some_and(|v| !v.trim().is_empty())
}

pub async fn resolve(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;

    // Optional inputs. We allow exactly ONE of `id` or (`notebook` + `hpath`).
    // Whitespace-only strings count as absent.
    let id_raw = optional_string(&map, "id");
    let nb_raw = optional_string(&map, "notebook");
    let hp_raw = optional_string(&map, "hpath");

    let has_id = is_present(id_raw.as_deref());
    let has_nb = is_present(nb_raw.as_deref());
    let has_hp = is_present(hp_raw.as_deref());

    // The hpath branch needs both fields together; partial supply is misuse.
    let has_hpath_branch = has_nb || has_hp;

    if has_id && has_hpath_branch {
        return Err(McpError::invalid_params(
            "provide either `id` or (`notebook` + `hpath`), not both",
            None,
        ));
    }
    if !has_id && !has_hpath_branch {
        return Err(McpError::invalid_params(
            "provide either `id` or (`notebook` + `hpath`)",
            None,
        ));
    }

    let lookup = if has_id {
        DocLookup::ById(parse_block_id(
            id_raw.as_deref().unwrap_or_default().trim(),
        )?)
    } else {
        // Hpath branch: both halves are required.
        if !has_nb {
            return Err(McpError::invalid_params(
                "`notebook` is required when looking up by hpath",
                None,
            ));
        }
        if !has_hp {
            return Err(McpError::invalid_params(
                "`hpath` is required when looking up by notebook",
                None,
            ));
        }
        let notebook = parse_notebook_id(nb_raw.as_deref().unwrap_or_default().trim())?;
        // Preserve the user-supplied hpath verbatim — the kernel uses `/` as
        // the canonical root and we don't want to silently rewrite it.
        let hpath = hp_raw.unwrap_or_default();
        DocLookup::ByHpath { notebook, hpath }
    };

    let docs = resolve_doc_meta(client, lookup)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(json!({ "docs": docs }))
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
        "Filesystem-level mutation: document title updated. siyuan_doc_resolve reflects the \
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
         siyuan_doc_resolve reflects the change immediately. \
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
    async fn resolve_rejects_both_id_and_hpath() {
        let client = dummy_client();
        let args = json!({
            "id": "20260501090000-doc0001",
            "notebook": "20260501000000-nb00001",
            "hpath": "/Plan",
        });
        let err = resolve(&client, args)
            .await
            .expect_err("both id and hpath branch must be rejected");
        assert!(
            err.message.contains("not both"),
            "error message should explain mutual exclusion; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn resolve_rejects_neither() {
        let client = dummy_client();
        let args = json!({});
        let err = resolve(&client, args)
            .await
            .expect_err("missing both modes must be rejected");
        // Match on the user-facing phrasing rather than a single keyword so
        // we catch any future rewording that drops the disambiguation hint.
        assert!(
            err.message.contains("`id`") && err.message.contains("`hpath`"),
            "error message should mention both input modes; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn resolve_rejects_partial_hpath_branch_missing_hpath() {
        let client = dummy_client();
        let args = json!({
            "notebook": "20260501000000-nb00001",
        });
        let err = resolve(&client, args)
            .await
            .expect_err("notebook without hpath must be rejected");
        assert!(
            err.message.contains("hpath"),
            "error message should reference `hpath`; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn resolve_rejects_partial_hpath_branch_missing_notebook() {
        let client = dummy_client();
        let args = json!({
            "hpath": "/Plan",
        });
        let err = resolve(&client, args)
            .await
            .expect_err("hpath without notebook must be rejected");
        assert!(
            err.message.contains("notebook"),
            "error message should reference `notebook`; got: {}",
            err.message
        );
    }
}
