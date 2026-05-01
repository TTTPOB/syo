use std::path::Path;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Deserialize)]
pub struct UploadResult {
    /// Map from original filename -> kernel-stored relative path (e.g.
    /// `assets/foo-20260501093000-abcdefg.png`).
    #[serde(default, rename = "succMap")]
    pub succ_map: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "errFiles")]
    pub err_files: Vec<String>,
}

impl SiyuanClient {
    /// Upload a single file as an asset. Returns the kernel-relative path,
    /// e.g. `assets/myimg-20260501093000-abcdefg.png`, suitable for embedding
    /// in markdown as `![alt](assets/...)`.
    pub async fn upload_asset(&self, file_path: &Path) -> Result<String, SiyuanError> {
        let bytes = std::fs::read(file_path)
            .map_err(|e| SiyuanError::Http(format!("read {}: {e}", file_path.display())))?;
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
        if !upload.err_files.is_empty() {
            return Err(SiyuanError::Api {
                code: -1,
                msg: format!("upload failed for: {:?}", upload.err_files),
            });
        }
        upload
            .succ_map
            .get(&filename)
            .cloned()
            .ok_or_else(|| SiyuanError::Parse(format!("succMap missing entry for {filename}")))
    }
}
