use std::collections::BTreeMap;

use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

use super::util::{anyhow_to_mcp, ensure_object, object_field, required_string, with_hint};

pub async fn get_attrs(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let output = syo_core::attr::get(client, syo_core::attr::GetAttrsInput { id })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(json!({ "id": output.id, "attrs": output.attrs }))
}

pub async fn set_attrs(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let attrs_obj = object_field(&map, "attrs")?;
    // Convert to BTreeMap<String, String>; values must be strings.
    let mut attrs: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in attrs_obj {
        let s = v.as_str().ok_or_else(|| {
            McpError::invalid_params(format!("attrs value for `{k}` must be a string"), None)
        })?;
        attrs.insert(k, s.to_owned());
    }

    syo_core::attr::set(client, syo_core::attr::SetAttrsInput { id, attrs })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Attribute mutation completed at the kernel. Only the listed keys are modified; existing \
         keys not in this request are left intact (kernel semantics). Custom keys must start with \
         `custom-`. SQL-indexed reads may briefly show stale state for ~100–500 ms.",
    ))
}

pub async fn set_icon(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;
    let icon = required_string(&map, "icon")?;
    syo_core::attr::set_icon(client, syo_core::attr::SetIconInput { id, icon })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Icon set at the kernel. Empty string clears the icon. SQL-indexed reads may \
         briefly show stale state for ~100–500 ms.",
    ))
}

pub async fn set_sort(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;
    let sort = map.get("sort").and_then(|v| v.as_i64()).ok_or_else(|| {
        McpError::invalid_params("missing or invalid `sort` (must be integer)", None)
    })?;
    syo_core::attr::set_sort(client, syo_core::attr::SetSortInput { id, sort })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "ok": true }),
        "Sort set at the kernel. SQL-indexed reads may briefly show stale state for ~100–500 ms.",
    ))
}
