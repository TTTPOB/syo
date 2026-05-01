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
