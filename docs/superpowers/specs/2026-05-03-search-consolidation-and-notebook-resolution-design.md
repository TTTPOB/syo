# Search consolidation, notebook name resolution, and hpath clarification

## Scope

Three tightly-scoped changes to the `syo` CLI and MCP server:

1. **Search consolidation** — remove `search text` subcommand and `search blocks`
   subcommand; the `search` command (and `syo_siyuan_search` MCP tool) directly
   exposes the old `search blocks` functionality.
2. **Notebook name resolution** — every parameter that previously required a
   notebook id (`\d{14}-[0-9a-z]{7}`) also accepts a notebook display name.
   Duplicate names are rejected with a diagnostic listing all matching ids.
3. **Hpath clarification** — add help text to every MCP tool and CLI command
   that involves an hpath, explaining that the first `/`-delimited segment is
   NOT a notebook name.

## Motivation

- **Search**: Two search subcommands (fulltext `markdown` vs `type`+`content`)
  confuse agents. The fulltext variant searches raw markdown (includes syntax
  markers) which overlaps heavily with `syo_siyuan_sql`; the type+content
  variant is the more useful one. Consolidating eliminates a decision point.
- **Notebook names**: Agents frequently have a human-readable notebook name
  (from `syo_siyuan_notebook_ls`) but tools demand an id. Forcing an extra
  `doc_resolve` round-trip or manual id transcription is friction that the
  harness can eliminate.
- **Hpath confusion**: Agents routinely misinterpret the first hpath segment
  (`/hello/world`) as a notebook name rather than a folder inside the notebook.
  The tool descriptions must preempt this.

## Current architecture (relevant parts)

```
syo-cli (main.rs + commands/)
  ├── search/mod.rs        → SearchCmd enum { Text, Blocks }
  ├── search/text.rs       → calls syo_core::search::fulltext
  ├── search/blocks.rs     → calls syo_core::search::blocks
  ├── doc/create.rs        → NotebookId::parse(&args.notebook)
  ├── doc/lookup.rs        → NotebookId::parse() for hpath-branch
  ├── notebook/rename.rs   → NotebookId::parse(&args.id)
  └── notebook/remove.rs   → NotebookId::parse(&args.id)

syo-core
  ├── search.rs            → fulltext() + blocks() + SearchHit/FulltextInput/BlocksInput
  └── notebook.rs          → ls / create / rename / remove (no name→id resolver)

syo-mcp (registry.rs + tools/)
  ├── registry.rs          → syo_siyuan_search_text + syo_siyuan_search_blocks tools
  ├── tools/sql.rs         → search_text() + search_blocks() handlers
  ├── tools/doc.rs         → create_doc parses notebook via NotebookId::parse
  ├── tools/filetree.rs    → parse_notebook_id() (pure), parse_doc_lookup() etc.
  └── tools/notebook.rs    → parse_notebook_id() (pure), rename/remove handlers
```

## Design

### 1. Search consolidation

#### syo-core

- Remove `FulltextInput` struct and `fulltext()` function from `search.rs`.
- Rename `BlocksInput` → `SearchInput`; `blocks()` → `search()`.
- `SearchHit` and `SearchOutput` unchanged (they are already shared; the
  rename from `type`→`block_type` field alias still applies).
- `search::blocks` tests move to `search::search` equivalents; `fulltext`
  tests are deleted (their coverage is subsumed by the MCP/CLI integration
  tests).

#### syo-cli

- Delete `commands/search/text.rs` and `commands/search/blocks.rs`.
- `commands/search/mod.rs`:
  - Remove `SearchCmd` enum.
  - Inline the old `blocks::Args` as `SearchArgs` (fields: `--type`,
    `--contains`, `--limit`, `--format`).
  - `run()` calls `syo_core::search::search()` directly.
- `main.rs`: `Cmd::Search` no longer wraps a subcommand — it carries
  `SearchArgs` directly (same pattern as `Cmd::Sql(SqlArgs)`).
- `commands/search/hit.rs` stays (shared output formatting).

#### syo-mcp

- Remove `syo_siyuan_search_text` and `syo_siyuan_search_blocks` from
  `registry.rs`.
- Register a single `syo_siyuan_search` tool with the schema:
  ```json
  {
    "type": "object",
    "properties": {
      "type":     { "type": "string", "description": "Block type code (e.g. p, h, d)" },
      "contains": { "type": "string", "description": "Substring match against content column" },
      "limit":    { "type": "integer", "default": 50 }
    },
    "additionalProperties": true
  }
  ```
- In `tools/sql.rs`: remove `search_text()`, rename `search_blocks()` →
  `search()`, update to call `syo_core::search::search()`.
- Tool description includes a note that this replaces the deprecated
  `search_text` / `search_blocks` split.

#### Tests

- `crates/syo-cli/tests/search.rs`: update snapshots / assertions to match
  the new flat `syo search --type ... --contains ...` command shape.
- `crates/syo-mcp` tests in `tools/sql.rs`: `search_text` tests removed;
  `search_blocks`/`search` tests updated.

### 2. Notebook name resolution

#### New function in `syo-core::notebook`

```rust
/// Resolve a user-supplied string to a [`NotebookId`].
///
/// If `input` matches the notebook-id format (`\d{14}-[0-9a-z]{7}`) it is
/// returned immediately as a [`NotebookId`] — no network call is made.
/// Otherwise the function calls `ls_notebooks()` and matches by **exact**
/// display name:
///
/// - 0 matches → error `notebook {name:?} not found`
/// - 1 match  → returns that notebook's id
/// - >1 match → error listing all matching `(id, name)` pairs so the caller
///   can disambiguate by id
pub async fn resolve_notebook_id(
    client: &SiyuanClient,
    input: &str,
) -> Result<NotebookId>
```

Placement: `crates/syo-core/src/notebook.rs`.

#### Error variants

The existing `SiyuanError` enum already has `NotFound(String)`. Two new
variants are needed (in `siyuan-types/src/error.rs`):

```rust
#[error("notebook {name:?} not found")]
NotebookNotFound { name: String },

#[error("ambiguous notebook name {name:?} — matches: {candidates}")]
AmbiguousNotebook { name: String, candidates: String },
```

Or, to keep the error surface smaller, use `SiyuanError::Other(msg)`. But
structured errors let the MCP layer produce better `invalid_params` messages.
Decision deferred to implementation — start with `Other` and promote to
variants if the MCP layer needs programmatic access.

#### CLI update sites

Every place that calls `NotebookId::parse(&args.xxx)` gains a preceding
async resolution step. Specifically:

| File | Current | New |
|------|---------|-----|
| `commands/doc/create.rs:56` | `NotebookId::parse(&args.notebook)` | `resolve_notebook_id(client, &args.notebook).await` |
| `commands/doc/lookup.rs:19` | `NotebookId::parse(nb.trim())` | `resolve_notebook_id(client, nb.trim()).await` |
| `commands/notebook/rename.rs:18` | `NotebookId::parse(&args.id)` | `resolve_notebook_id(client, &args.id).await` |
| `commands/notebook/remove.rs:15` | `NotebookId::parse(&args.id)` | `resolve_notebook_id(client, &args.id).await` |

`commands/doc/lookup.rs` becomes async (currently it is a pure `fn`). All
callers of `build_single_doc_lookup` — `rename.rs`, `remove.rs`, `resolve.rs`,
`tree.rs` — are already in async contexts so this is a signature change only.

#### MCP update sites

**Strategy**: MCP handler functions resolve the user-supplied notebook string
to a `NotebookId` BEFORE passing it to the existing pure-parser helpers. The
pure parsers (`parse_doc_lookup`, `parse_tree_lookup`, `parse_doc_lookup_batch`)
continue to accept `NotebookId` values and remain synchronous and unit-testable.

Concrete changes:

| File | Change |
|------|--------|
| `tools/doc.rs` `create_doc()` | Replace `NotebookId::parse(&notebook_str)?` with `syo_core::notebook::resolve_notebook_id(client, &notebook_str).await` |
| `tools/filetree.rs` `resolve()` | Extract `notebook` from map, resolve name→id, put resolved id string back into map, then call `parse_doc_lookup()` |
| `tools/filetree.rs` `rename_doc()` | Same pattern |
| `tools/filetree.rs` `move_doc()` | Same pattern (both `notebook` and `to_notebook`) |
| `tools/filetree.rs` `remove_doc()` | Same pattern |
| `tools/filetree.rs` `tree()` | Same pattern |
| `tools/notebook.rs` `rename()` | Replace `parse_notebook_id()` with `syo_core::notebook::resolve_notebook_id()` |
| `tools/notebook.rs` `remove()` | Same |

The local `parse_notebook_id()` helper in both `filetree.rs` and
`notebook.rs` is kept for pure-format validation (e.g., in tests), but
handlers use `resolve_notebook_id()`.

### 3. Hpath clarification notice

Define a single constant (e.g. in `syo-mcp/src/registry.rs` and in
`syo-cli/src/commands/doc/mod.rs`) so the note text lives in one place:

```rust
const HPATH_NOTE: &str = "\
Note: the first `/`-delimited segment of an hpath is NOT a notebook name \
— it is the top-level folder INSIDE the target notebook. The notebook is \
always supplied separately via the `notebook` parameter. \
Example: notebook `expnote`, hpath `/year2026/month12` means \
`expnote:/year2026/month12`. Even when the notebook is named `hello` and \
the hpath is `/hello/world`, the first segment is still a folder: \
`hello[notebook]:/hello/world`.";
```

This constant is interpolated into every MCP tool description and CLI
command help that involves a `notebook` + `hpath` pair.

Affected MCP tools:
- `syo_siyuan_doc_create`
- `syo_siyuan_doc_resolve`
- `syo_siyuan_doc_rename`
- `syo_siyuan_doc_move`
- `syo_siyuan_doc_remove`
- `syo_siyuan_doc_tree`

Affected CLI commands (via doc comments on the arg structs):
- `syo doc create --hpath`
- `syo doc resolve --hpath`
- `syo doc rename --hpath`
- `syo doc move --from-hpaths`
- `syo doc remove --hpath`
- `syo doc tree --hpath`

The hpath note is also added to the `search` command's help for the `--type` /
`--contains` arguments where relevant (actually no — search doesn't take
notebook+hpath; only doc commands do).

## Data flow (notebook resolution)

```
User/Agent input: "Inbox" (a notebook name)
        │
        ▼
resolve_notebook_id(client, "Inbox")
        │
        ├── NotebookId::parse("Inbox") → Err (not an id format)
        │
        ├── client.ls_notebooks() → [ {id: "2025...-nb01", name: "Inbox"}, ... ]
        │
        ├── filter by name == "Inbox" → exactly 1 match
        │
        └── return NotebookId("20250101000000-nb00001")

User/Agent input: "20250101000000-nb00001" (already an id)
        │
        ▼
resolve_notebook_id(client, "20250101000000-nb00001")
        │
        └── NotebookId::parse(...) → Ok(id) → return immediately (no network)
```

## Error handling

- **Search**: empty `--type` AND empty `--contains` is valid (selects all
  blocks up to `--limit`). Empty/whitespace validation for `--contains` is
  not needed (unlike `search text --query` which required non-empty).
- **Notebook resolution**: `resolve_notebook_id` surfaces errors as
  `anyhow::Error` via `bail!()`. MCP layer converts to `invalid_params`.
  CLI layer propagates via `?` / `.context()`.
- **Duplicate notebook names**: rejected with a message listing all matching
  `(id, name)` pairs. The agent can then pick one by id.

## Testing strategy

- **Unit**: `resolve_notebook_id` with a mock client (or a real client pointed
  at the test container — follow the existing dummy-client pattern from
  `filetree.rs` tests).
- **Integration**: `crates/syo-cli/tests/search.rs` updated for new `syo search`
  shape. Notebook name resolution exercised through the existing doc-create
  integration test path.
- **MCP handler tests**: existing `tools/sql.rs` tests updated; new tests for
  notebook name→id resolution in parameter validation.

## Rollback / compatibility

- `syo_siyuan_search_text` and `syo_siyuan_search_blocks` are REMOVED from the
  MCP tool list. Agents using the old tool names will get `unknown tool` errors
  — the new `syo_siyuan_search` name makes the migration explicit.
- CLI: `syo search text --query ...` becomes an error. Users migrate to
  `syo search --contains ...` or `syo sql --stmt "SELECT ... LIKE ..."`.
- The `syo_siyuan_search` MCP tool has the identical parameter schema to the
  old `syo_siyuan_search_blocks`, so migration is a one-word rename in agent
  tool-calling code.
