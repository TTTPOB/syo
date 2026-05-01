use serde::Serialize;
use siyuan_types::BlockId;

#[derive(Debug, Serialize)]
pub(crate) struct ById<'a> {
    pub(crate) id: &'a BlockId,
}
