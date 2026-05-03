# syo-core: Common Intermediate Layer for CLI and MCP

## Problem

CLI (`syo`) and MCP (`syo-mcp`) independently implement the same semantic operations
on top of `siyuan-client` / `siyuan-model`. This has caused drift:

- `doc set-icon` / `doc set-sort` exist only in CLI; MCP uses generic `attrs_set`
- `attrs get` exists only in MCP; CLI has no `attrs get` command
- `search blocks` exists only in CLI; MCP must hand-roll SQL
- `graph backlinks` / `graph outgoing` convenience commands exist only in CLI;
  MCP only has `graph_neighborhood`
- `asset reference` pure formatter exists only in CLI
- `block move` supports 8 positions in CLI but only 2 in MCP

## Solution

Extract a `syo-core` crate that is the single source of truth for every domain
operation. CLI and MCP become thin wrappers that parse their respective input
formats (clap args / JSON), call `syo-core`, and format the output.

### New Workspace Layout

```
crates/
├── siyuan-types      (unchanged)
├── siyuan-client     (unchanged)
├── siyuan-model      (unchanged)
├── siyuan-render     (unchanged)
├── syo-core          NEW — shared operations layer
├── syo-cli           RENAMED from syo — CLI binary
├── syo-mcp           (unchanged — tool implementations thinned)
└── siyuan-testkit    (unchanged)
```

### Target Architecture

```
syo-cli (commands/)  ──→ syo-core ──→ siyuan-client / siyuan-model
syo-mcp (tools/)     ──→ syo-core ──→ siyuan-client / siyuan-model
```

## syo-core Crate Design

### Module Structure

```
crates/syo-core/src/
├── lib.rs
├── system.rs       // status()
├── doc.rs          // get, create, resolve, rename, move, remove, tree,
│                   //   set_icon, set_sort
├── block.rs        // get, update, insert, delete, move (8 positions)
├── notebook.rs     // ls, create, rename, remove
├── attr.rs         // get, set
├── graph.rs        // neighborhood, backlinks (convenience), outgoing (convenience)
├── search.rs       // fulltext, blocks (type+content filter)
├── tag.rs          // ls, search
├── asset.rs        // upload, reference
└── sql.rs          // raw (read-only guarded)
```

### Operation Signature Pattern

Every operation follows this shape:

```rust
// Typed input struct — the operation's contract
pub struct SetIconInput {
    pub id: BlockId,
    pub icon: String,
}

// Typed output struct — serializable, no presentation concerns
pub struct SetIconOutput;

// Pure async fn — no CLI/MCP knowledge, only client + input → output
pub async fn set_icon(client: &SiyuanClient, input: SetIconInput)
    -> Result<SetIconOutput>;
```

- **Input structs** are plain data (owned, `Debug`). No serde derives needed.
- **Output structs** derive `Serialize` so CLI and MCP can format them.
- **Errors** use `anyhow::Error` — the caller maps to its own error type.
- **No logging, no println, no exit** — pure library code.

### CLI Side (syo-cli)

Each command becomes:

1. clap `Args` → parse CLI flags
2. Construct typed `Input` from `Args`
3. Call `syo_core::<domain>::<op>(client, input).await?`
4. Format `Output` for terminal (e.g. `println!("ok")` or `serde_json::to_string_pretty`)

### MCP Side (syo-mcp)

Each tool becomes:

1. JSON `Value` → parse into typed `Input` (via `util` helpers)
2. Call `syo_core::<domain>::<op>(client, input).await`
3. Map error with `siyuan_to_mcp` / `anyhow_to_mcp`
4. Convert `Output` to `serde_json::Value` with `serde_json::to_value`
5. Wrap in `with_hint()` envelope

## Alignment Table (CLI as Standard)

| Operation | CLI before | MCP before | syo-core op | CLI after | MCP after |
|---|---|---|---|---|---|
| status | `status` cmd | `syo_siyuan_status` tool | `system::status` | via core | via core |
| notebook ls | `notebook ls` | `syo_siyuan_notebook_ls` | `notebook::ls` | via core | via core |
| notebook create | `notebook create` | `syo_siyuan_notebook_create` | `notebook::create` | via core | via core |
| notebook rename | `notebook rename` | `syo_siyuan_notebook_rename` | `notebook::rename` | via core | via core |
| notebook remove | `notebook remove` | `syo_siyuan_notebook_remove` | `notebook::remove` | via core | via core |
| doc get | `doc get` | `syo_siyuan_doc_get` | `doc::get` | via core | via core |
| doc create | `doc create` | `syo_siyuan_doc_create` | `doc::create` | via core | via core |
| doc resolve | `doc resolve` | `syo_siyuan_doc_resolve` | `doc::resolve` | via core | via core |
| doc rename | `doc rename` | `syo_siyuan_doc_rename` | `doc::rename` | via core | via core |
| doc move | `doc move` | `syo_siyuan_doc_move` | `doc::move` | via core | via core |
| doc remove | `doc remove` | `syo_siyuan_doc_remove` | `doc::remove` | via core | via core |
| doc tree | `doc tree` | `syo_siyuan_doc_tree` | `doc::tree` | via core | via core |
| doc set-icon | `doc set-icon` | (missing) | `doc::set_icon` | via core | NEW dedicated tool |
| doc set-sort | `doc set-sort` | (missing) | `doc::set_sort` | via core | NEW dedicated tool |
| block get | `block get` | `syo_siyuan_block_get` | `block::get` | via core | via core |
| block update | `block update` | `syo_siyuan_block_update` | `block::update` | via core | via core |
| block insert | `block insert` | `syo_siyuan_block_insert` | `block::insert` | via core | via core |
| block delete | `block delete` | `syo_siyuan_block_delete` | `block::delete` | via core | via core |
| block move | `block move` (8 pos) | `syo_siyuan_block_move` (2 pos) | `block::move` (8 pos) | via core | via core (extend to 8) |
| attr get | (missing) | `syo_siyuan_attrs_get` | `attr::get` | NEW `attrs get` cmd | via core |
| attr set | `attrs set` | `syo_siyuan_attrs_set` | `attr::set` | via core | via core |
| search blocks | `search blocks` | (missing) | `search::blocks` | via core | NEW dedicated tool |
| search text | `search text` | `syo_siyuan_search_text` | `search::fulltext` | via core | via core |
| tag ls | `tag ls` | `syo_siyuan_tag_ls` | `tag::ls` | via core | via core |
| tag search | `tag search` | `syo_siyuan_tag_search` | `tag::search` | via core | via core |
| sql | `sql` | `syo_siyuan_sql` | `sql::raw` | via core | via core |
| asset upload | `asset upload` | `syo_siyuan_asset_upload` | `asset::upload` | via core | via core |
| asset reference | `asset reference` | (missing) | `asset::reference` | via core | NEW dedicated tool |
| graph neighborhood | `graph neighborhood` | `syo_siyuan_graph_neighborhood` | `graph::neighborhood` | via core | via core |
| graph backlinks | `graph backlinks` | (missing) | `graph::backlinks` | via core (convenience) | NEW dedicated tool |
| graph outgoing | `graph outgoing` | (missing) | `graph::outgoing` | via core (convenience) | NEW dedicated tool |

## What Stays Where

### siyuan-model (existing code preserved)
- `graph::neighborhood()` — the BFS traversal engine; syo-core calls it
- `load::load_doc()` — document loading
- `doc_tree::doc_tree()` — file tree walking
- `pagination`, `section`, `bundle`, `relations`, `tag`, `sql_guard`

### syo-core (new code)
- All domain operations: typed input/output structs + execute functions
- Calls `siyuan-client` directly for single-API-call ops
- Calls `siyuan-model` for composite ops (graph, doc_tree, section resolution)
- `graph::backlinks` / `graph::outgoing` as depth=1 convenience wrappers
  around `neighborhood`
- `asset::reference` as a pure formatting function (no client needed)
- `search::blocks` SQL builder (type + content filter, currently in
  CLI `commands/search/blocks.rs`)

### syo-cli (thinned)
- clap argument structs and `run` functions
- Argument parsing and conversion to syo-core Input types
- Output formatting (terminal-appropriate)
- `serve_mcp` command (not in syo-core — MCP doesn't need it)

### syo-mcp (thinned)
- MCP schema strings in `registry.rs` (JSON Schema for tool inputs)
- JSON `Value` → Input type parsing
- Output → `Value` conversion + `with_hint` envelope
- Error mapping (`siyuan_to_mcp`, `anyhow_to_mcp`)
- `resolve_section_end` (currently duplicated from CLI — moves to syo-core)

## Implementation Order (Sequential)

1. **Create `syo-core` crate** — empty scaffold, Cargo.toml, lib.rs
2. **Move operations into syo-core** — one domain at a time, starting with
   simple ops (system, notebook, attr) then complex ones (block, doc, search,
   graph, asset)
3. **Rename `syo` → `syo-cli`** — update crate name, directory, Cargo.toml,
   all internal references
4. **Rewire CLI commands** — each command calls syo-core instead of
   client/model directly
5. **Rewire MCP tools** — each tool calls syo-core instead of client/model
   directly
6. **Add missing MCP tools** — set_icon, set_sort, search_blocks, backlinks,
   outgoing, asset_reference
7. **Add missing CLI command** — `attrs get`
8. **Extend MCP block_move** — from 2 positions to full 8
9. **Integration tests** — ensure all tests pass

## Non-Goals

- `serve-mcp` stays in syo-cli, not extracted to syo-core
- siyuan-model is not refactored — syo-core calls it, doesn't replace it
- No API breaking changes to MCP tool names or schemas (additions only)
- No behavior changes to existing operations — pure refactoring + gap fill
