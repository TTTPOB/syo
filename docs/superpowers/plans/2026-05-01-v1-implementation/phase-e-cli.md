# Phase E: CLI

> **Part of:** [v1 Implementation Plan](../2026-05-01-v1-implementation.md) · **Prev:** [Phase D: Render](phase-d-render.md) · **Next:** [Phase F: Integration tests](phase-f-integration.md)
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this phase task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Wire `siyuan-cli` (binary `siyuan`): clap subcommands for read (`get-doc`, `get-block`), write (`create-doc`, `update-block`, `insert-blocks`, `move-block`, `delete-block`, `set-attrs`), metadata (`notebook`, `doc`), and discovery (`tag`, `asset`, `graph`, `search`).

---

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

