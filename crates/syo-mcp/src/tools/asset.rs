use std::path::Path;

use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{ensure_object, required_string, siyuan_to_mcp, with_hint};

pub async fn upload(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let file_path = required_string(&map, "file_path")?;

    let asset_path = client
        .upload_asset(Path::new(&file_path))
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(with_hint(
        json!({ "asset_path": asset_path }),
        "Asset stored at the returned path. To embed it, insert a markdown image like \
         `![alt](<asset_path>)` via syo_siyuan_block_insert, or include it in syo_siyuan_doc_create \
         markdown. The path is kernel-relative and usable directly in SiYuan markdown.",
    ))
}
