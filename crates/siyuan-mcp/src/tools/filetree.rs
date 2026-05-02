use rmcp::ErrorData as McpError;
use serde_json::{Map, Value, json};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, resolve as resolve_doc_meta, resolve_one_storage};
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

/// Validate the `siyuan_doc_resolve` argument map and produce a [`DocLookup`].
///
/// Enforces the "exactly one of `id` or (`notebook` + `hpath`)" invariant at
/// the boundary so the model layer can stay enum-driven. Whitespace-only
/// strings count as absent. Kept as a separate function so the validation
/// rules are unit-testable without a live `SiyuanClient`.
pub(crate) fn parse_doc_lookup(map: &Map<String, Value>) -> Result<DocLookup, McpError> {
    let id_raw = optional_string(map, "id");
    let nb_raw = optional_string(map, "notebook");
    let hp_raw = optional_string(map, "hpath");

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

    if has_id {
        return Ok(DocLookup::ById(parse_block_id(
            id_raw.as_deref().unwrap_or_default().trim(),
        )?));
    }

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
    Ok(DocLookup::ByHpath { notebook, hpath })
}

/// Validate the `siyuan_doc_move` argument map and produce one
/// [`DocLookup`] per source document.
///
/// Mirrors [`parse_doc_lookup`]'s id-XOR-(notebook+hpath) rule, but for the
/// batch case: the caller provides EITHER `from_ids: [string, ...]` OR
/// `(notebook + from_hpaths: [string, ...])`. Empty arrays in the supplied
/// mode are rejected (preserving the old `move_doc` contract that refused
/// empty `from_paths` to avoid a silent kernel no-op). Whitespace-only
/// notebook strings count as absent so agents can't squeak past the rule
/// by passing `"   "`.
pub(crate) fn parse_doc_lookup_batch(map: &Map<String, Value>) -> Result<Vec<DocLookup>, McpError> {
    let id_mode_present = map.contains_key("from_ids");
    let hpath_mode_present = map.contains_key("from_hpaths");
    let nb_raw = optional_string(map, "notebook");
    let nb_present = is_present(nb_raw.as_deref());

    // Mutual exclusion. `notebook` belongs to the hpath branch, so it
    // also conflicts with `from_ids`.
    if id_mode_present && (hpath_mode_present || nb_present) {
        return Err(McpError::invalid_params(
            "provide either `from_ids` or (`notebook` + `from_hpaths`), not both",
            None,
        ));
    }
    if !id_mode_present && !hpath_mode_present && !nb_present {
        return Err(McpError::invalid_params(
            "provide either `from_ids` or (`notebook` + `from_hpaths`)",
            None,
        ));
    }

    if id_mode_present {
        let from_ids = string_array(map, "from_ids")?;
        if from_ids.is_empty() {
            return Err(McpError::invalid_params(
                "`from_ids` must contain at least one source id",
                None,
            ));
        }
        let mut out = Vec::with_capacity(from_ids.len());
        for raw in &from_ids {
            out.push(DocLookup::ById(parse_block_id(raw.trim())?));
        }
        return Ok(out);
    }

    // Hpath batch mode: `notebook` and `from_hpaths` are both required.
    if !nb_present {
        return Err(McpError::invalid_params(
            "`notebook` is required when looking up sources by hpath",
            None,
        ));
    }
    if !hpath_mode_present {
        return Err(McpError::invalid_params(
            "`from_hpaths` is required when `notebook` is supplied",
            None,
        ));
    }
    let from_hpaths = string_array(map, "from_hpaths")?;
    if from_hpaths.is_empty() {
        return Err(McpError::invalid_params(
            "`from_hpaths` must contain at least one source hpath",
            None,
        ));
    }
    let notebook = parse_notebook_id(nb_raw.as_deref().unwrap_or_default().trim())?;
    let mut out = Vec::with_capacity(from_hpaths.len());
    for hp in from_hpaths {
        out.push(DocLookup::ByHpath {
            notebook: notebook.clone(),
            hpath: hp,
        });
    }
    Ok(out)
}

pub async fn resolve(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let lookup = parse_doc_lookup(&map)?;
    let docs = resolve_doc_meta(client, lookup)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(json!({ "docs": docs }))
}

pub async fn rename_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let lookup = parse_doc_lookup(&map)?;
    let title = required_string(&map, "title")?;

    let (notebook, storage_path) = resolve_one_storage(client, lookup)
        .await
        .map_err(siyuan_to_mcp)?;
    client
        .rename_doc(&notebook, &storage_path, &title)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Filesystem-level mutation: document title updated. siyuan_doc_resolve reflects \
         the new title immediately. Address the document by id or (notebook + hpath) — \
         storage `.sy` paths are no longer accepted on this tool.",
    ))
}

pub async fn move_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let source_lookups = parse_doc_lookup_batch(&map)?;
    let to_notebook = parse_notebook_id(&required_string(&map, "to_notebook")?)?;
    let to_path = required_string(&map, "to_path")?;

    // Resolve each source to its storage path. Sequential because the
    // typical batch is small (<10) and resolve() internally hits a cheap
    // `lsNotebooks` + a single SQL `IN` query — parallelism would not pay
    // for the added complexity.
    let mut from_paths = Vec::with_capacity(source_lookups.len());
    for lookup in source_lookups {
        let (_nb, storage_path) = resolve_one_storage(client, lookup)
            .await
            .map_err(siyuan_to_mcp)?;
        from_paths.push(storage_path);
    }

    client
        .move_docs(&from_paths, &to_notebook, &to_path)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Filesystem-level mutation: documents moved to the target notebook/path. \
         siyuan_doc_resolve reflects the change immediately. Address sources by ids \
         (`from_ids`) or by (notebook + `from_hpaths`) — storage `.sy` paths are no \
         longer accepted on this tool.",
    ))
}

pub async fn remove_doc(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let lookup = parse_doc_lookup(&map)?;

    let (notebook, storage_path) = resolve_one_storage(client, lookup)
        .await
        .map_err(siyuan_to_mcp)?;
    client
        .remove_doc(&notebook, &storage_path)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Filesystem-level mutation: document permanently removed including all child \
         blocks. This action is irreversible. siyuan_doc_resolve will no longer find \
         this document.",
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
    async fn move_doc_rejects_empty_from_ids() {
        let client = dummy_client();
        let args = json!({
            "from_ids": [],
            "to_notebook": "20260501000000-nb00001",
            "to_path": "/Target",
        });
        let err = move_doc(&client, args)
            .await
            .expect_err("empty from_ids must be rejected");
        assert!(
            err.message.contains("from_ids"),
            "error message should reference `from_ids`; got: {}",
            err.message
        );
    }

    // The legacy `from_paths` field is gone. Pass-through callers that still
    // send it (and nothing else) must be told to migrate, not silently
    // accepted as a no-op. The current rejection path surfaces as
    // "provide either `from_ids` or (`notebook` + `from_hpaths`)" which is
    // good enough — assert on the keyword to lock the contract.
    #[tokio::test]
    async fn move_doc_rejects_legacy_from_paths_field() {
        let client = dummy_client();
        let args = json!({
            "from_paths": ["/20260501090000-doc0001.sy"],
            "to_notebook": "20260501000000-nb00001",
            "to_path": "/Target",
        });
        let err = move_doc(&client, args)
            .await
            .expect_err("legacy from_paths must not be accepted");
        assert!(
            err.message.contains("from_ids") || err.message.contains("from_hpaths"),
            "error must redirect to the new locator surface; got: {}",
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

    // Direct unit test for `parse_doc_lookup`. The handler tests above cover
    // the same rules end-to-end through the dummy client; this one pins down
    // the helper's contract so future refactors that route validation
    // through a different call site don't silently regress it.
    fn args_map(v: Value) -> serde_json::Map<String, Value> {
        match v {
            Value::Object(m) => m,
            _ => panic!("test fixture must be a JSON object"),
        }
    }

    #[test]
    fn parse_doc_lookup_accepts_id_only() {
        let map = args_map(json!({ "id": "20260501090000-doc0001" }));
        let lookup = parse_doc_lookup(&map).expect("id-only input is valid");
        assert!(matches!(lookup, DocLookup::ById(_)));
    }

    #[test]
    fn parse_doc_lookup_accepts_notebook_plus_hpath() {
        let map = args_map(json!({
            "notebook": "20260501000000-nb00001",
            "hpath": "/Plan",
        }));
        let lookup = parse_doc_lookup(&map).expect("notebook+hpath is valid");
        match lookup {
            DocLookup::ByHpath { hpath, .. } => assert_eq!(hpath, "/Plan"),
            DocLookup::ById(_) => panic!("expected ByHpath variant"),
        }
    }

    #[test]
    fn parse_doc_lookup_rejects_whitespace_only_id() {
        let map = args_map(json!({ "id": "   " }));
        let err = parse_doc_lookup(&map).expect_err("whitespace id is treated as absent");
        assert!(
            err.message.contains("`id`") && err.message.contains("`hpath`"),
            "got: {}",
            err.message
        );
    }

    // ---- parse_doc_lookup_batch ------------------------------------------
    //
    // The batch helper drives `siyuan_doc_move`. Mirrors the rules of
    // `parse_doc_lookup` for the multi-source case: id-mode XOR hpath-mode,
    // both empty is an error, and the hpath mode requires BOTH `notebook`
    // and `from_hpaths` together. Whitespace-only `notebook` counts as
    // absent.

    #[test]
    fn parse_doc_lookup_batch_accepts_from_ids() {
        let map = args_map(json!({
            "from_ids": ["20260501090000-doc0001", "20260501090000-doc0002"],
        }));
        let lookups = parse_doc_lookup_batch(&map).expect("from_ids only is valid");
        assert_eq!(lookups.len(), 2);
        assert!(matches!(lookups[0], DocLookup::ById(_)));
        assert!(matches!(lookups[1], DocLookup::ById(_)));
    }

    #[test]
    fn parse_doc_lookup_batch_accepts_notebook_plus_hpaths() {
        let map = args_map(json!({
            "notebook": "20260501000000-nb00001",
            "from_hpaths": ["/Plan", "/Notes"],
        }));
        let lookups = parse_doc_lookup_batch(&map).expect("notebook+from_hpaths is valid");
        assert_eq!(lookups.len(), 2);
        for l in &lookups {
            match l {
                DocLookup::ByHpath { hpath, .. } => {
                    assert!(hpath == "/Plan" || hpath == "/Notes");
                }
                DocLookup::ById(_) => panic!("expected ByHpath variant"),
            }
        }
    }

    #[test]
    fn parse_doc_lookup_batch_rejects_both_modes() {
        let map = args_map(json!({
            "from_ids": ["20260501090000-doc0001"],
            "notebook": "20260501000000-nb00001",
            "from_hpaths": ["/Plan"],
        }));
        let err = parse_doc_lookup_batch(&map).expect_err("supplying both modes must be rejected");
        assert!(
            err.message.contains("not both"),
            "error must explain mutual exclusion; got: {}",
            err.message
        );
    }

    #[test]
    fn parse_doc_lookup_batch_rejects_neither_mode() {
        let map = args_map(json!({}));
        let err = parse_doc_lookup_batch(&map).expect_err("missing both modes must be rejected");
        assert!(
            err.message.contains("from_ids") && err.message.contains("from_hpaths"),
            "error must mention both modes; got: {}",
            err.message
        );
    }

    #[test]
    fn parse_doc_lookup_batch_rejects_partial_hpath_mode_missing_notebook() {
        let map = args_map(json!({
            "from_hpaths": ["/Plan"],
        }));
        let err = parse_doc_lookup_batch(&map)
            .expect_err("from_hpaths without notebook must be rejected");
        assert!(
            err.message.contains("notebook"),
            "error must reference `notebook`; got: {}",
            err.message
        );
    }

    #[test]
    fn parse_doc_lookup_batch_rejects_partial_hpath_mode_missing_hpaths() {
        let map = args_map(json!({
            "notebook": "20260501000000-nb00001",
        }));
        let err = parse_doc_lookup_batch(&map)
            .expect_err("notebook without from_hpaths must be rejected");
        assert!(
            err.message.contains("from_hpaths"),
            "error must reference `from_hpaths`; got: {}",
            err.message
        );
    }

    #[test]
    fn parse_doc_lookup_batch_rejects_empty_from_ids_array() {
        let map = args_map(json!({ "from_ids": [] }));
        let err = parse_doc_lookup_batch(&map).expect_err("empty from_ids array must be rejected");
        assert!(
            err.message.contains("from_ids"),
            "error must reference `from_ids`; got: {}",
            err.message
        );
    }

    #[test]
    fn parse_doc_lookup_batch_rejects_empty_from_hpaths_array() {
        let map = args_map(json!({
            "notebook": "20260501000000-nb00001",
            "from_hpaths": [],
        }));
        let err =
            parse_doc_lookup_batch(&map).expect_err("empty from_hpaths array must be rejected");
        assert!(
            err.message.contains("from_hpaths"),
            "error must reference `from_hpaths`; got: {}",
            err.message
        );
    }

    // ---- handler-level pre-flight validation -----------------------------

    #[tokio::test]
    async fn rename_doc_rejects_legacy_path_field() {
        // The old `path` field is gone. A caller still sending it (without
        // the new `id` / `notebook+hpath` locator) must see a clear error,
        // not a silent kernel call that fails opaquely.
        let client = dummy_client();
        let args = json!({
            "notebook": "20260501000000-nb00001",
            "path": "/20260501090000-doc0001.sy",
            "title": "X",
        });
        let err = rename_doc(&client, args)
            .await
            .expect_err("legacy `path` plus partial locator must be rejected");
        assert!(
            err.message.contains("hpath") || err.message.contains("id"),
            "error must redirect to new locator; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn remove_doc_rejects_legacy_path_field() {
        let client = dummy_client();
        let args = json!({
            "notebook": "20260501000000-nb00001",
            "path": "/20260501090000-doc0001.sy",
        });
        let err = remove_doc(&client, args)
            .await
            .expect_err("legacy `path` plus partial locator must be rejected");
        assert!(
            err.message.contains("hpath") || err.message.contains("id"),
            "error must redirect to new locator; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn rename_doc_requires_title() {
        // Validate that the title-required check still fires after the
        // locator change. Use the id mode so the lookup succeeds at the
        // boundary (the resolve call itself will fail at network time, but
        // the missing-title error fires earlier).
        let client = dummy_client();
        let args = json!({ "id": "20260501090000-doc0001" });
        let err = rename_doc(&client, args)
            .await
            .expect_err("missing title must be rejected before any I/O");
        assert!(
            err.message.contains("title"),
            "error must reference `title`; got: {}",
            err.message
        );
    }
}
