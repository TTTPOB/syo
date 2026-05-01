# Phase A: Foundation

> **Part of:** [v1 Implementation Plan](../2026-05-01-v1-implementation.md) · **Prev:** — · **Next:** [Phase B: HTTP Client](phase-b-client.md)
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this phase task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Establish the cargo workspace skeleton and the no-dependency `siyuan-types` crate (`BlockId` / `NotebookId`, `BlockNode` + role classification, `Position` enum, `SiyuanError`).

---

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
insta = { version = "1", features = ["yaml", "json", "redactions"] }
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

`crates/siyuan-types/src/lib.rs` — **start minimal in A1**, then activate modules as their content lands in A2–A4:

```rust
//! Core data types for the SiYuan harness.

// Modules and re-exports populated in Tasks A2–A4.
```

After A2/A3/A4 the file should reach this final shape:

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

`crates/siyuan-client/src/lib.rs` — **A1 only declares `pub mod api;`**; `client` and `response` are added in Phase B when their files exist.

```rust
//! Typed HTTP client for the SiYuan kernel.
pub mod api;
```

Final shape after Phase B:

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

- [ ] **Step 2: 激活模块**

Update `crates/siyuan-types/src/lib.rs` to add the new module and re-exports:

```rust
//! Core data types for the SiYuan harness.

pub mod id;

pub use id::{BlockId, NotebookId};
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p siyuan-types id::`

Expected: 6 passed.

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-types/src/id.rs crates/siyuan-types/src/lib.rs
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
    fn unknown_round_trips_via_fallback() {
        // `Unknown.as_kernel()` returns "unknown"; `from_kernel("unknown")`
        // also lands on Unknown via the wildcard arm. The serialised form
        // is therefore stable even for unrecognised kernel types.
        assert_eq!(BlockType::Unknown.as_kernel(), "unknown");
        assert_eq!(BlockType::from_kernel("unknown"), BlockType::Unknown);
        assert_eq!(BlockType::from_kernel(""), BlockType::Unknown);
    }

    #[test]
    fn role_classification_covers_all_variants() {
        use BlockRole::*;
        use BlockType::*;
        let cases: &[(BlockType, BlockRole)] = &[
            (Document, Container),
            (SuperBlock, Container),
            (List, Container),
            (ListItem, Container),
            (Blockquote, Container),
            (Heading, HeadingSectionOwner),
            (Paragraph, Leaf),
            (Code, Leaf),
            (Math, Leaf),
            (Table, Leaf),
            (ThematicBreak, Leaf),
            (QueryEmbed, Leaf),
            (AttributeView, Leaf),
            (Html, Leaf),
            (IFrame, Leaf),
            (Widget, Leaf),
            (Audio, Leaf),
            (Video, Leaf),
            (Unknown, Leaf),
        ];
        for (bt, expected) in cases {
            assert_eq!(BlockRole::for_block_type(*bt), *expected, "{bt:?}");
        }
    }
}
```

- [ ] **Step 2: 激活模块**

Update `crates/siyuan-types/src/lib.rs` to expose `block`:

```rust
//! Core data types for the SiYuan harness.

pub mod block;
pub mod id;

pub use block::{BlockNode, BlockRole, BlockSubtype, BlockType};
pub use id::{BlockId, NotebookId};
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p siyuan-types block::`

Expected: 4 passed.

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-types/src/block.rs crates/siyuan-types/src/lib.rs
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

- [ ] **Step 3: 激活模块（最终形态）**

Update `crates/siyuan-types/src/lib.rs` to expose every module added in A2–A4:

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

- [ ] **Step 4: 跑测试**

Run: `cargo test -p siyuan-types`

Expected: 全部通过（id + block + position + error 加起来 ~14 个）。

- [ ] **Step 5: 提交**

```bash
git add crates/siyuan-types/src/position.rs crates/siyuan-types/src/error.rs crates/siyuan-types/src/lib.rs
git commit -m "feat(types): Position enum and SiyuanError model"
```

