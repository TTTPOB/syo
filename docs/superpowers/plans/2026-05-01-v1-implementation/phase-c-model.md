# Phase C: Model layer

> **Part of:** [v1 Implementation Plan](../2026-05-01-v1-implementation.md) · **Prev:** [Phase B: HTTP Client](phase-b-client.md) · **Next:** [Phase D: Render](phase-d-render.md)
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this phase task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Build the semantic layer `siyuan-model`: the `DocBundle` shape, `load_doc` pipeline (SQL → DFS → section/container detection → 50/page pagination), relation hints from refs/spans, tag listing/search, and BFS graph neighborhoods.

---

## Task C1: bundle types + load_doc + pagination + section/container

**Files:**
- Modify: `crates/siyuan-model/src/bundle.rs`
- Modify: `crates/siyuan-model/src/section.rs`
- Modify: `crates/siyuan-model/src/container.rs`
- Modify: `crates/siyuan-model/src/load.rs`
- Modify: `crates/siyuan-model/src/pagination.rs`

**Background:** Load 流水线：
1. SQL 查 `blocks WHERE root_id = ?` 拿全部块
2. 按 `parent_id + sort` 重建 DFS 顺序
3. 对每个 heading 块，扫后续兄弟直到遇到同级或更高级 heading，把这段记到 `section_children`
4. 切 50 块/页

- [ ] **Step 1: 写 `bundle.rs`**

Replace:

```rust
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
    pub page: usize,        // 1-indexed
    pub page_size: usize,
    pub total_blocks: usize,
    pub total_pages: usize,
}

impl DocBundle {
    pub const SCHEMA: &'static str = "siyuan-agent.doc-bundle.v1";
}
```

- [ ] **Step 2: 写 `section.rs`**

Replace:

```rust
use siyuan_types::{BlockNode, BlockType};

/// Compute heading sections by walking the DFS-ordered block list. For each
/// heading h_n at level L, the section spans subsequent siblings until the next
/// heading whose level is <= L (or end of doc).
pub fn populate_section_children(blocks: &mut [BlockNode]) {
    // First, snapshot heading positions and levels.
    let mut headings: Vec<(usize, u8)> = Vec::new(); // (index, level)
    for (i, b) in blocks.iter().enumerate() {
        if b.block_type == BlockType::Heading {
            let level = parse_heading_level(b.subtype.as_deref());
            headings.push((i, level));
        }
    }

    // For each heading, walk forward to find section end among the same parent.
    for (h_idx, level) in headings.iter().copied() {
        let parent = blocks[h_idx].parent_id.clone();
        let mut section: Vec<_> = Vec::new();
        for j in (h_idx + 1)..blocks.len() {
            if blocks[j].parent_id != parent {
                continue;
            }
            if blocks[j].block_type == BlockType::Heading {
                let other = parse_heading_level(blocks[j].subtype.as_deref());
                if other <= level {
                    break;
                }
            }
            section.push(blocks[j].id.clone());
        }
        blocks[h_idx].section_children = section;
    }
}

fn parse_heading_level(subtype: Option<&str>) -> u8 {
    match subtype {
        Some("h1") => 1,
        Some("h2") => 2,
        Some("h3") => 3,
        Some("h4") => 4,
        Some("h5") => 5,
        Some("h6") => 6,
        _ => 6, // unknown → deepest, so it gets absorbed by anything
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use siyuan_types::{BlockId, BlockRole, BlockType, NotebookId};
    use std::collections::BTreeMap;

    fn mk(id: &str, parent: Option<&str>, root: &str, ty: BlockType, sub: Option<&str>) -> BlockNode {
        BlockNode {
            id: BlockId::parse(id).unwrap(),
            root_id: BlockId::parse(root).unwrap(),
            parent_id: parent.map(|p| BlockId::parse(p).unwrap()),
            notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
            block_type: ty,
            subtype: sub.map(String::from),
            role: BlockRole::for_block_type(ty),
            markdown: String::new(),
            kramdown: None,
            ial: None,
            attrs: BTreeMap::new(),
            hash: None,
            created: None,
            updated: None,
            sort: None,
            structural_children: vec![],
            section_children: vec![],
        }
    }

    #[test]
    fn h2_section_stops_at_next_h2() {
        let root = "20260501000001-doc0001";
        let mut blocks = vec![
            mk("20260501000010-h2aaaaa", Some(root), root, BlockType::Heading, Some("h2")),
            mk("20260501000020-paaaaaa", Some(root), root, BlockType::Paragraph, None),
            mk("20260501000030-paaaaab", Some(root), root, BlockType::Paragraph, None),
            mk("20260501000040-h2bbbbb", Some(root), root, BlockType::Heading, Some("h2")),
            mk("20260501000050-paaaaac", Some(root), root, BlockType::Paragraph, None),
        ];
        populate_section_children(&mut blocks);
        let h2a_section: Vec<_> = blocks[0].section_children.iter().map(|id| id.as_str().to_owned()).collect();
        assert_eq!(h2a_section, vec!["20260501000020-paaaaaa", "20260501000030-paaaaab"]);
    }

    #[test]
    fn h2_section_includes_h3_inside_it() {
        let root = "20260501000001-doc0001";
        let mut blocks = vec![
            mk("20260501000010-h2aaaaa", Some(root), root, BlockType::Heading, Some("h2")),
            mk("20260501000020-h3aaaaa", Some(root), root, BlockType::Heading, Some("h3")),
            mk("20260501000030-paaaaab", Some(root), root, BlockType::Paragraph, None),
            mk("20260501000040-h2bbbbb", Some(root), root, BlockType::Heading, Some("h2")),
        ];
        populate_section_children(&mut blocks);
        let ids: Vec<_> = blocks[0].section_children.iter().map(|id| id.as_str().to_owned()).collect();
        assert_eq!(ids, vec!["20260501000020-h3aaaaa", "20260501000030-paaaaab"]);
    }
}
```

- [ ] **Step 3: 写 `container.rs`**

Replace:

```rust
use siyuan_types::{BlockNode, BlockType};

/// Mark each container's `structural_children` field. Assumes `blocks` is in
/// canonical DFS order with `parent_id` set on every non-doc block.
pub fn populate_structural_children(blocks: &mut [BlockNode]) {
    use std::collections::HashMap;
    let mut map: HashMap<_, Vec<_>> = HashMap::new();
    for b in blocks.iter() {
        if let Some(parent) = b.parent_id.clone() {
            map.entry(parent).or_default().push(b.id.clone());
        }
    }
    for b in blocks.iter_mut() {
        if matches!(
            b.block_type,
            BlockType::Document | BlockType::SuperBlock | BlockType::List | BlockType::ListItem | BlockType::Blockquote
        ) {
            if let Some(children) = map.remove(&b.id) {
                b.structural_children = children;
            }
        }
    }
}
```

- [ ] **Step 4: 写 `pagination.rs`**

Replace:

```rust
pub const DEFAULT_PAGE_SIZE: usize = 50;

#[derive(Debug, Clone, Copy)]
pub struct PageRequest {
    pub page: usize,       // 1-indexed
    pub page_size: usize,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self { page: 1, page_size: DEFAULT_PAGE_SIZE }
    }
}

pub struct PageOutcome<T> {
    pub items: Vec<T>,
    pub page: usize,
    pub page_size: usize,
    pub total: usize,
    pub total_pages: usize,
}

pub fn paginate<T: Clone>(all: &[T], req: PageRequest) -> PageOutcome<T> {
    let page_size = req.page_size.max(1);
    let total = all.len();
    let total_pages = total.div_ceil(page_size).max(1);
    let page = req.page.max(1).min(total_pages);
    let start = (page - 1) * page_size;
    let end = (start + page_size).min(total);
    PageOutcome {
        items: all[start..end].to_vec(),
        page,
        page_size,
        total,
        total_pages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_page_default_size() {
        let xs: Vec<i32> = (0..120).collect();
        let out = paginate(&xs, PageRequest::default());
        assert_eq!(out.items.len(), 50);
        assert_eq!(out.items[0], 0);
        assert_eq!(out.total, 120);
        assert_eq!(out.total_pages, 3);
        assert_eq!(out.page, 1);
    }

    #[test]
    fn last_page_partial() {
        let xs: Vec<i32> = (0..120).collect();
        let out = paginate(&xs, PageRequest { page: 3, page_size: 50 });
        assert_eq!(out.items.len(), 20);
        assert_eq!(out.items[0], 100);
    }

    #[test]
    fn empty_input_yields_one_empty_page() {
        let xs: Vec<i32> = vec![];
        let out = paginate(&xs, PageRequest::default());
        assert!(out.items.is_empty());
        assert_eq!(out.total_pages, 1);
        assert_eq!(out.page, 1);
    }
}
```

- [ ] **Step 5: 写 `load.rs`**

Replace:

```rust
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
    content: String,
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
        r#"SELECT id, parent_id, root_id, box AS "box_", hpath, content, markdown,
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

    let (notebook_id, hpath) = doc_meta
        .ok_or_else(|| anyhow::anyhow!("no document block (`type=d`) in result"))?;

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
```

- [ ] **Step 6: 跑 unit tests + check**

Run: `cargo test -p siyuan-model`

Expected: section + pagination + container = 5+ passed.

Run: `cargo check -p siyuan-model`

Expected: 通过。

- [ ] **Step 7: 提交**

```bash
git add crates/siyuan-model/src
git commit -m "feat(model): doc loading, section/container detection, pagination"
```

---

## Task C2: relation hints (refs/spans queries)

**Files:**
- Modify: `crates/siyuan-model/src/relations.rs`
- Modify: `crates/siyuan-model/src/tag.rs`

- [ ] **Step 1: 写 `relations.rs`**

Replace:

```rust
use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefHint {
    pub source_id: BlockId,
    pub target_id: BlockId,
    pub anchor: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockRelationSummary {
    pub outgoing_refs: Vec<RefHint>,
    pub incoming_refs_count: usize,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OutgoingRow {
    block_id: String,
    def_block_id: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize)]
struct IncomingRow {
    def_block_id: String,
    n: i64,
}

#[derive(Debug, Deserialize)]
struct TagRow {
    block_id: String,
    #[serde(default)]
    content: String,
}

/// Build a per-block relation summary for every id in `block_ids`.
pub async fn relations_for(
    client: &SiyuanClient,
    block_ids: &[BlockId],
) -> Result<BTreeMap<BlockId, BlockRelationSummary>> {
    if block_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let id_list = block_ids.iter().map(|i| format!("'{}'", i.as_str())).collect::<Vec<_>>().join(",");

    // Outgoing refs.
    let outgoing: Vec<OutgoingRow> = client
        .sql_typed(&format!(
            "SELECT block_id, def_block_id, content FROM refs WHERE block_id IN ({id_list})"
        ))
        .await
        .context("query outgoing refs")?;

    // Incoming counts.
    let incoming: Vec<IncomingRow> = client
        .sql_typed(&format!(
            "SELECT def_block_id, COUNT(*) AS n FROM refs WHERE def_block_id IN ({id_list}) GROUP BY def_block_id"
        ))
        .await
        .context("query incoming refs")?;

    // Tag spans.
    let tags: Vec<TagRow> = client
        .sql_typed(&format!(
            "SELECT block_id, content FROM spans WHERE type LIKE '%tag%' AND block_id IN ({id_list})"
        ))
        .await
        .context("query tags")?;

    let mut map: BTreeMap<BlockId, BlockRelationSummary> = BTreeMap::new();
    for id in block_ids {
        map.entry(id.clone()).or_default();
    }

    for r in outgoing {
        if let (Ok(src), Ok(tgt)) = (BlockId::parse(&r.block_id), BlockId::parse(&r.def_block_id)) {
            map.entry(src.clone()).or_default().outgoing_refs.push(RefHint {
                source_id: src,
                target_id: tgt,
                anchor: r.content,
            });
        }
    }

    for r in incoming {
        if let Ok(id) = BlockId::parse(&r.def_block_id) {
            map.entry(id).or_default().incoming_refs_count = r.n as usize;
        }
    }

    for r in tags {
        if let Ok(id) = BlockId::parse(&r.block_id) {
            map.entry(id).or_default().tags.push(r.content);
        }
    }

    Ok(map)
}
```

- [ ] **Step 2: 写 `tag.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagBlockHit {
    pub block_id: BlockId,
    pub root_id: BlockId,
    pub markdown_preview: String,
}

#[derive(Debug, Deserialize)]
struct Row {
    block_id: String,
    root_id: String,
    #[serde(default)]
    markdown: String,
}

/// List every distinct tag string in the workspace (sorted).
pub async fn list_tags(client: &SiyuanClient) -> Result<Vec<String>> {
    #[derive(Debug, Deserialize)]
    struct TagRow {
        content: String,
    }
    let rows: Vec<TagRow> = client
        .sql_typed("SELECT DISTINCT content FROM spans WHERE type LIKE '%tag%' ORDER BY content")
        .await
        .context("list tags")?;
    Ok(rows.into_iter().map(|r| r.content).collect())
}

/// Find every block that has the given tag.
pub async fn search_by_tag(client: &SiyuanClient, tag: &str) -> Result<Vec<TagBlockHit>> {
    let escaped = tag.replace('\'', "''");
    let stmt = format!(
        "SELECT b.id AS block_id, b.root_id, b.markdown
         FROM blocks b
         JOIN spans s ON s.block_id = b.id
         WHERE s.type LIKE '%tag%' AND s.content = '{escaped}'
         ORDER BY b.updated DESC
         LIMIT 200"
    );
    let rows: Vec<Row> = client.sql_typed(&stmt).await.context("search by tag")?;
    rows.into_iter()
        .map(|r| {
            Ok(TagBlockHit {
                block_id: BlockId::parse(&r.block_id).map_err(|e| anyhow::anyhow!(e))?,
                root_id: BlockId::parse(&r.root_id).map_err(|e| anyhow::anyhow!(e))?,
                markdown_preview: truncate(r.markdown.as_str(), 160),
            })
        })
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() } else { format!("{}…", &s[..max]) }
}
```

- [ ] **Step 3: cargo check**

Run: `cargo check -p siyuan-model`

Expected: 通过。

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-model/src/relations.rs crates/siyuan-model/src/tag.rs
git commit -m "feat(model): relation hints + tag list/search"
```

---

## Task C3: graph neighborhood BFS

**Files:**
- Modify: `crates/siyuan-model/src/graph.rs`

- [ ] **Step 1: 写实现**

Replace:

```rust
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: BlockId,
    pub root_id: BlockId,
    pub block_type: String,
    pub markdown_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: BlockId,
    pub target: BlockId,
    pub anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub schema: String, // "siyuan-agent.graph.v1"
    pub center: BlockId,
    pub depth: usize,
    pub direction: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
struct EdgeRow {
    block_id: String,
    def_block_id: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize)]
struct NodeRow {
    id: String,
    root_id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    markdown: String,
}

const NODE_LIMIT: usize = 500;
const EDGE_LIMIT: usize = 1000;

pub async fn neighborhood(
    client: &SiyuanClient,
    center: &BlockId,
    depth: usize,
    direction: Direction,
) -> Result<Graph> {
    let mut visited: BTreeSet<BlockId> = BTreeSet::new();
    visited.insert(center.clone());
    let mut frontier: VecDeque<BlockId> = VecDeque::new();
    frontier.push_back(center.clone());

    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut truncated = false;

    for _ in 0..depth {
        let current: Vec<BlockId> = std::mem::take(&mut frontier).into_iter().collect();
        if current.is_empty() {
            break;
        }
        let id_list = current.iter().map(|i| format!("'{}'", i.as_str())).collect::<Vec<_>>().join(",");
        let mut next_ids: BTreeSet<BlockId> = BTreeSet::new();

        if matches!(direction, Direction::Outgoing | Direction::Both) {
            let rows: Vec<EdgeRow> = client
                .sql_typed(&format!(
                    "SELECT block_id, def_block_id, content FROM refs WHERE block_id IN ({id_list})"
                ))
                .await
                .context("graph outgoing")?;
            for r in rows {
                if edges.len() >= EDGE_LIMIT { truncated = true; break; }
                let (src, tgt) = match (BlockId::parse(&r.block_id), BlockId::parse(&r.def_block_id)) {
                    (Ok(s), Ok(t)) => (s, t),
                    _ => continue,
                };
                edges.push(GraphEdge { source: src, target: tgt.clone(), anchor: r.content });
                if !visited.contains(&tgt) {
                    next_ids.insert(tgt);
                }
            }
        }
        if matches!(direction, Direction::Incoming | Direction::Both) {
            let rows: Vec<EdgeRow> = client
                .sql_typed(&format!(
                    "SELECT block_id, def_block_id, content FROM refs WHERE def_block_id IN ({id_list})"
                ))
                .await
                .context("graph incoming")?;
            for r in rows {
                if edges.len() >= EDGE_LIMIT { truncated = true; break; }
                let (src, tgt) = match (BlockId::parse(&r.block_id), BlockId::parse(&r.def_block_id)) {
                    (Ok(s), Ok(t)) => (s, t),
                    _ => continue,
                };
                edges.push(GraphEdge { source: src.clone(), target: tgt, anchor: r.content });
                if !visited.contains(&src) {
                    next_ids.insert(src);
                }
            }
        }

        for id in next_ids {
            if visited.len() >= NODE_LIMIT {
                truncated = true;
                break;
            }
            visited.insert(id.clone());
            frontier.push_back(id);
        }
    }

    // Fetch node metadata for everyone in `visited`.
    let id_list = visited.iter().map(|i| format!("'{}'", i.as_str())).collect::<Vec<_>>().join(",");
    let stmt = format!(
        "SELECT id, root_id, type, markdown FROM blocks WHERE id IN ({id_list})"
    );
    let rows: Vec<NodeRow> = client.sql_typed(&stmt).await.context("graph nodes")?;
    let mut node_map: BTreeMap<BlockId, GraphNode> = BTreeMap::new();
    for r in rows {
        if let (Ok(id), Ok(root)) = (BlockId::parse(&r.id), BlockId::parse(&r.root_id)) {
            let preview = if r.markdown.len() <= 100 {
                r.markdown
            } else {
                format!("{}…", &r.markdown[..100])
            };
            node_map.insert(
                id.clone(),
                GraphNode {
                    id,
                    root_id: root,
                    block_type: r.block_type,
                    markdown_preview: preview,
                },
            );
        }
    }

    let direction_s = match direction {
        Direction::Incoming => "incoming",
        Direction::Outgoing => "outgoing",
        Direction::Both => "both",
    };

    Ok(Graph {
        schema: "siyuan-agent.graph.v1".to_string(),
        center: center.clone(),
        depth,
        direction: direction_s.to_string(),
        nodes: node_map.into_values().collect(),
        edges,
        truncated,
    })
}
```

- [ ] **Step 2: cargo check**

Run: `cargo check -p siyuan-model`

Expected: 通过。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-model/src/graph.rs
git commit -m "feat(model): graph neighborhood BFS with limits"
```

