use serde::Serialize;

use siyuan_types::{BlockId, NotebookId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct CreateDocReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
    markdown: &'a str,
}

#[derive(Debug, Serialize)]
struct RenameDocReq<'a> {
    #[serde(rename = "notebook")]
    notebook: &'a NotebookId,
    path: &'a str,
    title: &'a str,
}

#[derive(Debug, Serialize)]
struct MoveDocsReq<'a> {
    #[serde(rename = "fromPaths")]
    from_paths: &'a [String],
    #[serde(rename = "toNotebook")]
    to_notebook: &'a NotebookId,
    #[serde(rename = "toPath")]
    to_path: &'a str,
}

#[derive(Debug, Serialize)]
struct RemoveDocReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
}

#[derive(Debug, Serialize)]
struct GetIdsReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
}

#[derive(Debug, Serialize)]
struct GetHPathReq<'a> {
    id: &'a BlockId,
}

impl SiyuanClient {
    /// `/api/filetree/createDocWithMd` — returns the new doc's block id.
    pub async fn create_doc_with_md(
        &self,
        notebook: &NotebookId,
        hpath: &str,
        markdown: &str,
    ) -> Result<BlockId, SiyuanError> {
        let raw: String = self
            .post(
                "/api/filetree/createDocWithMd",
                &CreateDocReq {
                    notebook,
                    path: hpath,
                    markdown,
                },
            )
            .await?;
        BlockId::parse(raw).map_err(|e| SiyuanError::Parse(e.to_string()))
    }

    pub async fn rename_doc(
        &self,
        notebook: &NotebookId,
        path: &str,
        new_title: &str,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/filetree/renameDoc",
                &RenameDocReq {
                    notebook,
                    path,
                    title: new_title,
                },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn move_docs(
        &self,
        from_paths: &[String],
        to_notebook: &NotebookId,
        to_path: &str,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/filetree/moveDocs",
                &MoveDocsReq {
                    from_paths,
                    to_notebook,
                    to_path,
                },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn remove_doc(&self, notebook: &NotebookId, path: &str) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/filetree/removeDoc", &RemoveDocReq { notebook, path })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    /// `/api/filetree/getIDsByHPath` — resolve a human path to one or more block ids.
    pub async fn get_ids_by_hpath(
        &self,
        notebook: &NotebookId,
        hpath: &str,
    ) -> Result<Vec<BlockId>, SiyuanError> {
        let raw: Vec<String> = self
            .post(
                "/api/filetree/getIDsByHPath",
                &GetIdsReq {
                    notebook,
                    path: hpath,
                },
            )
            .await?;
        raw.into_iter()
            .map(|s| BlockId::parse(s).map_err(|e| SiyuanError::Parse(e.to_string())))
            .collect()
    }

    /// `/api/filetree/getHPathByID` — opposite of above.
    pub async fn get_hpath_by_id(&self, id: &BlockId) -> Result<String, SiyuanError> {
        self.post("/api/filetree/getHPathByID", &GetHPathReq { id })
            .await
    }
}
