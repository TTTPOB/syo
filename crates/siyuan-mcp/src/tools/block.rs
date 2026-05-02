use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

use super::util::{ensure_object, optional_string, required_string, siyuan_to_mcp, with_hint};

fn parse_block_id(s: &str) -> Result<BlockId, McpError> {
    BlockId::parse(s).map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))
}

pub async fn update_block(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;
    let markdown = required_string(&map, "markdown")?;

    client
        .update_block_markdown(&id, &markdown)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Mutation completed at the kernel. SQL-indexed reads (siyuan_get_doc, siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once.",
    ))
}

pub async fn insert_block(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let markdown = required_string(&map, "markdown")?;

    let previous_id = optional_string(&map, "previous_id")
        .map(|s| parse_block_id(&s))
        .transpose()?;
    let next_id = optional_string(&map, "next_id")
        .map(|s| parse_block_id(&s))
        .transpose()?;
    let parent_id = optional_string(&map, "parent_id")
        .map(|s| parse_block_id(&s))
        .transpose()?;

    // Exactly one of the three anchor fields must be present.
    let anchor_count = [
        previous_id.is_some(),
        next_id.is_some(),
        parent_id.is_some(),
    ]
    .iter()
    .filter(|&&b| b)
    .count();
    if anchor_count != 1 {
        return Err(McpError::invalid_params(
            "exactly one of `previous_id`, `next_id`, `parent_id` must be provided",
            None,
        ));
    }

    let new_id = client
        .insert_block_markdown(
            &markdown,
            previous_id.as_ref(),
            next_id.as_ref(),
            parent_id.as_ref(),
        )
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "id": new_id }),
        "Block inserted at the kernel. SQL-indexed reads (siyuan_get_doc, siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once. The returned id is the new block's id.",
    ))
}

pub async fn append_block(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let markdown = required_string(&map, "markdown")?;
    let parent_id = parse_block_id(&required_string(&map, "parent_id")?)?;

    let new_id = client
        .append_block_markdown(&markdown, &parent_id)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "id": new_id }),
        "Block appended at the kernel. SQL-indexed reads (siyuan_get_doc, siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once. The returned id is the new block's id.",
    ))
}

pub async fn prepend_block(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let markdown = required_string(&map, "markdown")?;
    let parent_id = parse_block_id(&required_string(&map, "parent_id")?)?;

    let new_id = client
        .prepend_block_markdown(&markdown, &parent_id)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "id": new_id }),
        "Block prepended at the kernel. SQL-indexed reads (siyuan_get_doc, siyuan_sql) may \
         briefly show stale state for ~100–500 ms; if a follow-up read returns unexpected data, \
         retry once. The returned id is the new block's id.",
    ))
}

pub async fn move_block(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;

    // The MCP shape models the kernel's anchor fields directly; we never
    // accepted a `position` string. If the caller sends one (e.g. porting a
    // mental model from the CLI's older free-form `--position`), reject it
    // with a hint that points at the right surface for each former kind.
    if let Some(pos) = optional_string(&map, "position") {
        return Err(McpError::invalid_params(
            format!(
                "`position` is not accepted; set `previous_id` or `parent_id` instead. \
                 For position kinds not supported by move \
                 (before_block, append_section, prepend_section), see `siyuan_insert_block` \
                 (got position={pos:?})"
            ),
            None,
        ));
    }

    let previous_id = optional_string(&map, "previous_id")
        .map(|s| parse_block_id(&s))
        .transpose()?;
    let parent_id = optional_string(&map, "parent_id")
        .map(|s| parse_block_id(&s))
        .transpose()?;

    // Exactly one anchor required.
    let anchor_count = [previous_id.is_some(), parent_id.is_some()]
        .iter()
        .filter(|&&b| b)
        .count();
    if anchor_count != 1 {
        return Err(McpError::invalid_params(
            "exactly one of `previous_id`, `parent_id` must be provided",
            None,
        ));
    }

    client
        .move_block(&id, previous_id.as_ref(), parent_id.as_ref())
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Block moved at the kernel. SQL-indexed reads (siyuan_get_doc, siyuan_sql) may briefly \
         show stale state for ~100–500 ms; if a follow-up read returns unexpected data, retry once.",
    ))
}

pub async fn delete_block(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id = parse_block_id(&required_string(&map, "id")?)?;

    client.delete_block(&id).await.map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Block permanently deleted at the kernel. SQL-indexed reads (siyuan_get_doc, siyuan_sql) \
         may briefly show stale state for ~100–500 ms after deletion.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_client() -> SiyuanClient {
        SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds")
    }

    #[tokio::test]
    async fn move_block_rejects_legacy_position_field() {
        // The dummy client points at an unreachable port, so a parser
        // regression that forwarded the request would surface a network
        // error instead of `invalid_params`. Pinning to the message keeps
        // that contract obvious.
        let client = dummy_client();
        let args = json!({
            "id": "20260501090000-blk0001",
            "position": "before_block",
            "previous_id": "20260501090000-blk0002",
        });
        let err = move_block(&client, args)
            .await
            .expect_err("position field must be rejected client-side");
        assert!(
            err.message.contains("`position` is not accepted"),
            "error should explain that `position` is not accepted; got: {}",
            err.message
        );
        assert!(
            err.message.contains("siyuan_insert_block"),
            "error should point at siyuan_insert_block for the unsupported kinds; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn move_block_rejects_zero_anchors() {
        let client = dummy_client();
        let args = json!({ "id": "20260501090000-blk0001" });
        let err = move_block(&client, args)
            .await
            .expect_err("missing anchor must be rejected");
        assert!(
            err.message.contains("previous_id"),
            "error should mention previous_id/parent_id; got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn move_block_rejects_both_anchors() {
        let client = dummy_client();
        let args = json!({
            "id": "20260501090000-blk0001",
            "previous_id": "20260501090000-blk0002",
            "parent_id": "20260501090000-blk0003",
        });
        let err = move_block(&client, args)
            .await
            .expect_err("two anchors must be rejected");
        assert!(
            err.message.contains("exactly one"),
            "error should require exactly one anchor; got: {}",
            err.message
        );
    }
}
