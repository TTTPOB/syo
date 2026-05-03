# siyuan-cli

An agent-friendly harness for [SiYuan](https://github.com/siyuan-note/siyuan), built around its kernel HTTP API. Ships **one binary** (`siyuan`) on top of a typed Rust client. The same binary serves a CLI for human/script use and an MCP (Model Context Protocol) server for LLM agents — invoked as `siyuan serve-mcp`.

Library crates (`siyuan-types`, `siyuan-client`, `siyuan-model`, `siyuan-render`) are reusable independently of the binary.

> 🇨🇳 中文版本见 [`docs/readme/README.cn.md`](docs/readme/README.cn.md).
> 🧭 Design tradeoffs and reversal triggers are logged in [`docs/decisions.md`](docs/decisions.md).

## Status

v1, single-workspace, single-user. Targets the SiYuan kernel HTTP API as of 2026-05.
The kernel itself is the source of truth — there is no local cache, snapshot token, or two-phase commit. SQL-indexed reads (search, tag, raw `siyuan_sql`) are eventually consistent and may lag mutations by ~100–500 ms.

## Prerequisites

- A running SiYuan kernel reachable over HTTP (default `http://127.0.0.1:6806`).
- An API token. Set it under *Settings → About → API token* in the SiYuan UI.
- Rust toolchain ≥ 1.85 (workspace pins `edition = "2024"`).
- (Optional) Podman, only if you want to run the integration tests in `siyuan-testkit`.

## Build

```sh
cargo build --release
# binary lands in target/release/siyuan
./target/release/siyuan --help
```

For local hacking `cargo run -p siyuan-cli -- <args>` works too.

## Configure

| Variable            | Default                  | Notes                                                         |
| ------------------- | ------------------------ | ------------------------------------------------------------- |
| `SIYUAN_BASE_URL`   | `http://127.0.0.1:6806`  | Kernel HTTP root. Override with `--base-url`.                 |
| `SIYUAN_TOKEN`      | *(required)*             | Sent as `Authorization: Token <value>`. Required for every subcommand except `serve-mcp`, where it can be injected at MCP request time instead. |
| `SIYUAN_TIMEOUT_MS` | `30000` (`serve-mcp`)    | Per-request timeout for the MCP server. `0` disables it. Other subcommands use the client default. |
| `RUST_LOG`          | `info`                   | Standard `tracing-subscriber` filter; logs always go to stderr (so `serve-mcp` doesn't pollute its stdio JSON-RPC channel). |

The CLI also accepts `--base-url` / `--token` as global flags that override the env.

## CLI usage

Smoke test:

```sh
export SIYUAN_TOKEN=...your-token...
siyuan status
# prints the kernel version, e.g. 3.1.x
```

The CLI is organised as flat commands plus a few subcommand groups. Run `siyuan --help` (and `siyuan <cmd> --help`) for the full list — quick tour:

```sh
# Notebooks
siyuan notebook ls
siyuan notebook create --name "Inbox"
# (notebook open/close are not exposed; use the SiYuan UI if you need them)

# Resolve a document — accepts EITHER --id OR (--notebook + --hpath)
siyuan doc resolve --id 20260501090000-doc0001
siyuan doc resolve --notebook 20260501000000-nb00001 --hpath "/Projects/Plan"
# Output is a JSON array of { id, hpath, notebook_id, notebook_name, title, storage_path }

# List a notebook/folder subtree as a tree
# (--depth defaults to 1; pass an integer or `all`. --format defaults to agent-md.)
siyuan doc tree --notebook 20260501000000-nb00001                                # top-level docs
siyuan doc tree --notebook 20260501000000-nb00001 --hpath /Projects --depth all
siyuan doc tree --id 20260501090000-doc0001 --depth 2 --format json-pretty

# Doc filetree mutations — accept EITHER --id OR (--notebook + --hpath).
# Storage `.sy` paths are NOT accepted; the CLI resolves them internally.
siyuan doc rename --id 20260501090000-doc0001 --title "Q3 Plan"
siyuan doc rename --notebook 20260501000000-nb00001 --hpath "/Projects/Plan" --title "Q3 Plan"
siyuan doc remove --id 20260501090000-doc0001
siyuan doc remove --notebook 20260501000000-nb00001 --hpath "/Projects/Plan"
# `doc move`: source is --from-ids XOR (--notebook --from-hpaths); destination is hpath form.
siyuan doc move --from-ids 20260501090000-doc0001 \
  --to-notebook 20260501000000-nb00002 --to-path /Archive
siyuan doc move --notebook 20260501000000-nb00001 --from-hpaths /Plan /Notes \
  --to-notebook 20260501000000-nb00002 --to-path /Archive

# Read a doc as agent-readable markdown (default), or as JSON
siyuan doc get --id 20260501090000-doc0001
siyuan doc get --id 20260501090000-doc0001 --format json-pretty
siyuan doc get --id 20260501090000-doc0001 --page 2 --page-size 50

# Read a single block's raw kramdown
siyuan block get --id 20260501090000-blk0001

# Create a doc from a markdown file (or stdin via `-`)
siyuan doc create \
  --notebook 20260501000000-nb00001 \
  --hpath "/Projects/New Page" \
  --markdown-file ./page.md

# Block writes
siyuan block update   --id <block-id> --markdown-file ./new.md
siyuan block insert  --position after_block --anchor <block-id> --markdown-file ./snippet.md
siyuan block move     --id <block-id> --position append_child --anchor <container-id>
siyuan block delete   --id <block-id>
# Note: document root blocks (type='d') are rejected by block delete; use `siyuan doc remove` to delete a document.

# Attributes (custom keys must be `custom-...`; empty value clears a key)
siyuan attrs set --id <block-id> --attr custom-status=done --attr custom-owner=alice

# Tags & search
siyuan tag ls
siyuan tag search --tag project
siyuan search text   --query "load_doc" --limit 20
siyuan search blocks --type h --contains "Roadmap"

# Raw SQL escape hatch (read-only; caller escapes single quotes)
siyuan sql --stmt "SELECT id, hpath FROM blocks WHERE type = 'd' LIMIT 5"

# Link graph (BFS up to N hops, capped at 500 nodes / 1000 edges)
siyuan graph backlinks    --id <block-id>
siyuan graph outgoing     --id <block-id>
siyuan graph neighborhood --id <block-id> --depth 2 --direction both

# Assets
siyuan asset upload    --file ./diagram.png
siyuan asset reference --path assets/diagram-20260501-abc.png --alt "Diagram"
```

### Output formats

`doc get` and `block get` accept `--format`:

- `agent-md` *(default)* — markdown with `<!-- sy:doc … -->` / `<!-- sy:block … -->` HTML-comment markers carrying ids, types and pagination metadata. Designed for LLMs to round-trip reads back into block-targeted writes.
- `json` / `json-pretty` — the canonical structured bundle (`DocBundle`) including full block metadata.
  When a doc spans multiple pages, `agent-md` output includes a `<!-- sy:page X/Y blocks remaining: Z -->` footer after the last rendered block.

`notebook ls`, `tag ls`, `tag search`, `search text`, and `search blocks` also accept `--format`. Their default `agent-md` is the legacy TSV (or one-per-line for `tag ls`) for backward compatibility; `json` / `json-pretty` emit a structured array of the same fields (`{status,id,name}`, tag strings, `{block_id,markdown_preview}`, or `{id,type,markdown_preview}` respectively).

`doc resolve` accepts `--format json` / `--format json-pretty` (default `json-pretty`); `agent-md` is rejected because the output is structured metadata. `sql` always emits pretty JSON. Mutating commands print a single id or `ok`.

### Position kinds

`block insert` and `block move` share these `--position` values:

| kind             | meaning                                  | anchor                |
| ---------------- | ---------------------------------------- | --------------------- |
| `after_block`    | as a sibling immediately after anchor    | block id              |
| `before_block`   | as a sibling immediately before anchor   | block id              |
| `append_child`   | as the last child of the container       | container block id    |
| `prepend_child`  | as the first child of the container      | container block id    |
| `append_section` | as the last block of a heading's section | heading block id              |
| `prepend_section`| right after the heading itself           | heading block id              |
| `append_doc`     | as the last block of a document          | document root id      |
| `prepend_doc`    | as the first block of a document         | document root id      |

`block move` supports all 8 position kinds. Note: `prepend_child` and `prepend_doc` place the block at the end of the container due to kernel API limitations; if strict prepend placement is required, follow up with an `after_block` adjustment.

## MCP server usage

`siyuan serve-mcp` speaks JSON-RPC over **stdio**. Wire it into any MCP-aware client (Claude Desktop, Claude Code, custom hosts) by spawning the binary with the SiYuan env injected.

### Claude Desktop / Claude Code

```json
{
  "mcpServers": {
    "siyuan": {
      "command": "/abs/path/to/siyuan",
      "args": ["serve-mcp"],
      "env": {
        "SIYUAN_BASE_URL": "http://127.0.0.1:6806",
        "SIYUAN_TOKEN": "your-token-here"
      }
    }
  }
}
```

To set the request timeout per server, add `"args": ["serve-mcp", "--timeout-ms", "60000"]`.

Tools exposed (one-line summary; full agent-friendly descriptions live in `crates/siyuan-mcp/src/registry.rs`):

| Tool                       | Purpose                                              |
| -------------------------- | ---------------------------------------------------- |
| `siyuan_status`            | Kernel reachability + version.                       |
| `siyuan_doc_get`           | Load a doc as agent-md (default) or JSON, paginated. |
| `siyuan_block_get`         | Raw kramdown of one block.                           |
| `siyuan_doc_create`        | Create a doc from GFM markdown.                      |
| `siyuan_block_update`      | Replace block content.                               |
| `siyuan_block_insert`      | Add a new block.                                    |
| `siyuan_block_move`        | Reposition a block (keeps id + children).            |
| `siyuan_block_delete`      | Permanently delete a block + subtree.                |
| `siyuan_attrs_get` / `siyuan_attrs_set` | Read / partial-update block attributes. |
| `siyuan_notebook_ls` / `_create` / `_rename` / `_remove` | Notebook management. (open/close not exposed.) |
| `siyuan_doc_resolve`       | Unified lookup by id OR (notebook + hpath); returns array of doc metadata including `storage_path`. |
| `siyuan_doc_tree`          | List a notebook/folder subtree as a tree (id XOR notebook[+hpath], `depth` 1..N or `all`). |
| `siyuan_doc_rename` / `_move` / `_remove` | Filetree ops. Accept id XOR (notebook + hpath); the harness resolves storage `.sy` paths internally. |
| `siyuan_tag_ls` / `siyuan_tag_search` | Enumerate tags / find blocks by tag. |
| `siyuan_search_text`       | LIKE-substring search across the `blocks` table.     |
| `siyuan_sql`               | Read-only raw SQL. Power tool — escape values yourself. |
| `siyuan_asset_upload`      | Upload a local file as a SiYuan asset.               |
| `siyuan_graph_neighborhood`| BFS over the link graph (depth ≤ 8, capped 500 nodes / 1000 edges). |

Mutating tools wrap their response as `{"data": <payload>, "_hint": "..."}`. The hint surfaces post-call expectations (SQL index lag, follow-up tool to call, etc.) and is informational only.

Errors map to typed MCP errors (`InvalidParams`, `NotFound`, `Unauthorized`, etc.) when the kernel returns a recognised error code; otherwise they surface as `InternalError` with the kernel message.

## Library use

If you only want a typed Rust client, depend on `siyuan-client`:

```toml
[dependencies]
siyuan-client = { git = "https://github.com/tpob/siyuan-cli" }
siyuan-types  = { git = "https://github.com/tpob/siyuan-cli" }
tokio = { version = "1", features = ["full"] }
```

```rust
let client = siyuan_client::SiyuanClient::new("http://127.0.0.1:6806", "TOKEN")?;
let v = client.system_version().await?;
println!("kernel = {v}");
```

`siyuan-model` adds higher-level pipelines (`load_doc`, sectioning, pagination, link-graph BFS, tags, doc-meta resolve). `siyuan-render` turns a `DocBundle` into agent-md or canonical JSON.

## Testing

```sh
cargo test --workspace                          # unit tests, no kernel needed
cargo test --workspace -- --ignored --nocapture # integration tests
```

Ignored tests spin up disposable SiYuan kernels via Podman through `siyuan-testkit`, so they need a working `podman` on `PATH`. They are gated behind `--ignored` to keep the default `cargo test` hermetic.

## Not yet covered

The v1 surface deliberately omits the following — they are out of scope for this iteration:

- **Two-phase plan / apply** and `--dry-run` modes. Every write hits the kernel directly.
- **Concurrency guards** (`expected_hash`, snapshot tokens). Last-writer-wins.
- **Notebook open/close.** Removed from the public surface; the harness does not handle user-closed notebooks. See `docs/decisions.md` §2.
- **Attribute view (AV) editing** — read access is available via `siyuan_sql`, mutations are not.
- **Super-block creation and layout mutations.** Existing super-blocks are surfaced as a read-only `:::sy-superblock` fence.
- **WebSocket / push notifications.** All calls are synchronous HTTP.
- **History, trash, daily notes, templates** — use the SiYuan UI; the kernel already owns these flows.
- **Multi-workspace switching.** A binary instance talks to one workspace at a time.
- **Local backup / sync orchestration.**
- **Rate limiting and retry policy.** A single in-flight request per call, no backoff.
- **Packaged distribution.** No prebuilt binaries, Homebrew tap, or Docker image yet — build from source.
- **Telemetry / metrics.** Logging only, via `tracing` to stderr.

If you need any of these, the kernel HTTP API is well-documented; `siyuan-client` is a thin enough layer to extend in-tree.

## Repo layout

```
crates/
  siyuan-types/    # BlockId, BlockType, Position, errors — no deps
  siyuan-client/   # typed reqwest wrapper over the kernel HTTP API
  siyuan-model/    # DocBundle, load_doc, sectioning, pagination, graph BFS, tags, doc-meta resolve
  siyuan-render/   # agent-md + canonical JSON renderers
  siyuan-cli/      # `siyuan` binary (clap) — provides both CLI and `serve-mcp`
  siyuan-mcp/      # library crate consumed by siyuan-cli's `serve-mcp` subcommand
  siyuan-testkit/  # Podman-driven disposable SiYuan instances
docs/
  decisions.md          # design tradeoffs log
  superpowers/plans/    # design notes & implementation plans
  readme/               # translated READMEs
```

## License

Dual-licensed under MIT or Apache-2.0, at your option.
