use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use siyuan_client::SiyuanClient;
use siyuan_types::{BlockId, BlockNode, BlockRole, BlockType, NotebookId};

use crate::bundle::{DocBundle, DocMeta, PageInfo};
use crate::container::populate_structural_children;
use crate::pagination::{PageRequest, paginate};
use crate::section::populate_section_children;

#[derive(Debug, Deserialize)]
struct BlockRow {
    id: String,
    #[serde(default)]
    parent_id: String,
    #[serde(default)]
    root_id: String,
    #[serde(default)]
    box_: String, // serde rename below
    #[serde(default)]
    hpath: String,
    #[serde(default)]
    markdown: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    ial: String,
    #[serde(default)]
    sort: i64,
    #[serde(default)]
    created: String,
    #[serde(default)]
    updated: String,
    #[serde(default)]
    hash: String,
}

pub async fn load_doc(
    client: &SiyuanClient,
    doc_id: &BlockId,
    page: PageRequest,
) -> Result<DocBundle> {
    // 1. Pull every block in this doc via SQL.
    let stmt = format!(
        r#"SELECT id, parent_id, root_id, box AS "box_", hpath, markdown,
                  type, subtype, ial, sort, created, updated, hash
           FROM blocks
           WHERE root_id = '{}'
           ORDER BY sort, id"#,
        doc_id.as_str()
    );
    let rows: Vec<BlockRow> = client.sql_typed(&stmt).await.context("load doc blocks")?;

    if rows.is_empty() {
        bail!("doc {} has no blocks (does it exist?)", doc_id);
    }

    // 2. Lift into BlockNode in DFS order.
    let mut nodes: Vec<BlockNode> = Vec::with_capacity(rows.len());
    let mut doc_meta: Option<(NotebookId, String)> = None;
    for r in &rows {
        let id = BlockId::parse(&r.id).map_err(|e| anyhow::anyhow!(e))?;
        let root_id = BlockId::parse(&r.root_id).map_err(|e| anyhow::anyhow!(e))?;
        let parent_id = if r.parent_id.is_empty() {
            None
        } else {
            Some(BlockId::parse(&r.parent_id).map_err(|e| anyhow::anyhow!(e))?)
        };
        let notebook_id = NotebookId::parse(&r.box_).map_err(|e| anyhow::anyhow!(e))?;
        let block_type = BlockType::from_kernel(&r.block_type);
        let role = BlockRole::for_block_type(block_type);

        if block_type == BlockType::Document {
            doc_meta = Some((notebook_id.clone(), r.hpath.clone()));
        }

        nodes.push(BlockNode {
            id,
            root_id,
            parent_id,
            notebook_id,
            block_type,
            subtype: (!r.subtype.is_empty()).then(|| r.subtype.clone()),
            role,
            markdown: r.markdown.clone(),
            kramdown: None,
            ial: (!r.ial.is_empty()).then(|| r.ial.clone()),
            attrs: BTreeMap::new(),
            hash: (!r.hash.is_empty()).then(|| r.hash.clone()),
            created: (!r.created.is_empty()).then(|| r.created.clone()),
            updated: (!r.updated.is_empty()).then(|| r.updated.clone()),
            sort: Some(r.sort),
            structural_children: vec![],
            section_children: vec![],
        });
    }

    // 3. Populate semantic children fields.
    populate_structural_children(&mut nodes);
    populate_section_children(&mut nodes);

    // 4. Paginate.
    let outcome = paginate(&nodes, page);

    let (notebook_id, hpath) =
        doc_meta.ok_or_else(|| anyhow::anyhow!("no document block (`type=d`) in result"))?;

    let title = hpath.rsplit('/').next().unwrap_or("(untitled)").to_string();

    Ok(DocBundle {
        schema: DocBundle::SCHEMA.to_string(),
        doc: DocMeta {
            id: doc_id.clone(),
            notebook_id,
            hpath,
            title,
        },
        page: PageInfo {
            page: outcome.page,
            page_size: outcome.page_size,
            total_blocks: outcome.total,
            total_pages: outcome.total_pages,
        },
        blocks: outcome.items,
    })
}
