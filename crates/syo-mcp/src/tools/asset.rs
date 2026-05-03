use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{anyhow_to_mcp, ensure_object, optional_string, required_string, with_hint};

pub async fn upload(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let file_path = required_string(&map, "file_path")?;

    let output = syo_core::asset::upload(client, syo_core::asset::UploadInput { file_path })
        .await
        .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "asset_path": output.asset_path }),
        "Asset stored at the returned path. To embed it, insert a markdown image like \
         `![alt](<asset_path>)` via syo_siyuan_block_insert, or include it in syo_siyuan_doc_create \
         markdown. The path is kernel-relative and usable directly in SiYuan markdown.",
    ))
}

pub fn reference(args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let path = required_string(&map, "path")?;
    let alt = optional_string(&map, "alt").unwrap_or_default();

    let output = syo_core::asset::reference(syo_core::asset::ReferenceInput { path, alt });
    Ok(with_hint(
        json!({ "markdown": output.markdown }),
        "Formatted markdown image reference. Use this markdown string in syo_siyuan_block_insert \
         or syo_siyuan_block_update to embed the asset.",
    ))
}
