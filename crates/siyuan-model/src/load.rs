use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::Deserialize;

use siyuan_client::SiyuanClient;
use siyuan_types::{BlockId, BlockNode, BlockRole, BlockType, NotebookId, SiyuanError};

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
    box_: String,
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

/// Build a depth-first ordered list of nodes.
///
/// `children_map` maps parent id → ordered child ids (document order).
/// Siblings not present in `children_map` fall back to `sort, id` ordering.
fn dfs_order(
    nodes: Vec<BlockNode>,
    children_map: &std::collections::HashMap<String, Vec<String>>,
    doc_id: &BlockId,
) -> Vec<BlockNode> {
    use std::collections::HashMap;

    // Index nodes by id string for O(1) retrieval.
    let mut id_to_idx: HashMap<&str, usize> = HashMap::with_capacity(nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        id_to_idx.insert(n.id.as_str(), i);
    }

    let mut out: Vec<usize> = Vec::with_capacity(nodes.len());
    let mut visited: Vec<bool> = vec![false; nodes.len()];

    // Iterative DFS; stack holds ids to visit.
    let mut stack: Vec<String> = vec![doc_id.as_str().to_string()];
    while let Some(id) = stack.pop() {
        let Some(&idx) = id_to_idx.get(id.as_str()) else {
            continue;
        };
        if visited[idx] {
            continue;
        }
        visited[idx] = true;
        out.push(idx);

        // Determine child order: use children_map if available (document order),
        // otherwise fall back to sort,id from the SQL result.
        let kids: Vec<String> = if let Some(ordered) = children_map.get(&id) {
            ordered.clone()
        } else {
            // Collect children of this node from the nodes slice in SQL order.
            nodes
                .iter()
                .filter(|n| n.parent_id.as_ref().map(|p| p.as_str()) == Some(id.as_str()))
                .map(|n| n.id.as_str().to_string())
                .collect()
        };

        // Push in reverse order so pop() yields them in forward document order.
        for kid in kids.iter().rev() {
            stack.push(kid.clone());
        }
    }

    // Append unreachable blocks (shouldn't happen in a well-formed document).
    for (i, &vis) in visited.iter().enumerate() {
        if !vis {
            out.push(i);
        }
    }

    out.into_iter().map(|i| nodes[i].clone()).collect()
}

pub async fn load_doc(
    client: &SiyuanClient,
    doc_id: &BlockId,
    page: PageRequest,
) -> Result<DocBundle> {
    // 1. Pull every block in this doc via SQL (sort,id for a stable baseline).
    let stmt = format!(
        r#"SELECT id, parent_id, root_id, box AS "box_", hpath, markdown,
                  type, subtype, ial, sort, created, updated, hash
           FROM blocks
           WHERE root_id = '{}'
           ORDER BY sort, id"#,
        doc_id.as_str()
    );
    let rows: Vec<BlockRow> = client.sql_typed(&stmt).await.context("load doc blocks")?;

    // A real document always has at least its root row (the doc block itself,
    // with `root_id = id` self-reference). An empty result therefore means the
    // doc does not exist; surface a typed NotFound rather than a misleading
    // "no blocks" message so the MCP layer can map it to a proper error kind.
    if rows.is_empty() {
        return Err(SiyuanError::NotFound(doc_id.to_string()).into());
    }

    // 2. Lift into BlockNode.
    let mut nodes: Vec<BlockNode> = Vec::with_capacity(rows.len());
    let mut doc_meta: Option<(NotebookId, String)> = None;
    // Collect container block ids whose children need document-order resolution.
    let mut container_ids: Vec<BlockId> = Vec::new();

    for r in &rows {
        let id = BlockId::parse(&r.id).context("parsing block id")?;
        let root_id = BlockId::parse(&r.root_id).context("parsing root id")?;
        let parent_id = if r.parent_id.is_empty() {
            None
        } else {
            Some(BlockId::parse(&r.parent_id).context("parsing parent id")?)
        };
        let notebook_id = NotebookId::parse(&r.box_).context("parsing notebook id")?;
        let block_type = BlockType::from_kernel(&r.block_type);
        let role = BlockRole::for_block_type(block_type);

        if block_type == BlockType::Document {
            doc_meta = Some((notebook_id.clone(), r.hpath.clone()));
        }

        // Container types can have children that need ordering.
        if matches!(
            block_type,
            BlockType::Document
                | BlockType::Heading
                | BlockType::List
                | BlockType::ListItem
                | BlockType::SuperBlock
                | BlockType::Blockquote
        ) {
            container_ids.push(id.clone());
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

    // 3. Fetch document-order child lists for each container block.
    //    SiYuan's getChildBlocks returns children in correct document order,
    //    which may differ from sort,id order for bulk-created documents.
    let mut children_map: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for cid in &container_ids {
        match client.get_child_blocks(cid).await {
            Ok(kids) if !kids.is_empty() => {
                children_map.insert(
                    cid.as_str().to_string(),
                    kids.into_iter()
                        .map(|k| k.id.as_str().to_string())
                        .collect(),
                );
            }
            // No children or API error: fall back to SQL order for this parent.
            _ => {}
        }
    }

    // 4. Re-order nodes into DFS using the document-order children map.
    let mut nodes = dfs_order(nodes, &children_map, doc_id);

    // 5. Populate semantic children fields.
    populate_structural_children(&mut nodes);
    populate_section_children(&mut nodes);

    // 6. Paginate.
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
