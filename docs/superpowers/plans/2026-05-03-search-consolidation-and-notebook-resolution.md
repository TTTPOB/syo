# Search consolidation + notebook name resolution + hpath clarification — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate `search text`/`search blocks` into a single `search` command (at CLI + MCP layers), let all notebook parameters accept names (not just ids), and document that hpath first-segment is never a notebook name.

**Architecture:** Three independent changes layered bottom-up. Core changes first (`resolve_notebook_id` + rename `search::blocks`→`search`), then CLI updates, then MCP updates. Hpath note extracted as a constant reused across all tool descriptions.

**Tech Stack:** Rust, clap (CLI arg parsing), rmcp (MCP framework), siyuan-client (kernel HTTP API)

---

## File map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/siyuan-types/src/error.rs` | Mod | Add `NotebookNotFound`/`AmbiguousNotebook` error variants |
| `crates/syo-core/src/notebook.rs` | Mod | Add `resolve_notebook_id()` |
| `crates/syo-core/src/search.rs` | Mod | Remove `FulltextInput`+`fulltext()`, rename `BlocksInput`→`SearchInput`, `blocks()`→`search()` |
| `crates/syo-cli/src/main.rs` | Mod | `Search` variant takes `SearchArgs` directly, no subcommand |
| `crates/syo-cli/src/commands/search/mod.rs` | Rewrite | Flat args struct, single `run()` |
| `crates/syo-cli/src/commands/search/text.rs` | Delete | — |
| `crates/syo-cli/src/commands/search/blocks.rs` | Delete | — |
| `crates/syo-cli/src/commands/doc/mod.rs` | Mod | Add `HPATH_NOTE` constant |
| `crates/syo-cli/src/commands/doc/lookup.rs` | Mod | Accept pre-resolved `NotebookId` instead of raw string |
| `crates/syo-cli/src/commands/doc/create.rs` | Mod | Use `resolve_notebook_id()` |
| `crates/syo-cli/src/commands/doc/rename.rs` | Mod | Resolve notebook before lookup |
| `crates/syo-cli/src/commands/doc/remove.rs` | Mod | Same |
| `crates/syo-cli/src/commands/doc/resolve.rs` | Mod | Same |
| `crates/syo-cli/src/commands/doc/tree.rs` | Mod | Same |
| `crates/syo-cli/src/commands/notebook/rename.rs` | Mod | Use `resolve_notebook_id()` |
| `crates/syo-cli/src/commands/notebook/remove.rs` | Mod | Use `resolve_notebook_id()` |
| `crates/syo-cli/tests/search.rs` | Mod | Update for flat `syo search` CLI |
| `crates/syo-mcp/src/registry.rs` | Mod | Merge search tools, add `HPATH_NOTE`, insert note into descriptions |
| `crates/syo-mcp/src/tools/sql.rs` | Mod | Remove `search_text()`, rename `search_blocks()`→`search()` |
| `crates/syo-mcp/src/tools/doc.rs` | Mod | Use `resolve_notebook_id()` in `create_doc()` |
| `crates/syo-mcp/src/tools/filetree.rs` | Mod | Resolve notebook in handlers before pure parsers |
| `crates/syo-mcp/src/tools/notebook.rs` | Mod | Use `resolve_notebook_id()` in `rename()`/`remove()` |

---

### Task 1: Add notebook error variants to `siyuan-types`

**Files:**
- Modify: `crates/siyuan-types/src/error.rs`

- [ ] **Step 1: Add error variants**

Read the current `SiyuanError` enum, then add two new variants:

```rust
#[error("notebook {name:?} not found")]
NotebookNotFound { name: String },

#[error("ambiguous notebook name {name:?} — matches: {candidates}")]
AmbiguousNotebook { name: String, candidates: String },
```

- [ ] **Step 2: Build to verify compilation**

Run: `cargo build -p siyuan-types 2>&1`
Expected: compiles cleanly (no warnings from this crate)

- [ ] **Step 3: Commit**

```bash
git add crates/siyuan-types/src/error.rs
git commit -m "feat(siyuan-types): add NotebookNotFound and AmbiguousNotebook error variants

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: Add `resolve_notebook_id` to `syo-core`

**Files:**
- Modify: `crates/syo-core/src/notebook.rs`

- [ ] **Step 1: Add the function**

Append after the `remove()` function (after line ~49):

```rust
use anyhow::bail;
use siyuan_types::NotebookId;

/// Resolve a user-supplied string to a [`NotebookId`].
///
/// If `input` matches the notebook-id format it is returned immediately —
/// no network call is made. Otherwise `ls_notebooks()` is called and the
/// input is matched by exact display name. Duplicate names are rejected
/// with a diagnostic listing all matching ids.
pub async fn resolve_notebook_id(
    client: &SiyuanClient,
    input: &str,
) -> anyhow::Result<NotebookId> {
    // If it parses as a valid notebook id, return it directly.
    if let Ok(id) = NotebookId::parse(input) {
        return Ok(id);
    }

    let notebooks = client.ls_notebooks().await?;
    let mut matches: Vec<&siyuan_client::api::notebook::Notebook> = notebooks
        .iter()
        .filter(|n| n.name == input)
        .collect();

    match matches.len() {
        0 => bail!("notebook {input:?} not found"),
        1 => Ok(matches.remove(0).id.clone()),
        _ => {
            let ids: Vec<String> = matches
                .iter()
                .map(|n| format!("{} ({})", n.id.as_str(), n.name))
                .collect();
            bail!(
                "ambiguous notebook name {input:?} — matches: {}",
                ids.join(", ")
            );
        }
    }
}
```

Note: remove the existing `use siyuan_types::NotebookId;` import from line 5 since it's added in this block; the new `use anyhow::bail;` also gets added.

- [ ] **Step 2: Build to verify**

Run: `cargo build -p syo-core 2>&1`
Expected: compiles cleanly

- [ ] **Step 3: Commit**

```bash
git add crates/syo-core/src/notebook.rs
git commit -m "feat(syo-core): add resolve_notebook_id for name-to-id resolution

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: Consolidate search in `syo-core`

**Files:**
- Modify: `crates/syo-core/src/search.rs`

- [ ] **Step 1: Remove `FulltextInput`, `fulltext()`, rename `BlocksInput`→`SearchInput`, `blocks()`→`search()`**

The current file has FulltextInput struct (lines 24-28), BlocksInput struct (lines 30-35), fulltext() fn (lines 51-67), blocks() fn (lines 73-97). Make these changes:

1. Delete `FulltextInput` struct (lines 24-28)
2. Delete `fulltext()` function (lines 46-67)  
3. Rename `BlocksInput` → `SearchInput`
4. Rename `blocks()` → `search()`

Replace `BlocksInput` with `SearchInput`:

```rust
#[derive(Debug)]
pub struct SearchInput {
    pub block_type: String,
    pub contains: String,
    pub limit: usize,
}
```

Replace `pub async fn blocks(...)` with `pub async fn search(...)`, and update its doc comment:

```rust
/// Search for blocks by type (`=`) and content (`LIKE`) filter.
///
/// Empty `block_type` and/or `contains` disable the corresponding filter.
/// When both are empty the result is equivalent to `SELECT ... WHERE 1=1`.
pub async fn search(client: &SiyuanClient, input: SearchInput) -> Result<SearchOutput> {
```

- [ ] **Step 2: Update tests in the same file**

The test `structs_derive_debug` (lines 119-139) references `FulltextInput` and `BlocksInput`. Replace:

```rust
#[test]
fn structs_derive_debug() {
    fn _assert_debug<T: std::fmt::Debug>(_t: &T) {}

    let si = SearchInput {
        block_type: "p".into(),
        contains: "hello".into(),
        limit: 10,
    };
    _assert_debug(&si);

    let so = SearchOutput { hits: vec![] };
    _assert_debug(&so);
}
```

Delete the `FulltextInput` related assertion. Keep the two `SearchHit` deserialization tests.

- [ ] **Step 3: Build and run unit tests**

Run: `cargo test -p syo-core 2>&1`
Expected: all tests pass (2 search_hit tests + structs_derive_debug test)

- [ ] **Step 4: Commit**

```bash
git add crates/syo-core/src/search.rs
git commit -m "refactor(syo-core): consolidate search — remove fulltext, rename blocks to search

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: Consolidate search in `syo-cli`

**Files:**
- Delete: `crates/syo-cli/src/commands/search/text.rs`
- Delete: `crates/syo-cli/src/commands/search/blocks.rs`
- Rewrite: `crates/syo-cli/src/commands/search/mod.rs`
- Modify: `crates/syo-cli/src/main.rs`

- [ ] **Step 1: Rewrite `commands/search/mod.rs`**

Delete the subcommand enum and inline the search args directly:

```rust
use anyhow::Result;
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;

use crate::output::OutputFormat;

mod hit;
use hit::{Hit, emit_hits};

/// Filter blocks by type and/or content substring.
///
/// Sibling commands: `syo tag search` is exact tag match; `syo sql` is
/// the raw escape hatch for arbitrary queries (joins, aggregates, LIKE on
/// markdown).
///
/// Inputs:
///   --type (optional): block type letter — common values:
///     `d` document, `h` heading, `p` paragraph, `l` list, `i` list
///     item, `c` code, `t` table, `b` blockquote, `m` math,
///     `s` super-block. Empty (default) means no type filter.
///   --contains (optional): substring matched against block `content`
///     (visible text, no markdown formatting). Empty (default) means
///     no content filter. LIKE meta-chars (`%`, `_`) are NOT
///     escaped — they behave as wildcards.
///   --limit (optional, default 50): maximum hits, capped by
///     `MAX_SEARCH_LIMIT`.
///   --format (default agent-md): one of `agent-md` (the TSV form
///     described above), `json` (compact array of
///     `{id, type, markdown_preview}`), or `json-pretty` (indented).
///
/// Output is one hit per line: `<id>\t<type>\t<markdown-preview>`.
/// SQL index lag (~100-500 ms) applies.
///
/// Example:
///   in:  --type h --contains Plan --limit 5
///   out: 20260501090000-blk0001    h    # Plan
#[derive(ClapArgs, Debug)]
#[command(verbatim_doc_comment)]
pub struct SearchArgs {
    /// Block type letter (e.g. `h`, `p`, `c`). Empty disables the filter.
    #[arg(long, default_value = "")]
    pub r#type: String,

    /// Substring to match against block content. Empty disables the filter.
    #[arg(long, default_value = "")]
    pub contains: String,

    /// Maximum hits returned. Default 50, capped by `MAX_SEARCH_LIMIT`.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Output format: `agent-md` (default; TSV `id\ttype\tmarkdown_preview`),
    /// `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: SearchArgs) -> Result<()> {
    let result = syo_core::search::search(
        client,
        syo_core::search::SearchInput {
            block_type: args.r#type,
            contains: args.contains,
            limit: args.limit,
        },
    )
    .await?;
    let hits: Vec<Hit> = result
        .hits
        .into_iter()
        .map(|h| Hit {
            id: h.id,
            block_type: h.block_type,
            markdown: h.markdown,
        })
        .collect();
    emit_hits(hits, args.format)
}
```

- [ ] **Step 2: Update `main.rs`**

Change the `Search` variant in `Cmd` enum from:

```rust
    /// Search blocks by full-text or by type/contains predicates.
    Search {
        #[command(subcommand)]
        cmd: commands::search::SearchCmd,
    },
```

To:

```rust
    /// Filter blocks by type and/or content substring.
    Search(commands::search::SearchArgs),
```

And update the dispatch:

```rust
        Cmd::Search { cmd } => commands::search::run(&client, cmd).await?,
```

To:

```rust
        Cmd::Search(a) => commands::search::run(&client, a).await?,
```

- [ ] **Step 3: Delete old files**

```bash
rm crates/syo-cli/src/commands/search/text.rs crates/syo-cli/src/commands/search/blocks.rs
```

- [ ] **Step 4: Build and verify CLI compiles**

Run: `cargo build -p syo-cli 2>&1`
Expected: compiles cleanly

- [ ] **Step 5: Verify help output**

Run: `cargo run -- search --help 2>&1`
Expected: shows flat `--type`, `--contains`, `--limit`, `--format` options (no subcommands)

- [ ] **Step 6: Commit**

```bash
git add crates/syo-cli/src/commands/search/mod.rs crates/syo-cli/src/commands/search/text.rs crates/syo-cli/src/commands/search/blocks.rs crates/syo-cli/src/main.rs
git commit -m "refactor(syo-cli): flatten search command, remove text/blocks subcommands

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: Consolidate search in `syo-mcp`

**Files:**
- Modify: `crates/syo-mcp/src/registry.rs`
- Modify: `crates/syo-mcp/src/tools/sql.rs`

- [ ] **Step 1: Remove search_text handler and rename search_blocks in `tools/sql.rs`**

Delete the `search_text()` function entirely (lines 26-49). Rename `search_blocks()` to `search()` and update the core call:

```rust
pub async fn search(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let map = ensure_object(args)?;
    let block_type = optional_string(&map, "type").unwrap_or_default();
    let contains = optional_string(&map, "contains").unwrap_or_default();
    let limit = optional_u64(&map, "limit").unwrap_or(50) as usize;

    let output = syo_core::search::search(
        client,
        syo_core::search::SearchInput {
            block_type,
            contains,
            limit,
        },
    )
    .await
    .map_err(anyhow_to_mcp)?;
    Ok(with_hint(
        json!({ "hits": output.hits }),
        "Results are SQL-filtered by block type (=) and/or content (LIKE). When both \
         `type` and `contains` are empty, all blocks are returned up to `limit`. \
         Results may lag recent mutations by ~100–500 ms.",
    ))
}
```

Also update the test `search_text_rejects_whitespace_query` — delete it (that validation was for search_text only; search_blocks/search doesn't reject empty contains). The `raw_sql` tests stay.

- [ ] **Step 2: Update `registry.rs` — remove old search tools, add new one**

Remove the two registry blocks:
- `syo_siyuan_search_text` (lines ~839-868)
- `syo_siyuan_search_blocks` (lines ~869-953)

Replace with a single `syo_siyuan_search` tool:

```rust
    // ---- search ----
    {
        let c = Arc::clone(&client);
        reg!(
            "syo_siyuan_search",
            "Search for blocks by type and/or content filter.\n\
             \n\
             Sibling tools: `syo_siyuan_tag_search` finds blocks by exact tag match; \
             `syo_siyuan_sql` is the raw escape hatch for arbitrary queries (joins, \
             aggregates, fulltext LIKE against markdown). syo_siyuan_search filters by \
             block type (=) and/or content (LIKE); empty filters select all blocks up \
             to `limit`.\n\
             \n\
             Inputs: `type` (optional) is the block type code (e.g. `p`, `h`, `d`). \
             `contains` (optional) is a substring to match against the `content` column \
             (visible text). `limit` (optional, default 50) caps the result count.\n\
             \n\
             Example:\n\
               in:  { \"type\": \"h\", \"contains\": \"Plan\", \"limit\": 10 }\n\
               out: { \"data\": { \"hits\": [ { \"id\": \"20260501090000-blk0001\", \"type\": \"h\", \"markdown\": \"## Plan\" } ] } }",
            r#"{"type":"object","properties":{"type":{"type":"string","description":"Block type code (e.g. p, h, d)"},"contains":{"type":"string","description":"Substring match against content column"},"limit":{"type":"integer","default":50}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::sql::search(&c, args).await }
            })
        );
    }
```

- [ ] **Step 3: Build and run MCP unit tests**

Run: `cargo test -p syo-mcp 2>&1`
Expected: `raw_sql_*` tests pass; the old `search_text_rejects_whitespace_query` test no longer exists

- [ ] **Step 4: Commit**

```bash
git add crates/syo-mcp/src/registry.rs crates/syo-mcp/src/tools/sql.rs
git commit -m "refactor(syo-mcp): merge search_text and search_blocks into single search tool

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: Update CLI integration tests for new search shape

**Files:**
- Modify: `crates/syo-cli/tests/search.rs`

- [ ] **Step 1: Update test file**

The current test has 4 tests using direct SQL. These are fine — they test the kernel behavior, not the CLI surface. But rename the test module doc and remove references to "search text":

Replace the module doc comment (line 1-3):

```rust
//! Integration tests for search.
//!
//! Run with: `cargo test -p syo --test search -- --ignored --test-threads=1`
```

Rename `search_text_finds_matching_blocks` → `search_finds_matching_blocks`, update its doc comment. The test body stays the same (it uses direct SQL, not the CLI).

- [ ] **Step 2: Build tests**

Run: `cargo test -p syo --test search --no-run 2>&1`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add crates/syo-cli/tests/search.rs
git commit -m "test(syo-cli): update search integration test names for consolidated search

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Apply notebook name resolution to `syo-cli`

**Files:**
- Modify: `crates/syo-cli/src/commands/doc/lookup.rs`
- Modify: `crates/syo-cli/src/commands/doc/create.rs`
- Modify: `crates/syo-cli/src/commands/doc/rename.rs`
- Modify: `crates/syo-cli/src/commands/doc/remove.rs`
- Modify: `crates/syo-cli/src/commands/doc/resolve.rs`
- Modify: `crates/syo-cli/src/commands/doc/tree.rs`
- Modify: `crates/syo-cli/src/commands/notebook/rename.rs`
- Modify: `crates/syo-cli/src/commands/notebook/remove.rs`

- [ ] **Step 1: Change `lookup.rs` to accept pre-resolved `NotebookId`**

Change `build_single_doc_lookup` signature to take `notebook: Option<NotebookId>`:

```rust
use anyhow::{Context, Result, anyhow};

use siyuan_model::doc_meta::DocLookup;
use siyuan_types::{BlockId, NotebookId};

/// Build a single-document `DocLookup` from clap-parsed pieces.
///
/// Clap's `ArgGroup` already filters out partial / conflicting input, but
/// this helper is the canonical CLI-side validator so programmatic callers
/// get the same error shape as the command line.
pub(super) fn build_single_doc_lookup(
    id: Option<&str>,
    notebook: Option<NotebookId>,
    hpath: Option<&str>,
) -> Result<DocLookup> {
    match (id, notebook, hpath) {
        (Some(id), None, None) => Ok(DocLookup::ById(BlockId::parse(id.trim()).context("--id")?)),
        (None, Some(nb), Some(hp)) => Ok(DocLookup::ByHpath {
            notebook: nb,
            hpath: hp,
        }),
        (Some(_), _, _) => Err(anyhow!(
            "--id conflicts with --notebook/--hpath; pick exactly one input mode"
        )),
        _ => Err(anyhow!(
            "provide either --id, or both --notebook and --hpath"
        )),
    }
}
```

- [ ] **Step 2: Update `create.rs`**

Change line 56 from `NotebookId::parse(&args.notebook)` to:

```rust
    let notebook = syo_core::notebook::resolve_notebook_id(client, &args.notebook)
        .await
        .context("--notebook")?;
```

Remove the `use siyuan_types::NotebookId;` import (no longer needed).

- [ ] **Step 3: Update `rename.rs` (doc rename)**

Read the current file. It calls `build_single_doc_lookup(args.id.as_deref(), args.notebook.as_deref(), args.hpath.as_deref())`. Replace with:

```rust
    let notebook = match &args.notebook {
        Some(nb) => Some(
            syo_core::notebook::resolve_notebook_id(client, nb)
                .await
                .context("--notebook")?,
        ),
        None => None,
    };
    let lookup = super::lookup::build_single_doc_lookup(
        args.id.as_deref(),
        notebook,
        args.hpath.as_deref(),
    )?;
```

- [ ] **Step 4: Update `remove.rs` (doc remove)**

Same pattern as rename.rs — resolve notebook before passing to `build_single_doc_lookup`.

- [ ] **Step 5: Update `resolve.rs` (doc resolve)**

Same pattern — resolve notebook before calling `build_single_doc_lookup`.

- [ ] **Step 6: Update `tree.rs` (doc tree)**

Same pattern — resolve notebook before calling `build_single_doc_lookup`.

- [ ] **Step 7: Update `notebook/rename.rs`**

Replace:
```rust
    let id = NotebookId::parse(&args.id).context("--id")?;
```
With:
```rust
    let id = syo_core::notebook::resolve_notebook_id(client, &args.id)
        .await
        .context("--id")?;
```
Remove the `use siyuan_types::NotebookId;` import.

- [ ] **Step 8: Update `notebook/remove.rs`**

Same as rename.rs — replace `NotebookId::parse` with `resolve_notebook_id`.

- [ ] **Step 9: Build and verify**

Run: `cargo build -p syo-cli 2>&1`
Expected: compiles cleanly

- [ ] **Step 10: Commit**

```bash
git add crates/syo-cli/src/commands/doc/lookup.rs crates/syo-cli/src/commands/doc/create.rs crates/syo-cli/src/commands/doc/rename.rs crates/syo-cli/src/commands/doc/remove.rs crates/syo-cli/src/commands/doc/resolve.rs crates/syo-cli/src/commands/doc/tree.rs crates/syo-cli/src/commands/notebook/rename.rs crates/syo-cli/src/commands/notebook/remove.rs
git commit -m "feat(syo-cli): accept notebook names in addition to ids

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Apply notebook name resolution to `syo-mcp`

**Files:**
- Modify: `crates/syo-mcp/src/tools/doc.rs`
- Modify: `crates/syo-mcp/src/tools/filetree.rs`
- Modify: `crates/syo-mcp/src/tools/notebook.rs`

- [ ] **Step 1: Add a shared resolver helper in `tools/util.rs`**

Add to `crates/syo-mcp/src/tools/util.rs`:

```rust
use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

/// Resolve a user-supplied string (id or name) to a [`NotebookId`].
///
/// Wraps `syo_core::notebook::resolve_notebook_id` and maps errors to MCP
/// `invalid_params` so every handler doesn't repeat the error conversion.
pub async fn resolve_notebook_id(client: &SiyuanClient, input: &str) -> Result<NotebookId, McpError> {
    syo_core::notebook::resolve_notebook_id(client, input)
        .await
        .map_err(|e| McpError::invalid_params(format!("invalid notebook: {:#}", e), None))
}
```

- [ ] **Step 2: Update `tools/doc.rs` `create_doc()`**

Replace:
```rust
    let notebook = siyuan_types::NotebookId::parse(&notebook_str)
        .map_err(|e| McpError::invalid_params(format!("invalid notebook id: {e}"), None))?;
```
With:
```rust
    let notebook = super::util::resolve_notebook_id(client, &notebook_str).await?;
```

Remove the `use siyuan_types::NotebookId;` import if no other use.

- [ ] **Step 3: Move `is_present` to `tools/util.rs`**

The `is_present()` helper currently lives as a private `fn` in `filetree.rs`. Move it to `tools/util.rs` and make it `pub(crate)` so both `filetree.rs` handlers and the util module can use it:

In `tools/util.rs`, add:
```rust
/// Treat whitespace-only inputs as absent.
pub(crate) fn is_present(s: Option<&str>) -> bool {
    s.is_some_and(|v| !v.trim().is_empty())
}
```

Remove the same function from `filetree.rs`.

- [ ] **Step 4: Update `filetree.rs` handlers — add pre-resolution**

In each handler (`resolve`, `rename_doc`, `move_doc`, `remove_doc`, `tree`), add a block BEFORE the pure parser call. For example, in `resolve()`:

```rust
pub async fn resolve(client: &SiyuanClient, args: Value) -> Result<Value, McpError> {
    let mut map = ensure_object(args)?;
    // Pre-resolve notebook name→id before passing to pure parser.
    if let Some(nb) = map.get("notebook").and_then(|v| v.as_str()) {
        if super::util::is_present(Some(nb)) {
            let resolved = super::util::resolve_notebook_id(client, nb).await?;
            map.insert("notebook".to_string(), Value::String(resolved.to_string()));
        }
    }
    let lookup = parse_doc_lookup(&map)?;
    // ... rest unchanged
}
```

Apply the same pattern to: `rename_doc()`, `remove_doc()`, `tree()`. For `move_doc()`, also resolve `to_notebook`:

```rust
    if let Some(nb) = map.get("notebook").and_then(|v| v.as_str()) {
        if super::util::is_present(Some(nb)) {
            let resolved = super::util::resolve_notebook_id(client, nb).await?;
            map.insert("notebook".to_string(), Value::String(resolved.to_string()));
        }
    }
    if let Some(nb) = map.get("to_notebook").and_then(|v| v.as_str()) {
        if super::util::is_present(Some(nb)) {
            let resolved = super::util::resolve_notebook_id(client, nb).await?;
            map.insert("to_notebook".to_string(), Value::String(resolved.to_string()));
        }
    }
```

- [ ] **Step 5: Remove local `parse_notebook_id` from `filetree.rs`**

Delete the local `fn parse_notebook_id()` — it's no longer used by handlers. Keep it in tests if needed, or replace test usage with direct `NotebookId::parse()`.

- [ ] **Step 6: Update `tools/notebook.rs`**

In `rename()` and `remove()`, replace:
```rust
    let id = parse_notebook_id(&required_string(&map, "id")?)?;
```
With:
```rust
    let id = super::util::resolve_notebook_id(client, &required_string(&map, "id")?).await?;
```

Remove the local `parse_notebook_id()` function and the `use siyuan_types::NotebookId;` import.

- [ ] **Step 7: Build and run MCP tests**

Run: `cargo test -p syo-mcp 2>&1`
Expected: all tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/syo-mcp/src/tools/util.rs crates/syo-mcp/src/tools/doc.rs crates/syo-mcp/src/tools/filetree.rs crates/syo-mcp/src/tools/notebook.rs
git commit -m "feat(syo-mcp): accept notebook names in addition to ids across all tools

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Add hpath clarification note to MCP tools

**Files:**
- Modify: `crates/syo-mcp/src/registry.rs`

- [ ] **Step 1: Add `HPATH_NOTE` constant at top of `registry.rs`**

After the `use` statements and before `fn schema()`:

```rust
/// Hpath clarification note — embedded in every tool description that
/// involves a `notebook` + `hpath` pair.
const HPATH_NOTE: &str = "\
Note: the first `/`-delimited segment of an hpath is NOT a notebook name \
— it is a top-level document title INSIDE the target notebook. (SiYuan \
has no folder concept — every path segment is a document.) The notebook \
is always supplied separately via the `notebook` parameter. \
Example: notebook `expnote`, hpath `/year2026/month12` means \
`expnote:/year2026/month12` (the notebook is `expnote`, the top-level \
document is `year2026`). Even when the notebook is named `hello` and \
the hpath is `/hello/world`, the first segment is still a document \
title: `hello[notebook]:/hello/world`.";
```

- [ ] **Step 2: Interpolate `HPATH_NOTE` into affected tool descriptions**

For each of these tools, append `\n\n{HPATH_NOTE}` to the description string using `format!()`:
- `syo_siyuan_doc_create` (line ~140)
- `syo_siyuan_doc_resolve` (line ~580, specifically in the hpath branch documentation)
- `syo_siyuan_doc_rename` (line ~630)
- `syo_siyuan_doc_move` (line ~680)
- `syo_siyuan_doc_remove` (line ~760)
- `syo_siyuan_doc_tree` (line ~720)

Each tool description string literal changes from a bare `"..."` to `format!("...\n\n{HPATH_NOTE}")`. Since the `reg!` macro expects a string literal for the description, change the description argument from a literal to a `&format!(...)` expression.

Example for `syo_siyuan_doc_create`:
```rust
        reg!(
            "syo_siyuan_doc_create",
            &format!("Create a new document in a notebook from GFM markdown.\n\
             \n\
             Sibling tools: ...\n\
             \n\
             {HPATH_NOTE}"),
            r#"..."#,
            ...
        );
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p syo-mcp 2>&1`
Expected: compiles cleanly

- [ ] **Step 4: Commit**

```bash
git add crates/syo-mcp/src/registry.rs
git commit -m "docs(syo-mcp): add hpath clarification note to all notebook+hpath tools

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: Add hpath clarification note to CLI commands

**Files:**
- Modify: `crates/syo-cli/src/commands/doc/mod.rs`
- Modify: `crates/syo-cli/src/commands/doc/create.rs`
- Modify: `crates/syo-cli/src/commands/doc/resolve.rs`
- Modify: `crates/syo-cli/src/commands/doc/rename.rs`
- Modify: `crates/syo-cli/src/commands/doc/move.rs`
- Modify: `crates/syo-cli/src/commands/doc/remove.rs`
- Modify: `crates/syo-cli/src/commands/doc/tree.rs`

- [ ] **Step 1: Add `HPATH_NOTE` constant in `doc/mod.rs`**

After imports, add:

```rust
pub(crate) const HPATH_NOTE: &str = "\
Note: the first `/`-delimited segment of an hpath is NOT a notebook name \
— it is a top-level document title INSIDE the target notebook. (SiYuan \
has no folder concept — every path segment is a document.) The notebook \
is always supplied separately via the `notebook` parameter. \
Example: notebook `expnote`, hpath `/year2026/month12` means \
`expnote:/year2026/month12` (the notebook is `expnote`, the top-level \
document is `year2026`). Even when the notebook is named `hello` and \
the hpath is `/hello/world`, the first segment is still a document \
title: `hello[notebook]:/hello/world`.";
```

This constant serves as the canonical source of the note text.

- [ ] **Step 2: Add the note to each args struct's doc comment**

For each of these files, append the hpath note to the existing struct-level doc comment. The note text is inlined because `#[doc = super::HPATH_NOTE]` does not resolve in const-attr context on stable Rust.

Files to update:

**`create.rs`** — `CreateDocArgs`: Add at end of existing `/// ...` doc comment:
```
/// ...
/// Note: the first `/`-delimited segment of an hpath is NOT a notebook
/// name — it is a top-level document title INSIDE the target notebook.
/// (SiYuan has no folder concept — every path segment is a document.)
/// The notebook is always supplied separately via `--notebook`.
/// Example: notebook `expnote`, hpath `/year2026/month12` means
/// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
/// `/hello/world`, the first segment is still a document title:
/// `hello[notebook]:/hello/world`.
```

**`resolve.rs`** — `ResolveArgs`: same note appended.

**`rename.rs`** — `RenameArgs`: same note appended.

**`move.rs`** — `MoveArgs`: same note appended.

**`remove.rs`** — `RemoveArgs`: same note appended.

**`tree.rs`** — `TreeArgs`: same note appended.

- [ ] **Step 3: Build and verify help output**

Run:
```bash
cargo run -- doc create --help 2>&1
```
Expected: help output includes the hpath note.

- [ ] **Step 4: Commit**

```bash
git add crates/syo-cli/src/commands/doc/
git commit -m "docs(syo-cli): add hpath clarification note to doc subcommand help

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: Final build and integration test check

- [ ] **Step 1: Full workspace build**

Run: `cargo build 2>&1`
Expected: entire workspace compiles cleanly

- [ ] **Step 2: Run all unit tests**

Run: `cargo test --lib 2>&1`
Expected: all unit tests pass

- [ ] **Step 3: Verify no dead code warnings**

Run: `cargo build 2>&1 | grep -i warning`
Expected: no warnings (or only pre-existing warnings unrelated to this change)

- [ ] **Step 4: Commit any remaining changes**

```bash
git status
git add -A
git commit -m "chore: final cleanup after search/notebook/hpath changes

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```
