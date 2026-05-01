use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, SiyuanError};

use super::common::ById;
use crate::SiyuanClient;

// -------- request types --------

#[derive(Debug, Serialize)]
struct InsertReq<'a> {
    #[serde(rename = "dataType")]
    data_type: &'a str,
    data: &'a str,
    #[serde(rename = "previousID", skip_serializing_if = "Option::is_none")]
    previous_id: Option<&'a BlockId>,
    #[serde(rename = "nextID", skip_serializing_if = "Option::is_none")]
    next_id: Option<&'a BlockId>,
    #[serde(rename = "parentID", skip_serializing_if = "Option::is_none")]
    parent_id: Option<&'a BlockId>,
}

#[derive(Debug, Serialize)]
struct AppendOrPrependReq<'a> {
    #[serde(rename = "dataType")]
    data_type: &'a str,
    data: &'a str,
    #[serde(rename = "parentID")]
    parent_id: &'a BlockId,
}

#[derive(Debug, Serialize)]
struct UpdateReq<'a> {
    id: &'a BlockId,
    #[serde(rename = "dataType")]
    data_type: &'a str,
    data: &'a str,
}

#[derive(Debug, Serialize)]
struct MoveReq<'a> {
    id: &'a BlockId,
    #[serde(rename = "previousID", skip_serializing_if = "Option::is_none")]
    previous_id: Option<&'a BlockId>,
    #[serde(rename = "parentID", skip_serializing_if = "Option::is_none")]
    parent_id: Option<&'a BlockId>,
}

// -------- response types --------

#[derive(Debug, Deserialize)]
pub struct DoOperation {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(rename = "parentID", default)]
    pub parent_id: Option<String>,
    #[serde(rename = "previousID", default)]
    pub previous_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    #[serde(rename = "doOperations", default)]
    pub do_operations: Vec<DoOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildBlock {
    pub id: BlockId,
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub subtype: String,
}

#[derive(Debug, Deserialize)]
pub struct BlockKramdown {
    pub id: BlockId,
    pub kramdown: String,
}

// -------- helpers --------

fn first_new_id(txs: &[Transaction]) -> Result<BlockId, SiyuanError> {
    for tx in txs {
        for op in &tx.do_operations {
            if let Some(id) = op.id.as_deref() {
                return BlockId::parse(id).map_err(|e| SiyuanError::Parse(e.to_string()));
            }
        }
    }
    Err(SiyuanError::Parse(
        "no id found in transaction operations".into(),
    ))
}

// -------- methods --------

impl SiyuanClient {
    pub async fn get_block_kramdown(&self, id: &BlockId) -> Result<BlockKramdown, SiyuanError> {
        self.post("/api/block/getBlockKramdown", &ById { id }).await
    }

    pub async fn get_child_blocks(&self, id: &BlockId) -> Result<Vec<ChildBlock>, SiyuanError> {
        self.post("/api/block/getChildBlocks", &ById { id }).await
    }

    /// Insert before/after an anchor block. Pass exactly one of
    /// `previous_id` / `next_id` (typically `previous_id` for "after").
    pub async fn insert_block_markdown(
        &self,
        markdown: &str,
        previous_id: Option<&BlockId>,
        next_id: Option<&BlockId>,
        parent_id: Option<&BlockId>,
    ) -> Result<BlockId, SiyuanError> {
        let txs: Vec<Transaction> = self
            .post(
                "/api/block/insertBlock",
                &InsertReq {
                    data_type: "markdown",
                    data: markdown,
                    previous_id,
                    next_id,
                    parent_id,
                },
            )
            .await?;
        first_new_id(&txs)
    }

    pub async fn append_block_markdown(
        &self,
        markdown: &str,
        parent_id: &BlockId,
    ) -> Result<BlockId, SiyuanError> {
        let txs: Vec<Transaction> = self
            .post(
                "/api/block/appendBlock",
                &AppendOrPrependReq {
                    data_type: "markdown",
                    data: markdown,
                    parent_id,
                },
            )
            .await?;
        first_new_id(&txs)
    }

    pub async fn prepend_block_markdown(
        &self,
        markdown: &str,
        parent_id: &BlockId,
    ) -> Result<BlockId, SiyuanError> {
        let txs: Vec<Transaction> = self
            .post(
                "/api/block/prependBlock",
                &AppendOrPrependReq {
                    data_type: "markdown",
                    data: markdown,
                    parent_id,
                },
            )
            .await?;
        first_new_id(&txs)
    }

    pub async fn update_block_markdown(
        &self,
        id: &BlockId,
        markdown: &str,
    ) -> Result<(), SiyuanError> {
        let _: Vec<Transaction> = self
            .post(
                "/api/block/updateBlock",
                &UpdateReq {
                    id,
                    data_type: "markdown",
                    data: markdown,
                },
            )
            .await?;
        Ok(())
    }

    pub async fn delete_block(&self, id: &BlockId) -> Result<(), SiyuanError> {
        let _: Vec<Transaction> = self.post("/api/block/deleteBlock", &ById { id }).await?;
        Ok(())
    }

    pub async fn move_block(
        &self,
        id: &BlockId,
        previous_id: Option<&BlockId>,
        parent_id: Option<&BlockId>,
    ) -> Result<(), SiyuanError> {
        // /api/block/moveBlock returns {"code":0,"data":null} on success;
        // use post_envelope + into_result_or_unit to tolerate the null data field.
        let _: serde_json::Value = self
            .post_envelope(
                "/api/block/moveBlock",
                &MoveReq {
                    id,
                    previous_id,
                    parent_id,
                },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_new_id_extracts_from_transaction() {
        let json = r#"[{"doOperations":[{"action":"insert","id":"20260501093000-abc1234"}]}]"#;
        let txs: Vec<Transaction> = serde_json::from_str(json).unwrap();
        let id = first_new_id(&txs).unwrap();
        assert_eq!(id.as_str(), "20260501093000-abc1234");
    }

    #[test]
    fn first_new_id_errors_on_empty() {
        let txs: Vec<Transaction> = vec![];
        assert!(first_new_id(&txs).is_err());
    }
}
