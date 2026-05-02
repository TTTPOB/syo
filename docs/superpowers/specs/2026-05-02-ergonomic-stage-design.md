# Ergonomic Stage — Design

Date: 2026-05-02
Scope: CLI + MCP. Pre-v1 internal harness; breaking changes are acceptable.

## Motivation

A round of dogfooding surfaced consistency gaps in the doc-locator surface,
output formats, and a few stale "v1 NOT SUPPORTED" stubs. Three classes of pain:

1. **Storage-path leakage.** `doc rename / remove / move` (CLI and MCP) require
   the on-disk `.sy` storage path even though every other doc-touching command
   accepts a block id. Storage paths look like
   `/20260416133256-zjie5n0/20260416133307-hy6bv84.sy` — unreadable, and forces
   a `doc resolve` round trip every time.
2. **Output format drift.** `get-doc` / `get-block` ship with `--format
   agent-md|json|json-pretty`. `notebook ls`, `tag ls/search`, `search
   text/blocks` print TSV with no opt-in JSON. `sql` and `doc resolve` are
   JSON-only. Four read commands, four format conventions.
3. **Missing list-doc-tree primitive.** The original ask: there is no command to
   list documents under a notebook/folder. Browsing today goes through raw
   `siyuan sql`, which is wrong tool for navigation.

Plus four small fixes: `tag search --limit`, `move-block` runtime-failure
position kinds, `get-doc` 404-vs-empty error text, and the `sql` command's
unverified read-only claim.

## Design

The stage breaks into seven items (A–G). Each is independently shippable so
implementation can land as atomic commits with semantic messages.

### A. New `doc tree` command

**Purpose.** Single source of truth for "what documents live under here". One
command for both filetree navigation (depth=1) and full-subtree dumps
(depth=all).

**Surface.**

CLI:
```
siyuan doc tree --id <doc_id>                   [--depth N|all] [--format ...]
siyuan doc tree --notebook <nb_id> [--hpath /]  [--depth N|all] [--format ...]
```
- Address modes mutually exclusive (`clap::ArgGroup`), mirrors `doc resolve`.
- `--hpath` defaults to `/` when in `--notebook` mode.
- `--depth` integer or literal `all`. Default `1`. `0` is rejected.
- `--format`: `agent-md` (default), `json`, `json-pretty`.

MCP: new tool `siyuan_doc_tree` with the same input shape (`id` XOR
`notebook[+hpath]` plus `depth`). Returns `{ tree: ... }` wrapped via
`with_hint` for the standard envelope.

**Node payload (full set).** For each doc in the tree:
```
{ id, title, hpath, has_children, doc_count_recursive,
  created, updated, sort, icon,
  notebook_id, notebook_name, storage_path }
```
- `has_children` — boolean.
- `doc_count_recursive` — count of all descendant docs (type='d') under this
  node, regardless of slice depth. Computed in-memory from the loaded subtree.
- `storage_path` — included for parity with `doc resolve` output. Internal
  callers route through it; users can ignore it.
- `notebook_name` — empty if the notebook id is not in the live notebook list,
  same as `doc resolve`.

**Address semantics.**
- `--id X` mode: tree root is X itself. Output includes X plus `--depth` levels
  of descendants. `X` must be `type='d'` — non-doc block ids return `NotFound`.
- `--notebook X --hpath /` mode: virtual root (no doc), output is the top-level
  docs in the notebook plus `--depth - 1` further levels. The virtual root has
  empty `id`/`title`/`hpath=/` so the output shape is uniform.
- `--notebook X --hpath /Foo` mode: tree root is the doc at `/Foo`. Same shape
  as `--id` mode after the resolve.

**agent-md format.** Indented bullet list per level, `<!-- sy:doc id=... -->`
HTML-comment markers per node (mirrors existing `get-block`/`get-doc` style).
Trailing comment summarises depth/total when partial.

**Implementation backend.** New module `siyuan-model/src/doc_tree.rs`. Single
SQL pull for the whole subtree:
```sql
SELECT id, hpath, path, sort, created, updated, ial
FROM blocks
WHERE box = ? AND type = 'd' AND (path = ? OR path LIKE '<prefix>/%')
ORDER BY path
```
Build the tree in-memory from `path` parent-prefix relationships, slice to
`depth`, compute `doc_count_recursive` from the full preload. Notebook name
join via the `lsNotebooks` map already used by `doc_meta::resolve`.

### B. Dual-mode locator for `doc rename / remove / move`

Replace storage-path-only inputs with `--id` XOR `(--notebook --hpath)` (or the
multi-source variant for `move`). CLI internally resolves to `(notebook_id,
storage_path)` before calling the kernel.

**New helper.** `siyuan-model/src/doc_meta.rs` grows
```rust
pub async fn resolve_one_storage(
    client: &SiyuanClient,
    lookup: DocLookup,
) -> Result<(NotebookId, String), SiyuanError>
```
Returns `NotFound` on 0 hits, `AmbiguousPath` on >1 hits. Reuses
`doc_meta::resolve` internally.

**CLI surface change.**

| Command | Before | After |
|---|---|---|
| `doc rename` | `--notebook --path --title` | `(--id \| --notebook --hpath) --title` |
| `doc remove` | `--notebook --path` | `--id \| --notebook --hpath` |
| `doc move`   | `--from-paths ... --to-notebook --to-path` | `(--from-ids ... \| --notebook --from-hpaths ...) --to-notebook --to-path` |

For `doc move`'s batch input, the two address modes (`--from-ids` and
`--notebook --from-hpaths`) are mutually exclusive. Resolving N sources runs N
sequential `resolve_one_storage` calls — N is small in practice. `--to-path`
remains the destination FOLDER specified in hpath form (e.g. `/Projects` or
`/`); folders have no `.sy` suffix so hpath and storage path coincide for
folder targets, and the help text states this explicitly.

**MCP mirror.** `siyuan_doc_rename`, `siyuan_doc_remove`, `siyuan_doc_move`
take the same shape. Drop `path` and `from_paths` from inputs entirely. The
existing `parse_doc_lookup` helper is the foundation; add a `parse_doc_lookup_batch`
sibling for `siyuan_doc_move`'s `from_ids` / `from_hpaths` array. Empty arrays
remain rejected (existing behaviour preserved).

**`storage_path` retention.** Keep the field on `ResolvedDoc` (so `doc resolve`
output stays useful for power users wiring up `curl`), but it is no longer an
input to any other command.

### C. `--format` propagated to remaining read commands

Add `--format <agent-md|json|json-pretty>` (existing `OutputFormat` enum) to:

- `notebook ls`
- `tag ls`
- `tag search`
- `search text`
- `search blocks`
- `doc resolve`

Default for each preserves the current behaviour exactly:

- `notebook ls` / `tag ls` / `tag search` / `search text` / `search blocks` —
  default is the existing TSV / one-per-line shape (treat as the "agent-md"
  variant for these commands; no new prose form is invented). `--format json`
  emits a compact JSON array; `--format json-pretty` emits the indented form.
- `doc resolve` — default stays `json-pretty` (current behaviour). `--format
  json` opts into the compact form. There is no `agent-md` variant for
  `resolve` since it is already structured.

The `OutputFormat` enum gets used as-is; per-command `default_value_t` selects
the correct default. `sql` is intentionally NOT touched — it is structurally
JSON and adding `agent-md` to it adds no value.

MCP is not affected: MCP outputs are already structured JSON.

### D. `tag search --limit`

Add `--limit usize` (default 50, capped by `MAX_SEARCH_LIMIT`) to `siyuan tag
search` and the underlying `siyuan_model::tag::search_by_tag`. MCP
`siyuan_tag_search` mirror.

### E. `move-block` position kinds — clap ValueEnum

Replace the string match in `move_block.rs` with a `ValueEnum`-derived enum
that lists ONLY the supported kinds:

```
after_block, append_child, prepend_child, append_doc, prepend_doc
```

`before_block`, `append_section`, `prepend_section` are removed from the
accepted set; clap rejects them at parse time. Help text updates the
"NOT SUPPORTED in v1" notes to "see `insert-blocks` for those kinds".

The runtime `bail!` arms stay as defense-in-depth.

MCP `siyuan_move_block` mirrors with the same restricted enum: argument
validation in `siyuan-mcp/src/tools/block.rs` (or wherever the move-block
parser lives) rejects the three unsupported kinds at parse time and returns
`invalid_params` with a "see `siyuan_insert_block` for those kinds" hint.

### F. `get-doc` NotFound vs empty doc

In `siyuan-model/src/load.rs`, replace
```rust
bail!("doc {} has no blocks (does it exist?)", doc_id);
```
with
```rust
return Err(SiyuanError::NotFound(doc_id.to_string()).into());
```

A real document always has at least its root row (`root_id = id` self-reference),
so 0 rows means "no such doc". The misleading "empty doc" branch is dead code
in practice.

The existing test in `crates/siyuan-cli/tests/pagination_errors.rs:305` and
helper at `tests/common/mod.rs:65` need updating to match the new error.

### G. `sql` client-side read-only check

In `commands/sql.rs::run` and MCP `tools/sql.rs`, after the trim/blank check,
validate the leading keyword:

```rust
let head = stmt.trim_start().to_ascii_lowercase();
if !(head.starts_with("select") || head.starts_with("with")) {
    bail!("--stmt must be a read-only SELECT (or WITH) query");
}
```

Catches obvious abuse (`DROP TABLE …`, `INSERT INTO …`) before the kernel
round trip. Kernel rejection remains the authoritative gate.

### Help / docs / README updates

Every command above must update its `verbatim_doc_comment` clap docstring AND
its MCP tool description. Five elements per Decision #6 (what / when vs
neighbours / inputs / example / async-index lag note).

After all of A–G land, refresh:
- `README.md` (English) — sample commands with the new locator forms,
  reference to `doc tree`.
- `docs/readme/README.cn.md` — same updates.
- `docs/decisions.md` — append entry **#10 Ergonomic stage**: dual-mode
  locator, `doc tree`, format flag propagation; rationale, tradeoff,
  reversal trigger.

### Integration tests

Sit alongside existing patterns in `crates/siyuan-cli/tests/*.rs` — `--ignored
--test-threads=1` against a live kernel.

| Item | Test |
|---|---|
| A | `tests/doc_tree.rs` — create nested docs `/A/B/C`, hit `doc tree` at depth 1 / 2 / all from each address mode (id, notebook root, notebook+hpath); verify field set, `has_children`, `doc_count_recursive` |
| B | extend `tests/notebook_filetree.rs` — rename / remove / move via id mode AND hpath mode; verify storage-path mode is gone (clap rejects `--path`) |
| C | sample test: `notebook ls --format json` parses with `serde_json::from_str::<Vec<NotebookView>>` |
| D | `tag_search --limit 2` returns ≤ 2 hits when ≥ 3 exist |
| E | unit test in `move_block.rs` — clap rejects `before_block` at parse time |
| F | call `get-doc --id <bogus>` → assert `SiyuanError::NotFound` propagates |
| G | unit test: `sql --stmt "DROP TABLE blocks"` rejected client-side |

MCP coverage rides on the existing `tools/*.rs` unit-test pattern (dummy
client + arg validation) — extend `parse_doc_lookup` tests to cover the new
parameters.

## Out of scope

- Block-level children listing (the second meaning of "children" in SiYuan):
  remains under `get-block` / SQL. `doc tree` is filetree-only by design.
- Removing `storage_path` from `ResolvedDoc` output — breaks `doc resolve`
  consumers who pipe to `curl`. Marked as a possible future stage.
- `notebook open` / `notebook close` exposure — already settled in
  decisions.md #2 and #8.
- Sort order in `doc tree` — kernel `sort` attribute is included as a node
  field, but ordering nodes by it is left to the caller (jq, agent logic).
  Default backend order is by `path` (lexicographic), stable.

## Implementation order and commits

Each item is one atomic commit with a semantic message. Suggested order
matches dependency:

1. F — `fix(model): map missing doc to NotFound in load_doc`
2. G — `feat(cli,mcp): reject non-SELECT statements in sql command`
3. E — `refactor(cli,mcp): tighten move-block position kinds via ValueEnum`
4. D — `feat(cli,mcp): add --limit to tag search`
5. C — `feat(cli): add --format flag to notebook/tag/search/doc-resolve`
6. B — `feat(cli,mcp): accept id or hpath in doc rename/remove/move`
7. A — `feat(cli,mcp): add doc tree listing command`
8. README + decisions.md refresh — `docs: document ergonomic stage`

B precedes A so `resolve_one_storage` lands first; A reuses the same
notebook-name join helper but does not depend on the storage-path resolver,
so they can in principle land in either order — the chosen order minimises
review thrash.
