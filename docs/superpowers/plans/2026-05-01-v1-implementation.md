# SiYuan CLI v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 SiYuan Block Graph Harness 第一版：一个 Rust CLI（`siyuan`），围绕 SiYuan HTTP API 提供面向 Agent 的稳定块级操作；同时把核心模型抽成可复用的 library crates。

**Architecture:** Cargo workspace 多 crate 拆分：`siyuan-types`（无依赖的数据模型）、`siyuan-client`（typed HTTP client）、`siyuan-model`（语义层：bundle / section / container / pagination / graph）、`siyuan-render`（agent-md + JSON 渲染）、`siyuan-cli`（clap 命令层）。CLI 是唯一前端，MCP 留给后续。

**Tech Stack:** rust 2024、tokio、reqwest、serde、clap (derive)、anyhow / thiserror、tracing、insta（端到端 snapshot）、`siyuan-testkit`（来自 test-framework plan）。

**Prerequisites:** test-framework plan 已经执行完毕（`siyuan-testkit` 可用）。

**Out of scope（来自设计讨论的明确否决）:** plan/apply 两阶段、`dry_run` mode、snapshot token、并发保护（`expected_hash`）、超级块创建/layout 修改、AV 编辑、WebSocket、multi-workspace、本地 backup、MCP transport、history/trash（思源原生有）、daily note、template、打包发布。

---

## File Structure

新增/修改的所有 crate 与文件，按 phase 分组：

```
crates/
  siyuan-types/                       # Phase A
    Cargo.toml
    src/
      lib.rs                          # re-exports
      id.rs                           # BlockId, NotebookId, hpath types
      block.rs                        # BlockType, BlockSubtype, BlockRole, BlockNode
      position.rs                     # Position enum (insert/move targets)
      error.rs                        # SiyuanError, ErrorKind
  siyuan-client/                      # Phase B
    Cargo.toml
    src/
      lib.rs
      client.rs                       # SiyuanClient + post helper
      response.rs                     # SiyuanResponse<T> + code → ErrorKind mapping
      api/
        mod.rs
        system.rs                     # version, currentTime
        notebook.rs                   # ls/open/close/create/rename/remove notebook
        filetree.rs                   # createDocWithMd / rename / move / remove / list / getHPathByID / getIDsByHPath
        block.rs                      # get/insert/append/prepend/update/delete/move + getChildBlocks + getBlockKramdown
        attr.rs                       # get/setBlockAttrs
        query.rs                      # sql
        asset.rs                      # upload
        export.rs                     # exportMdContent
  siyuan-model/                       # Phase C
    Cargo.toml
    src/
      lib.rs
      bundle.rs                       # DocBundle, BlockBundle structs
      load.rs                         # load_doc pipeline
      section.rs                      # heading section boundary
      container.rs                    # container vs leaf classification
      pagination.rs                   # 50 blocks/page slicing
      relations.rs                    # relation hints from refs/spans
      graph.rs                        # neighborhood BFS
      tag.rs                          # tag listing/search
  siyuan-render/                      # Phase D
    Cargo.toml
    src/
      lib.rs
      agent_md.rs                     # annotated markdown
      json_bundle.rs                  # canonical JSON
  siyuan-cli/                         # Phase E
    Cargo.toml
    src/
      main.rs
      config.rs                       # env / flag resolution
      output.rs                       # OutputFormat enum (json / agent-md)
      commands/
        mod.rs
        get_doc.rs
        get_block.rs
        create_doc.rs
        update_block.rs
        insert_blocks.rs
        move_block.rs
        delete_block.rs
        set_attrs.rs
        notebook.rs
        doc.rs                        # rename / move / set-icon / set-sort
        tag.rs
        asset.rs
        graph.rs
        search.rs
    tests/
      cli_integration.rs              # uses siyuan-testkit
```

Repo 根 `Cargo.toml`（workspace 已在 test-framework plan 建立）会扩 `members` 列表，并把原本的根 `[package]` / `src/main.rs` 删除（迁移到 `crates/siyuan-cli/`）。

---

# Phase A: Foundation

## Task A1: 扩 workspace、迁移旧 main.rs、占位所有新 crate

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/siyuan-types/{Cargo.toml,src/lib.rs}`
- Create: `crates/siyuan-client/{Cargo.toml,src/lib.rs}`
- Create: `crates/siyuan-model/{Cargo.toml,src/lib.rs}`
- Create: `crates/siyuan-render/{Cargo.toml,src/lib.rs}`
- Create: `crates/siyuan-cli/{Cargo.toml,src/main.rs}`
- Delete: `src/main.rs`（移到 `crates/siyuan-cli/src/main.rs`）

- [ ] **Step 1: 创建 5 个新 crate 骨架**

```bash
cargo new --lib crates/siyuan-types --name siyuan-types --vcs none
cargo new --lib crates/siyuan-client --name siyuan-client --vcs none
cargo new --lib crates/siyuan-model --name siyuan-model --vcs none
cargo new --lib crates/siyuan-render --name siyuan-render --vcs none
cargo new --bin crates/siyuan-cli --name siyuan-cli --vcs none
```

- [ ] **Step 2: 把根 `src/main.rs` 内容覆盖到 `crates/siyuan-cli/src/main.rs`**

写入 `crates/siyuan-cli/src/main.rs`：

```rust
fn main() {
    println!("siyuan-cli (skeleton)");
}
```

- [ ] **Step 3: 改写根 `Cargo.toml`，去掉 `[package]`，扩 workspace dependencies**

写入 `Cargo.toml`：

```toml
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/tpob/siyuan-cli"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "multipart"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_with = "3"
clap = { version = "4", features = ["derive", "env"] }
tempfile = "3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
thiserror = "2"
insta = { version = "1", features = ["yaml", "json"] }
regex = "1"
once_cell = "1"
camino = "1"

# Path deps for in-workspace crates
siyuan-types = { path = "crates/siyuan-types" }
siyuan-client = { path = "crates/siyuan-client" }
siyuan-model = { path = "crates/siyuan-model" }
siyuan-render = { path = "crates/siyuan-render" }
siyuan-testkit = { path = "crates/siyuan-testkit" }
```

- [ ] **Step 4: 删掉旧的根级 `src/`**

```bash
rm -rf src/
```

- [ ] **Step 5: 给每个新 crate 写最小 Cargo.toml + lib.rs**

`crates/siyuan-types/Cargo.toml`:
```toml
[package]
name = "siyuan-types"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Core data types for the SiYuan harness."

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
regex = { workspace = true }
once_cell = { workspace = true }
```

`crates/siyuan-types/src/lib.rs`:
```rust
//! Core data types for the SiYuan harness.

pub mod block;
pub mod error;
pub mod id;
pub mod position;

pub use block::{BlockNode, BlockRole, BlockSubtype, BlockType};
pub use error::{ErrorKind, SiyuanError};
pub use id::{BlockId, NotebookId};
pub use position::Position;
```

`crates/siyuan-client/Cargo.toml`:
```toml
[package]
name = "siyuan-client"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
siyuan-types = { workspace = true }
tokio = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
serde_with = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
```

`crates/siyuan-client/src/lib.rs`:
```rust
//! Typed HTTP client for the SiYuan kernel.
pub mod api;
pub mod client;
pub mod response;

pub use client::SiyuanClient;
```

Create `crates/siyuan-client/src/api/mod.rs`:
```rust
pub mod attr;
pub mod asset;
pub mod block;
pub mod export;
pub mod filetree;
pub mod notebook;
pub mod query;
pub mod system;
```

For each of the 8 submodules listed above, create the file under `crates/siyuan-client/src/api/` with the single line:
```rust
// stub, populated in Phase B
```

So you end up with 8 one-line files: `attr.rs`, `asset.rs`, `block.rs`, `export.rs`, `filetree.rs`, `notebook.rs`, `query.rs`, `system.rs`.

`crates/siyuan-model/Cargo.toml`:
```toml
[package]
name = "siyuan-model"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
siyuan-types = { workspace = true }
siyuan-client = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
```

`crates/siyuan-model/src/lib.rs`:
```rust
//! Semantic layer over the SiYuan client: bundles, sections, pagination, graph.
pub mod bundle;
pub mod container;
pub mod graph;
pub mod load;
pub mod pagination;
pub mod relations;
pub mod section;
pub mod tag;
```

Create 8 stub files in `crates/siyuan-model/src/`, each containing only:
```rust
// stub, populated in Phase C
```
The files are: `bundle.rs`, `container.rs`, `graph.rs`, `load.rs`, `pagination.rs`, `relations.rs`, `section.rs`, `tag.rs`.

`crates/siyuan-render/Cargo.toml`:
```toml
[package]
name = "siyuan-render"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
siyuan-types = { workspace = true }
siyuan-model = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
```

`crates/siyuan-render/src/lib.rs`:
```rust
//! Render bundles into agent-friendly Markdown or canonical JSON.
pub mod agent_md;
pub mod json_bundle;
```

Create `crates/siyuan-render/src/agent_md.rs` and `crates/siyuan-render/src/json_bundle.rs`, each containing only:
```rust
// stub, populated in Phase D
```

`crates/siyuan-cli/Cargo.toml`:
```toml
[package]
name = "siyuan-cli"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "siyuan"
path = "src/main.rs"

[dependencies]
siyuan-types = { workspace = true }
siyuan-client = { workspace = true }
siyuan-model = { workspace = true }
siyuan-render = { workspace = true }
clap = { workspace = true }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }

[dev-dependencies]
siyuan-testkit = { workspace = true }
insta = { workspace = true }
reqwest = { workspace = true }
```

- [ ] **Step 6: cargo check 全 workspace**

Run: `cargo check --workspace`

Expected: 全部编译通过；`siyuan-cli` 二进制名是 `siyuan`。

- [ ] **Step 7: 提交**

```bash
git add Cargo.toml crates/ -A
git rm -r src 2>/dev/null || true
git commit -m "chore: split repo into types/client/model/render/cli crates"
```

---

## Task A2: `siyuan-types::id` — BlockId / NotebookId

**Files:**
- Modify: `crates/siyuan-types/src/id.rs`

**Background:** SiYuan block ID 形如 `20260501093000-abc1234`（14 位时间戳 + `-` + 7 位小写字母数字）。这些 ID 在整个 harness 里都用强类型而不是裸 `String`，避免误把 markdown 当 ID 传。

- [ ] **Step 1: 写实现 + 测试**

Replace `crates/siyuan-types/src/id.rs`:

```rust
use std::fmt;
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

static BLOCK_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\d{14}-[0-9a-z]{7}$").expect("compile-time-valid regex"));

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdError {
    #[error("invalid SiYuan block id: {0:?} (expected 14-digit timestamp + '-' + 7 lowercase alnum)")]
    Invalid(String),
}

/// A SiYuan block identifier.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(String);

impl BlockId {
    pub fn parse(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if BLOCK_ID_RE.is_match(&s) {
            Ok(Self(s))
        } else {
            Err(IdError::Invalid(s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BlockId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A SiYuan notebook identifier. Same shape as a block id.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NotebookId(String);

impl NotebookId {
    pub fn parse(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if BLOCK_ID_RE.is_match(&s) {
            Ok(Self(s))
        } else {
            Err(IdError::Invalid(s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NotebookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for NotebookId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_block_id() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        assert_eq!(id.as_str(), "20260501093000-abc1234");
    }

    #[test]
    fn rejects_uppercase() {
        assert!(BlockId::parse("20260501093000-ABC1234").is_err());
    }

    #[test]
    fn rejects_short_suffix() {
        assert!(BlockId::parse("20260501093000-abc123").is_err());
    }

    #[test]
    fn rejects_missing_dash() {
        assert!(BlockId::parse("20260501093000abc1234").is_err());
    }

    #[test]
    fn from_str_works() {
        let id: BlockId = "20260501093000-abc1234".parse().unwrap();
        assert_eq!(id.to_string(), "20260501093000-abc1234");
    }

    #[test]
    fn serde_round_trip() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"20260501093000-abc1234\"");
        let back: BlockId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p siyuan-types id::`

Expected: 6 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-types/src/id.rs
git commit -m "feat(types): typed BlockId / NotebookId with regex validation"
```

---

## Task A3: `siyuan-types::block` — BlockType / BlockNode

**Files:**
- Modify: `crates/siyuan-types/src/block.rs`

- [ ] **Step 1: 写实现 + 测试**

Replace `crates/siyuan-types/src/block.rs`:

```rust
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::{BlockId, NotebookId};

/// SiYuan's first-class block kinds. Variants match the `type` column in the
/// `blocks` table: `d`, `h`, `p`, `l`, `i`, `s`, `b`, `c`, `m`, `t`, `tb`,
/// `query_embed`, `av`, `html`, `iframe`, `widget`, plus media leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Document,
    Heading,
    Paragraph,
    List,
    ListItem,
    SuperBlock,
    Blockquote,
    Code,
    Math,
    Table,
    ThematicBreak,
    QueryEmbed,
    AttributeView,
    Html,
    IFrame,
    Widget,
    Audio,
    Video,
    Unknown,
}

impl BlockType {
    /// Parse the single-letter / underscored type returned by the kernel.
    pub fn from_kernel(raw: &str) -> Self {
        match raw {
            "d" => Self::Document,
            "h" => Self::Heading,
            "p" => Self::Paragraph,
            "l" => Self::List,
            "i" => Self::ListItem,
            "s" => Self::SuperBlock,
            "b" => Self::Blockquote,
            "c" => Self::Code,
            "m" => Self::Math,
            "t" => Self::Table,
            "tb" => Self::ThematicBreak,
            "query_embed" => Self::QueryEmbed,
            "av" => Self::AttributeView,
            "html" => Self::Html,
            "iframe" => Self::IFrame,
            "widget" => Self::Widget,
            "audio" => Self::Audio,
            "video" => Self::Video,
            _ => Self::Unknown,
        }
    }

    pub fn as_kernel(&self) -> &'static str {
        match self {
            Self::Document => "d",
            Self::Heading => "h",
            Self::Paragraph => "p",
            Self::List => "l",
            Self::ListItem => "i",
            Self::SuperBlock => "s",
            Self::Blockquote => "b",
            Self::Code => "c",
            Self::Math => "m",
            Self::Table => "t",
            Self::ThematicBreak => "tb",
            Self::QueryEmbed => "query_embed",
            Self::AttributeView => "av",
            Self::Html => "html",
            Self::IFrame => "iframe",
            Self::Widget => "widget",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Unknown => "unknown",
        }
    }
}

/// Heading level / list ordering / etc. — opaque string passed through.
pub type BlockSubtype = String;

/// Semantic role: how the harness treats this block for editing/insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockRole {
    /// `d/s/l/i/b` — accepts `append_child` / `prepend_child`.
    Container,
    /// `h` — accepts `append_section` / `prepend_section`.
    HeadingSectionOwner,
    /// `p/c/m/t/tb/query_embed/...` — leaf, no child operations.
    Leaf,
}

impl BlockRole {
    pub fn for_block_type(t: BlockType) -> Self {
        match t {
            BlockType::Document
            | BlockType::SuperBlock
            | BlockType::List
            | BlockType::ListItem
            | BlockType::Blockquote => Self::Container,
            BlockType::Heading => Self::HeadingSectionOwner,
            _ => Self::Leaf,
        }
    }
}

/// One block in a document tree, with semantic annotations beyond what the
/// raw `blocks` table provides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockNode {
    pub id: BlockId,
    pub root_id: BlockId,
    pub parent_id: Option<BlockId>,
    pub notebook_id: NotebookId,

    pub block_type: BlockType,
    pub subtype: Option<BlockSubtype>,
    pub role: BlockRole,

    pub markdown: String,
    pub kramdown: Option<String>,
    pub ial: Option<String>,
    #[serde(default)]
    pub attrs: BTreeMap<String, String>,

    pub hash: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub sort: Option<i64>,

    /// Children whose `parent_id == self.id` (data-structure children).
    #[serde(default)]
    pub structural_children: Vec<BlockId>,

    /// Heading section content. Empty unless `block_type == Heading`.
    #[serde(default)]
    pub section_children: Vec<BlockId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_type_round_trips_through_kernel_form() {
        for t in [
            BlockType::Document,
            BlockType::Heading,
            BlockType::Paragraph,
            BlockType::List,
            BlockType::ListItem,
            BlockType::SuperBlock,
            BlockType::Blockquote,
            BlockType::Code,
            BlockType::QueryEmbed,
            BlockType::AttributeView,
            BlockType::ThematicBreak,
        ] {
            assert_eq!(BlockType::from_kernel(t.as_kernel()), t, "round trip {t:?}");
        }
    }

    #[test]
    fn unknown_kernel_type_falls_back() {
        assert_eq!(BlockType::from_kernel("xyzzy"), BlockType::Unknown);
    }

    #[test]
    fn role_classification() {
        assert_eq!(BlockRole::for_block_type(BlockType::Heading), BlockRole::HeadingSectionOwner);
        assert_eq!(BlockRole::for_block_type(BlockType::SuperBlock), BlockRole::Container);
        assert_eq!(BlockRole::for_block_type(BlockType::Paragraph), BlockRole::Leaf);
    }
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p siyuan-types block::`

Expected: 3 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-types/src/block.rs
git commit -m "feat(types): BlockType, BlockRole, BlockNode"
```

---

## Task A4: `siyuan-types::position` + `error`

**Files:**
- Modify: `crates/siyuan-types/src/position.rs`
- Modify: `crates/siyuan-types/src/error.rs`

- [ ] **Step 1: 写 `position.rs`**

Replace `crates/siyuan-types/src/position.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::id::BlockId;

/// Where to drop blocks for `insert_blocks` / `move_block`.
///
/// Variants are deliberately distinct so the harness — not the agent — picks
/// the correct combination of SiYuan's `previousID` / `nextID` / `parentID`
/// arguments. In particular, `AppendSection` (heading) and `AppendChild`
/// (container) are different ops with different semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Position {
    AfterBlock { block_id: BlockId },
    BeforeBlock { block_id: BlockId },
    AppendChild { container_id: BlockId },
    PrependChild { container_id: BlockId },
    AppendSection { heading_id: BlockId },
    PrependSection { heading_id: BlockId },
    AppendDoc { doc_id: BlockId },
    PrependDoc { doc_id: BlockId },
}

impl Position {
    /// The id this position is anchored on, regardless of variant.
    pub fn anchor_id(&self) -> &BlockId {
        match self {
            Self::AfterBlock { block_id }
            | Self::BeforeBlock { block_id } => block_id,
            Self::AppendChild { container_id }
            | Self::PrependChild { container_id } => container_id,
            Self::AppendSection { heading_id }
            | Self::PrependSection { heading_id } => heading_id,
            Self::AppendDoc { doc_id }
            | Self::PrependDoc { doc_id } => doc_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_with_kind_tag() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let pos = Position::AfterBlock { block_id: id.clone() };
        let json = serde_json::to_string(&pos).unwrap();
        assert!(json.contains("\"kind\":\"after_block\""));
        assert!(json.contains("\"block_id\":\"20260501093000-abc1234\""));
    }

    #[test]
    fn deserialises_section_position() {
        let raw = r#"{"kind":"append_section","heading_id":"20260501093000-abc1234"}"#;
        let pos: Position = serde_json::from_str(raw).unwrap();
        match pos {
            Position::AppendSection { heading_id } => {
                assert_eq!(heading_id.as_str(), "20260501093000-abc1234");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn anchor_id_extracts_underlying_id() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let pos = Position::AppendChild { container_id: id.clone() };
        assert_eq!(pos.anchor_id(), &id);
    }
}
```

- [ ] **Step 2: 写 `error.rs`**

Replace `crates/siyuan-types/src/error.rs`:

```rust
use thiserror::Error;

use crate::id::BlockId;

/// Categorised harness error. `kind()` gives a stable enum-shaped value for
/// programmatic handling; `Display` gives the human message.
#[derive(Debug, Error)]
pub enum SiyuanError {
    #[error("HTTP transport error: {0}")]
    Http(String),

    #[error("authentication missing or invalid")]
    Auth,

    #[error("SiYuan API returned code {code}: {msg}")]
    Api { code: i32, msg: String },

    #[error("block not found: {0}")]
    NotFound(String),

    #[error("path is ambiguous: {hpath:?} resolves to multiple ids: {candidates:?}")]
    AmbiguousPath { hpath: String, candidates: Vec<BlockId> },

    #[error("operation {op:?} is not supported on block {id} of type {block_type}")]
    UnsupportedOp { id: BlockId, block_type: String, op: String },

    #[error("SQL query unavailable (publish mode disables /api/query/sql)")]
    SqlUnavailable,

    #[error("graph result exceeded limit ({limit}); refine query")]
    GraphLimit { limit: usize },

    #[error("parse error: {0}")]
    Parse(String),

    #[error("verification after write failed: {0}")]
    VerifyFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Http,
    Auth,
    Api,
    NotFound,
    AmbiguousPath,
    UnsupportedOp,
    SqlUnavailable,
    GraphLimit,
    Parse,
    VerifyFailed,
}

impl SiyuanError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Http(_) => ErrorKind::Http,
            Self::Auth => ErrorKind::Auth,
            Self::Api { .. } => ErrorKind::Api,
            Self::NotFound(_) => ErrorKind::NotFound,
            Self::AmbiguousPath { .. } => ErrorKind::AmbiguousPath,
            Self::UnsupportedOp { .. } => ErrorKind::UnsupportedOp,
            Self::SqlUnavailable => ErrorKind::SqlUnavailable,
            Self::GraphLimit { .. } => ErrorKind::GraphLimit,
            Self::Parse(_) => ErrorKind::Parse,
            Self::VerifyFailed(_) => ErrorKind::VerifyFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_matches_variant() {
        let err = SiyuanError::Auth;
        assert_eq!(err.kind(), ErrorKind::Auth);
    }

    #[test]
    fn api_error_displays_code_and_msg() {
        let err = SiyuanError::Api { code: 21, msg: "Bad token".into() };
        let s = err.to_string();
        assert!(s.contains("21"));
        assert!(s.contains("Bad token"));
    }
}
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p siyuan-types`

Expected: 全部通过（id + block + position + error 加起来 ~14 个）。

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-types/src/position.rs crates/siyuan-types/src/error.rs
git commit -m "feat(types): Position enum and SiyuanError model"
```

---

# Phase B: HTTP Client

## Task B1: SiyuanClient core + response decode

**Files:**
- Modify: `crates/siyuan-client/src/client.rs`
- Modify: `crates/siyuan-client/src/response.rs`

**Background:** SiYuan kernel HTTP API 基本规则：
- 全是 `POST` + `application/json`
- Body 是 JSON object（即使没参数也传 `{}`）
- 鉴权 header：`Authorization: Token <token>`
- 响应统一 `{"code": <i32>, "msg": <string>, "data": <T?>}`，code=0 成功

把这个统一封装在 `client.rs::post_json`，业务模块直接拿到反序列化好的 `T`。

- [ ] **Step 1: 写 `response.rs`**

Replace `crates/siyuan-client/src/response.rs`:

```rust
use serde::Deserialize;

use siyuan_types::SiyuanError;

#[derive(Debug, Deserialize)]
pub struct SiyuanResponse<T> {
    pub code: i32,
    #[serde(default)]
    pub msg: String,
    #[serde(default = "Option::default")]
    pub data: Option<T>,
}

impl<T> SiyuanResponse<T> {
    pub fn into_result(self) -> Result<T, SiyuanError> {
        if self.code == 0 {
            self.data.ok_or_else(|| SiyuanError::Parse(
                "kernel returned code=0 but no data field".into(),
            ))
        } else {
            Err(SiyuanError::Api { code: self.code, msg: self.msg })
        }
    }

    /// Some endpoints (e.g. removeNotebook) legitimately return `data: null`
    /// on success.
    pub fn into_result_or_unit(self) -> Result<Option<T>, SiyuanError> {
        if self.code == 0 {
            Ok(self.data)
        } else {
            Err(SiyuanError::Api { code: self.code, msg: self.msg })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_returns_data() {
        let raw = r#"{"code":0,"msg":"","data":{"v":42}}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        let out = r.into_result().unwrap();
        assert_eq!(out["v"], 42);
    }

    #[test]
    fn nonzero_code_becomes_api_error() {
        let raw = r#"{"code":21,"msg":"bad","data":null}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        match r.into_result() {
            Err(SiyuanError::Api { code, msg }) => {
                assert_eq!(code, 21);
                assert_eq!(msg, "bad");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn null_data_with_code_zero_is_unit_ok() {
        let raw = r#"{"code":0,"msg":"","data":null}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        assert!(r.into_result_or_unit().unwrap().is_none());
    }
}
```

- [ ] **Step 2: 写 `client.rs`**

Replace `crates/siyuan-client/src/client.rs`:

```rust
use std::time::Duration;

use reqwest::Url;
use serde::{Serialize, de::DeserializeOwned};
use tracing::{debug, trace};

use siyuan_types::SiyuanError;

use crate::response::SiyuanResponse;

/// Thin HTTP wrapper over the SiYuan kernel API.
#[derive(Debug, Clone)]
pub struct SiyuanClient {
    base_url: Url,
    token: String,
    http: reqwest::Client,
}

impl SiyuanClient {
    pub fn new(base_url: impl AsRef<str>, token: impl Into<String>) -> Result<Self, SiyuanError> {
        let parsed = Url::parse(base_url.as_ref())
            .map_err(|e| SiyuanError::Parse(format!("base_url: {e}")))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| SiyuanError::Http(e.to_string()))?;
        Ok(Self { base_url: parsed, token: token.into(), http })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// POST `<base>/<path>` with `body` as JSON, decode `data` into `R`.
    pub async fn post<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, SiyuanError> {
        let resp: SiyuanResponse<R> = self.post_envelope(path, body).await?;
        resp.into_result()
    }

    /// POST returning the raw envelope, for endpoints whose `data` may be null.
    pub async fn post_envelope<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<SiyuanResponse<R>, SiyuanError> {
        let url = self
            .base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| SiyuanError::Parse(format!("join {path}: {e}")))?;
        debug!(method = "POST", %url, "siyuan call");

        let resp = self
            .http
            .post(url.clone())
            .header("Authorization", format!("Token {}", self.token))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| SiyuanError::Http(e.to_string()))?;

        let status = resp.status();
        let body_text = resp.text().await.map_err(|e| SiyuanError::Http(e.to_string()))?;
        trace!(%status, body = %body_text, "siyuan response");

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SiyuanError::Auth);
        }
        if !status.is_success() {
            return Err(SiyuanError::Http(format!("HTTP {status}: {body_text}")));
        }

        serde_json::from_str(&body_text)
            .map_err(|e| SiyuanError::Parse(format!("decode {url}: {e}; body={body_text}")))
    }
}
```

- [ ] **Step 3: cargo check + 单测**

Run: `cargo test -p siyuan-client response::`

Expected: 3 passed.

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-client/src/client.rs crates/siyuan-client/src/response.rs
git commit -m "feat(client): SiyuanClient + envelope decoder"
```

---

## Task B2: api/system + api/notebook

**Files:**
- Modify: `crates/siyuan-client/src/api/system.rs`
- Modify: `crates/siyuan-client/src/api/notebook.rs`

- [ ] **Step 1: 写 `api/system.rs`**

Replace `crates/siyuan-client/src/api/system.rs`:

```rust
use serde::Deserialize;
use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub version: Option<String>,
}

impl SiyuanClient {
    /// `/api/system/version` — returns the kernel version string.
    pub async fn system_version(&self) -> Result<String, SiyuanError> {
        // Endpoint returns `data` as a plain string.
        self.post::<_, String>("/api/system/version", &serde_json::json!({})).await
    }
}
```

- [ ] **Step 2: 写 `api/notebook.rs`**

Replace `crates/siyuan-client/src/api/notebook.rs`:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{NotebookId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    pub id: NotebookId,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub sort: i64,
    #[serde(default)]
    pub closed: bool,
}

#[derive(Debug, Deserialize)]
struct LsNotebooksData {
    notebooks: Vec<Notebook>,
}

#[derive(Debug, Serialize)]
struct OneNotebook<'a> {
    notebook: &'a NotebookId,
}

#[derive(Debug, Serialize)]
struct CreateNotebook<'a> {
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct RenameNotebook<'a> {
    notebook: &'a NotebookId,
    name: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedNotebook {
    pub notebook: Notebook,
}

impl SiyuanClient {
    pub async fn ls_notebooks(&self) -> Result<Vec<Notebook>, SiyuanError> {
        let data: LsNotebooksData = self.post("/api/notebook/lsNotebooks", &serde_json::json!({})).await?;
        Ok(data.notebooks)
    }

    pub async fn open_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/openNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn close_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/closeNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn create_notebook(&self, name: &str) -> Result<Notebook, SiyuanError> {
        let data: CreatedNotebook = self.post("/api/notebook/createNotebook", &CreateNotebook { name }).await?;
        Ok(data.notebook)
    }

    pub async fn rename_notebook(&self, id: &NotebookId, new_name: &str) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/renameNotebook", &RenameNotebook { notebook: id, name: new_name })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn remove_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/removeNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }
}
```

- [ ] **Step 3: cargo check**

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-client/src/api/system.rs crates/siyuan-client/src/api/notebook.rs
git commit -m "feat(client): system.version + notebook CRUD"
```

---

## Task B3: api/filetree

**Files:**
- Modify: `crates/siyuan-client/src/api/filetree.rs`

**Background:** `createDocWithMd` 返回新建文档的 block id。`getIDsByHPath` 把 hpath 解析为 block id 列表（多匹配时多于 1 个）。

- [ ] **Step 1: 写实现**

Replace `crates/siyuan-client/src/api/filetree.rs`:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, NotebookId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct CreateDocReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
    markdown: &'a str,
}

#[derive(Debug, Serialize)]
struct DocId<'a> {
    id: &'a BlockId,
}

#[derive(Debug, Serialize)]
struct RenameDocReq<'a> {
    #[serde(rename = "notebook")]
    notebook: &'a NotebookId,
    path: &'a str,
    title: &'a str,
}

#[derive(Debug, Serialize)]
struct MoveDocsReq<'a> {
    #[serde(rename = "fromPaths")]
    from_paths: &'a [String],
    #[serde(rename = "toNotebook")]
    to_notebook: &'a NotebookId,
    #[serde(rename = "toPath")]
    to_path: &'a str,
}

#[derive(Debug, Serialize)]
struct RemoveDocReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
}

#[derive(Debug, Serialize)]
struct GetIdsReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
}

#[derive(Debug, Serialize)]
struct GetHPathReq<'a> {
    id: &'a BlockId,
}

impl SiyuanClient {
    /// `/api/filetree/createDocWithMd` — returns the new doc's block id.
    pub async fn create_doc_with_md(
        &self,
        notebook: &NotebookId,
        hpath: &str,
        markdown: &str,
    ) -> Result<BlockId, SiyuanError> {
        let raw: String = self
            .post(
                "/api/filetree/createDocWithMd",
                &CreateDocReq { notebook, path: hpath, markdown },
            )
            .await?;
        BlockId::parse(raw).map_err(|e| SiyuanError::Parse(e.to_string()))
    }

    pub async fn rename_doc(
        &self,
        notebook: &NotebookId,
        path: &str,
        new_title: &str,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/filetree/renameDoc",
                &RenameDocReq { notebook, path, title: new_title },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn move_docs(
        &self,
        from_paths: &[String],
        to_notebook: &NotebookId,
        to_path: &str,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/filetree/moveDocs",
                &MoveDocsReq { from_paths, to_notebook, to_path },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn remove_doc(&self, notebook: &NotebookId, path: &str) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/filetree/removeDoc", &RemoveDocReq { notebook, path })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    /// `/api/filetree/getIDsByHPath` — resolve a human path to one or more block ids.
    pub async fn get_ids_by_hpath(
        &self,
        notebook: &NotebookId,
        hpath: &str,
    ) -> Result<Vec<BlockId>, SiyuanError> {
        let raw: Vec<String> = self
            .post("/api/filetree/getIDsByHPath", &GetIdsReq { notebook, path: hpath })
            .await?;
        raw.into_iter()
            .map(|s| BlockId::parse(s).map_err(|e| SiyuanError::Parse(e.to_string())))
            .collect()
    }

    /// `/api/filetree/getHPathByID` — opposite of above.
    pub async fn get_hpath_by_id(&self, id: &BlockId) -> Result<String, SiyuanError> {
        self.post("/api/filetree/getHPathByID", &GetHPathReq { id }).await
    }
}
```

- [ ] **Step 2: cargo check**

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-client/src/api/filetree.rs
git commit -m "feat(client): filetree CRUD + hpath resolution"
```

---

## Task B4: api/block (the big one)

**Files:**
- Modify: `crates/siyuan-client/src/api/block.rs`

**Background:** This is where SiYuan's block ops live. SiYuan returns "transactions" containing `doOperations`/`undoOperations` arrays; we extract the `id` of the first new block from `doOperations[0].id`.

- [ ] **Step 1: 写实现**

Replace `crates/siyuan-client/src/api/block.rs`:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, SiyuanError};

use crate::SiyuanClient;

// -------- request types --------

#[derive(Debug, Serialize)]
struct ById<'a> {
    id: &'a BlockId,
}

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
    Err(SiyuanError::Parse("no id found in transaction operations".into()))
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
                &AppendOrPrependReq { data_type: "markdown", data: markdown, parent_id },
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
                &AppendOrPrependReq { data_type: "markdown", data: markdown, parent_id },
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
                &UpdateReq { id, data_type: "markdown", data: markdown },
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
        let _: Vec<Transaction> = self
            .post("/api/block/moveBlock", &MoveReq { id, previous_id, parent_id })
            .await?;
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
```

- [ ] **Step 2: cargo test**

Run: `cargo test -p siyuan-client api::block::`

Expected: 2 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-client/src/api/block.rs
git commit -m "feat(client): block get/insert/append/prepend/update/delete/move"
```

---

## Task B5: attr / asset / query / export

**Files:**
- Modify: `crates/siyuan-client/src/api/attr.rs`
- Modify: `crates/siyuan-client/src/api/asset.rs`
- Modify: `crates/siyuan-client/src/api/query.rs`
- Modify: `crates/siyuan-client/src/api/export.rs`

- [ ] **Step 1: 写 `api/attr.rs`**

Replace:

```rust
use std::collections::BTreeMap;

use serde::Serialize;

use siyuan_types::{BlockId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct ById<'a> {
    id: &'a BlockId,
}

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
```

- [ ] **Step 2a: 给 `client.rs` 加一个 crate 内 token 访问器**

Asset 上传走 multipart，不能复用 `post_json`。在 `crates/siyuan-client/src/client.rs` 的 `impl SiyuanClient` 块内追加：

```rust
    pub(crate) fn token(&self) -> &str {
        &self.token
    }
```

- [ ] **Step 2b: 写 `api/asset.rs`**

Replace `crates/siyuan-client/src/api/asset.rs`:

```rust
use std::path::Path;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Deserialize)]
pub struct UploadResult {
    /// Map from original filename → kernel-stored relative path (e.g.
    /// `assets/foo-20260501093000-abcdefg.png`).
    #[serde(default, rename = "succMap")]
    pub succ_map: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "errFiles")]
    pub err_files: Vec<String>,
}

impl SiyuanClient {
    /// Upload a single file as an asset. Returns the kernel-relative path,
    /// e.g. `assets/myimg-20260501093000-abcdefg.png`, suitable for embedding
    /// in markdown as `![alt](assets/...)`.
    pub async fn upload_asset(&self, file_path: &Path) -> Result<String, SiyuanError> {
        let bytes = std::fs::read(file_path)
            .map_err(|e| SiyuanError::Http(format!("read {}: {e}", file_path.display())))?;
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| SiyuanError::Parse(format!("bad file name: {}", file_path.display())))?
            .to_string();

        let part = Part::bytes(bytes).file_name(filename.clone());
        let form = Form::new().part("file[]", part);

        let url = self
            .base_url()
            .join("api/asset/upload")
            .map_err(|e| SiyuanError::Parse(e.to_string()))?;

        let resp = reqwest::Client::new()
            .post(url)
            .header("Authorization", format!("Token {}", self.token()))
            .multipart(form)
            .send()
            .await
            .map_err(|e| SiyuanError::Http(e.to_string()))?;

        let body = resp.text().await.map_err(|e| SiyuanError::Http(e.to_string()))?;
        let parsed: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| SiyuanError::Parse(e.to_string()))?;
        let code = parsed.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            return Err(SiyuanError::Api {
                code: code as i32,
                msg: parsed
                    .get("msg")
                    .and_then(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        let upload: UploadResult = serde_json::from_value(
            parsed.get("data").cloned().unwrap_or(serde_json::Value::Null),
        )
        .map_err(|e| SiyuanError::Parse(e.to_string()))?;
        if !upload.err_files.is_empty() {
            return Err(SiyuanError::Api {
                code: -1,
                msg: format!("upload failed for: {:?}", upload.err_files),
            });
        }
        upload
            .succ_map
            .get(&filename)
            .cloned()
            .ok_or_else(|| SiyuanError::Parse(format!("succMap missing entry for {filename}")))
    }
}
```

- [ ] **Step 3: 写 `api/query.rs`**

Replace:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct SqlReq<'a> {
    stmt: &'a str,
}

impl SiyuanClient {
    /// `/api/query/sql` — read-only SQL. Returns rows as JSON objects.
    /// Note: in publish mode the kernel disables this endpoint and returns a
    /// non-zero code; callers should handle `SiyuanError::Api` and surface
    /// `SqlUnavailable` if recognised.
    pub async fn sql(&self, stmt: &str) -> Result<Vec<serde_json::Value>, SiyuanError> {
        match self
            .post::<_, Vec<serde_json::Value>>("/api/query/sql", &SqlReq { stmt })
            .await
        {
            Ok(rows) => Ok(rows),
            Err(SiyuanError::Api { code, msg }) if msg.to_lowercase().contains("publish") => {
                let _ = code;
                Err(SiyuanError::SqlUnavailable)
            }
            Err(e) => Err(e),
        }
    }

    /// Typed convenience: deserialise rows into `T`.
    pub async fn sql_typed<T: for<'de> Deserialize<'de>>(
        &self,
        stmt: &str,
    ) -> Result<Vec<T>, SiyuanError> {
        let rows = self.sql(stmt).await?;
        rows.into_iter()
            .map(|v| serde_json::from_value::<T>(v).map_err(|e| SiyuanError::Parse(e.to_string())))
            .collect()
    }
}
```

- [ ] **Step 4: 写 `api/export.rs`**

Replace:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct ExportReq<'a> {
    id: &'a BlockId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedDoc {
    #[serde(default)]
    pub h_path: String,
    pub content: String,
}

impl SiyuanClient {
    pub async fn export_md_content(&self, doc_id: &BlockId) -> Result<ExportedDoc, SiyuanError> {
        self.post("/api/export/exportMdContent", &ExportReq { id: doc_id }).await
    }
}
```

- [ ] **Step 5: cargo check + 单测**

Run: `cargo test -p siyuan-client`

Expected: 之前的 5 个测试通过；新模块只 cargo check（没集成测试就不写单测）。

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 6: 提交**

```bash
git add crates/siyuan-client/src/api crates/siyuan-client/src/client.rs
git commit -m "feat(client): attr/asset/query/export APIs"
```

---

# Phase C: Model layer

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

---

# Phase D: Render

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

---

# Phase E: CLI

## Task E1: clap skeleton + config + tracing

**Files:**
- Modify: `crates/siyuan-cli/src/main.rs`
- Create: `crates/siyuan-cli/src/config.rs`
- Create: `crates/siyuan-cli/src/output.rs`
- Create: `crates/siyuan-cli/src/commands/mod.rs`

- [ ] **Step 1: 写 `config.rs`**

Create `crates/siyuan-cli/src/config.rs`:

```rust
use anyhow::{Context, Result};

use siyuan_client::SiyuanClient;

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub token: String,
}

impl Config {
    /// Read from CLI flags first, then env, then a default of localhost.
    pub fn resolve(flag_base_url: Option<String>, flag_token: Option<String>) -> Result<Self> {
        let base_url = flag_base_url
            .or_else(|| std::env::var("SIYUAN_BASE_URL").ok())
            .unwrap_or_else(|| "http://127.0.0.1:6806".into());
        let token = flag_token
            .or_else(|| std::env::var("SIYUAN_TOKEN").ok())
            .context("--token (or SIYUAN_TOKEN env var) is required")?;
        Ok(Self { base_url, token })
    }

    pub fn into_client(self) -> Result<SiyuanClient> {
        Ok(SiyuanClient::new(&self.base_url, &self.token).map_err(anyhow::Error::from)?)
    }
}
```

- [ ] **Step 2: 写 `output.rs`**

Create `crates/siyuan-cli/src/output.rs`:

```rust
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum OutputFormat {
    AgentMd,
    Json,
    JsonPretty,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::AgentMd
    }
}
```

- [ ] **Step 3: 写 `commands/mod.rs`**

Create `crates/siyuan-cli/src/commands/mod.rs`:

```rust
pub mod asset;
pub mod create_doc;
pub mod delete_block;
pub mod doc;
pub mod get_block;
pub mod get_doc;
pub mod graph;
pub mod insert_blocks;
pub mod move_block;
pub mod notebook;
pub mod search;
pub mod set_attrs;
pub mod tag;
pub mod update_block;
```

- [ ] **Step 4: 给每个 command 文件先写最小 stub**

For each file in `crates/siyuan-cli/src/commands/`:

```rust
// stub, populated in a later task
```

- [ ] **Step 5: 写 `main.rs`**

Replace `crates/siyuan-cli/src/main.rs`:

```rust
mod commands;
mod config;
mod output;

use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "siyuan", version, about = "Agent harness for SiYuan")]
struct Cli {
    /// Base URL of the SiYuan kernel HTTP API.
    #[arg(long, env = "SIYUAN_BASE_URL", global = true)]
    base_url: Option<String>,

    /// API token (Authorization: Token <value>).
    #[arg(long, env = "SIYUAN_TOKEN", global = true)]
    token: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print kernel version (smoke test).
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let cfg = config::Config::resolve(cli.base_url, cli.token)?;
    let client = cfg.into_client()?;

    match cli.cmd {
        Cmd::Status => {
            let v = client.system_version().await?;
            info!(%v, "siyuan ok");
            println!("{v}");
        }
    }
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
}
```

- [ ] **Step 6: cargo build**

Run: `cargo build -p siyuan-cli`

Expected: `target/debug/siyuan` 二进制构建出来。

Run: `./target/debug/siyuan --help`

Expected: 看到 `status` 子命令。

- [ ] **Step 7: 提交**

```bash
git add crates/siyuan-cli/src
git commit -m "feat(cli): clap skeleton + status command"
```

---

## Task E2: read commands — get-doc, get-block

**Files:**
- Modify: `crates/siyuan-cli/src/commands/get_doc.rs`
- Modify: `crates/siyuan-cli/src/commands/get_block.rs`
- Modify: `crates/siyuan-cli/src/main.rs`

- [ ] **Step 1: 写 `commands/get_doc.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::{
    load::load_doc,
    pagination::{DEFAULT_PAGE_SIZE, PageRequest},
};
use siyuan_render::agent_md::render_doc;
use siyuan_render::json_bundle::render_bundle;
use siyuan_types::BlockId;

use crate::output::OutputFormat;

#[derive(Args, Debug)]
pub struct GetDocArgs {
    /// Document block id.
    #[arg(long)]
    pub id: String,

    #[arg(long, default_value_t = 1)]
    pub page: usize,

    #[arg(long, default_value_t = DEFAULT_PAGE_SIZE)]
    pub page_size: usize,

    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: GetDocArgs) -> Result<()> {
    let id = BlockId::parse(args.id).context("--id is not a valid block id")?;
    let bundle = load_doc(client, &id, PageRequest { page: args.page, page_size: args.page_size }).await?;
    let s = match args.format {
        OutputFormat::AgentMd => render_doc(&bundle),
        OutputFormat::Json => render_bundle(&bundle, false)?,
        OutputFormat::JsonPretty => render_bundle(&bundle, true)?,
    };
    println!("{s}");
    Ok(())
}
```

- [ ] **Step 2: 写 `commands/get_block.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

use crate::output::OutputFormat;

#[derive(Args, Debug)]
pub struct GetBlockArgs {
    #[arg(long)]
    pub id: String,

    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

#[derive(Debug, Serialize)]
struct BlockView {
    id: String,
    kramdown: String,
    attrs: std::collections::BTreeMap<String, String>,
}

pub async fn run(client: &SiyuanClient, args: GetBlockArgs) -> Result<()> {
    let id = BlockId::parse(args.id).context("--id is not a valid block id")?;
    let kr = client.get_block_kramdown(&id).await?;
    let attrs = client.get_block_attrs(&id).await.unwrap_or_default();

    let view = BlockView { id: kr.id.to_string(), kramdown: kr.kramdown, attrs };
    let s = match args.format {
        OutputFormat::AgentMd => format!("<!-- sy:block id={} -->\n{}", view.id, view.kramdown),
        OutputFormat::Json => serde_json::to_string(&view)?,
        OutputFormat::JsonPretty => serde_json::to_string_pretty(&view)?,
    };
    println!("{s}");
    Ok(())
}
```

- [ ] **Step 3: 接进 `main.rs`**

Modify `crates/siyuan-cli/src/main.rs` — extend `Cmd` and the dispatcher:

```rust
#[derive(Subcommand, Debug)]
enum Cmd {
    Status,
    GetDoc(commands::get_doc::GetDocArgs),
    GetBlock(commands::get_block::GetBlockArgs),
}
```

In `match cli.cmd`:

```rust
Cmd::Status => {
    let v = client.system_version().await?;
    println!("{v}");
}
Cmd::GetDoc(a) => commands::get_doc::run(&client, a).await?,
Cmd::GetBlock(a) => commands::get_block::run(&client, a).await?,
```

- [ ] **Step 4: cargo build + smoke**

Run: `cargo build -p siyuan-cli`

Expected: build OK.

Run: `./target/debug/siyuan get-doc --help`

Expected: prints `--id`, `--page`, `--page-size`, `--format` flags.

- [ ] **Step 5: 提交**

```bash
git add crates/siyuan-cli/src
git commit -m "feat(cli): get-doc and get-block read commands"
```

---

## Task E3: write commands — create-doc, update-block, insert-blocks

**Files:**
- Modify: `crates/siyuan-cli/src/commands/create_doc.rs`
- Modify: `crates/siyuan-cli/src/commands/update_block.rs`
- Modify: `crates/siyuan-cli/src/commands/insert_blocks.rs`
- Modify: `crates/siyuan-cli/src/main.rs`

**Background:** `insert-blocks` 是 v1 最重要的写入工具。它把 `Position` 翻译成 SiYuan API 调用：
- `after_block` / `before_block` → `insertBlock` with `previous_id` / `next_id`
- `append_child` → `appendBlock` with `parent_id`
- `prepend_child` → `prependBlock` with `parent_id`
- `append_section` → `insertBlock` with `previous_id = section_end(heading_id)`（先用 model 的 section 逻辑算出 section 的最后一块 id）
- `prepend_section` → `insertBlock` with `previous_id = heading_id`
- `append_doc` → `appendBlock` with `parent_id = doc_id`
- `prepend_doc` → `prependBlock` with `parent_id = doc_id`

为了保证"输入 markdown 中块顺序 = 最终页面顺序"，第一版用最简方案：把 markdown 一次性传给思源 API，让思源 kernel 自己解析并按顺序插入。如果将来发现 kernel 顺序不一致，再改成 harness 内部按段循环 + cursor。

- [ ] **Step 1: 把共享的 markdown 读入 helper 加到 `commands/mod.rs`**

Replace `crates/siyuan-cli/src/commands/mod.rs`:

```rust
pub mod asset;
pub mod create_doc;
pub mod delete_block;
pub mod doc;
pub mod get_block;
pub mod get_doc;
pub mod graph;
pub mod insert_blocks;
pub mod move_block;
pub mod notebook;
pub mod search;
pub mod set_attrs;
pub mod tag;
pub mod update_block;

use anyhow::Result;

/// Read markdown content from a file path, or stdin if path is `-`.
pub fn read_markdown_input(path: &str) -> Result<String> {
    use std::io::Read;
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(std::fs::read_to_string(path)?)
    }
}
```

- [ ] **Step 2: 写 `commands/create_doc.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

#[derive(Args, Debug)]
pub struct CreateDocArgs {
    #[arg(long)]
    pub notebook: String,

    /// Human path, e.g. "/Projects/New Page".
    #[arg(long)]
    pub hpath: String,

    /// Path to a markdown file. Use `-` for stdin.
    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: CreateDocArgs) -> Result<()> {
    let notebook = NotebookId::parse(&args.notebook).context("--notebook")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    let id = client.create_doc_with_md(&notebook, &args.hpath, &markdown).await?;
    println!("{id}");
    Ok(())
}
```

- [ ] **Step 3: 写 `commands/update_block.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct UpdateBlockArgs {
    #[arg(long)]
    pub id: String,

    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: UpdateBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    client.update_block_markdown(&id, &markdown).await?;
    println!("ok");
    Ok(())
}
```

- [ ] **Step 4: 写 `commands/insert_blocks.rs`**

Replace:

```rust
use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::section::populate_section_children;
use siyuan_types::{BlockId, BlockType, Position};

#[derive(Args, Debug)]
pub struct InsertBlocksArgs {
    /// Position kind. One of: after_block, before_block, append_child,
    /// prepend_child, append_section, prepend_section, append_doc, prepend_doc.
    #[arg(long)]
    pub position: String,

    /// Anchor block id (interpretation depends on position kind).
    #[arg(long)]
    pub anchor: String,

    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: InsertBlocksArgs) -> Result<()> {
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    let position = parse_position(&args.position, anchor.clone())?;

    let new_id = match position {
        Position::AfterBlock { block_id } => {
            client.insert_block_markdown(&markdown, Some(&block_id), None, None).await?
        }
        Position::BeforeBlock { block_id } => {
            client.insert_block_markdown(&markdown, None, Some(&block_id), None).await?
        }
        Position::AppendChild { container_id } => {
            client.append_block_markdown(&markdown, &container_id).await?
        }
        Position::PrependChild { container_id } => {
            client.prepend_block_markdown(&markdown, &container_id).await?
        }
        Position::AppendSection { heading_id } => {
            let section_end = resolve_section_end(client, &heading_id).await?;
            client.insert_block_markdown(&markdown, Some(&section_end), None, None).await?
        }
        Position::PrependSection { heading_id } => {
            // Right after the heading itself.
            client.insert_block_markdown(&markdown, Some(&heading_id), None, None).await?
        }
        Position::AppendDoc { doc_id } => {
            client.append_block_markdown(&markdown, &doc_id).await?
        }
        Position::PrependDoc { doc_id } => {
            client.prepend_block_markdown(&markdown, &doc_id).await?
        }
    };
    println!("{new_id}");
    Ok(())
}

fn parse_position(kind: &str, anchor: BlockId) -> Result<Position> {
    Ok(match kind {
        "after_block" => Position::AfterBlock { block_id: anchor },
        "before_block" => Position::BeforeBlock { block_id: anchor },
        "append_child" => Position::AppendChild { container_id: anchor },
        "prepend_child" => Position::PrependChild { container_id: anchor },
        "append_section" => Position::AppendSection { heading_id: anchor },
        "prepend_section" => Position::PrependSection { heading_id: anchor },
        "append_doc" => Position::AppendDoc { doc_id: anchor },
        "prepend_doc" => Position::PrependDoc { doc_id: anchor },
        other => bail!("unknown --position kind: {other}"),
    })
}

/// Find the last block in the section owned by `heading_id`. We do this by
/// loading the heading's doc and running our section detector — sufficient for
/// v1 (small docs). For huge docs this should be optimised by querying SQL
/// directly for the heading's section range.
async fn resolve_section_end(client: &SiyuanClient, heading_id: &BlockId) -> Result<BlockId> {
    use siyuan_model::load::load_doc;
    use siyuan_model::pagination::{DEFAULT_PAGE_SIZE, PageRequest};

    // Need root_id for the heading. SQL it.
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
    let root = rows.first().ok_or_else(|| anyhow::anyhow!("heading not found"))?;
    if root.ty != "h" {
        bail!("--anchor for append_section must be a heading block");
    }
    let root_id = BlockId::parse(&root.root_id).map_err(|e| anyhow::anyhow!(e))?;

    // For simplicity load the whole doc; v1 docs are bounded by 50/page but we
    // need full range for section detection. Issue a single-page big request.
    let bundle = load_doc(
        client,
        &root_id,
        PageRequest { page: 1, page_size: 100_000 },
    )
    .await?;
    let mut blocks = bundle.blocks;
    populate_section_children(&mut blocks);
    let heading = blocks.iter().find(|b| &b.id == heading_id).ok_or_else(|| anyhow::anyhow!("heading not in doc"))?;
    if heading.block_type != BlockType::Heading {
        bail!("anchor is not a heading after re-resolution");
    }
    if let Some(last) = heading.section_children.last() {
        Ok(last.clone())
    } else {
        // Empty section: treat heading itself as anchor.
        Ok(heading_id.clone())
    }
}
```

- [ ] **Step 5: 接 main**

In `main.rs`, extend `Cmd`:

```rust
#[derive(Subcommand, Debug)]
enum Cmd {
    Status,
    GetDoc(commands::get_doc::GetDocArgs),
    GetBlock(commands::get_block::GetBlockArgs),
    CreateDoc(commands::create_doc::CreateDocArgs),
    UpdateBlock(commands::update_block::UpdateBlockArgs),
    InsertBlocks(commands::insert_blocks::InsertBlocksArgs),
}
```

Add dispatch:
```rust
Cmd::CreateDoc(a) => commands::create_doc::run(&client, a).await?,
Cmd::UpdateBlock(a) => commands::update_block::run(&client, a).await?,
Cmd::InsertBlocks(a) => commands::insert_blocks::run(&client, a).await?,
```

- [ ] **Step 6: build**

Run: `cargo build -p siyuan-cli`

Expected: 通过。

- [ ] **Step 7: 提交**

```bash
git add crates/siyuan-cli/src
git commit -m "feat(cli): create-doc, update-block, insert-blocks"
```

---

## Task E4: move-block, delete-block, set-attrs

**Files:**
- Modify: `crates/siyuan-cli/src/commands/move_block.rs`
- Modify: `crates/siyuan-cli/src/commands/delete_block.rs`
- Modify: `crates/siyuan-cli/src/commands/set_attrs.rs`
- Modify: `crates/siyuan-cli/src/main.rs`

- [ ] **Step 1: 写 `move_block.rs`**

Replace:

```rust
use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct MoveBlockArgs {
    #[arg(long)]
    pub id: String,

    /// Destination position kind: after_block | before_block | append_child | prepend_child
    /// | append_section | prepend_section | append_doc | prepend_doc.
    #[arg(long)]
    pub position: String,

    #[arg(long)]
    pub anchor: String,
}

pub async fn run(client: &SiyuanClient, args: MoveBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    match args.position.as_str() {
        "after_block" => client.move_block(&id, Some(&anchor), None).await?,
        "append_child" | "append_doc" => client.move_block(&id, None, Some(&anchor)).await?,
        "before_block" => {
            // SiYuan moveBlock supports previousID for "after"; "before" via using
            // the predecessor of `anchor` as previous_id. For v1 we error out and
            // tell the caller to use after_block with the previous sibling instead.
            bail!("position=before_block is not supported by move; use after_block of the previous sibling");
        }
        "prepend_child" | "prepend_doc" => {
            // Equivalent to "no previous, parent=container" handled by moveBlock.
            client.move_block(&id, None, Some(&anchor)).await?;
        }
        "append_section" | "prepend_section" => {
            bail!("section-relative move is not supported in v1; resolve to a sibling block first");
        }
        other => bail!("unknown --position: {other}"),
    }
    println!("ok");
    Ok(())
}
```

- [ ] **Step 2: 写 `delete_block.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct DeleteBlockArgs {
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: DeleteBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    client.delete_block(&id).await?;
    println!("ok");
    Ok(())
}
```

- [ ] **Step 3: 写 `set_attrs.rs`**

Replace:

```rust
use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct SetAttrsArgs {
    #[arg(long)]
    pub id: String,

    /// Repeated `key=value` pairs. Custom attrs must be `custom-...`.
    #[arg(long = "attr", value_name = "KEY=VALUE")]
    pub attrs: Vec<String>,
}

pub async fn run(client: &SiyuanClient, args: SetAttrsArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for raw in &args.attrs {
        let (k, v) = raw.split_once('=').ok_or_else(|| anyhow::anyhow!("bad --attr {raw:?}; want KEY=VALUE"))?;
        if k.is_empty() {
            bail!("attr key may not be empty");
        }
        map.insert(k.into(), v.into());
    }
    client.set_block_attrs(&id, &map).await?;
    println!("ok");
    Ok(())
}
```

- [ ] **Step 4: 接 main**

Extend `Cmd`:

```rust
    MoveBlock(commands::move_block::MoveBlockArgs),
    DeleteBlock(commands::delete_block::DeleteBlockArgs),
    SetAttrs(commands::set_attrs::SetAttrsArgs),
```

Dispatch:
```rust
Cmd::MoveBlock(a) => commands::move_block::run(&client, a).await?,
Cmd::DeleteBlock(a) => commands::delete_block::run(&client, a).await?,
Cmd::SetAttrs(a) => commands::set_attrs::run(&client, a).await?,
```

- [ ] **Step 5: build**

Run: `cargo build -p siyuan-cli`

Expected: 通过。

- [ ] **Step 6: 提交**

```bash
git add crates/siyuan-cli/src
git commit -m "feat(cli): move-block, delete-block, set-attrs"
```

---

## Task E5: notebook + doc metadata commands

**Files:**
- Modify: `crates/siyuan-cli/src/commands/notebook.rs`
- Modify: `crates/siyuan-cli/src/commands/doc.rs`
- Modify: `crates/siyuan-cli/src/main.rs`

- [ ] **Step 1: 写 `notebook.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

#[derive(Subcommand, Debug)]
pub enum NotebookCmd {
    Ls,
    Open(IdArgs),
    Close(IdArgs),
    Create(NameArgs),
    Rename(RenameArgs),
    Remove(IdArgs),
}

#[derive(Args, Debug)]
pub struct IdArgs {
    #[arg(long)]
    pub id: String,
}

#[derive(Args, Debug)]
pub struct NameArgs {
    #[arg(long)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct RenameArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub name: String,
}

pub async fn run(client: &SiyuanClient, cmd: NotebookCmd) -> Result<()> {
    match cmd {
        NotebookCmd::Ls => {
            let nbs = client.ls_notebooks().await?;
            for nb in nbs {
                let status = if nb.closed { "closed" } else { "open  " };
                println!("{}\t{}\t{}", status, nb.id, nb.name);
            }
        }
        NotebookCmd::Open(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.open_notebook(&id).await?;
            println!("ok");
        }
        NotebookCmd::Close(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.close_notebook(&id).await?;
            println!("ok");
        }
        NotebookCmd::Create(a) => {
            let nb = client.create_notebook(&a.name).await?;
            println!("{}", nb.id);
        }
        NotebookCmd::Rename(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.rename_notebook(&id, &a.name).await?;
            println!("ok");
        }
        NotebookCmd::Remove(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.remove_notebook(&id).await?;
            println!("ok");
        }
    }
    Ok(())
}
```

- [ ] **Step 2: 写 `doc.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_types::{BlockId, NotebookId};

#[derive(Subcommand, Debug)]
pub enum DocCmd {
    Resolve(ResolveArgs),
    Rename(RenameArgs),
    Move(MoveArgs),
    SetIcon(IconArgs),
    SetSort(SortArgs),
    Remove(RemoveArgs),
}

#[derive(Args, Debug)]
pub struct ResolveArgs {
    #[arg(long)]
    pub notebook: String,
    #[arg(long)]
    pub hpath: String,
}

#[derive(Args, Debug)]
pub struct RenameArgs {
    #[arg(long)]
    pub notebook: String,
    /// Storage path (e.g. `/20260501090000-abc1234.sy`). Get via `doc resolve` then look up.
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub title: String,
}

#[derive(Args, Debug)]
pub struct MoveArgs {
    #[arg(long, num_args = 1.., value_name = "STORAGE_PATH")]
    pub from_paths: Vec<String>,
    #[arg(long)]
    pub to_notebook: String,
    #[arg(long)]
    pub to_path: String,
}

#[derive(Args, Debug)]
pub struct IconArgs {
    /// Document block id.
    #[arg(long)]
    pub id: String,
    /// Icon name (e.g. emoji shortcode like ":rocket:") or empty to clear.
    #[arg(long, default_value = "")]
    pub icon: String,
}

#[derive(Args, Debug)]
pub struct SortArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub sort: i64,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    #[arg(long)]
    pub notebook: String,
    #[arg(long)]
    pub path: String,
}

pub async fn run(client: &SiyuanClient, cmd: DocCmd) -> Result<()> {
    match cmd {
        DocCmd::Resolve(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            let ids = client.get_ids_by_hpath(&nb, &a.hpath).await?;
            for id in ids {
                println!("{id}");
            }
        }
        DocCmd::Rename(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            client.rename_doc(&nb, &a.path, &a.title).await?;
            println!("ok");
        }
        DocCmd::Move(a) => {
            let to_nb = NotebookId::parse(&a.to_notebook).context("--to-notebook")?;
            client.move_docs(&a.from_paths, &to_nb, &a.to_path).await?;
            println!("ok");
        }
        DocCmd::SetIcon(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("icon".to_string(), a.icon);
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::SetSort(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("sort".to_string(), a.sort.to_string());
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::Remove(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            client.remove_doc(&nb, &a.path).await?;
            println!("ok");
        }
    }
    Ok(())
}
```

- [ ] **Step 3: 接 main**

In `main.rs`:

```rust
#[derive(Subcommand, Debug)]
enum Cmd {
    Status,
    GetDoc(commands::get_doc::GetDocArgs),
    GetBlock(commands::get_block::GetBlockArgs),
    CreateDoc(commands::create_doc::CreateDocArgs),
    UpdateBlock(commands::update_block::UpdateBlockArgs),
    InsertBlocks(commands::insert_blocks::InsertBlocksArgs),
    MoveBlock(commands::move_block::MoveBlockArgs),
    DeleteBlock(commands::delete_block::DeleteBlockArgs),
    SetAttrs(commands::set_attrs::SetAttrsArgs),
    Notebook {
        #[command(subcommand)]
        cmd: commands::notebook::NotebookCmd,
    },
    Doc {
        #[command(subcommand)]
        cmd: commands::doc::DocCmd,
    },
}
```

Dispatch:
```rust
Cmd::Notebook { cmd } => commands::notebook::run(&client, cmd).await?,
Cmd::Doc { cmd } => commands::doc::run(&client, cmd).await?,
```

- [ ] **Step 4: build**

Run: `cargo build -p siyuan-cli`

Expected: 通过。

- [ ] **Step 5: 提交**

```bash
git add crates/siyuan-cli/src
git commit -m "feat(cli): notebook + doc metadata commands"
```

---

## Task E6: tag + asset + graph + search commands

**Files:**
- Modify: `crates/siyuan-cli/src/commands/tag.rs`
- Modify: `crates/siyuan-cli/src/commands/asset.rs`
- Modify: `crates/siyuan-cli/src/commands/graph.rs`
- Modify: `crates/siyuan-cli/src/commands/search.rs`
- Modify: `crates/siyuan-cli/src/main.rs`

- [ ] **Step 1: 写 `tag.rs`**

Replace:

```rust
use anyhow::Result;
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::tag::{list_tags, search_by_tag};

#[derive(Subcommand, Debug)]
pub enum TagCmd {
    Ls,
    Search(SearchArgs),
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    #[arg(long)]
    pub tag: String,
}

pub async fn run(client: &SiyuanClient, cmd: TagCmd) -> Result<()> {
    match cmd {
        TagCmd::Ls => {
            for t in list_tags(client).await? {
                println!("{t}");
            }
        }
        TagCmd::Search(a) => {
            for hit in search_by_tag(client, &a.tag).await? {
                println!("{}\t{}", hit.block_id, hit.markdown_preview);
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: 写 `asset.rs`**

Replace:

```rust
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;

#[derive(Subcommand, Debug)]
pub enum AssetCmd {
    Upload(UploadArgs),
    /// Print the markdown snippet for embedding an already-uploaded asset.
    Reference(ReferenceArgs),
}

#[derive(Args, Debug)]
pub struct UploadArgs {
    /// Local file to upload.
    #[arg(long)]
    pub file: PathBuf,
}

#[derive(Args, Debug)]
pub struct ReferenceArgs {
    /// Kernel-relative asset path (the value returned by `asset upload`).
    #[arg(long)]
    pub path: String,

    /// Alt text. For images, defaults to the file basename.
    #[arg(long, default_value = "")]
    pub alt: String,
}

pub async fn run(client: &SiyuanClient, cmd: AssetCmd) -> Result<()> {
    match cmd {
        AssetCmd::Upload(a) => {
            let path = client.upload_asset(&a.file).await?;
            println!("{path}");
        }
        AssetCmd::Reference(a) => {
            let alt = if a.alt.is_empty() {
                a.path.rsplit('/').next().unwrap_or("").to_string()
            } else {
                a.alt
            };
            // Image-style markdown reference.
            println!("![{alt}]({})", a.path);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: 写 `graph.rs`**

Replace:

```rust
use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::graph::{Direction, neighborhood};
use siyuan_types::BlockId;

#[derive(Subcommand, Debug)]
pub enum GraphCmd {
    Backlinks(IdArgs),
    Outgoing(IdArgs),
    Neighborhood(NeighborhoodArgs),
}

#[derive(Args, Debug)]
pub struct IdArgs {
    #[arg(long)]
    pub id: String,
}

#[derive(Args, Debug)]
pub struct NeighborhoodArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long, default_value_t = 2)]
    pub depth: usize,
    #[arg(long, default_value = "both")]
    pub direction: String,
}

pub async fn run(client: &SiyuanClient, cmd: GraphCmd) -> Result<()> {
    match cmd {
        GraphCmd::Backlinks(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let g = neighborhood(client, &id, 1, Direction::Incoming).await?;
            println!("{}", serde_json::to_string_pretty(&g)?);
        }
        GraphCmd::Outgoing(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let g = neighborhood(client, &id, 1, Direction::Outgoing).await?;
            println!("{}", serde_json::to_string_pretty(&g)?);
        }
        GraphCmd::Neighborhood(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let dir = match a.direction.as_str() {
                "in" | "incoming" => Direction::Incoming,
                "out" | "outgoing" => Direction::Outgoing,
                _ => Direction::Both,
            };
            let g = neighborhood(client, &id, a.depth, dir).await?;
            println!("{}", serde_json::to_string_pretty(&g)?);
        }
    }
    Ok(())
}
```

- [ ] **Step 4: 写 `search.rs`**

Replace:

```rust
use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Deserialize;

use siyuan_client::SiyuanClient;

#[derive(Subcommand, Debug)]
pub enum SearchCmd {
    Text(TextArgs),
    Blocks(BlocksArgs),
}

#[derive(Args, Debug)]
pub struct TextArgs {
    #[arg(long)]
    pub query: String,

    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct BlocksArgs {
    /// Block type letter (e.g. `h`, `p`, `c`).
    #[arg(long, default_value = "")]
    pub r#type: String,

    /// Substring to match against block content.
    #[arg(long, default_value = "")]
    pub contains: String,

    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
struct Hit {
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    markdown: String,
}

pub async fn run(client: &SiyuanClient, cmd: SearchCmd) -> Result<()> {
    match cmd {
        SearchCmd::Text(a) => {
            let needle = a.query.replace('\'', "''");
            let stmt = format!(
                "SELECT id, type, markdown FROM blocks WHERE markdown LIKE '%{needle}%' LIMIT {}",
                a.limit
            );
            let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
            for r in rows {
                println!("{}\t{}\t{}", r.id, r.block_type, oneline(&r.markdown));
            }
        }
        SearchCmd::Blocks(a) => {
            let mut conds = Vec::new();
            if !a.r#type.is_empty() {
                conds.push(format!("type = '{}'", a.r#type.replace('\'', "''")));
            }
            if !a.contains.is_empty() {
                conds.push(format!("content LIKE '%{}%'", a.contains.replace('\'', "''")));
            }
            let where_clause = if conds.is_empty() { "1=1".into() } else { conds.join(" AND ") };
            let stmt = format!(
                "SELECT id, type, markdown FROM blocks WHERE {where_clause} LIMIT {}",
                a.limit
            );
            let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
            for r in rows {
                println!("{}\t{}\t{}", r.id, r.block_type, oneline(&r.markdown));
            }
        }
    }
    Ok(())
}

fn oneline(s: &str) -> String {
    let one = s.replace('\n', " ");
    if one.chars().count() <= 80 {
        one
    } else {
        let truncated: String = one.chars().take(80).collect();
        format!("{truncated}…")
    }
}
```

- [ ] **Step 5: 接 main**

```rust
    Tag {
        #[command(subcommand)]
        cmd: commands::tag::TagCmd,
    },
    Asset {
        #[command(subcommand)]
        cmd: commands::asset::AssetCmd,
    },
    Graph {
        #[command(subcommand)]
        cmd: commands::graph::GraphCmd,
    },
    Search {
        #[command(subcommand)]
        cmd: commands::search::SearchCmd,
    },
```

```rust
Cmd::Tag { cmd } => commands::tag::run(&client, cmd).await?,
Cmd::Asset { cmd } => commands::asset::run(&client, cmd).await?,
Cmd::Graph { cmd } => commands::graph::run(&client, cmd).await?,
Cmd::Search { cmd } => commands::search::run(&client, cmd).await?,
```

- [ ] **Step 6: build**

Run: `cargo build -p siyuan-cli`

Expected: 通过。

Run: `./target/debug/siyuan --help`

Expected: 列出全部子命令：status, get-doc, get-block, create-doc, update-block, insert-blocks, move-block, delete-block, set-attrs, notebook, doc, tag, asset, graph, search.

- [ ] **Step 7: 提交**

```bash
git add crates/siyuan-cli/src
git commit -m "feat(cli): tag, asset, graph, search commands"
```

---

# Phase F: Integration tests

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

---

## Done check

After all tasks:

- [ ] `cargo check --workspace` 通过
- [ ] `cargo test --workspace` 通过（不含 `--ignored`，应该全部是 unit 测试）
- [ ] `cargo test --workspace -- --ignored --nocapture` 通过（含 testkit smoke + cli integration）
- [ ] `./target/debug/siyuan --help` 列出全部 15 个子命令
- [ ] `git log --oneline | wc -l` 至少 ~17 个新 commit
- [ ] `Cargo.toml`(workspace 根) `members` = `["crates/*"]`，包含 6 个 crate（types, client, model, render, cli, testkit）

后续阶段（不在本 plan 内）：MCP transport、AV 编辑、超级块创建、history/trash 包装、daily note、template、性能/rate-limit。
