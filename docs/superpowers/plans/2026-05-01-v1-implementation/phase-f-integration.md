# Phase F: Integration tests

> **Part of:** [v1 Implementation Plan](../2026-05-01-v1-implementation.md) · **Prev:** [Phase E: CLI](phase-e-cli.md) · **Next:** —
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this phase task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Add end-to-end CLI integration tests on top of `siyuan-testkit`: a shared seed fixture and `#[ignore]`-gated read/write happy-path tests run via `cargo test --test cli_integration -- --ignored`.

---

## Task F1: integration test harness — seed helper

**Files:**
- Create: `crates/siyuan-cli/tests/common/mod.rs`

**Background:** 给端到端测试一个固定的 fixture：用 testkit 起容器、创建一个 notebook、创建一个含多种块的复杂文档。所有 cli 测试共享。

- [ ] **Step 1: 写 fixture helper**

Create `crates/siyuan-cli/tests/common/mod.rs`:

```rust
//! Shared scaffolding for cli integration tests.

use anyhow::Result;

use siyuan_client::SiyuanClient;
use siyuan_testkit::SiyuanContainer;
use siyuan_types::{BlockId, NotebookId};

pub struct Fixture {
    pub container: SiyuanContainer,
    pub client: SiyuanClient,
    pub notebook_id: NotebookId,
    pub doc_id: BlockId,
}

pub async fn boot_with_seed() -> Result<Fixture> {
    siyuan_testkit::init_tracing();
    let container = SiyuanContainer::start().await?;
    let client = SiyuanClient::new(container.base_url(), container.token())?;

    let nb = client.create_notebook("integration-test").await?;
    // Newly created notebook is closed by default in some versions; open it.
    let _ = client.open_notebook(&nb.id).await;

    let markdown = "\
# Integration Test Doc

## Goals

This is the first paragraph.

This paragraph references later content.

## Targets

A target paragraph.

- bullet one
- bullet two

## Empty Section
";
    let doc_id = client
        .create_doc_with_md(&nb.id, "/IntegrationTestDoc", markdown)
        .await?;

    Ok(Fixture { container, client, notebook_id: nb.id, doc_id })
}
```

- [ ] **Step 2: cargo check**

Run: `cargo check -p siyuan-cli --tests`

Expected: 通过。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-cli/tests
git commit -m "test(cli): shared fixture helper"
```

---

## Task F2: integration tests — read + write happy paths

**Files:**
- Create: `crates/siyuan-cli/tests/cli_integration.rs`

**Background:** 一个 `#[tokio::test]` 跑一类操作。所有测试共用一个 fixture（per-test 起新容器，确保隔离），打 `--ignored`。

- [ ] **Step 1: 写测试文件**

Create `crates/siyuan-cli/tests/cli_integration.rs`:

```rust
//! End-to-end CLI integration tests.
//!
//! Run with: `cargo test -p siyuan-cli --test cli_integration -- --ignored --nocapture`

mod common;

use common::boot_with_seed;
use siyuan_model::{load::load_doc, pagination::PageRequest};
use siyuan_render::agent_md::render_doc;
use siyuan_types::BlockId;

#[tokio::test]
#[ignore]
async fn get_doc_returns_seeded_content() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(&f.client, &f.doc_id, PageRequest::default())
        .await
        .expect("load_doc");
    assert!(bundle.blocks.iter().any(|b| b.markdown.contains("Goals")), "should contain heading 'Goals'");
    let md = render_doc(&bundle);
    assert!(md.contains("<!-- sy:doc"));
    assert!(md.contains("Goals"));
}

#[tokio::test]
#[ignore]
async fn update_block_then_reload_reflects_change() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .expect("load_doc");

    let target = bundle
        .blocks
        .iter()
        .find(|b| b.markdown == "This is the first paragraph.")
        .expect("seed contains the first paragraph");

    f.client
        .update_block_markdown(&target.id, "Replaced text.")
        .await
        .expect("update");

    let reloaded = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .unwrap();
    let updated = reloaded.blocks.iter().find(|b| b.id == target.id).unwrap();
    assert_eq!(updated.markdown, "Replaced text.");
}

#[tokio::test]
#[ignore]
async fn insert_blocks_after_anchor_preserves_order() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .expect("load_doc");

    let anchor = bundle
        .blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .expect("seed contains target paragraph");

    let new_md = "Inserted A.\n\nInserted B.\n\nInserted C.";
    f.client
        .insert_block_markdown(new_md, Some(&anchor.id), None, None)
        .await
        .expect("insert");

    let reloaded = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .unwrap();
    let positions: Vec<_> = reloaded
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.markdown.starts_with("Inserted "))
        .map(|(i, b)| (i, b.markdown.clone()))
        .collect();
    assert_eq!(positions.len(), 3, "all three inserted blocks should be present");
    let texts: Vec<_> = positions.iter().map(|(_, m)| m.clone()).collect();
    assert_eq!(texts, vec!["Inserted A.", "Inserted B.", "Inserted C."]);
}

#[tokio::test]
#[ignore]
async fn create_doc_returns_resolvable_id() {
    let f = boot_with_seed().await.expect("boot");
    let id = f
        .client
        .create_doc_with_md(&f.notebook_id, "/AnotherDoc", "# Another\n\nHello.")
        .await
        .expect("create");
    assert!(BlockId::parse(id.as_str()).is_ok());
    let bundle = load_doc(&f.client, &id, PageRequest::default()).await.unwrap();
    assert!(bundle.blocks.iter().any(|b| b.markdown == "Hello."));
}

#[tokio::test]
#[ignore]
async fn delete_block_removes_it() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .unwrap();
    let target = bundle
        .blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .unwrap();
    let target_id = target.id.clone();

    f.client.delete_block(&target_id).await.expect("delete");

    let reloaded = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .unwrap();
    assert!(
        !reloaded.blocks.iter().any(|b| b.id == target_id),
        "deleted block should not appear in reload"
    );
}

#[tokio::test]
#[ignore]
async fn append_section_inserts_at_section_end() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .unwrap();

    let goals_heading = bundle
        .blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("seed contains Goals heading");

    // Resolve section end via the same helper the cli uses.
    use siyuan_model::section::populate_section_children;
    let mut blocks = bundle.blocks.clone();
    populate_section_children(&mut blocks);
    let h = blocks.iter().find(|b| b.id == goals_heading.id).unwrap();
    let section_end = h.section_children.last().expect("Goals section has content").clone();

    let new = "End-of-section content.";
    f.client
        .insert_block_markdown(new, Some(&section_end), None, None)
        .await
        .expect("insert");

    let reloaded = load_doc(&f.client, &f.doc_id, PageRequest { page: 1, page_size: 100 })
        .await
        .unwrap();

    // Find the new block and the next heading; new must precede next heading.
    let new_idx = reloaded
        .blocks
        .iter()
        .position(|b| b.markdown == "End-of-section content.")
        .expect("new block present");
    let next_heading_idx = reloaded
        .blocks
        .iter()
        .position(|b| b.markdown.starts_with("## Targets"))
        .expect("next heading present");
    assert!(
        new_idx < next_heading_idx,
        "inserted block ({new_idx}) must come before next h2 ({next_heading_idx})"
    );
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p siyuan-cli --test cli_integration -- --ignored --nocapture`

Expected: 6 passed. 单测耗时较长（每个测试启一个容器约 30–60s）。

如果某些测试失败：
- `get_doc_returns_seeded_content` 失败：先验证 `cargo run --bin siyuan -- status` 在容器里能跑通；如果不能，回到 testkit smoke。
- `insert_blocks_after_anchor_preserves_order` 失败：说明 SiYuan kernel 解析多段 markdown 一次插入时顺序不保证；回到 plan 改 `insert_block_markdown` 为按段循环 + cursor 维持。
- `append_section_inserts_at_section_end` 失败：说明 `getChildBlocks` / SQL `parent_id` 与 DFS 推导有偏差，需要在 `section.rs` 里调整。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-cli/tests/cli_integration.rs
git commit -m "test(cli): integration tests for read/write happy paths"
```

