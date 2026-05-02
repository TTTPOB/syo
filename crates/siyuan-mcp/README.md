# siyuan-mcp

The library crate that powers the `siyuan serve-mcp` subcommand. It exposes the SiYuan note-taking kernel's API as a set of structured MCP (Model Context Protocol) tools consumable by LLM agents. Communication is JSON-RPC 2.0 over stdio; the surface implements `tools/list` and `tools/call`. Backed by `siyuan-client`, `siyuan-model`, and `siyuan-render` from the same workspace.

This crate is **not** a binary. To run the server, install the workspace and invoke `siyuan serve-mcp`. See the [top-level README](../../README.md#mcp-server-usage) for host configuration.

## Response envelope

Read-only tools (`siyuan_status`, `siyuan_get_doc`, `siyuan_get_block`, `siyuan_get_attrs`, `siyuan_doc_resolve`) return their payload directly as a bare JSON object.

Mutating and post-call-sensitive tools wrap the payload to surface follow-up expectations:

```json
{
  "data": { ...payload... },
  "_hint": "Human-readable string telling the agent what to expect next."
}
```

The hint is informational only — never required for correctness. It surfaces kernel quirks (SQL index lag, truncation limits, filesystem vs. SQL consistency) so agents can decide whether to retry, paginate, or narrow a query.

Agents should check for the presence of `_hint` rather than assuming a fixed shape.

## Tool catalogue

| Tool | Summary |
|------|---------|
| `siyuan_status` | Health-check: returns kernel version. |
| `siyuan_get_doc` | Load a document as agent-markdown or JSON bundle, with pagination. |
| `siyuan_get_block` | Fetch raw kramdown source of a single block. |
| `siyuan_create_doc` | Create a new document from GFM markdown at an hpath. |
| `siyuan_update_block` | Replace a block's content with new markdown. |
| `siyuan_insert_block` | Insert a block relative to an anchor (before/after/as child). |
| `siyuan_append_block` | Append a block as the last child of a container. |
| `siyuan_prepend_block` | Prepend a block as the first child of a container. |
| `siyuan_move_block` | Move a block to a new position in the tree. |
| `siyuan_delete_block` | Permanently delete a block and all its children. |
| `siyuan_get_attrs` | Read all attributes of a block. |
| `siyuan_set_attrs` | Partially update attributes on a block (`custom-` prefix required for custom keys). |
| `siyuan_notebook_ls` | List all notebooks (open and closed). |
| `siyuan_notebook_create` | Create a new notebook. |
| `siyuan_notebook_rename` | Rename a notebook. |
| `siyuan_notebook_remove` | Permanently remove a notebook and all its documents. |
| `siyuan_doc_resolve` | Unified lookup by id OR (notebook + hpath); returns array of doc metadata including `storage_path`. |
| `siyuan_doc_rename` | Rename a document (requires storage `.sy` path, NOT hpath). |
| `siyuan_doc_move` | Move documents to a different notebook/path (storage paths). |
| `siyuan_doc_remove` | Permanently remove a document (requires storage `.sy` path). |
| `siyuan_tag_ls` | List all tags in the workspace. |
| `siyuan_tag_search` | Find blocks carrying a specific tag (without `#` prefix). |
| `siyuan_search_text` | LIKE substring search across block markdown content. |
| `siyuan_sql` | Execute a raw read-only SQL SELECT against the SiYuan database. |
| `siyuan_asset_upload` | Upload a local file as a SiYuan asset; returns the asset path. |
| `siyuan_graph_neighborhood` | Compute the link-graph neighborhood around a block. |

`notebook_open` and `notebook_close` are intentionally not exposed; see `docs/decisions.md §2` for the rationale.

## Configuration

Both env vars and CLI flags are inherited from the parent `siyuan` binary:

| Variable | Description | Default |
|----------|-------------|---------|
| `SIYUAN_BASE_URL` | Base URL of the SiYuan kernel HTTP API | `http://127.0.0.1:6806` |
| `SIYUAN_TOKEN` | API token (set in SiYuan Settings → About) | _(none, but tolerated for serve-mcp)_ |
| `SIYUAN_TIMEOUT_MS` | HTTP request timeout in milliseconds (`0` = no timeout) | `30000` |

Per-invocation flags:

```
siyuan serve-mcp [OPTIONS]

Options:
  --timeout-ms <MS>       HTTP timeout in milliseconds (overrides SIYUAN_TIMEOUT_MS)

Inherited globals (from `siyuan`):
  --base-url <URL>        Kernel base URL                       [env: SIYUAN_BASE_URL]
  --token <TOKEN>         API authentication token              [env: SIYUAN_TOKEN]
```

## MCP host configuration (Claude / claude.json style)

```json
{
  "mcpServers": {
    "siyuan": {
      "command": "/abs/path/to/siyuan",
      "args": ["serve-mcp"],
      "env": {
        "SIYUAN_BASE_URL": "http://127.0.0.1:6806",
        "SIYUAN_TOKEN": "your-token-here"
      },
      "transport": "stdio"
    }
  }
}
```

The server reads from stdin and writes to stdout using newline-delimited JSON-RPC 2.0 messages. Stderr is used for tracing/log output and does not carry protocol messages.
