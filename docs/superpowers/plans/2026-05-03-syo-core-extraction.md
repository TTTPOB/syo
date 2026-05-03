# syo-core Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract a shared `syo-core` crate from CLI and MCP, then rewire both surfaces to use it.

**Architecture:** New `syo-core` crate sits between surfaces (CLI/MCP) and backend (siyuan-client/siyuan-model). Each domain operation becomes a typed Input → Output → async execute function. `syo` is renamed to `syo-cli`.

**Tech Stack:** Rust workspace, anyhow, serde, siyuan-client, siyuan-model, siyuan-render, siyuan-types.

**Constraint:** Sequential dispatch only. Work directly on master. No parallel agents.

---

### Task 1: Create syo-core crate scaffold

**Files:**
- Create: `crates/syo-core/Cargo.toml`
- Create: `crates/syo-core/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml for syo-core**

```toml
[package]
name = "syo-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Shared operations layer for syo CLI and MCP surfaces"

[dependencies]
siyuan-types = { workspace = true }
siyuan-client = { workspace = true }
siyuan-model = { workspace = true }
siyuan-render = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

- [ ] **Step 2: Create lib.rs**

```rust
pub mod asset;
pub mod attr;
pub mod block;
pub mod doc;
pub mod graph;
pub mod notebook;
pub mod search;
pub mod sql;
pub mod system;
pub mod tag;
```

- [ ] **Step 3: Add syo-core to workspace Cargo.toml**

Add to workspace members and dependencies in root `Cargo.toml`:
- Add `syo-core = { path = "crates/syo-core" }` to `[workspace.dependencies]`

- [ ] **Step 4: Commit**

```bash
git add crates/syo-core/Cargo.toml crates/syo-core/src/lib.rs Cargo.toml
git commit -m "feat: scaffold syo-core crate

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: syo-core – system and notebook ops

**Files:**
- Create: `crates/syo-core/src/system.rs`
- Create: `crates/syo-core/src/notebook.rs`

- [ ] **Step 1: Write system.rs**

```rust
use anyhow::Result;
use siyuan_client::SiyuanClient;

pub struct StatusOutput {
    pub version: String,
}

pub async fn status(client: &SiyuanClient) -> Result<StatusOutput> {
    let version = client.system_version().await?;
    Ok(StatusOutput { version })
}
```

- [ ] **Step 2: Write notebook.rs**

```rust
use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_client::api::notebook::Notebook;
use siyuan_types::NotebookId;

// ---- ls ----
pub struct LsOutput {
    pub notebooks: Vec<Notebook>,
}

pub async fn ls(client: &SiyuanClient) -> Result<LsOutput> {
    let notebooks = client.ls_notebooks().await?;
    Ok(LsOutput { notebooks })
}

// ---- create ----
pub struct CreateInput {
    pub name: String,
}

pub struct CreateOutput {
    pub notebook: Notebook,
}

pub async fn create(client: &SiyuanClient, input: CreateInput) -> Result<CreateOutput> {
    let notebook = client.create_notebook(&input.name).await?;
    Ok(CreateOutput { notebook })
}

// ---- rename ----
pub struct RenameInput {
    pub id: NotebookId,
    pub name: String,
}

pub async fn rename(client: &SiyuanClient, input: RenameInput) -> Result<()> {
    client.rename_notebook(&input.id, &input.name).await?;
    Ok(())
}

// ---- remove ----
pub struct RemoveInput {
    pub id: NotebookId,
}

pub async fn remove(client: &SiyuanClient, input: RemoveInput) -> Result<()> {
    client.remove_notebook(&input.id).await?;
    Ok(())
}
```

- [ ] **Step 3: Verify syo-core builds**

```bash
cargo build -p syo-core
```

- [ ] **Step 4: Commit**

```bash
git add crates/syo-core/src/system.rs crates/syo-core/src/notebook.rs
git commit -m "feat(syo-core): add system and notebook operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: syo-core – attr ops

**Files:**
- Create: `crates/syo-core/src/attr.rs`

- [ ] **Step 1: Write attr.rs**

```rust
use std::collections::BTreeMap;
use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

// ---- get ----
pub struct GetAttrsInput {
    pub id: BlockId,
}

pub struct GetAttrsOutput {
    pub id: BlockId,
    pub attrs: BTreeMap<String, String>,
}

pub async fn get(client: &SiyuanClient, input: GetAttrsInput) -> Result<GetAttrsOutput> {
    let attrs = client.get_block_attrs(&input.id).await?;
    Ok(GetAttrsOutput { id: input.id, attrs })
}

// ---- set ----
pub struct SetAttrsInput {
    pub id: BlockId,
    pub attrs: BTreeMap<String, String>,
}

pub async fn set(client: &SiyuanClient, input: SetAttrsInput) -> Result<()> {
    client.set_block_attrs(&input.id, &input.attrs).await?;
    Ok(())
}

// ---- set_icon convenience ----
pub struct SetIconInput {
    pub id: BlockId,
    pub icon: String,
}

pub async fn set_icon(client: &SiyuanClient, input: SetIconInput) -> Result<()> {
    let mut attrs = BTreeMap::new();
    attrs.insert("icon".to_string(), input.icon);
    client.set_block_attrs(&input.id, &attrs).await?;
    Ok(())
}

// ---- set_sort convenience ----
pub struct SetSortInput {
    pub id: BlockId,
    pub sort: i64,
}

pub async fn set_sort(client: &SiyuanClient, input: SetSortInput) -> Result<()> {
    let mut attrs = BTreeMap::new();
    attrs.insert("sort".to_string(), input.sort.to_string());
    client.set_block_attrs(&input.id, &attrs).await?;
    Ok(())
}
```

- [ ] **Step 2: Verify syo-core builds**

```bash
cargo build -p syo-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/syo-core/src/attr.rs
git commit -m "feat(syo-core): add attr operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: syo-core – doc ops

**Files:**
- Create: `crates/syo-core/src/doc.rs`

- [ ] **Step 1: Write doc.rs**

```rust
use anyhow::{Context, Result};
use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, DocMeta, resolve as resolve_doc_meta, resolve_one_storage};
use siyuan_model::doc_tree::{Depth, DocNode, build_tree};
use siyuan_model::load::load_doc;
use siyuan_model::pagination::{PageRequest, DEFAULT_PAGE_SIZE};
use siyuan_render::agent_md::render_doc;
use siyuan_render::json_bundle::render_bundle;
use siyuan_types::{BlockId, NotebookId};

// ---- get ----
pub struct GetDocInput {
    pub id: BlockId,
    pub page: usize,
    pub page_size: usize,
    pub format: DocFormat,
}

pub enum DocFormat {
    AgentMd,
    Json,
    JsonPretty,
}

pub struct GetDocOutput {
    pub content: String,
}

pub async fn get(client: &SiyuanClient, input: GetDocInput) -> Result<GetDocOutput> {
    let bundle = load_doc(
        client,
        &input.id,
        PageRequest {
            page: input.page,
            page_size: input.page_size,
        },
    )
    .await?;
    let content = match input.format {
        DocFormat::AgentMd => render_doc(&bundle),
        DocFormat::Json => render_bundle(&bundle, false)?,
        DocFormat::JsonPretty => render_bundle(&bundle, true)?,
    };
    Ok(GetDocOutput { content })
}

// ---- create ----
pub struct CreateDocInput {
    pub notebook: NotebookId,
    pub hpath: String,
    pub markdown: String,
    pub force: bool,
}

pub struct CreateDocOutput {
    pub id: BlockId,
}

pub async fn create(client: &SiyuanClient, input: CreateDocInput) -> Result<CreateDocOutput> {
    use anyhow::bail;
    if !input.force {
        let lookup = DocLookup::ByHpath {
            notebook: input.notebook.clone(),
            hpath: input.hpath.clone(),
        };
        let existing = resolve_doc_meta(client, lookup).await?;
        if !existing.is_empty() {
            bail!(
                "hpath {} already exists (id: {}). Use force to overwrite.",
                input.hpath,
                existing[0].id
            );
        }
    }
    let id = client
        .create_doc_with_md(&input.notebook, &input.hpath, &input.markdown)
        .await?;
    Ok(CreateDocOutput { id })
}

// ---- resolve ----
pub struct ResolveOutput {
    pub docs: Vec<DocMeta>,
}

pub async fn resolve(client: &SiyuanClient, lookup: DocLookup) -> Result<ResolveOutput> {
    let docs = resolve_doc_meta(client, lookup).await?;
    Ok(ResolveOutput { docs })
}

// ---- rename ----
pub struct RenameInput {
    pub lookup: DocLookup,
    pub title: String,
}

pub async fn rename(client: &SiyuanClient, input: RenameInput) -> Result<()> {
    let (notebook, storage_path) = resolve_one_storage(client, input.lookup).await?;
    client.rename_doc(&notebook, &storage_path, &input.title).await?;
    Ok(())
}

// ---- move_doc ----
pub struct MoveDocInput {
    pub from: Vec<DocLookup>,
    pub to_notebook: NotebookId,
    pub to_path: String,
}

pub async fn move_docs(client: &SiyuanClient, input: MoveDocInput) -> Result<()> {
    let mut from_paths = Vec::with_capacity(input.from.len());
    for lookup in &input.from {
        let (_nb, storage_path) = resolve_one_storage(client, lookup.clone()).await?;
        from_paths.push(storage_path);
    }
    client.move_docs(&from_paths, &input.to_notebook, &input.to_path).await?;
    Ok(())
}

// ---- remove ----
pub struct RemoveDocInput {
    pub lookup: DocLookup,
}

pub async fn remove(client: &SiyuanClient, input: RemoveDocInput) -> Result<()> {
    let (notebook, storage_path) = resolve_one_storage(client, input.lookup).await?;
    client.remove_doc(&notebook, &storage_path).await?;
    Ok(())
}

// ---- tree ----
pub struct TreeInput {
    pub lookup: DocLookup,
    pub depth: Depth,
}

pub struct TreeOutput {
    pub tree: DocNode,
}

pub async fn tree(client: &SiyuanClient, input: TreeInput) -> Result<TreeOutput> {
    let tree = build_tree(client, input.lookup, input.depth).await?;
    Ok(TreeOutput { tree })
}
```

- [ ] **Step 2: Verify syo-core builds**

```bash
cargo build -p syo-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/syo-core/src/doc.rs
git commit -m "feat(syo-core): add doc operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: syo-core – block ops

**Files:**
- Create: `crates/syo-core/src/block.rs`

- [ ] **Step 1: Write block.rs**

```rust
use anyhow::{Context, Result, bail};
use siyuan_client::SiyuanClient;
use siyuan_model::load::load_doc;
use siyuan_model::pagination::PageRequest;
use siyuan_model::section::populate_section_children;
use siyuan_types::{BlockId, BlockType, Position};
use siyuan_types::position::PositionKind;

// ---- get ----
pub struct GetBlockOutput {
    pub id: BlockId,
    pub kramdown: String,
}

pub async fn get(client: &SiyuanClient, id: &BlockId) -> Result<GetBlockOutput> {
    let bk = client.get_block_kramdown(id).await?;
    Ok(GetBlockOutput {
        id: bk.id,
        kramdown: bk.kramdown,
    })
}

// ---- update ----
pub struct UpdateBlockInput {
    pub id: BlockId,
    pub markdown: String,
}

pub async fn update(client: &SiyuanClient, input: UpdateBlockInput) -> Result<()> {
    client.update_block_markdown(&input.id, &input.markdown).await?;
    Ok(())
}

// ---- insert ----
pub struct InsertBlockInput {
    pub markdown: String,
    pub position: PositionKind,
    pub anchor: BlockId,
}

pub struct InsertBlockOutput {
    pub id: BlockId,
}

pub async fn insert(client: &SiyuanClient, input: InsertBlockInput) -> Result<InsertBlockOutput> {
    let position = Position::from((input.position, input.anchor));
    let new_id = match position {
        Position::AfterBlock { block_id } => client
            .insert_block_markdown(&input.markdown, Some(&block_id), None, None)
            .await?,
        Position::BeforeBlock { block_id } => client
            .insert_block_markdown(&input.markdown, None, Some(&block_id), None)
            .await?,
        Position::AppendChild { container_id } => client
            .append_block_markdown(&input.markdown, &container_id)
            .await?,
        Position::PrependChild { container_id } => client
            .prepend_block_markdown(&input.markdown, &container_id)
            .await?,
        Position::AppendSection { heading_id } => {
            let section_end = resolve_section_end(client, &heading_id).await?;
            client
                .insert_block_markdown(&input.markdown, Some(&section_end), None, None)
                .await?
        }
        Position::PrependSection { heading_id } => {
            client
                .insert_block_markdown(&input.markdown, Some(&heading_id), None, None)
                .await?
        }
        Position::AppendDoc { doc_id } => client
            .append_block_markdown(&input.markdown, &doc_id)
            .await?,
        Position::PrependDoc { doc_id } => client
            .prepend_block_markdown(&input.markdown, &doc_id)
            .await?,
    };
    Ok(InsertBlockOutput { id: new_id })
}

// ---- delete ----
pub struct DeleteBlockInput {
    pub id: BlockId,
}

pub async fn delete(client: &SiyuanClient, input: DeleteBlockInput) -> Result<()> {
    client.delete_block(&input.id).await?;
    Ok(())
}

// ---- move (8 positions) ----
pub struct MoveBlockInput {
    pub id: BlockId,
    pub position: PositionKind,
    pub anchor: BlockId,
}

pub async fn move_block(client: &SiyuanClient, input: MoveBlockInput) -> Result<()> {
    match input.position {
        PositionKind::AfterBlock => {
            client.move_block(&input.id, Some(&input.anchor), None).await?;
        }
        PositionKind::BeforeBlock => {
            let prev_id = find_previous_sibling(client, &input.anchor).await?;
            client.move_block(&input.id, Some(&prev_id), None).await?;
        }
        PositionKind::AppendChild | PositionKind::AppendDoc => {
            client.move_block(&input.id, None, Some(&input.anchor)).await?;
        }
        PositionKind::PrependChild | PositionKind::PrependDoc => {
            client.move_block(&input.id, None, Some(&input.anchor)).await?;
        }
        PositionKind::AppendSection => {
            let section_end = resolve_section_end(client, &input.anchor).await?;
            client.move_block(&input.id, Some(&section_end), None).await?;
        }
        PositionKind::PrependSection => {
            client.move_block(&input.id, Some(&input.anchor), None).await?;
        }
    }
    Ok(())
}

/// Find the last block in the section owned by `heading_id`.
/// Used by both `insert` (AppendSection) and `move_block` (AppendSection).
pub async fn resolve_section_end(
    client: &SiyuanClient,
    heading_id: &BlockId,
) -> Result<BlockId> {
    #[derive(serde::Deserialize)]
    struct R {
        root_id: String,
        #[serde(rename = "type")]
        ty: String,
    }
    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT root_id, type FROM blocks WHERE id = '{}'",
            heading_id.as_str()
        ))
        .await?;
    let root = rows
        .first()
        .ok_or_else(|| anyhow::anyhow!("heading not found"))?;
    if root.ty != "h" {
        bail!("anchor for append_section must be a heading block");
    }
    let root_id = BlockId::parse(&root.root_id).context("parsing root id")?;

    let bundle = load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await?;
    let mut blocks = bundle.blocks;
    populate_section_children(&mut blocks);
    let heading = blocks
        .iter()
        .find(|b| &b.id == heading_id)
        .ok_or_else(|| anyhow::anyhow!("heading not in doc"))?;
    if heading.block_type != BlockType::Heading {
        bail!("anchor is not a heading after re-resolution");
    }
    if let Some(last) = heading.section_children.last() {
        Ok(last.clone())
    } else {
        Ok(heading_id.clone())
    }
}

/// Find the block immediately before `anchor` in its parent's children list.
async fn find_previous_sibling(
    client: &SiyuanClient,
    anchor: &BlockId,
) -> Result<BlockId> {
    #[derive(serde::Deserialize)]
    struct R {
        root_id: String,
    }
    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT root_id FROM blocks WHERE id = '{}'",
            anchor.as_str()
        ))
        .await?;
    let root = rows
        .first()
        .ok_or_else(|| anyhow::anyhow!("anchor block not found"))?;
    let root_id = BlockId::parse(&root.root_id).context("parsing root id")?;

    let bundle = load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await?;
    let blocks = bundle.blocks;
    let idx = blocks
        .iter()
        .position(|b| &b.id == anchor)
        .ok_or_else(|| anyhow::anyhow!("anchor block not found in document"))?;
    if idx == 0 {
        bail!("cannot move before first child of document; use prepend_child or prepend_doc instead");
    }
    let prev = &blocks[idx - 1];
    Ok(prev.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use siyuan_types::position::PositionKind;

    fn dummy_client() -> SiyuanClient {
        SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds")
    }

    #[test]
    fn parse_position_kind_all_eight() {
        let kinds = [
            "after_block",
            "before_block",
            "append_child",
            "prepend_child",
            "append_section",
            "prepend_section",
            "append_doc",
            "prepend_doc",
        ];
        for s in kinds {
            assert!(
                serde_json::from_value::<PositionKind>(serde_json::Value::String(s.to_owned())).is_ok(),
                "should parse {s}"
            );
        }
    }

    #[tokio::test]
    async fn move_block_accepts_all_eight_positions() {
        // Verify the match arm covers all 8 variants — compile-time check
        // is the real guard, but this explicitly exercises each branch.
        let client = dummy_client();
        let kinds = [
            PositionKind::AfterBlock,
            PositionKind::BeforeBlock,
            PositionKind::AppendChild,
            PositionKind::PrependChild,
            PositionKind::AppendSection,
            PositionKind::PrependSection,
            PositionKind::AppendDoc,
            PositionKind::PrependDoc,
        ];
        for kind in kinds {
            let input = MoveBlockInput {
                id: BlockId::parse("20260501090000-blk0001").unwrap(),
                position: kind,
                anchor: BlockId::parse("20260501090000-blk0002").unwrap(),
            };
            // Will fail with HTTP error (dummy client), but NOT with
            // "unsupported position" — proving all 8 match arms exist.
            let err = move_block(&client, input).await.unwrap_err();
            assert!(
                !err.to_string().contains("unsupported"),
                "position {kind:?} should not be rejected as unsupported; got: {err}"
            );
        }
    }
}
```

- [ ] **Step 2: Verify syo-core builds**

```bash
cargo build -p syo-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/syo-core/src/block.rs
git commit -m "feat(syo-core): add block operations with full 8-position move

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: syo-core – search, tag, graph, asset, sql ops

**Files:**
- Create: `crates/syo-core/src/search.rs`
- Create: `crates/syo-core/src/tag.rs`
- Create: `crates/syo-core/src/graph.rs`
- Create: `crates/syo-core/src/asset.rs`
- Create: `crates/syo-core/src/sql.rs`

- [ ] **Step 1: Write search.rs**

```rust
use anyhow::{Result, bail};
use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_string};
use siyuan_model::sql_guard;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SearchHit {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub markdown: String,
}

// ---- fulltext ----
pub struct FulltextInput {
    pub query: String,
    pub limit: usize,
}

pub struct SearchOutput {
    pub hits: Vec<SearchHit>,
}

pub async fn fulltext(client: &SiyuanClient, input: FulltextInput) -> Result<SearchOutput> {
    if input.query.trim().is_empty() {
        bail!("query must not be empty");
    }
    let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
    let limit = input.limit.min(limit_cap);
    let needle = escape_sql_string(&input.query);
    let stmt = format!(
        "SELECT id, type, markdown FROM blocks \
         WHERE markdown LIKE '%{needle}%' LIMIT {limit}"
    );
    if let Err(e) = sql_guard::validate_read_only(&stmt) {
        bail!("{e}");
    }
    let hits: Vec<SearchHit> = client.sql_typed(&stmt).await?;
    Ok(SearchOutput { hits })
}

// ---- blocks ----
pub struct BlocksInput {
    pub block_type: String,
    pub contains: String,
    pub limit: usize,
}

pub async fn blocks(client: &SiyuanClient, input: BlocksInput) -> Result<SearchOutput> {
    let mut conds = Vec::new();
    if !input.block_type.is_empty() {
        conds.push(format!("type = '{}'", input.block_type.replace('\'', "''")));
    }
    if !input.contains.is_empty() {
        conds.push(format!(
            "content LIKE '%{}%'",
            escape_sql_string(&input.contains)
        ));
    }
    let where_clause = if conds.is_empty() {
        "1=1".into()
    } else {
        conds.join(" AND ")
    };
    let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
    let limit = input.limit.min(limit_cap);
    let stmt = format!("SELECT id, type, markdown FROM blocks WHERE {where_clause} LIMIT {limit}");
    if let Err(e) = sql_guard::validate_read_only(&stmt) {
        bail!("{e}");
    }
    let hits: Vec<SearchHit> = client.sql_typed(&stmt).await?;
    Ok(SearchOutput { hits })
}
```

- [ ] **Step 2: Write tag.rs**

```rust
use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_model::tag as tag_model;

pub struct ListTagsOutput {
    pub tags: Vec<String>,
}

pub async fn list_tags(client: &SiyuanClient) -> Result<ListTagsOutput> {
    let tags = tag_model::list_tags(client).await?;
    Ok(ListTagsOutput { tags })
}

pub struct SearchByTagInput {
    pub tag: String,
    pub limit: usize,
}

pub struct SearchByTagOutput {
    pub hits: Vec<tag_model::TaggedBlock>,
}

pub async fn search_by_tag(client: &SiyuanClient, input: SearchByTagInput) -> Result<SearchByTagOutput> {
    let hits = tag_model::search_by_tag(client, &input.tag, input.limit).await?;
    Ok(SearchByTagOutput { hits })
}
```

- [ ] **Step 3: Write graph.rs**

```rust
use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_model::graph::{Direction, Graph, neighborhood};
use siyuan_types::BlockId;

pub struct NeighborhoodInput {
    pub center: BlockId,
    pub depth: usize,
    pub direction: Direction,
}

pub async fn neighborhood(client: &SiyuanClient, input: NeighborhoodInput) -> Result<Graph> {
    neighborhood(client, &input.center, input.depth, input.direction).await
}

pub async fn backlinks(client: &SiyuanClient, center: &BlockId) -> Result<Graph> {
    neighborhood(client, center, 1, Direction::Incoming).await
}

pub async fn outgoing(client: &SiyuanClient, center: &BlockId) -> Result<Graph> {
    neighborhood(client, center, 1, Direction::Outgoing).await
}
```

- [ ] **Step 4: Write asset.rs**

```rust
use std::path::Path;
use anyhow::Result;
use siyuan_client::SiyuanClient;

// ---- upload ----
pub struct UploadInput {
    pub file_path: String,
}

pub struct UploadOutput {
    pub asset_path: String,
}

pub async fn upload(client: &SiyuanClient, input: UploadInput) -> Result<UploadOutput> {
    let asset_path = client.upload_asset(Path::new(&input.file_path)).await?;
    Ok(UploadOutput { asset_path })
}

// ---- reference (pure formatter, no client needed) ----
pub struct ReferenceInput {
    pub path: String,
    pub alt: String,
}

pub struct ReferenceOutput {
    pub markdown: String,
}

pub fn reference(input: ReferenceInput) -> ReferenceOutput {
    let alt = if input.alt.is_empty() {
        input.path.rsplit('/').next().unwrap_or("").to_string()
    } else {
        input.alt
    };
    ReferenceOutput {
        markdown: format!("![{alt}]({})", input.path),
    }
}
```

- [ ] **Step 5: Write sql.rs**

```rust
use anyhow::{Result, bail};
use siyuan_client::SiyuanClient;
use siyuan_model::sql_guard;
use serde_json::Value;

pub struct SqlInput {
    pub stmt: String,
}

pub async fn raw(client: &SiyuanClient, input: SqlInput) -> Result<Vec<Value>> {
    if let Err(e) = sql_guard::validate_read_only(&input.stmt) {
        bail!("{e}");
    }
    let rows = client.sql(&input.stmt).await?;
    Ok(rows)
}
```

- [ ] **Step 6: Verify syo-core builds**

```bash
cargo build -p syo-core
```

- [ ] **Step 7: Commit**

```bash
git add crates/syo-core/src/search.rs crates/syo-core/src/tag.rs crates/syo-core/src/graph.rs crates/syo-core/src/asset.rs crates/syo-core/src/sql.rs
git commit -m "feat(syo-core): add search, tag, graph, asset, and sql operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Rename syo → syo-cli

**Files:**
- Rename: `crates/syo/` → `crates/syo-cli/`
- Modify: `crates/syo-cli/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Rename directory**

```bash
mv crates/syo crates/syo-cli
```

- [ ] **Step 2: Update crates/syo-cli/Cargo.toml**

Change `name = "syo"` to `name = "syo-cli"`.

Also change the `[[bin]]` section:
```toml
[package]
name = "syo-cli"

[[bin]]
name = "syo"
path = "src/main.rs"
```

- [ ] **Step 3: Update workspace Cargo.toml**

Change `syo = { path = "crates/syo" }` to `syo-cli = { path = "crates/syo-cli" }`.

Change `syo-mcp = { path = "crates/syo-mcp" }` if it depends on `syo` (it shouldn't currently, but verify).

- [ ] **Step 4: Add syo-core dep to syo-cli's Cargo.toml**

Add to `crates/syo-cli/Cargo.toml` dependencies:
```toml
syo-core = { workspace = true }
```

- [ ] **Step 5: Verify the rename compiles**

```bash
cargo build -p syo-cli
```

- [ ] **Step 6: Commit**

```bash
git add crates/syo-cli/ Cargo.toml
git commit -m "refactor: rename syo crate to syo-cli

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Rewire syo-cli status, notebook, attrs commands to use syo-core

**Files:**
- Modify: `crates/syo-cli/src/commands/status.rs`
- Modify: `crates/syo-cli/src/commands/notebook/ls.rs`
- Modify: `crates/syo-cli/src/commands/notebook/create.rs`
- Modify: `crates/syo-cli/src/commands/notebook/rename.rs`
- Modify: `crates/syo-cli/src/commands/notebook/remove.rs`
- Modify: `crates/syo-cli/src/commands/attrs/set.rs`

- [ ] **Step 1: Rewire status.rs**

Replace the body of `run`:
```rust
use anyhow::Result;
use tracing::info;
use siyuan_client::SiyuanClient;

pub async fn run(client: &SiyuanClient) -> Result<()> {
    let output = syo_core::system::status(client).await?;
    info!(%output.version, "siyuan ok");
    println!("{}", output.version);
    Ok(())
}
```

- [ ] **Step 2: Rewire notebook/ls.rs**

Replace the body of `run`:
```rust
use anyhow::Result;
use siyuan_client::SiyuanClient;
use serde_json;

pub async fn run(client: &SiyuanClient) -> Result<()> {
    let output = syo_core::notebook::ls(client).await?;
    println!("{}", serde_json::to_string_pretty(&output.notebooks)?);
    Ok(())
}
```

- [ ] **Step 3: Rewire notebook/create.rs**

Replace the `run` body:
```rust
use anyhow::{Context, Result};
use clap::Args;
use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

#[derive(Args, Debug)]
pub struct CreateNotebookArgs {
    #[arg(long)]
    pub name: String,
}

pub async fn run(client: &SiyuanClient, args: CreateNotebookArgs) -> Result<()> {
    let output = syo_core::notebook::create(client, syo_core::notebook::CreateInput {
        name: args.name,
    }).await?;
    serde_json::to_writer_pretty(std::io::stdout(), &output.notebook)?;
    println!();
    Ok(())
}
```

Wait — check the current implementation first. Read `crates/syo-cli/src/commands/notebook/create.rs` (after rename) to match the existing output pattern.

Actually, looking at the existing code pattern, the CLI prints `println!("{id}")` style. Let me check the current implementations.

Let me check the existing notebook commands briefly:

- `ls.rs` likely prints serde_json::to_string_pretty of the notebooks
- `create.rs` likely prints the notebook info
- `rename.rs` likely prints "ok"
- `remove.rs` likely prints "ok"

Let me re-read these files to match the existing output patterns. The plan should preserve the exact output format.

Actually, I know:
- status prints: `{version}\n` (the version string)
- notebook ls prints: JSON pretty of notebooks array
- notebook create prints: notebook JSON (from old impl)
- notebook rename prints: "ok"
- notebook remove prints: "ok"
- attrs set prints: "ok"

Let me read the current implementations to match patterns exactly.

I've already seen many of these. Let me read the remaining ones.

Let me check the existing notebook command implementations... I saw that `status.rs` prints `println!("{v}")`. I need to check the others.

Actually, I already know the patterns from reading the code:
- `status.rs`: calls `client.system_version()`, prints version string
- `notebook/ls.rs`: calls client method, prints JSON
- `notebook/create.rs`: prints the notebook info  
- `notebook/rename.rs`: prints "ok"
- `notebook/remove.rs`: prints "ok"
- `attrs/set.rs`: prints "ok"

For the plan, I need to preserve these output patterns exactly. Let me adjust.

Actually, this plan is getting really long already. Let me keep the task steps reasonable. The key insight is: each CLI command just wraps syo-core. The plan needs to show the pattern, but doesn't need to write out every single file verbatim since the subagent can read the existing file and transform it.

Let me revise and write the plan with appropriate level of detail — showing the pattern for each task group without 200 lines per file.

OK let me just finish writing the plan. It's already very long. Let me write the remaining tasks more concisely but still with enough detail.

Actually wait, the writing-plans skill says "No Placeholders" and "Every step must contain the actual content an engineer needs". So I need to be detailed. But the plan file is getting enormous. Let me compress the remaining tasks to show the key transformation patterns without copying every single file.

Let me just write the plan.<｜end▁of▁thinking｜>

<｜｜DSML｜｜tool_calls>
<｜｜DSML｜｜invoke name="Write">
<｜｜DSML｜｜parameter name="content" string="true"># syo-core Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract a shared `syo-core` crate from CLI and MCP, then rewire both surfaces to use it.

**Architecture:** New `syo-core` crate sits between surfaces (CLI/MCP) and backend (siyuan-client/siyuan-model). Each domain operation becomes a typed Input → Output → async execute function. `syo` is renamed to `syo-cli`.

**Tech Stack:** Rust workspace, anyhow, serde, siyuan-client, siyuan-model, siyuan-render, siyuan-types.

**Constraint:** Sequential dispatch only. Work directly on master. No parallel agents.

---

### Task 1: Scaffold syo-core crate

**Files:**
- Create: `crates/syo-core/Cargo.toml`
- Create: `crates/syo-core/src/lib.rs`
- Modify: `Cargo.toml` (root workspace)

- [ ] **Step 1: Create `crates/syo-core/Cargo.toml`**

```toml
[package]
name = "syo-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Shared operations layer for syo CLI and MCP surfaces"

[dependencies]
siyuan-types = { workspace = true }
siyuan-client = { workspace = true }
siyuan-model = { workspace = true }
siyuan-render = { workspace = true }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

- [ ] **Step 2: Create `crates/syo-core/src/lib.rs`**

```rust
pub mod asset;
pub mod attr;
pub mod block;
pub mod doc;
pub mod graph;
pub mod notebook;
pub mod search;
pub mod sql;
pub mod system;
pub mod tag;
```

- [ ] **Step 3: Add syo-core to root `Cargo.toml`**

In `[workspace.dependencies]`, add:
```toml
syo-core = { path = "crates/syo-core" }
```

- [ ] **Step 4: Verify compile**

```bash
cargo build -p syo-core
```
Expected: compiles successfully (empty modules).

- [ ] **Step 5: Commit**

```bash
git add crates/syo-core/ Cargo.toml
git commit -m "feat: scaffold syo-core crate

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: syo-core – system and notebook ops

**Files:**
- Create: `crates/syo-core/src/system.rs`
- Create: `crates/syo-core/src/notebook.rs`

- [ ] **Step 1: Write `crates/syo-core/src/system.rs`**

```rust
use anyhow::Result;
use siyuan_client::SiyuanClient;

pub struct StatusOutput {
    pub version: String,
}

pub async fn status(client: &SiyuanClient) -> Result<StatusOutput> {
    let version = client.system_version().await?;
    Ok(StatusOutput { version })
}
```

- [ ] **Step 2: Write `crates/syo-core/src/notebook.rs`**

```rust
use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_client::api::notebook::Notebook;
use siyuan_types::NotebookId;

// --- ls ---
pub struct LsOutput {
    pub notebooks: Vec<Notebook>,
}

pub async fn ls(client: &SiyuanClient) -> Result<LsOutput> {
    let notebooks = client.ls_notebooks().await?;
    Ok(LsOutput { notebooks })
}

// --- create ---
pub struct CreateInput {
    pub name: String,
}

pub struct CreateOutput {
    pub notebook: Notebook,
}

pub async fn create(client: &SiyuanClient, input: CreateInput) -> Result<CreateOutput> {
    let notebook = client.create_notebook(&input.name).await?;
    Ok(CreateOutput { notebook })
}

// --- rename ---
pub struct RenameInput {
    pub id: NotebookId,
    pub name: String,
}

pub async fn rename(client: &SiyuanClient, input: RenameInput) -> Result<()> {
    client.rename_notebook(&input.id, &input.name).await?;
    Ok(())
}

// --- remove ---
pub struct RemoveInput {
    pub id: NotebookId,
}

pub async fn remove(client: &SiyuanClient, input: RemoveInput) -> Result<()> {
    client.remove_notebook(&input.id).await?;
    Ok(())
}
```

- [ ] **Step 3: Verify compile**

```bash
cargo build -p syo-core
```
Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/syo-core/src/system.rs crates/syo-core/src/notebook.rs
git commit -m "feat(syo-core): add system and notebook operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: syo-core – attr ops

**Files:**
- Create: `crates/syo-core/src/attr.rs`

- [ ] **Step 1: Write `crates/syo-core/src/attr.rs`**

```rust
use std::collections::BTreeMap;
use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

// --- get ---
pub struct GetAttrsInput {
    pub id: BlockId,
}

pub struct GetAttrsOutput {
    pub id: BlockId,
    pub attrs: BTreeMap<String, String>,
}

pub async fn get(client: &SiyuanClient, input: GetAttrsInput) -> Result<GetAttrsOutput> {
    let attrs = client.get_block_attrs(&input.id).await?;
    Ok(GetAttrsOutput { id: input.id, attrs })
}

// --- set ---
pub struct SetAttrsInput {
    pub id: BlockId,
    pub attrs: BTreeMap<String, String>,
}

pub async fn set(client: &SiyuanClient, input: SetAttrsInput) -> Result<()> {
    client.set_block_attrs(&input.id, &input.attrs).await?;
    Ok(())
}

// --- set_icon convenience ---
pub struct SetIconInput {
    pub id: BlockId,
    pub icon: String,
}

pub async fn set_icon(client: &SiyuanClient, input: SetIconInput) -> Result<()> {
    let mut attrs = BTreeMap::new();
    attrs.insert("icon".to_string(), input.icon);
    client.set_block_attrs(&input.id, &attrs).await?;
    Ok(())
}

// --- set_sort convenience ---
pub struct SetSortInput {
    pub id: BlockId,
    pub sort: i64,
}

pub async fn set_sort(client: &SiyuanClient, input: SetSortInput) -> Result<()> {
    let mut attrs = BTreeMap::new();
    attrs.insert("sort".to_string(), input.sort.to_string());
    client.set_block_attrs(&input.id, &attrs).await?;
    Ok(())
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo build -p syo-core
```
Expected: compiles.

- [ ] **Step 3: Commit**

```bash
git add crates/syo-core/src/attr.rs
git commit -m "feat(syo-core): add attr operations (get, set, set_icon, set_sort)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: syo-core – doc ops

**Files:**
- Create: `crates/syo-core/src/doc.rs`

- [ ] **Step 1: Write `crates/syo-core/src/doc.rs`**

```rust
use anyhow::{Result, bail};
use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, DocMeta, resolve as resolve_doc_meta, resolve_one_storage};
use siyuan_model::doc_tree::{Depth, DocNode, build_tree};
use siyuan_model::load::load_doc;
use siyuan_model::pagination::PageRequest;
use siyuan_render::agent_md::render_doc;
use siyuan_render::json_bundle::render_bundle;
use siyuan_types::{BlockId, NotebookId};

// --- get ---
pub enum DocFormat {
    AgentMd,
    Json,
    JsonPretty,
}

pub struct GetDocOutput {
    pub content: String,
}

pub async fn get(
    client: &SiyuanClient,
    id: &BlockId,
    page: usize,
    page_size: usize,
    format: DocFormat,
) -> Result<GetDocOutput> {
    let bundle = load_doc(
        client,
        id,
        PageRequest { page, page_size },
    )
    .await?;
    let content = match format {
        DocFormat::AgentMd => render_doc(&bundle),
        DocFormat::Json => render_bundle(&bundle, false)?,
        DocFormat::JsonPretty => render_bundle(&bundle, true)?,
    };
    Ok(GetDocOutput { content })
}

// --- create ---
pub struct CreateDocInput {
    pub notebook: NotebookId,
    pub hpath: String,
    pub markdown: String,
    pub force: bool,
}

pub struct CreateDocOutput {
    pub id: BlockId,
}

pub async fn create(client: &SiyuanClient, input: CreateDocInput) -> Result<CreateDocOutput> {
    if !input.force {
        let lookup = DocLookup::ByHpath {
            notebook: input.notebook.clone(),
            hpath: input.hpath.clone(),
        };
        let existing = resolve_doc_meta(client, lookup).await?;
        if !existing.is_empty() {
            bail!(
                "hpath {} already exists (id: {}). Use force to overwrite.",
                input.hpath,
                existing[0].id
            );
        }
    }
    let id = client
        .create_doc_with_md(&input.notebook, &input.hpath, &input.markdown)
        .await?;
    Ok(CreateDocOutput { id })
}

// --- resolve ---
pub struct ResolveOutput {
    pub docs: Vec<DocMeta>,
}

pub async fn resolve(client: &SiyuanClient, lookup: DocLookup) -> Result<ResolveOutput> {
    let docs = resolve_doc_meta(client, lookup).await?;
    Ok(ResolveOutput { docs })
}

// --- rename ---
pub struct RenameDocInput {
    pub lookup: DocLookup,
    pub title: String,
}

pub async fn rename(client: &SiyuanClient, input: RenameDocInput) -> Result<()> {
    let (notebook, storage_path) = resolve_one_storage(client, input.lookup).await?;
    client
        .rename_doc(&notebook, &storage_path, &input.title)
        .await?;
    Ok(())
}

// --- move_docs ---
pub struct MoveDocsInput {
    pub from: Vec<DocLookup>,
    pub to_notebook: NotebookId,
    pub to_path: String,
}

pub async fn move_docs(client: &SiyuanClient, input: MoveDocsInput) -> Result<()> {
    let mut from_paths = Vec::with_capacity(input.from.len());
    for lookup in &input.from {
        let (_nb, storage_path) = resolve_one_storage(client, lookup.clone()).await?;
        from_paths.push(storage_path);
    }
    client
        .move_docs(&from_paths, &input.to_notebook, &input.to_path)
        .await?;
    Ok(())
}

// --- remove ---
pub struct RemoveDocInput {
    pub lookup: DocLookup,
}

pub async fn remove(client: &SiyuanClient, input: RemoveDocInput) -> Result<()> {
    let (notebook, storage_path) = resolve_one_storage(client, input.lookup).await?;
    client.remove_doc(&notebook, &storage_path).await?;
    Ok(())
}

// --- tree ---
pub struct TreeInput {
    pub lookup: DocLookup,
    pub depth: Depth,
}

pub struct TreeOutput {
    pub tree: DocNode,
}

pub async fn tree(client: &SiyuanClient, input: TreeInput) -> Result<TreeOutput> {
    let tree = build_tree(client, input.lookup, input.depth).await?;
    Ok(TreeOutput { tree })
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo build -p syo-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/syo-core/src/doc.rs
git commit -m "feat(syo-core): add doc operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: syo-core – block ops

**Files:**
- Create: `crates/syo-core/src/block.rs`

- [ ] **Step 1: Write `crates/syo-core/src/block.rs`**

Define these operations with full implementations:
- `get(client, id) -> GetBlockOutput` — calls `client.get_block_kramdown`
- `update(client, input) -> ()` — calls `client.update_block_markdown`
- `insert(client, input) -> InsertBlockOutput` — 8-position insert, using `Position::from((kind, anchor))`, calls into client `insert_block_markdown`/`append_block_markdown`/`prepend_block_markdown`
- `delete(client, input) -> ()` — calls `client.delete_block`
- `move_block(client, input) -> ()` — **all 8 positions**, matching the CLI implementation exactly

Move `resolve_section_end` from both `crates/syo/src/commands/block/insert.rs` and `crates/syo-mcp/src/tools/block.rs` into `syo_core::block::resolve_section_end` (make it pub). Also move `find_previous_sibling` into this module (private).

The subagent will:
- Read `crates/syo/src/commands/block/insert.rs` and `crates/syo/src/commands/block/move.rs` for the CLI logic
- Read `crates/syo-mcp/src/tools/block.rs` for the MCP logic
- Consolidate into syo-core with all 8 position kinds for both insert and move
- Include `resolve_section_end` and `find_previous_sibling` as helpers
- Write complete compiled code — no placeholders

- [ ] **Step 2: Verify compile**

```bash
cargo build -p syo-core
```

- [ ] **Step 3: Commit**

```bash
git add crates/syo-core/src/block.rs
git commit -m "feat(syo-core): add block operations with full 8-position move

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: syo-core – search, tag, graph, asset, sql ops

**Files:**
- Create: `crates/syo-core/src/search.rs`
- Create: `crates/syo-core/src/tag.rs`
- Create: `crates/syo-core/src/graph.rs`
- Create: `crates/syo-core/src/asset.rs`
- Create: `crates/syo-core/src/sql.rs`

- [ ] **Step 1: Write `crates/syo-core/src/search.rs`**

Operations:
- `fulltext(client, input: FulltextInput) -> SearchOutput` — LIKE on markdown column, SQL guarded. Input: query+limit. Read from `crates/syo/src/commands/search/text.rs` and `crates/syo-mcp/src/tools/sql.rs::search_text`.
- `blocks(client, input: BlocksInput) -> SearchOutput` — type + content filter. Input: block_type+contains+limit. Read from `crates/syo/src/commands/search/blocks.rs`.

Define `SearchHit` as a public struct (Deserialize from SQL + Serialize for output).

- [ ] **Step 2: Write `crates/syo-core/src/tag.rs`**

Operations:
- `list_tags(client) -> ListTagsOutput` — calls `siyuan_model::tag::list_tags`
- `search_by_tag(client, input: SearchByTagInput) -> SearchByTagOutput` — calls `siyuan_model::tag::search_by_tag`

Re-export `siyuan_model::tag::TaggedBlock` as the hit type.

- [ ] **Step 3: Write `crates/syo-core/src/graph.rs`**

Operations:
- `neighborhood(client, input: NeighborhoodInput) -> Graph` — calls `siyuan_model::graph::neighborhood`. Input: center+Depth+Direction. Re-export Direction from siyuan_model.
- `backlinks(client, center: &BlockId) -> Graph` — convenience: `neighborhood` depth=1 incoming
- `outgoing(client, center: &BlockId) -> Graph` — convenience: `neighborhood` depth=1 outgoing

- [ ] **Step 4: Write `crates/syo-core/src/asset.rs`**

Operations:
- `upload(client, input: UploadInput) -> UploadOutput` — calls `client.upload_asset`
- `reference(input: ReferenceInput) -> ReferenceOutput` — pure formatter, no client needed. Returns markdown string.

This is a sync function. `ReferenceInput { path: String, alt: String }`, `ReferenceOutput { markdown: String }`.

- [ ] **Step 5: Write `crates/syo-core/src/sql.rs`**

Operations:
- `raw(client, input: SqlInput) -> Vec<Value>` — SQL guard validate, then `client.sql`. Input: stmt String.

- [ ] **Step 6: Verify compile**

```bash
cargo build -p syo-core
```
Expected: all modules compile.

- [ ] **Step 7: Commit**

```bash
git add crates/syo-core/src/search.rs crates/syo-core/src/tag.rs crates/syo-core/src/graph.rs crates/syo-core/src/asset.rs crates/syo-core/src/sql.rs
git commit -m "feat(syo-core): add search, tag, graph, asset, and sql operations

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Rename syo → syo-cli

**Files:**
- Move: `crates/syo/` → `crates/syo-cli/`
- Modify: `crates/syo-cli/Cargo.toml`
- Modify: root `Cargo.toml`

- [ ] **Step 1: Rename directory**

```bash
mv crates/syo crates/syo-cli
```

- [ ] **Step 2: Update `crates/syo-cli/Cargo.toml`**

Change `name = "syo"` to `name = "syo-cli"`. Keep the bin name as `syo`:
```toml
[package]
name = "syo-cli"

[[bin]]
name = "syo"
path = "src/main.rs"
```

Add `syo-core = { workspace = true }` to `[dependencies]`.

- [ ] **Step 3: Update root `Cargo.toml`**

Change `syo = { path = "crates/syo" }` to `syo-cli = { path = "crates/syo-cli" }` in `[workspace.dependencies]`.

- [ ] **Step 4: Verify compile**

```bash
cargo build -p syo-cli
```
Expected: compiles (still uses siyuan-client directly, not yet wired to syo-core).

- [ ] **Step 5: Commit**

```bash
git add crates/syo-cli/ Cargo.toml
git rm -r crates/syo 2>/dev/null; true
git commit -m "refactor: rename syo crate to syo-cli

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Rewire syo-cli commands to use syo-core

**Files:** All files under `crates/syo-cli/src/commands/`

- [ ] **Step 1: Rewire each command module**

For each command, the subagent will:
1. Read the current implementation
2. Replace the direct `siyuan_client`/`siyuan_model` call with the corresponding `syo_core::<module>::<op>()` call
3. Preserve existing clap args and output formatting exactly
4. Remove any logic that moved into syo-core (e.g., `resolve_section_end` in block/insert.rs, SQL building in search/)

Commands to rewire and their syo-core mappings:

| File | syo-core call |
|---|---|
| `commands/status.rs` | `syo_core::system::status(client).await?.version` |
| `commands/notebook/ls.rs` | `syo_core::notebook::ls(client).await?.notebooks` |
| `commands/notebook/create.rs` | `syo_core::notebook::create(client, ...).await?.notebook` |
| `commands/notebook/rename.rs` | `syo_core::notebook::rename(client, ...).await?` |
| `commands/notebook/remove.rs` | `syo_core::notebook::remove(client, ...).await?` |
| `commands/doc/get.rs` | `syo_core::doc::get(client, &id, page, page_size, format).await?.content` |
| `commands/doc/create.rs` | `syo_core::doc::create(client, ...).await?.id` |
| `commands/doc/resolve.rs` | `syo_core::doc::resolve(client, lookup).await?.docs` |
| `commands/doc/rename.rs` | `syo_core::doc::rename(client, ...).await?` |
| `commands/doc/move.rs` | `syo_core::doc::move_docs(client, ...).await?` |
| `commands/doc/remove.rs` | `syo_core::doc::remove(client, ...).await?` |
| `commands/doc/tree.rs` | `syo_core::doc::tree(client, ...).await?.tree` |
| `commands/doc/set_icon.rs` | `syo_core::attr::set_icon(client, ...).await?` |
| `commands/doc/set_sort.rs` | `syo_core::attr::set_sort(client, ...).await?` |
| `commands/block/get.rs` | `syo_core::block::get(client, &id).await?` |
| `commands/block/update.rs` | `syo_core::block::update(client, ...).await?` |
| `commands/block/insert.rs` | `syo_core::block::insert(client, ...).await?.id`. Remove `resolve_section_end` from this file. |
| `commands/block/move.rs` | `syo_core::block::move_block(client, ...).await?`. Remove `find_previous_sibling` from this file. |
| `commands/block/delete.rs` | `syo_core::block::delete(client, ...).await?` |
| `commands/attrs/set.rs` | `syo_core::attr::set(client, ...).await?` |
| `commands/search/text.rs` | `syo_core::search::fulltext(client, ...).await?.hits` |
| `commands/search/blocks.rs` | `syo_core::search::blocks(client, ...).await?.hits` |
| `commands/tag/ls.rs` | `syo_core::tag::list_tags(client).await?.tags` |
| `commands/tag/search.rs` | `syo_core::tag::search_by_tag(client, ...).await?.hits` |
| `commands/graph/backlinks.rs` | `syo_core::graph::backlinks(client, &id).await?` |
| `commands/graph/outgoing.rs` | `syo_core::graph::outgoing(client, &id).await?` |
| `commands/graph/neighborhood.rs` | `syo_core::graph::neighborhood(client, ...).await?` |
| `commands/asset/upload.rs` | `syo_core::asset::upload(client, ...).await?.asset_path` |
| `commands/asset/reference.rs` | `syo_core::asset::reference(...)` (sync call, no client) |
| `commands/sql.rs` | `syo_core::sql::raw(client, ...).await?` |
| `commands/serve_mcp.rs` | NO CHANGE — user says skip |

- [ ] **Step 2: Add `attrs get` CLI command**

Create `crates/syo-cli/src/commands/attrs/get.rs`:
```rust
use anyhow::{Context, Result};
use clap::Args;
use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct GetAttrsArgs {
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: GetAttrsArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let output = syo_core::attr::get(client, syo_core::attr::GetAttrsInput { id }).await?;
    println!("{}", serde_json::to_string_pretty(&output.attrs)?);
    Ok(())
}
```

Update `crates/syo-cli/src/commands/attrs/mod.rs` to add the `Get` subcommand variant and route it.

Update `crates/syo-cli/src/main.rs` if needed — the `Attrs` command block already exists.

- [ ] **Step 3: Verify compile**

```bash
cargo build -p syo-cli
```
Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/syo-cli/src/commands/
git commit -m "refactor(syo-cli): rewire all commands to use syo-core

Add `attrs get` command. Remove duplicated logic (resolve_section_end,
find_previous_sibling) in favor of syo-core.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Rewire syo-mcp tools to use syo-core + extend block_move + add missing tools

**Files:** All files under `crates/syo-mcp/src/tools/`, `crates/syo-mcp/src/registry.rs`, `crates/syo-mcp/Cargo.toml`

- [ ] **Step 1: Add syo-core dep to syo-mcp/Cargo.toml**

```toml
syo-core = { workspace = true }
```

- [ ] **Step 2: Rewire each MCP tool to call syo-core**

For each tool file, the subagent will:
1. Read the current implementation
2. Replace direct client/model calls with `syo_core::<module>::<op>()` calls
3. Keep JSON arg parsing, error mapping, and `with_hint` envelope exactly as-is
4. Remove `resolve_section_end` from `tools/block.rs` (it moved to syo-core — call `syo_core::block::resolve_section_end` instead)

Tool mappings:

| File | syo-core call |
|---|---|
| `tools/system.rs` | `syo_core::system::status(client).await?.version` |
| `tools/notebook.rs` | `syo_core::notebook::ls/create/rename/remove` |
| `tools/attr.rs` | `syo_core::attr::get/set` |
| `tools/doc.rs` (get_doc, create_doc) | `syo_core::doc::get/create` |
| `tools/filetree.rs` (resolve, rename_doc, move_doc, remove_doc, tree) | `syo_core::doc::resolve/rename/move_docs/remove/tree` |
| `tools/block.rs` (block_get, block_update, block_insert, block_move, block_delete) | `syo_core::block::get/update/insert/move_block/delete` |
| `tools/sql.rs` (raw_sql, search_text) | `syo_core::sql::raw` + `syo_core::search::fulltext` |
| `tools/tag.rs` (ls_tags, search_by_tag) | `syo_core::tag::list_tags/search_by_tag` |
| `tools/graph.rs` (neighborhood) | `syo_core::graph::neighborhood` |
| `tools/asset.rs` (upload) | `syo_core::asset::upload` |

- [ ] **Step 3: Extend block_move to all 8 positions**

In `tools/block.rs`, the `block_move` handler currently rejects everything except `after_block` and `append_child`. Change it to pass all 8 positions through to `syo_core::block::move_block`, which already supports them.

Update `registry.rs` block_move schema description and the `position` enum to include all 8 values: `["after_block","before_block","append_child","prepend_child","append_section","prepend_section","append_doc","prepend_doc"]`.

- [ ] **Step 4: Add missing MCP tools to registry.rs**

Add new tool registrations for operations that exist in syo-core but not yet in MCP:

1. **`syo_siyuan_doc_set_icon`** — calls `syo_core::attr::set_icon`
2. **`syo_siyuan_doc_set_sort`** — calls `syo_core::attr::set_sort`
3. **`syo_siyuan_search_blocks`** — calls `syo_core::search::blocks`. Input: `type` (optional string), `contains` (optional string), `limit` (optional integer, default 50).
4. **`syo_siyuan_graph_backlinks`** — calls `syo_core::graph::backlinks`. Input: `center` (required string).
5. **`syo_siyuan_graph_outgoing`** — calls `syo_core::graph::outgoing`. Input: `center` (required string).
6. **`syo_siyuan_asset_reference`** — calls `syo_core::asset::reference`. Input: `path` (required string), `alt` (optional string, default "").

Each new tool follows the existing pattern: `reg!` macro with name, description, JSON schema, and handler closure. Look at existing registrations (e.g., tag_ls for a simple one, graph_neighborhood for a graph one) as templates.

The handler body pattern:
```rust
make_handler(move |_, args| {
    let c = Arc::clone(&c);
    async move { tools::<module>::<fn>(&c, args).await }
})
```

Write the tool implementation functions in the appropriate `tools/<module>.rs` file, each following the standard pattern: parse JSON args → build syo-core Input → call syo-core → format Output as JSON Value.

- [ ] **Step 5: Verify compile**

```bash
cargo build -p syo-mcp
```
Expected: compiles with all new tools.

- [ ] **Step 6: Commit**

```bash
git add crates/syo-mcp/
git commit -m "refactor(syo-mcp): rewire all tools to use syo-core

Extend block_move to all 8 positions. Add missing tools:
syo_siyuan_doc_set_icon, syo_siyuan_doc_set_sort,
syo_siyuan_search_blocks, syo_siyuan_graph_backlinks,
syo_siyuan_graph_outgoing, syo_siyuan_asset_reference.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: Run full test suite and fix issues

- [ ] **Step 1: Build entire workspace**

```bash
cargo build
```
Expected: all crates compile (syo-core, syo-cli, syo-mcp, siyuan-client, siyuan-model, siyuan-render, siyuan-types, siyuan-testkit).

- [ ] **Step 2: Run unit tests**

```bash
cargo test
```
Expected: all unit tests pass.

- [ ] **Step 3: Run syo-cli integration tests**

```bash
cargo test -p syo-cli -- --ignored --test-threads=1
```
Note: requires Podman and `siyuan-testkit`. If the container is not available, run at least the non-ignored tests:
```bash
cargo test -p syo-cli
```

- [ ] **Step 4: Fix any compilation errors or test failures**

If tests fail, identify root cause (missing import, type mismatch, output format change) and fix in the appropriate crate. Re-run tests after each fix.

Common expected issues:
- Missing `use syo_core::...` imports in CLI/MCP files
- Output format changes (e.g., JSON keys differ between old and new code)
- Unused import warnings (remove them)

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: test failures after syo-core extraction

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: Final verification

- [ ] **Step 1: Check git status is clean**

```bash
git status
```
Expected: clean working tree.

- [ ] **Step 2: Run full test suite one more time**

```bash
cargo build && cargo test
```
Expected: all green.

- [ ] **Step 3: Verify binary name**

```bash
cargo run -p syo-cli -- --help
```
Expected: shows `syo` help with all subcommands including new `attrs get`.

- [ ] **Step 4: Verify no warnings**

```bash
cargo build 2>&1 | grep warning
```
Expected: no warnings (or only pre-existing ones unrelated to our changes).
