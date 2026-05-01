use serde::{Deserialize, Serialize};

use siyuan_types::{NotebookId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    pub id: NotebookId,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub sort: i64,
    #[serde(default)]
    pub closed: bool,
}

#[derive(Debug, Deserialize)]
struct LsNotebooksData {
    notebooks: Vec<Notebook>,
}

#[derive(Debug, Serialize)]
struct OneNotebook<'a> {
    notebook: &'a NotebookId,
}

#[derive(Debug, Serialize)]
struct CreateNotebook<'a> {
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct RenameNotebook<'a> {
    notebook: &'a NotebookId,
    name: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedNotebook {
    pub notebook: Notebook,
}

impl SiyuanClient {
    pub async fn ls_notebooks(&self) -> Result<Vec<Notebook>, SiyuanError> {
        let data: LsNotebooksData = self
            .post("/api/notebook/lsNotebooks", &serde_json::json!({}))
            .await?;
        Ok(data.notebooks)
    }

    pub async fn open_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/openNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn close_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/closeNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn create_notebook(&self, name: &str) -> Result<Notebook, SiyuanError> {
        let data: CreatedNotebook = self
            .post("/api/notebook/createNotebook", &CreateNotebook { name })
            .await?;
        Ok(data.notebook)
    }

    pub async fn rename_notebook(
        &self,
        id: &NotebookId,
        new_name: &str,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/notebook/renameNotebook",
                &RenameNotebook {
                    notebook: id,
                    name: new_name,
                },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn remove_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/notebook/removeNotebook",
                &OneNotebook { notebook: id },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }
}
