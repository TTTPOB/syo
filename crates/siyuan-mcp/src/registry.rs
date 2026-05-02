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
            "Return the SiYuan kernel version and confirm the server is reachable.\n\
             \n\
             Sibling tools: this is the only health-check tool — every other tool \
             assumes the kernel is up. Call this first if you see connection-refused \
             or auth errors elsewhere.\n\
             \n\
             Inputs: none required (extra properties ignored).\n\
             \n\
             Example:\n\
               in:  {}\n\
               out: { \"version\": \"3.1.0\" }",
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
            "Load a SiYuan document by its ROOT block id and return it as agent-markdown \
             or a structured JSON bundle.\n\
             \n\
             Sibling tools: `siyuan_get_block` returns ONE block's raw kramdown — use that \
             when you need a single block's storage syntax, not a whole document. \
             `siyuan_doc_resolve` translates hpath<->id (this tool requires an id, not an \
             hpath). `siyuan_search_text` finds candidate ids by content.\n\
             \n\
             Inputs: `id` (required) is a document ROOT block id (14-digit timestamp + \
             7-char suffix); not an hpath. `page` (optional, default 1) is 1-indexed in \
             DFS document order. `page_size` (optional, default 50) is capped at 1000. \
             `format` (optional, default `agent-md`) is one of `agent-md` (compact \
             markdown with `<!-- sy:* -->` HTML-comment block markers), `json`, or \
             `json-pretty`. When `total_pages > page` the response is wrapped with a \
             `_hint` instructing the next-page fetch.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-doc0001\", \"format\": \"json\" }\n\
               out (last/only page): { \"format\": \"json\", \"content\": \"<stringified json bundle>\" }\n\
               out (more pages):     { \"data\": { \"format\": \"json\", \"content\": \"<stringified json bundle>\" }, \"_hint\": \"Pagination: this is page 1 of 3. Call again with page=2 to fetch the next page. ...\" }",
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
            "Fetch the raw kramdown source of a single block by id.\n\
             \n\
             Sibling tools: `siyuan_get_doc` returns the rendered document tree — reach for \
             that to read a whole document. `siyuan_get_attrs` returns just the attribute \
             map; this tool returns the kramdown body. `siyuan_search_text` finds candidate \
             ids when you do not have one yet.\n\
             \n\
             Inputs: `id` (required) is any block id (paragraph, heading, list item, \
             document root, etc.). NotFound is returned if the id does not exist.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-doc0001\" }\n\
               out: { \"id\": \"20260501090000-doc0001\", \"kramdown\": \"# Heading\\n\\nBody\\n\" }",
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
            "Create a new document in a notebook from GFM markdown.\n\
             \n\
             Sibling tools: `siyuan_update_block` replaces an existing block in place; \
             `siyuan_insert_block` / `siyuan_append_block` / `siyuan_prepend_block` add \
             blocks under an existing document. Reach for siyuan_create_doc only to mint \
             a NEW document.\n\
             \n\
             Inputs: `notebook` (required) is a notebook id from `siyuan_notebook_ls`. \
             `hpath` (required) is a HUMAN path inside the notebook, e.g. `/Folder/Title`; \
             must start with `/`. NOT to be confused with on-disk storage paths (`.sy`-suffixed) \
             — hpaths are titles separated by `/`, storage paths look like \
             `/20260501090000-abc1234.sy`. Intermediate folders are auto-created. \
             `markdown` (required) is GFM markdown stored verbatim. The response envelope \
             contains the new document's root block id under `data.id`, accompanied by a \
             `_hint` string.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"notebook\": \"20260501000000-nb00001\", \"hpath\": \"/Plan\", \"markdown\": \"# Plan\\n\" }\n\
               out: { \"data\": { \"id\": \"20260501090000-doc0001\" }, \"_hint\": \"Mutation completed at the kernel. ...\" }",
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
            "Replace the full content of an existing block with new GFM markdown.\n\
             \n\
             Sibling tools: `siyuan_insert_block` adds NEW blocks at a position relative to \
             an anchor; `siyuan_delete_block` removes a block; `siyuan_set_attrs` mutates \
             attributes (not body). siyuan_update_block is for in-place full-body overwrite. \
             Partial edits are NOT supported — read with `siyuan_get_block` first if part of \
             the existing content must be preserved.\n\
             \n\
             Inputs: `id` (required) is the block id to overwrite (use `siyuan_get_doc` or \
             `siyuan_search_text` to find it). `markdown` (required) is GFM markdown that \
             replaces the entire block body.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-blk0001\", \"markdown\": \"Updated body.\\n\" }\n\
               out: { \"ok\": true }",
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
            "Insert a new markdown block at a position relative to an anchor block.\n\
             \n\
             Sibling tools: `siyuan_append_block` is a shortcut for inserting as the LAST \
             child of a container (no anchor sibling needed); `siyuan_prepend_block` is the \
             FIRST-child shortcut; `siyuan_move_block` moves an EXISTING block (keeps id); \
             `siyuan_create_doc` mints a new document.\n\
             \n\
             Inputs: `markdown` (required) is the GFM markdown body for the new block. \
             EXACTLY ONE of `previous_id`, `next_id`, or `parent_id` must be supplied; \
             supplying zero or more than one is an error. The position kinds, in terms of \
             which field you set:\n\
               previous_id  → new block lands as a sibling immediately AFTER previous_id\n\
                              (anchor = any block id; siblings later in order shift down)\n\
               next_id      → new block lands as a sibling immediately BEFORE next_id\n\
                              (anchor = any block id; later siblings stay in order)\n\
               parent_id    → new block lands as the FIRST child of container parent_id\n\
                              (anchor = container id; existing children shift down)\n\
             For LAST-child use `siyuan_append_block` (cleaner than constructing a `parent_id` \
             call here, which inserts at the front, not the back). The kernel returns the new \
             block's id; the response envelope surfaces it as `data.id`. Existing blocks keep \
             their ids and children — only sibling order changes.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"markdown\": \"New paragraph.\\n\", \"previous_id\": \"20260501090000-blk0001\" }\n\
               out: { \"data\": { \"id\": \"20260501090500-blk0099\" }, \"_hint\": \"Block inserted at the kernel. ...\" }",
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
            "Append a new markdown block as the LAST child of a container.\n\
             \n\
             Sibling tools: `siyuan_prepend_block` adds as the FIRST child instead; \
             `siyuan_insert_block` inserts at a sibling position relative to an anchor (use \
             when the new block must be placed adjacent to a specific sibling). Use \
             siyuan_append_block when you want to add content at the end of a container \
             without needing the id of the last existing child.\n\
             \n\
             Inputs: `markdown` (required) is the GFM body. `parent_id` (required) is the \
             id of the container block — typically a document ROOT id or a list-item id. \
             The kernel chooses the destination as `parent_id`'s last position. Existing \
             children keep their ids and order.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"markdown\": \"Final paragraph.\\n\", \"parent_id\": \"20260501090000-doc0001\" }\n\
               out: { \"data\": { \"id\": \"20260501090500-blk0099\" }, \"_hint\": \"Block appended at the kernel. ...\" }",
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
            "Prepend a new markdown block as the FIRST child of a container.\n\
             \n\
             Sibling tools: `siyuan_append_block` adds as the LAST child instead; \
             `siyuan_insert_block` inserts at a sibling position relative to an anchor. \
             Use siyuan_prepend_block when you want to add content at the start of a \
             container without needing the id of the first existing child.\n\
             \n\
             Inputs: `markdown` (required) is the GFM body. `parent_id` (required) is the \
             id of the container block — typically a document ROOT id or a list-item id. \
             Existing children shift down by one position; their ids and subtrees are \
             unchanged.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"markdown\": \"Lead paragraph.\\n\", \"parent_id\": \"20260501090000-doc0001\" }\n\
               out: { \"data\": { \"id\": \"20260501090500-blk0099\" }, \"_hint\": \"Block prepended at the kernel. ...\" }",
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
            "Move an existing block to a new position within the document tree.\n\
             \n\
             Sibling tools: `siyuan_insert_block` / `siyuan_append_block` / \
             `siyuan_prepend_block` create NEW blocks (different ids) and cover the position \
             kinds that move-block does NOT — namely `before_block`, `append_section`, and \
             `prepend_section`; reach for those when the equivalent move would otherwise be \
             unsupported. `siyuan_doc_move` moves whole documents on disk (`.sy` files). \
             siyuan_move_block keeps the block's id and all its children — only its parent \
             and sibling order change.\n\
             \n\
             Inputs: `id` (required) is the block id to move. EXACTLY ONE of `previous_id` \
             or `parent_id` must be supplied; supplying zero or both is an error. There is \
             no separate `position` string field — the chosen anchor field IS the kind. \
             Sending a stray `position` argument (e.g. ported from an older CLI mental \
             model) is rejected with `invalid_params` and a hint pointing at \
             `siyuan_insert_block`. The supported kinds are:\n\
               previous_id  → moved block becomes a sibling immediately AFTER previous_id\n\
                              (anchor = any block id; this is the kernel's only relative-move\n\
                              direction — there is no `next_id` for move; use the previous\n\
                              sibling's id to achieve a 'before' move)\n\
               parent_id    → moved block becomes a child of parent_id; the kernel places it\n\
                              at the END of parent_id's children\n\
             The moved block keeps its id and entire subtree intact.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale position data for \
             ~100-500 ms after this call. The kernel is immediately consistent — only \
             the SQL index lags.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-blk0001\", \"previous_id\": \"20260501090000-blk0002\" }\n\
               out: { \"ok\": true }",
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
            "Permanently delete a block and all of its children.\n\
             \n\
             Sibling tools: `siyuan_update_block` with empty body clears a block in place \
             but keeps it; `siyuan_doc_remove` deletes a whole document by storage path \
             (and you can also delete a document by passing its root id here). \
             siyuan_delete_block removes the block and its subtree irreversibly.\n\
             \n\
             Inputs: `id` (required) is the block id to delete. Any block type is accepted, \
             including a document ROOT id (deletes the whole document). The action is \
             irreversible at the kernel level.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may briefly still return the block for \
             ~100-500 ms after this call. The kernel is immediately consistent — only the \
             SQL index lags.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-blk0001\" }\n\
               out: { \"ok\": true }",
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
            "Read all attributes (built-in and custom) of a block by id.\n\
             \n\
             Sibling tools: `siyuan_set_attrs` mutates attributes (partial update); call \
             siyuan_get_attrs first if you need to inspect existing values without \
             overwriting them. `siyuan_get_block` returns the block's kramdown body \
             (different concept).\n\
             \n\
             Inputs: `id` (required) is the block id. The response is a flat key-value \
             map: built-in keys are bare names (`id`, `type`, `title`, `icon`, `sort`, \
             ...); custom keys carry the `custom-` prefix.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-doc0001\" }\n\
               out: { \"id\": \"20260501090000-doc0001\", \"attrs\": { \"title\": \"Plan\", \"icon\": \":rocket:\", \"custom-priority\": \"high\" } }",
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
            "Set one or more attributes on a block (partial update).\n\
             \n\
             Sibling tools: `siyuan_get_attrs` reads the current map; `siyuan_update_block` \
             mutates the body, not attributes. There is no convenience tool for icon/sort \
             at the MCP layer (the CLI has them) — use this with `icon` / `sort` keys \
             directly.\n\
             \n\
             Inputs: `id` (required) is the block id. `attrs` (required) is an object of \
             `key: value` pairs. PARTIAL update: keys absent from `attrs` are left intact. \
             Empty value deletes the key. Custom keys MUST start with `custom-`; attempts \
             to set internal keys like `id` or `type` are silently ignored by the kernel.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-blk0001\", \"attrs\": { \"custom-priority\": \"high\", \"custom-owner\": \"alice\" } }\n\
               out: { \"ok\": true }",
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
            "List all notebooks in the workspace, both open and closed.\n\
             \n\
             Sibling tools: `siyuan_doc_resolve` looks up a single document by id or \
             hpath; this tool enumerates whole notebooks. Use it to discover notebook ids \
             before calling tools that need a `notebook` parameter \
             (`siyuan_create_doc`, `siyuan_doc_resolve`, etc.).\n\
             \n\
             Inputs: none required (extra properties ignored). Each notebook entry \
             includes `id`, `name`, `icon`, `sort`, and a `closed` boolean. Notebooks \
             closed in the SiYuan UI appear with `closed: true`; lookups inside them may \
             return empty results or a kernel error. Re-opening is a UI-only action — \
             this tool surface does not expose it.\n\
             \n\
             Example:\n\
               in:  {}\n\
               out: { \"notebooks\": [ { \"id\": \"20260501000000-nb00001\", \"name\": \"Inbox\", \"icon\": \"\", \"sort\": 0, \"closed\": false }, { \"id\": \"20250812000000-archived\", \"name\": \"Archive\", \"icon\": \"\", \"sort\": 1, \"closed\": true } ] }",
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
            "siyuan_notebook_create",
            "Create a new notebook with the given display name.\n\
             \n\
             Sibling tools: `siyuan_notebook_rename` only changes the display name of an \
             existing notebook. There is no programmatic open/close — the user opens or \
             closes notebooks in the SiYuan UI.\n\
             \n\
             Inputs: `name` (required) is any non-empty UTF-8 display string; duplicates \
             are allowed (the kernel disambiguates by id). The kernel assigns a unique id \
             automatically and surfaces it as `id` in the response (also under `data.id`).\n\
             \n\
             The new notebook is reachable for subsequent calls (`siyuan_doc_resolve`, \
             `siyuan_create_doc`, etc.). NOTE: some kernel versions create the notebook \
             in a CLOSED state — the harness still resolves it through `siyuan_doc_resolve` \
             and similar kernel-direct tools, but reads via `siyuan_sql` / \
             `siyuan_search_text` may return empty until the user opens it in the SiYuan UI.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"name\": \"Inbox\" }\n\
               out: { \"id\": \"20260501000000-nb00001\", \"data\": { \"id\": \"20260501000000-nb00001\", \"name\": \"Inbox\", \"icon\": \"\", \"sort\": 0, \"closed\": false } }",
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
            "Rename an existing notebook (display name only).\n\
             \n\
             Sibling tools: `siyuan_notebook_create` mints a new notebook; \
             `siyuan_notebook_remove` destroys one and all its documents. siyuan_notebook_rename \
             changes the display name only — the on-disk folder and the notebook id remain \
             stable, so storage paths inside it are unaffected.\n\
             \n\
             Inputs: `id` (required) is the notebook id (from `siyuan_notebook_ls`); \
             `name` (required) is the new display name.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags. `siyuan_notebook_ls` itself reflects the new name immediately.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501000000-nb00001\", \"name\": \"Triage\" }\n\
               out: { \"ok\": true }",
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
            "Permanently remove a notebook AND every document it contains.\n\
             \n\
             Sibling tools: `siyuan_doc_remove` removes a single document by storage path; \
             this tool destroys the whole notebook and is irreversible. Verify the \
             notebook id from `siyuan_notebook_ls` before calling.\n\
             \n\
             Inputs: `id` (required) is the notebook id.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501000000-nb00001\" }\n\
               out: { \"ok\": true }",
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
            "Look up document metadata by EITHER block id OR (notebook + hpath).\n\
             \n\
             Sibling tools: `siyuan_get_doc` returns the rendered document content (and \
             requires an id); this tool returns ONLY the metadata (id, hpath, notebook_id, \
             notebook_name, title, storage_path) and is the canonical hpath<->id translator. \
             `siyuan_notebook_ls` enumerates whole notebooks. Reach for this tool before \
             any rename/move/remove call to recover a storage path from an id or hpath.\n\
             \n\
             Inputs: provide EXACTLY ONE input mode — either `id` to recover hpath/notebook \
             from a known id (e.g. after a move/rename, or when only an id is in hand from \
             SQL/search), or `notebook` PLUS `hpath` to locate a document by its \
             human-readable path (must start with `/`). Supplying both modes, or neither, \
             is an error.\n\
             \n\
             Output is an array of matches under `docs`; an empty array means no such \
             document — this is NOT an error. The kernel allows duplicate hpaths in rare \
             edge cases so the array may contain more than one entry. Each entry has six \
             fields: `id` (block id), `hpath` (human path), `notebook_id`, `notebook_name`, \
             `title` (last `/`-delimited segment of `hpath`), and `storage_path` (the `.sy` \
             file path). The `storage_path` is what `siyuan_doc_rename`, `siyuan_doc_move`, \
             and `siyuan_doc_remove` take as their `path` / `from_paths` argument — those \
             endpoints accept STORAGE paths, NOT hpaths.\n\
             \n\
             Example:\n\
               in:  { \"id\": \"20260501090000-doc0001\" }\n\
               out: { \"docs\": [ { \"id\": \"20260501090000-doc0001\", \"hpath\": \"/Plan\", \"notebook_id\": \"20260501000000-nb00001\", \"notebook_name\": \"Inbox\", \"title\": \"Plan\", \"storage_path\": \"/20260501090000-doc0001.sy\" } ] }\n\
             \n\
               in:  { \"notebook\": \"20260501000000-nb00001\", \"hpath\": \"/Plan\" }\n\
               out: { \"docs\": [ { \"id\": \"20260501090000-doc0001\", \"hpath\": \"/Plan\", \"notebook_id\": \"20260501000000-nb00001\", \"notebook_name\": \"Inbox\", \"title\": \"Plan\", \"storage_path\": \"/20260501090000-doc0001.sy\" } ] }",
            r#"{"type":"object","properties":{"id":{"type":"string","description":"Document block id (use this OR notebook+hpath)"},"notebook":{"type":"string","description":"Notebook id (use with hpath)"},"hpath":{"type":"string","description":"Human path (use with notebook)"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::filetree::resolve(&c, args).await }
            })
        );
    }

    {
        let c = Arc::clone(&client);
        reg!(
            "siyuan_doc_rename",
            "Rename a document by changing its display title.\n\
             \n\
             Sibling tools: `siyuan_doc_move` changes the parent folder of a document; \
             this tool changes only its title (the last hpath segment). `siyuan_set_attrs` \
             with key `icon` is the analogous icon mutator.\n\
             \n\
             Inputs: `notebook` (required) is the notebook id. `path` (required) is the \
             on-disk STORAGE path with `.sy` suffix (e.g. `/20230101120000-abcdefg.sy`) — \
             NOT the human-readable hpath. Call `siyuan_doc_resolve` with the id to obtain \
             `storage_path` and pass that as `path` here. `title` (required) is the new \
             human-readable display name; a subsequent `siyuan_doc_resolve` reflects the \
             new title immediately.\n\
             \n\
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"notebook\": \"20260501000000-nb00001\", \"path\": \"/20260501090000-doc0001.sy\", \"title\": \"Q3 Plan\" }\n\
               out: { \"ok\": true }",
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
            "Move one or more documents to a different notebook/folder.\n\
             \n\
             Sibling tools: `siyuan_move_block` moves a block within a document tree \
             (block-level); `siyuan_doc_rename` only retitles. siyuan_doc_move relocates \
             whole `.sy` files in the file tree.\n\
             \n\
             Inputs: `from_paths` (required, non-empty array) holds source documents as \
             STORAGE `.sy` paths — NOT hpaths. `to_notebook` (required) is the destination \
             notebook id. `to_path` (required) is the destination FOLDER as a storage \
             path (e.g. `/Projects` or `/`); not an hpath, although for folders the two \
             often coincide because folders carry no `.sy` suffix. Each source's own `.sy` \
             filename is preserved at the target. Call `siyuan_doc_resolve` first to obtain \
             `storage_path` values from ids.\n\
             \n\
             After the move, `siyuan_doc_resolve` reflects the new location immediately. \
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"from_paths\": [\"/20260501090000-doc0001.sy\"], \"to_notebook\": \"20260501000000-nb00002\", \"to_path\": \"/\" }\n\
               out: { \"ok\": true }",
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
            "Permanently remove a document and all its child blocks.\n\
             \n\
             Sibling tools: `siyuan_delete_block` with the document root id deletes the \
             same content via the block API; `siyuan_doc_move` relocates instead of \
             deleting; `siyuan_notebook_remove` destroys an entire notebook. \
             siyuan_doc_remove is the per-document destroyer.\n\
             \n\
             Inputs: `notebook` (required) is the notebook id. `path` (required) is the \
             on-disk STORAGE path with `.sy` suffix — NOT the human-readable hpath. Call \
             `siyuan_doc_resolve` first to obtain the `storage_path` from an id or hpath.\n\
             \n\
             After removal, `siyuan_doc_resolve` no longer finds this document. \
             SiYuan indexes mutations asynchronously; SQL-based reads (siyuan_sql, \
             siyuan_search_text, siyuan_tag_search) may show stale data for ~100-500 ms \
             after this call. The kernel is immediately consistent — only the SQL index \
             lags.\n\
             \n\
             Example:\n\
               in:  { \"notebook\": \"20260501000000-nb00001\", \"path\": \"/20260501090000-doc0001.sy\" }\n\
               out: { \"ok\": true }",
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
            "List all distinct tags used anywhere in the workspace.\n\
             \n\
             Sibling tools: `siyuan_tag_search` finds blocks tagged with one specific tag; \
             this tool enumerates the available tags. `siyuan_search_text` is for free-text \
             content search, not tags.\n\
             \n\
             Inputs: none required. The output is a flat array of tag strings WITHOUT the \
             surrounding `#` characters — pass each value directly as the `tag` argument to \
             `siyuan_tag_search`. The list is derived from the SQL index and is eventually \
             consistent: freshly-created tags may take ~100-500 ms to appear (the kernel \
             itself is consistent immediately).\n\
             \n\
             Example:\n\
               in:  {}\n\
               out: { \"data\": { \"tags\": [\"project\", \"urgent\", \"idea\"] } }",
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
            "Find all blocks that carry a specific tag.\n\
             \n\
             Sibling tools: `siyuan_tag_ls` enumerates available tags; `siyuan_search_text` \
             does free-text matching instead of tag-exact match.\n\
             \n\
             Inputs: `tag` (required) is the tag content WITHOUT the surrounding `#` \
             characters — pass `project` to find blocks tagged `#project`. Match is exact \
             on the tag value. `limit` (optional, default 50) caps the result count and \
             is capped server-side at 1000.\n\
             \n\
             Results are eventually consistent with the SQL index — freshly-tagged blocks \
             may take ~100-500 ms to appear (the kernel is immediately consistent; only \
             the SQL index lags). The response envelope includes `data.hits` as an array \
             of block records.\n\
             \n\
             Example:\n\
               in:  { \"tag\": \"project\", \"limit\": 10 }\n\
               out: { \"data\": { \"hits\": [ { \"id\": \"20260501090000-blk0001\", \"root_id\": \"20260501090000-doc0001\", \"markdown\": \"Plan kickoff #project\" } ] } }",
            r#"{"type":"object","required":["tag"],"properties":{"tag":{"type":"string"},"limit":{"type":"integer","default":50}},"additionalProperties":true}"#,
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
            "Full-text search across all blocks using a SQL LIKE substring match.\n\
             \n\
             Sibling tools: `siyuan_tag_search` is exact tag match; `siyuan_sql` is the raw \
             escape hatch for arbitrary queries (joins, aggregates). siyuan_search_text \
             matches against the `markdown` column (includes inline syntax markers) — for \
             a `content` (visible-text) match, build the LIKE manually via `siyuan_sql`.\n\
             \n\
             Inputs: `query` (required, non-empty) is the substring. Single quotes are \
             escaped internally; LIKE meta-chars (`%`, `_`, `\\`) are NOT escaped — they \
             behave as wildcards. Matching is case-insensitive on most SQLite builds. \
             `limit` (optional, default 50) caps the result count.\n\
             \n\
             Results may lag recent mutations by ~100-500 ms (the kernel is immediately \
             consistent; only the SQL index lags). The response envelope includes \
             `data.hits` as an array of block records with `id`, `root_id`, `markdown`.\n\
             \n\
             Example:\n\
               in:  { \"query\": \"kickoff\", \"limit\": 10 }\n\
               out: { \"data\": { \"hits\": [ { \"id\": \"20260501090000-blk0001\", \"root_id\": \"20260501090000-doc0001\", \"markdown\": \"Plan kickoff for Q3\" } ] } }",
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
            "Execute a raw read-only SQL SELECT statement against the SiYuan SQLite database.\n\
             \n\
             Sibling tools: prefer `siyuan_search_text`, `siyuan_tag_search`, or \
             `siyuan_graph_neighborhood` when they cover the use case. Reach for siyuan_sql \
             ONLY for queries those do not (joins, aggregations, or access to internal \
             tables like `refs`, `attributes`, `spans`). The CLI exposes the same operation \
             as `siyuan sql --stmt ...`.\n\
             \n\
             Inputs: `stmt` (required) is a single SQL SELECT statement. A client-side \
             keyword check rejects non-SELECT/WITH statements before any kernel round trip \
             (whitespace and case are normalised; `WITH` is allowed for CTEs). The kernel \
             also rejects INSERT/UPDATE/DELETE/DDL on its own and will return an error if \
             anything slips past; in read-only / publish mode the endpoint itself is \
             disabled and returns `SqlUnavailable`.\n\
             \n\
             Critical caveat: the kernel does NOT parameterise the query — there is no \
             auto-escaping. Single quotes inside string literals must be doubled \
             (`'O''Brien'`); LIKE meta-chars (`%`, `_`, `\\`) must be escaped by the caller \
             and paired with an `ESCAPE '\\\\'` clause. Treat the value as literal SQL \
             text. `LIMIT` belongs inside the statement.\n\
             \n\
             Results are returned as an array of JSON objects where each key is a column \
             name. Some columns may be unstable internal fields. The SQL index lags writes \
             by ~100-500 ms — rows just inserted may not show up immediately even though \
             the kernel has them.\n\
             \n\
             Example:\n\
               in:  { \"stmt\": \"SELECT id, hpath FROM blocks WHERE box = '20260501000000-nb00001' AND type = 'd' LIMIT 5\" }\n\
               out: { \"data\": { \"rows\": [ { \"id\": \"20260501090000-doc0001\", \"hpath\": \"/Plan\" } ] }, \"_hint\": \"Power tool: results are raw rows ...\" }",
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
            "Upload a local file as a SiYuan asset.\n\
             \n\
             Sibling tools: there is no separate `reference` tool at the MCP layer (the CLI \
             has `siyuan asset reference` which is a pure formatter). Use the returned \
             asset path inside markdown that you pass to `siyuan_insert_block`, \
             `siyuan_append_block`, `siyuan_prepend_block`, `siyuan_update_block`, or \
             `siyuan_create_doc`.\n\
             \n\
             Inputs: `file_path` (required) is an ABSOLUTE path on the machine running \
             the MCP server (NOT on the SiYuan kernel host if they differ). The process \
             must have read access. The kernel copies the bytes into its `assets/` \
             directory and assigns a stable name; the response surfaces the kernel-relative \
             asset path under `data.asset_path`. To embed, include \
             `![alt](assets/<name>.<ext>)` in markdown.\n\
             \n\
             Example:\n\
               in:  { \"file_path\": \"/home/user/diagram.png\" }\n\
               out: { \"data\": { \"asset_path\": \"assets/diagram-20260501090000-abc.png\" }, \"_hint\": \"Asset stored at the returned path. ...\" }",
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
            "Compute the link-graph neighborhood around a center block up to a given depth.\n\
             \n\
             Sibling tools: `siyuan_sql` against the `refs` table gives unbounded results \
             when this tool's caps are insufficient. There are no separate backlinks / \
             outgoing tools at the MCP layer — set `direction` to `incoming` or `outgoing` \
             here to get those. The CLI exposes `siyuan graph backlinks` / `outgoing` as \
             depth-1 single-direction shortcuts.\n\
             \n\
             Inputs: `center` (required) is the center block id. `depth` (optional, default \
             1) is the hop count, CAPPED at 8 by the model layer. `direction` (optional, \
             default `both`) is one of `outgoing` (follow refs FROM the center to blocks it \
             references), `incoming` (follow refs TO the center), or `both`.\n\
             \n\
             Traversal stops at 500 nodes or 1000 edges per call. When either cap is hit, \
             `data.truncated` is `true` and the result is partial — narrow the query \
             (reduce depth, switch to a single direction, choose a more specific center) or \
             fall back to `siyuan_sql`. The response envelope contains `data.nodes`, \
             `data.edges`, and `data.truncated`.\n\
             \n\
             Example:\n\
               in:  { \"center\": \"20260501090000-blk0001\", \"depth\": 2, \"direction\": \"both\" }\n\
               out: { \"data\": { \"nodes\": [...], \"edges\": [...], \"truncated\": false } }",
            r#"{"type":"object","required":["center"],"properties":{"center":{"type":"string"},"depth":{"type":"integer","default":1},"direction":{"type":"string","enum":["outgoing","incoming","both"],"default":"both"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::graph::neighborhood(&c, args).await }
            })
        );
    }

    (tool_list, handlers)
}
