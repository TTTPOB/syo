# siyuan-mcp

An MCP (Model Context Protocol) server that exposes the SiYuan note-taking kernel's API as a set of structured tools consumable by LLM agents. The server communicates over stdio using JSON-RPC 2.0 and implements the MCP `tools/list` and `tools/call` protocol surface. It is backed by `siyuan-client`, `siyuan-model`, and `siyuan-render` crates from the same workspace.

## Response envelope

Most tools return the payload directly as a bare JSON object. Tools where post-call expectations matter (mutations with SQL index lag, paginated reads, search results, graph traversals) wrap their payload in a hint envelope:

```json
{
  "data": { ...payload... },
  "_hint": "Human-readable string telling the agent what to expect next."
}
```

Tools that do **not** add a hint return the bare payload with no `data` or `_hint` wrapper. Agents should check for the presence of `_hint` rather than assuming a fixed shape.

The hint is informational only — it is never required for correctness. It surfaces kernel quirks (SQL index lag, truncation limits, filesystem vs. SQL consistency) so that agents can decide whether to retry, paginate, or narrow a query.

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
| `siyuan_set_attrs` | Partially update attributes on a block (custom- prefix required). |
| `siyuan_notebook_ls` | List all notebooks (open and closed). |
| `siyuan_notebook_create` | Create a new notebook. |
| `siyuan_notebook_rename` | Rename a notebook. |
| `siyuan_notebook_remove` | Permanently remove a notebook and all its documents. |
| `siyuan_doc_resolve` | Convert an hpath to document block id(s). |
| `siyuan_doc_hpath_by_id` | Convert a block id to its human-readable hpath. |
| `siyuan_doc_rename` | Rename a document (requires storage .sy path, not hpath). |
| `siyuan_doc_move` | Move documents to a different notebook/path. |
| `siyuan_doc_remove` | Permanently remove a document (requires storage .sy path). |
| `siyuan_tag_ls` | List all tags in the workspace. |
| `siyuan_tag_search` | Find blocks carrying a specific tag (without # prefix). |
| `siyuan_search_text` | LIKE substring search across block markdown content. |
| `siyuan_sql` | Execute a raw read-only SQL SELECT against the SiYuan database. |
| `siyuan_asset_upload` | Upload a local file as a SiYuan asset; returns the asset path. |
| `siyuan_graph_neighborhood` | Compute the link-graph neighborhood around a block. |

## Configuration

### Environment variables

| Variable | Description | Default |
|----------|-------------|---------|
| `SIYUAN_BASE_URL` | Base URL of the SiYuan kernel HTTP API | `http://127.0.0.1:6806` |
| `SIYUAN_TOKEN` | API token (set in SiYuan Settings > About) | _(none)_ |
| `SIYUAN_TIMEOUT_MS` | HTTP request timeout in milliseconds | `30000` |

### CLI flags

```
siyuan-mcp [OPTIONS]

Options:
  --base-url <URL>        SiYuan kernel base URL [env: SIYUAN_BASE_URL]
  --token <TOKEN>         API authentication token [env: SIYUAN_TOKEN]
  --timeout-ms <MS>       HTTP timeout in milliseconds [env: SIYUAN_TIMEOUT_MS]
```

### MCP host configuration (Claude / claude.json style)

```json
{
  "mcpServers": {
    "siyuan": {
      "command": "/path/to/siyuan-mcp",
      "args": ["--base-url", "http://127.0.0.1:6806", "--token", "your-token-here"],
      "transport": "stdio"
    }
  }
}
```

The server reads from stdin and writes to stdout using newline-delimited JSON-RPC 2.0 messages. Stderr is used for tracing/log output and does not carry protocol messages.
