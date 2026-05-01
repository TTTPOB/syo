use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct ExportReq<'a> {
    id: &'a BlockId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedDoc {
    #[serde(default)]
    pub h_path: String,
    pub content: String,
}

impl SiyuanClient {
    pub async fn export_md_content(&self, doc_id: &BlockId) -> Result<ExportedDoc, SiyuanError> {
        self.post("/api/export/exportMdContent", &ExportReq { id: doc_id }).await
    }
}
