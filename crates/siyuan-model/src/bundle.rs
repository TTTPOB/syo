use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, BlockNode, NotebookId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocBundle {
    pub schema: String, // always "siyuan-agent.doc-bundle.v1"
    pub doc: DocMeta,
    pub page: PageInfo,
    pub blocks: Vec<BlockNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMeta {
    pub id: BlockId,
    pub notebook_id: NotebookId,
    pub hpath: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub page: usize, // 1-indexed
    pub page_size: usize,
    pub total_blocks: usize,
    pub total_pages: usize,
}

impl DocBundle {
    pub const SCHEMA: &'static str = "siyuan-agent.doc-bundle.v1";
}
