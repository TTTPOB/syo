use std::path::Path;

use rmcp::ErrorData as McpError;
use serde_json::{Value, json};

use siyuan_client::SiyuanClient;

use super::util::{ensure_object, required_string, siyuan_to_mcp};

pub async fn upload(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let file_path = required_string(&map, "file_path")?;

    let asset_path = client
        .upload_asset(Path::new(&file_path))
        .await
        .map_err(siyuan_to_mcp)?;
    Ok(json!({ "asset_path": asset_path }))
}
