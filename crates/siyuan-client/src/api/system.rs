use siyuan_types::SiyuanError;

use crate::SiyuanClient;

impl SiyuanClient {
    /// `/api/system/version` — returns the kernel version string.
    pub async fn system_version(&self) -> Result<String, SiyuanError> {
        // Endpoint returns `data` as a plain string.
        self.post::<_, String>("/api/system/version", &serde_json::json!({}))
            .await
    }
}
