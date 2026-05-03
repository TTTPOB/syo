use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;
use siyuan_types::position::PositionKind;

use super::util::{anyhow_to_mcp, ensure_object, required_string, with_hint};

fn parse_block_id(s: &str) -> Result<BlockId, McpError> {
    BlockId::parse(s).map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))
}

fn parse_position_kind(s: &str) -> Result<PositionKind, McpError> {
    serde_json::from_value(Value::String(s.to_owned()))
        .map_err(|e| McpError::invalid_params(format!("invalid position kind: {e}"), None))
}

// ---- block_get ----

pub async fn block_get(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let output = syo_core::block::get(client, &id)
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(json!({ "id": output.id, "kramdown": output.kramdown }))
}

// ---- block_update ----

pub async fn block_update(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;
    let markdown = required_string(&map, "markdown")?;

    syo_core::block::update(client, syo_core::block::UpdateBlockInput { id, markdown })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Mutation completed at the kernel. SQL-indexed reads (syo_siyuan_doc_get, syo_siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once.",
    ))
}

// ---- block_insert ----

pub async fn block_insert(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let markdown = required_string(&map, "markdown")?;
    let position_str = required_string(&map, "position")?;
    let anchor_str = required_string(&map, "anchor")?;

    let kind = parse_position_kind(&position_str)?;
    let anchor = parse_block_id(&anchor_str)?;

    let output = syo_core::block::insert(
        client,
        syo_core::block::InsertBlockInput {
            markdown,
            position: kind,
            anchor,
        },
    )
    .await
    .map_err(anyhow_to_mcp)?;

    Ok(with_hint(
        json!({ "id": output.id }),
        "Block inserted at the kernel. SQL-indexed reads (syo_siyuan_doc_get, syo_siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once. The returned id is the new block's id.",
    ))
}

// ---- block_move (all 8 positions) ----

pub async fn block_move(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;
    let position_str = required_string(&map, "position")?;
    let anchor_str = required_string(&map, "anchor")?;

    let kind = parse_position_kind(&position_str)?;
    let anchor = parse_block_id(&anchor_str)?;

    syo_core::block::move_block(
        client,
        syo_core::block::MoveBlockInput {
            id,
            position: kind,
            anchor,
        },
    )
    .await
    .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Block moved at the kernel. SQL-indexed reads (syo_siyuan_doc_get, syo_siyuan_sql) may briefly \
         show stale state for ~100–500 ms; if a follow-up read returns unexpected data, retry once.",
    ))
}

// ---- block_delete ----

pub async fn block_delete(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;

    syo_core::block::delete(client, syo_core::block::DeleteBlockInput { id })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Block permanently deleted at the kernel. SQL-indexed reads (syo_siyuan_doc_get, syo_siyuan_sql) \
         may briefly show stale state for ~100–500 ms after deletion.",
    ))
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

    // block_move now accepts all 8 position kinds (delegates to syo-core).
    #[tokio::test]
    async fn block_move_accepts_all_eight_position_kinds() {
        // Verify that parsing accepts all 8 kinds. The handler will fail
        // at the network layer because the dummy client cannot connect,
        // but the position-kind gate no longer rejects any variant.
        let client = dummy_client();
        let all_kinds = [
            "after_block",
            "before_block",
            "append_child",
            "prepend_child",
            "append_section",
            "prepend_section",
            "append_doc",
            "prepend_doc",
        ];
        for kind in all_kinds {
            let args = json!({
                "id": "20260501090000-blk0001",
                "position": kind,
                "anchor": "20260501090000-blk0002",
            });
            let result = block_move(&client, args).await;
            match result {
                Err(e) => {
                    // Expect a network-level error, NOT a position-rejection error.
                    assert!(
                        !e.message
                            .contains("not supported for syo_siyuan_block_move"),
                        "position {kind} should be accepted; got rejection: {}",
                        e.message
                    );
                }
                Ok(_) => {}
            }
        }
    }
}
