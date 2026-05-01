use std::collections::BTreeMap;

use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

use super::util::{ensure_object, object_field, required_string, siyuan_to_mcp};

pub async fn get_attrs(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let id_str = required_string(&map, "id")?;
    let id = BlockId::parse(&id_str)
        .map_err(|e| McpError::invalid_params(format!("invalid block id: {e}"), None))?;

    let attrs = client.get_block_attrs(&id).await.map_err(siyuan_to_mcp)?;
    Ok(json!({ "id": id, "attrs": attrs }))
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

    client
        .set_block_attrs(&id, &attrs)
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(json!({ "ok": true }))
}
