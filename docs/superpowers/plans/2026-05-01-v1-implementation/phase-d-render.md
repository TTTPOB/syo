# Phase D: Render

> **Part of:** [v1 Implementation Plan](../2026-05-01-v1-implementation.md) · **Prev:** [Phase C: Model layer](phase-c-model.md) · **Next:** [Phase E: CLI](phase-e-cli.md)
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this phase task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Render `DocBundle` (and other model values) into agent-friendly Markdown with `<!-- sy:* -->` annotations or canonical JSON.

---

## Task D1: agent-md renderer

**Files:**
- Modify: `crates/siyuan-render/src/agent_md.rs`

**Background:** 把 `DocBundle` 渲染成带 `<!-- sy:block ... -->` 注释的 markdown。约定：每个 block 前面一行注释，块内容紧跟其后。文档级元数据放在最顶。

- [ ] **Step 1: 写实现 + 测试**

Replace `crates/siyuan-render/src/agent_md.rs`:

```rust
use std::fmt::Write;

use siyuan_model::bundle::DocBundle;
use siyuan_types::{BlockNode, BlockType};

pub fn render_doc(bundle: &DocBundle) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "<!-- sy:doc id={} hpath={:?} page={} of {} -->",
        bundle.doc.id, bundle.doc.hpath, bundle.page.page, bundle.page.total_pages,
    );
    let _ = writeln!(out);

    for b in &bundle.blocks {
        render_block(&mut out, b);
        let _ = writeln!(out);
    }

    out
}

pub fn render_block(out: &mut String, b: &BlockNode) {
    let _ = writeln!(
        out,
        "<!-- sy:block id={} type={} subtype={} -->",
        b.id,
        b.block_type.as_kernel(),
        b.subtype.as_deref().unwrap_or(""),
    );
    if b.block_type == BlockType::SuperBlock {
        // Read-only superblock: wrap in a fence so the agent can see boundaries.
        let _ = writeln!(out, ":::sy-superblock id={}", b.id);
        let _ = writeln!(out, "{}", b.markdown);
        let _ = writeln!(out, ":::");
    } else {
        let _ = writeln!(out, "{}", b.markdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use siyuan_model::bundle::{DocBundle, DocMeta, PageInfo};
    use siyuan_types::{BlockId, BlockRole, BlockType, NotebookId};
    use std::collections::BTreeMap;

    fn mk_block(id: &str, ty: BlockType, md: &str) -> BlockNode {
        BlockNode {
            id: BlockId::parse(id).unwrap(),
            root_id: BlockId::parse("20260501000001-doc0001").unwrap(),
            parent_id: Some(BlockId::parse("20260501000001-doc0001").unwrap()),
            notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
            block_type: ty,
            subtype: None,
            role: BlockRole::for_block_type(ty),
            markdown: md.into(),
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
    fn renders_doc_header_and_blocks() {
        let bundle = DocBundle {
            schema: DocBundle::SCHEMA.into(),
            doc: DocMeta {
                id: BlockId::parse("20260501000001-doc0001").unwrap(),
                notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
                hpath: "/Demo".into(),
                title: "Demo".into(),
            },
            page: PageInfo { page: 1, page_size: 50, total_blocks: 2, total_pages: 1 },
            blocks: vec![
                mk_block("20260501000010-h2aaaaa", BlockType::Heading, "## Hello"),
                mk_block("20260501000020-paaaaaa", BlockType::Paragraph, "World."),
            ],
        };
        let md = render_doc(&bundle);
        insta::assert_snapshot!(md, @r###"
        <!-- sy:doc id=20260501000001-doc0001 hpath="/Demo" page=1 of 1 -->

        <!-- sy:block id=20260501000010-h2aaaaa type=h subtype= -->
        ## Hello

        <!-- sy:block id=20260501000020-paaaaaa type=p subtype= -->
        World.
        "###);
    }
}
```

- [ ] **Step 2: 跑测试 + 接受 snapshot**

Run: `INSTA_FORCE_PASS=1 INSTA_UPDATE=auto cargo test -p siyuan-render render_doc`

Expected: 1 passed (inline snapshot 自动写入)。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-render/src/agent_md.rs
git commit -m "feat(render): agent-md renderer with sy:* annotations"
```

---

## Task D2: JSON bundle renderer (passthrough)

**Files:**
- Modify: `crates/siyuan-render/src/json_bundle.rs`

- [ ] **Step 1: 写实现**

Replace:

```rust
use serde::Serialize;

use siyuan_model::bundle::DocBundle;

pub fn render<T: Serialize>(value: &T, pretty: bool) -> serde_json::Result<String> {
    if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
}

pub fn render_bundle(bundle: &DocBundle, pretty: bool) -> serde_json::Result<String> {
    render(bundle, pretty)
}
```

- [ ] **Step 2: cargo check**

Run: `cargo check -p siyuan-render`

Expected: 通过。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-render/src/json_bundle.rs
git commit -m "feat(render): JSON bundle passthrough"
```

