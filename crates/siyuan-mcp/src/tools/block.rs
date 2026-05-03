use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_model::pagination::PageRequest;
use siyuan_model::section::populate_section_children;
use siyuan_types::position::PositionKind;
use siyuan_types::{BlockId, BlockType, Position};

use super::util::{ensure_object, required_string, siyuan_to_mcp, with_hint};

fn parse_block_id(s: &str) -> Result<BlockId, McpError> {
    BlockId::parse(s).map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))
}

fn parse_position_kind(s: &str) -> Result<PositionKind, McpError> {
    serde_json::from_value(Value::String(s.to_owned()))
        .map_err(|e| McpError::invalid_params(format!("invalid position kind: {e}"), None))
}

// ---- block_get (moved from doc.rs) ----

pub async fn block_get(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
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

// ---- block_update ----

pub async fn block_update(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;
    let markdown = required_string(&map, "markdown")?;

    client
        .update_block_markdown(&id, &markdown)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Mutation completed at the kernel. SQL-indexed reads (siyuan_doc_get, siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once.",
    ))
}

// ---- block_insert (merged insert/append/prepend) ----

pub async fn block_insert(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let markdown = required_string(&map, "markdown")?;
    let position_str = required_string(&map, "position")?;
    let anchor_str = required_string(&map, "anchor")?;

    let kind = parse_position_kind(&position_str)?;
    let anchor = parse_block_id(&anchor_str)?;
    let position = Position::from((kind, anchor));

    let new_id = match position {
        Position::AfterBlock { block_id } => client
            .insert_block_markdown(&markdown, Some(&block_id), None, None)
            .await
            .map_err(siyuan_to_mcp)?,
        Position::BeforeBlock { block_id } => client
            .insert_block_markdown(&markdown, None, Some(&block_id), None)
            .await
            .map_err(siyuan_to_mcp)?,
        Position::AppendChild { container_id } => client
            .append_block_markdown(&markdown, &container_id)
            .await
            .map_err(siyuan_to_mcp)?,
        Position::PrependChild { container_id } => client
            .prepend_block_markdown(&markdown, &container_id)
            .await
            .map_err(siyuan_to_mcp)?,
        Position::AppendSection { heading_id } => {
            let section_end = resolve_section_end(client, &heading_id).await?;
            client
                .insert_block_markdown(&markdown, Some(&section_end), None, None)
                .await
                .map_err(siyuan_to_mcp)?
        }
        Position::PrependSection { heading_id } => {
            // Right after the heading itself.
            client
                .insert_block_markdown(&markdown, Some(&heading_id), None, None)
                .await
                .map_err(siyuan_to_mcp)?
        }
        Position::AppendDoc { doc_id } => client
            .append_block_markdown(&markdown, &doc_id)
            .await
            .map_err(siyuan_to_mcp)?,
        Position::PrependDoc { doc_id } => client
            .prepend_block_markdown(&markdown, &doc_id)
            .await
            .map_err(siyuan_to_mcp)?,
    };

    Ok(with_hint(
        json!({ "id": new_id }),
        "Block inserted at the kernel. SQL-indexed reads (siyuan_doc_get, siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once. The returned id is the new block's id.",
    ))
}

// ---- block_move (updated to use position + anchor) ----

pub async fn block_move(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;
    let position_str = required_string(&map, "position")?;
    let anchor_str = required_string(&map, "anchor")?;

    let kind = parse_position_kind(&position_str)?;
    let anchor = parse_block_id(&anchor_str)?;

    let (previous_id, parent_id) = match kind {
        PositionKind::AfterBlock => (Some(&anchor), None),
        PositionKind::AppendChild => (None, Some(&anchor)),
        other => {
            return Err(McpError::invalid_params(
                format!(
                    "position '{other:?}' is not supported for siyuan_block_move; \
                     use `after_block` or `append_child`",
                ),
                None,
            ));
        }
    };

    client
        .move_block(&id, previous_id, parent_id)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Block moved at the kernel. SQL-indexed reads (siyuan_doc_get, siyuan_sql) may briefly \
         show stale state for ~100–500 ms; if a follow-up read returns unexpected data, retry once.",
    ))
}

// ---- block_delete ----

pub async fn block_delete(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;

    client.delete_block(&id).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Block permanently deleted at the kernel. SQL-indexed reads (siyuan_doc_get, siyuan_sql) \
         may briefly show stale state for ~100–500 ms after deletion.",
    ))
}

// ---- resolve_section_end (duplicated from CLI insert_blocks) ----

/// Find the last block in the section owned by `heading_id`.
async fn resolve_section_end(
    client: &SiyuanClient,
    heading_id: &BlockId,
) -> Result<BlockId, McpError> {
    #[derive(serde::Deserialize)]
    struct R {
        root_id: String,
        #[serde(rename = "type")]
        ty: String,
    }
    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT root_id, type FROM blocks WHERE id = '{}'",
            heading_id.as_str()
        ))
        .await
        .map_err(siyuan_to_mcp)?;
    let root = rows
        .first()
        .ok_or_else(|| McpError::invalid_params("heading not found", None))?;
    if root.ty != "h" {
        return Err(McpError::invalid_params(
            "anchor for append_section must be a heading block",
            None,
        ));
    }
    let root_id = BlockId::parse(&root.root_id)
        .map_err(|e| McpError::invalid_params(format!("invalid root id: {e}"), None))?;

    let bundle = siyuan_model::load::load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await
    .map_err(|e| {
        // Downcast typed errors when possible.
        match e.downcast::<siyuan_types::SiyuanError>() {
            Ok(typed) => siyuan_to_mcp(typed),
            Err(other) => McpError::internal_error(other.to_string(), None),
        }
    })?;
    let mut blocks = bundle.blocks;
    populate_section_children(&mut blocks);
    let heading = blocks
        .iter()
        .find(|b| &b.id == heading_id)
        .ok_or_else(|| McpError::invalid_params("heading not in doc", None))?;
    if heading.block_type != BlockType::Heading {
        return Err(McpError::invalid_params(
            "anchor is not a heading after re-resolution",
            None,
        ));
    }
    if let Some(last) = heading.section_children.last() {
        Ok(last.clone())
    } else {
        // Empty section: treat heading itself as anchor.
        Ok(heading_id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_client() -> SiyuanClient {
        SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds")
    }

    // --- position kind parsing ---

    #[test]
    fn parse_position_kind_all_variants() {
        let valid = [
            "after_block",
            "before_block",
            "append_child",
            "prepend_child",
            "append_section",
            "prepend_section",
            "append_doc",
            "prepend_doc",
        ];
        for s in valid {
            assert!(parse_position_kind(s).is_ok(), "should parse {s}");
        }
    }

    #[test]
    fn parse_position_kind_rejects_invalid() {
        assert!(parse_position_kind("invalid_kind").is_err());
        assert!(parse_position_kind("AfterBlock").is_err());
        assert!(parse_position_kind("").is_err());
    }

    // --- block_move tests ---

    #[tokio::test]
    async fn block_move_rejects_unsupported_position_kinds() {
        let client = dummy_client();
        let unsupported = [
            "before_block",
            "append_section",
            "prepend_section",
            "append_doc",
            "prepend_doc",
        ];
        for kind in unsupported {
            let args = json!({
                "id": "20260501090000-blk0001",
                "position": kind,
                "anchor": "20260501090000-blk0002",
            });
            let err = block_move(&client, args)
                .await
                .expect_err(&format!("position {kind} must be rejected for block_move"));
            assert!(
                err.message.contains("not supported for siyuan_block_move"),
                "error should explain that position {kind} is not supported; got: {}",
                err.message
            );
        }
    }

    #[tokio::test]
    async fn block_move_rejects_missing_position_or_anchor() {
        let client = dummy_client();
        // Missing position
        let args = json!({
            "id": "20260501090000-blk0001",
            "anchor": "20260501090000-blk0002",
        });
        let err = block_move(&client, args)
            .await
            .expect_err("missing position must be rejected");
        assert!(
            err.message.contains("position"),
            "error should mention position; got: {}",
            err.message
        );

        // Missing anchor
        let args = json!({
            "id": "20260501090000-blk0001",
            "position": "after_block",
        });
        let err = block_move(&client, args)
            .await
            .expect_err("missing anchor must be rejected");
        assert!(
            err.message.contains("anchor"),
            "error should mention anchor; got: {}",
            err.message
        );
    }
}
