use std::collections::BTreeMap;

use serde::Serialize;

use siyuan_types::{BlockId, SiyuanError};

use super::common::ById;
use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct SetAttrsReq<'a> {
    id: &'a BlockId,
    attrs: &'a BTreeMap<String, String>,
}

impl SiyuanClient {
    pub async fn get_block_attrs(
        &self,
        id: &BlockId,
    ) -> Result<BTreeMap<String, String>, SiyuanError> {
        self.post("/api/attr/getBlockAttrs", &ById { id }).await
    }

    pub async fn set_block_attrs(
        &self,
        id: &BlockId,
        attrs: &BTreeMap<String, String>,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/attr/setBlockAttrs", &SetAttrsReq { id, attrs })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }
}
