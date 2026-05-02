use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use rmcp::{
    ErrorData as McpError,
    model::{JsonObject, Tool},
};
use serde_json::Value;

use siyuan_client::SiyuanClient;

use crate::tools;

// Boxed async fn: (client, args) -> Result<Value, McpError>
pub(crate) type Handler = Arc<
    dyn Fn(
            Arc<SiyuanClient>,
            Value,
        ) -> Pin<Box<dyn Future<Output = Result<Value, McpError>> + Send>>
        + Send
        + Sync,
>;

// Parse a JSON schema string into a JsonObject (Arc'd for Tool).
fn schema(s: &str) -> Arc<JsonObject> {
    Arc::new(
        serde_json::from_str::<JsonObject>(s).expect("static schema must be valid JSON object"),
    )
}

// Wrap an async fn into a Handler Arc.
fn make_handler<F, Fut>(f: F) -> Handler
where
    F: Fn(Arc<SiyuanClient>, Value) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Value, McpError>> + Send + 'static,
{
    Arc::new(move |c: Arc<SiyuanClient>, args: Value| {
        Box::pin(f(c, args)) as Pin<Box<dyn Future<Output = Result<Value, McpError>> + Send>>
    })
}

// Build the full registry. Called once at startup.
pub(crate) fn build(client: Arc<SiyuanClient>) -> (Vec<Tool>, HashMap<&'static str, Handler>) {
    let mut tool_list: Vec<Tool> = Vec::new();
    let mut handlers: HashMap<&'static str, Handler> = HashMap::new();

    macro_rules! reg {
        ($name:literal, $desc:literal, $schema_str:expr, $handler:expr) => {{
            handlers.insert($name, $handler);
            tool_list.push(Tool::new($name, $desc, schema($schema_str)));
        }};
    }

    // ---- system ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_status",
            "Return the SiYuan kernel version and confirm the server is reachable. \
             Use this as a health-check before issuing other calls — if it fails, the kernel \
             is offline or misconfigured. No parameters are required. \
             Response: { \"version\": \"<semver>\" }.",
            r#"{"type":"object","properties":{},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::system::status(&c, args).await }
            })
        );
    }

    // ---- doc reads ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_get_doc",
            "Load a SiYuan document by its root block id and return it as agent-readable \
             markdown (default) or a structured JSON bundle. Pagination uses DFS document \
             order; `page` is 1-indexed and `page_size` defaults to 50 blocks per page \
             (page_size is capped at 1000). \
             When `total_pages > page` the response is wrapped with a `_hint` that tells you \
             to fetch the next page. Use `format=agent-md` (default) for a compact markdown \
             representation with `<!-- sy:* -->` HTML-comment block markers; use `format=json` \
             or `format=json-pretty` when you need the raw structured bundle with full block \
             metadata. Always call `siyuan_doc_resolve` first to convert an hpath to an id; \
             this tool requires a block id, not an hpath.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string","description":"Document block id"},"page":{"type":"integer","default":1},"page_size":{"type":"integer","default":50},"format":{"type":"string","enum":["agent-md","json","json-pretty"],"default":"agent-md"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::doc::get_doc(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_get_block",
            "Fetch the raw kramdown source of a single block by its id. \
             Use this when you need the exact storage syntax of one block (e.g. to inspect \
             attributes embedded in kramdown) rather than the rendered document. \
             Do NOT use this to read an entire document — use `siyuan_get_doc` for that. \
             Response: { \"id\": \"<block-id>\", \"kramdown\": \"<raw-kramdown>\" }.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string","description":"Block id"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::doc::get_block(&c, args).await }
            })
        );
    }

    // ---- doc writes ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_create_doc",
            "Create a new document in a notebook from GFM markdown. \
             `hpath` follows the `/Folder/Title` convention; the kernel creates intermediate \
             folders automatically. `notebook` is a notebook id obtained from \
             `siyuan_notebook_ls`. The markdown is stored verbatim and then indexed \
             asynchronously. SiYuan indexes mutations into its SQL store asynchronously — \
             reads via `siyuan_get_doc` or `siyuan_sql` may briefly (<=500 ms) show stale \
             data after this call. The kernel itself is consistent; only the SQL index lags. \
             Response envelope includes the new document's root block id under `data.id`.",
            r#"{"type":"object","required":["notebook","hpath","markdown"],"properties":{"notebook":{"type":"string"},"hpath":{"type":"string","description":"Human path e.g. /Folder/Title"},"markdown":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::doc::create_doc(&c, args).await }
            })
        );
    }

    // ---- block writes ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_update_block",
            "Replace the full content of an existing block with new GFM markdown. \
             The block is identified by its id; use `siyuan_get_doc` or `siyuan_search_text` \
             to find the id first. This overwrites the block entirely — partial edits are not \
             supported; read the current content first if you need to preserve parts of it. \
             SiYuan indexes the change asynchronously, so SQL-based reads may briefly show \
             stale content for ~100–500 ms after this call.",
            r#"{"type":"object","required":["id","markdown"],"properties":{"id":{"type":"string"},"markdown":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::block::update_block(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_insert_block",
            "Insert a new markdown block at a position relative to an anchor block. \
             Exactly one of `previous_id` (insert after), `next_id` (insert before), or \
             `parent_id` (insert as first child) must be provided; supplying more than one \
             is an error. The response envelope contains the new block's id under `data.id`. \
             SiYuan indexes the insertion asynchronously — SQL-based reads may briefly \
             show stale data for ~100–500 ms after this call.",
            r#"{"type":"object","required":["markdown"],"properties":{"markdown":{"type":"string"},"previous_id":{"type":"string"},"next_id":{"type":"string"},"parent_id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::block::insert_block(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_append_block",
            "Append a new markdown block as the last child of a parent block or document. \
             Use this to add content to the end of a container without knowing the id of the \
             last existing child. `parent_id` must be the id of the container block (e.g. a \
             document root or a list item). The response envelope contains the new block's id \
             under `data.id`. SiYuan indexes the change asynchronously — SQL-based reads \
             may briefly show stale data for ~100–500 ms after this call.",
            r#"{"type":"object","required":["markdown","parent_id"],"properties":{"markdown":{"type":"string"},"parent_id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::block::append_block(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_prepend_block",
            "Prepend a new markdown block as the first child of a parent block or document. \
             Use this to add content at the beginning of a container without knowing the id \
             of the first existing child. `parent_id` must be the id of the container block. \
             The response envelope contains the new block's id under `data.id`. SiYuan \
             indexes the change asynchronously — SQL-based reads may briefly show stale data \
             for ~100–500 ms after this call.",
            r#"{"type":"object","required":["markdown","parent_id"],"properties":{"markdown":{"type":"string"},"parent_id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::block::prepend_block(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_move_block",
            "Move an existing block to a new position within the document tree. \
             Exactly one of `previous_id` (place after that block) or `parent_id` (place as \
             first child of that block) must be provided; supplying both is an error. \
             The block keeps its existing id and all its children. SiYuan indexes the move \
             asynchronously — SQL-based reads may briefly show stale position data for \
             ~100–500 ms after this call.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string"},"previous_id":{"type":"string"},"parent_id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::block::move_block(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_delete_block",
            "Permanently delete a block and all of its children. \
             This action is irreversible — the block and its subtree are gone immediately at \
             the kernel level. Use with caution; prefer `siyuan_update_block` with empty \
             content if you only want to clear a block. SiYuan indexes the deletion \
             asynchronously — SQL-based reads may briefly still return the block for \
             ~100–500 ms after this call.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::block::delete_block(&c, args).await }
            })
        );
    }

    // ---- attrs ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_get_attrs",
            "Read all custom and built-in attributes of a block by its id. \
             Returns a flat key-value map where built-in keys use `id`, `type`, etc. and \
             custom keys use the `custom-` prefix. Use this before `siyuan_set_attrs` if you \
             need to inspect existing values without overwriting them. \
             Response: { \"id\": \"<block-id>\", \"attrs\": { \"<key>\": \"<value>\", ... } }.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::attr::get_attrs(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_set_attrs",
            "Set one or more attributes on a block. \
             This call is a partial update — only the listed keys are modified; existing keys \
             not included in the request are left intact (kernel semantics). To delete a key, \
             set its value to an empty string. Custom keys must start with `custom-`; \
             attempting to set internal keys like `id` or `type` is silently ignored by the \
             kernel. SiYuan indexes the attribute change asynchronously — SQL-based reads \
             may briefly reflect the old values for ~100–500 ms after this call.",
            r#"{"type":"object","required":["id","attrs"],"properties":{"id":{"type":"string"},"attrs":{"type":"object","additionalProperties":{"type":"string"}}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::attr::set_attrs(&c, args).await }
            })
        );
    }

    // ---- notebook ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_notebook_ls",
            "List all notebooks in the workspace, including both open and closed ones. \
             Each notebook entry includes id, name, icon, sort order, and a `closed` flag. \
             Use this to discover notebook ids before calling tools that require a `notebook` \
             parameter (e.g. `siyuan_create_doc`, `siyuan_doc_resolve`). \
             Closed notebooks must be opened with `siyuan_notebook_open` before their \
             documents can be accessed. \
             Response: { \"notebooks\": [ { \"id\": \"...\", \"name\": \"...\", \"closed\": bool, ... } ] }.",
            r#"{"type":"object","properties":{},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::notebook::ls(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_notebook_open",
            "Open (mount) a notebook so its documents become accessible for reading and writing. \
             Notebooks that appear with `closed: true` in `siyuan_notebook_ls` must be opened \
             before their documents can be loaded with `siyuan_get_doc` or queried via SQL. \
             Already-open notebooks can be opened again without error. \
             `id` must be a valid notebook id from `siyuan_notebook_ls`.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string","description":"Notebook id"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::notebook::open(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_notebook_close",
            "Close (unmount) a notebook so its documents are no longer accessible. \
             After closing, the notebook appears with `closed: true` in `siyuan_notebook_ls` \
             and its documents cannot be read or modified until it is opened again. \
             Use this to reduce memory footprint or to explicitly isolate a notebook from \
             automated writes. Already-closed notebooks can be closed again without error.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string","description":"Notebook id"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::notebook::close(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_notebook_create",
            "Create a new notebook with the given display name. \
             The kernel assigns a unique id automatically; the id is returned in the response \
             alongside the notebook metadata. The new notebook is created in the open state. \
             Use the returned id in subsequent calls that require a `notebook` parameter. \
             Response envelope includes the notebook record under `data` \
             (`{ \"id\": \"...\", \"name\": \"...\", \"icon\": \"...\", \"sort\": N, \"closed\": false }`).",
            r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::notebook::create(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_notebook_rename",
            "Rename an existing notebook by giving it a new display name. \
             `id` is the notebook id (obtained from `siyuan_notebook_ls`); `name` is the \
             desired new display name. The notebook id does not change. The updated name \
             is reflected immediately in `siyuan_notebook_ls`. SQL-indexed reads may briefly \
             show the old name for ~100–500 ms. This does NOT rename any on-disk folder; \
             the storage path remains stable.",
            r#"{"type":"object","required":["id","name"],"properties":{"id":{"type":"string"},"name":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::notebook::rename(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_notebook_remove",
            "Permanently remove a notebook and ALL of its documents. \
             This action is irreversible — all content in the notebook is destroyed immediately. \
             Use with extreme caution; prefer `siyuan_notebook_close` if you only want to \
             hide the notebook from the active workspace. Verify the notebook id from \
             `siyuan_notebook_ls` before calling this tool.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::notebook::remove(&c, args).await }
            })
        );
    }

    // ---- filetree ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_doc_resolve",
            "Resolve a human-readable hpath to one or more document block ids. \
             `hpath` is the `/Folder/Title` style path as seen in the SiYuan UI; \
             `notebook` is the notebook id. Returns an array of matching ids (usually one, \
             but SiYuan allows duplicate hpaths in some edge cases). Use this as the first \
             step whenever you know a document's title/path but need its id for \
             `siyuan_get_doc`, `siyuan_update_block`, etc. \
             Response: { \"ids\": [\"<block-id>\", ...] }.",
            r#"{"type":"object","required":["notebook","hpath"],"properties":{"notebook":{"type":"string"},"hpath":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::filetree::resolve(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_doc_hpath_by_id",
            "Look up the human-readable hpath (`/Folder/Title` style) for a document given \
             its root block id. Use this to recover the display path after a move or rename, \
             or when you have an id from SQL results and want to present a human-readable \
             location to the user. This call reflects filesystem state immediately — it does \
             not go through the SQL index and is not subject to indexing lag. \
             Response: { \"hpath\": \"/Folder/Title\" }.",
            r#"{"type":"object","required":["id"],"properties":{"id":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::filetree::hpath_by_id(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_doc_rename",
            "Rename a document by changing its display title. \
             IMPORTANT: `path` is the on-disk storage path (with `.sy` suffix, e.g. \
             `/20230101120000-abcdefg.sy`), NOT the human-readable hpath. \
             To obtain the storage path from an id, use the SQL blocks table \
             (`SELECT path FROM blocks WHERE id = '<id>'`) or derive it from the \
             filesystem layout. The `title` parameter is the new human-readable display name. \
             The hpath returned by `siyuan_doc_hpath_by_id` reflects the new title immediately \
             after this call.",
            r#"{"type":"object","required":["notebook","path","title"],"properties":{"notebook":{"type":"string"},"path":{"type":"string","description":"Storage .sy path"},"title":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::filetree::rename_doc(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_doc_move",
            "Move one or more documents to a different location in the file tree. \
             IMPORTANT: `from_paths` contains storage `.sy` paths (NOT hpaths). \
             `to_notebook` is the destination notebook id; `to_path` is the destination \
             folder as a storage path (NOT an hpath). Use `siyuan_doc_hpath_by_id` or the \
             SQL blocks table to obtain storage paths from ids. After the move, \
             `siyuan_doc_resolve` and `siyuan_doc_hpath_by_id` reflect the new location \
             immediately. This is a filesystem-level mutation.",
            r#"{"type":"object","required":["from_paths","to_notebook","to_path"],"properties":{"from_paths":{"type":"array","items":{"type":"string"}},"to_notebook":{"type":"string"},"to_path":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::filetree::move_doc(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_doc_remove",
            "Permanently remove a document and all its child blocks. \
             This action is irreversible. IMPORTANT: `path` is the on-disk storage path \
             (with `.sy` suffix), NOT the human-readable hpath. Use the SQL blocks table or \
             `siyuan_doc_resolve` followed by a SQL lookup to obtain the storage path. \
             After removal, `siyuan_doc_resolve` will no longer find this path. \
             Verify the notebook and path before calling this tool.",
            r#"{"type":"object","required":["notebook","path"],"properties":{"notebook":{"type":"string"},"path":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::filetree::remove_doc(&c, args).await }
            })
        );
    }

    // ---- tag / search ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_tag_ls",
            "List all distinct tags used anywhere in the workspace. \
             Returns a flat array of tag strings WITHOUT the surrounding `#` characters. \
             The list is derived from the SQL index and is eventually consistent — freshly \
             created tags may take ~100–500 ms to appear. Use the tag values directly as \
             the `tag` argument to `siyuan_tag_search`. \
             Response envelope includes `data.tags` (array of strings).",
            r#"{"type":"object","properties":{},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::tag::ls_tags(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_tag_search",
            "Find all blocks that carry a specific tag. \
             The `tag` argument is the tag content WITHOUT the surrounding `#` characters \
             (e.g. pass `\"project\"` to find blocks tagged `#project`). Results are \
             eventually consistent with the SQL index — freshly-tagged blocks may take \
             ~100–500 ms to appear. Use `siyuan_tag_ls` to enumerate available tags first. \
             Response envelope includes `data.hits` (array of block records).",
            r#"{"type":"object","required":["tag"],"properties":{"tag":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::tag::search_by_tag(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_search_text",
            "Full-text search across all blocks using a SQL LIKE substring match. \
             The `query` is matched against the `markdown` column of the blocks table using \
             `LIKE '%query%'`; matching is case-insensitive on most SQLite builds. \
             Single quotes in the query are escaped to prevent injection, but this is not a \
             parameterised query — do not rely on it for security-critical use-cases. \
             Results may lag recent mutations by ~100–500 ms. \
             Increase or decrease `limit` (default 50) to control result count. \
             Response envelope includes `data.hits` (array of block records with id, root_id, markdown).",
            r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string"},"limit":{"type":"integer","default":50}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::sql::search_text(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_sql",
            "Execute a raw read-only SQL SELECT statement against the SiYuan SQLite database. \
             This is a power tool for advanced queries not covered by other tools — use it \
             when you need joins, aggregations, or access to internal tables (blocks, refs, \
             attributes, spans). Results are returned as an array of JSON objects where each \
             key is a column name. Some columns may be unstable internal fields. \
             The SQL index lags mutations by ~100–500 ms. \
             NEVER issue INSERT, UPDATE, DELETE, or DDL — the kernel enforces read-only \
             semantics and will return an error. \
             The kernel does NOT parameterise the query; escape user-supplied values manually.",
            r#"{"type":"object","required":["stmt"],"properties":{"stmt":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::sql::raw_sql(&c, args).await }
            })
        );
    }

    // ---- asset ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_asset_upload",
            "Upload a local file from the agent's filesystem as a SiYuan asset. \
             `file_path` is an absolute path on the machine running the MCP server. \
             The kernel copies the file into its assets directory and returns a \
             kernel-relative asset path (e.g. `assets/image-20230101-abc.png`). \
             To embed the asset in a document, insert a markdown image like \
             `![alt](assets/image-20230101-abc.png)` via `siyuan_insert_block` or \
             include it in `siyuan_create_doc` markdown. \
             Response envelope includes `data.asset_path`.",
            r#"{"type":"object","required":["file_path"],"properties":{"file_path":{"type":"string"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::asset::upload(&c, args).await }
            })
        );
    }

    // ---- graph ----
    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_graph_neighborhood",
            "Compute the link-graph neighborhood around a center block up to a given depth. \
             `direction` controls which edges are followed: `outgoing` follows references FROM \
             the center block to blocks it references; `incoming` follows references TO the \
             center from other blocks; `both` (default) follows both. `depth` (default 1) \
             controls how many hops to expand; depth is capped at 8. The traversal stops at \
             500 nodes or 1000 edges per call; when either limit is hit the `truncated` field \
             in the response is set to true. When `truncated` is true, narrow the query by \
             reducing depth, switching to a single direction, or querying a more specific \
             center block. \
             Alternatively use `siyuan_sql` to query the `refs` table directly for unbounded results. \
             Response envelope includes the full graph with `data.nodes`, `data.edges`, and `data.truncated`.",
            r#"{"type":"object","required":["center"],"properties":{"center":{"type":"string"},"depth":{"type":"integer","default":1},"direction":{"type":"string","enum":["outgoing","incoming","both"],"default":"both"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::graph::neighborhood(&c, args).await }
            })
        );
    }

    (tool_list, handlers)
}
