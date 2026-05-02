use std::path::Path;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Deserialize)]
pub struct UploadResult {
    /// Map from original filename -> kernel-stored relative path (e.g.
    /// `assets/foo-20260501093000-abcdefg.png`).
    // Kernel 3.6.5 sends `null` when there are no successes; treat null == empty.
    #[serde(default, rename = "succMap")]
    pub succ_map: Option<std::collections::BTreeMap<String, String>>,
    // Kernel 3.6.5 sends `null` when there are no failures; treat null == empty.
    #[serde(default, rename = "errFiles")]
    pub err_files: Option<Vec<String>>,
}

impl SiyuanClient {
    /// Upload a single file as an asset. Returns the kernel-relative path,
    /// e.g. `assets/myimg-20260501093000-abcdefg.png`, suitable for embedding
    /// in markdown as `![alt](assets/...)`.
    pub async fn upload_asset(&self, file_path: &Path) -> Result<String, SiyuanError> {
        // Use tokio::fs to keep the executor responsive: std::fs::read blocks
        // the worker thread, which can starve other tasks on a single-thread
        // runtime and stall the whole runtime under load on a multi-thread one.
        let bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| SiyuanError::Parse(format!("read {}: {e}", file_path.display())))?;
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| SiyuanError::Parse(format!("bad file name: {}", file_path.display())))?
            .to_string();

        let part = Part::bytes(bytes).file_name(filename.clone());
        let form = Form::new().part("file[]", part);

        let url = self
            .base_url()
            .join("api/asset/upload")
            .map_err(|e| SiyuanError::Parse(e.to_string()))?;

        let resp = self
            .http
            .post(url)
            .header("Authorization", format!("Token {}", self.token()))
            .multipart(form)
            .send()
            .await
            .map_err(|e| SiyuanError::Http(e.to_string()))?;

        let body = resp
            .text()
            .await
            .map_err(|e| SiyuanError::Http(e.to_string()))?;
        let upload: UploadResult = self.decode_envelope(&body)?;
        let err_files = upload.err_files.unwrap_or_default();
        if !err_files.is_empty() {
            return Err(SiyuanError::Api {
                code: -1,
                msg: format!("upload failed for: {:?}", err_files),
            });
        }
        upload
            .succ_map
            .unwrap_or_default()
            .get(&filename)
            .cloned()
            .ok_or_else(|| SiyuanError::Parse(format!("succMap missing entry for {filename}")))
    }
}
