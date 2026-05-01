use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use rmcp::{
    ErrorData as McpError,
    model::{JsonObject, Tool},
};
use serde_json::Value;

use siyuan_client::SiyuanClient;

use crate::tools;

// Boxed async fn: (client, args) -> Result<Value, McpError>
pub type Handler = Arc<
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
pub fn build(client: Arc<SiyuanClient>) -> (Vec<Tool>, HashMap<&'static str, Handler>) {
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
            "Return the SiYuan kernel version. No parameters required.",
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
            "Load a SiYuan document as agent-friendly markdown or JSON. Pagination via page/page_size; format selects rendering.",
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
            "Fetch the kramdown source of a single block by its id.",
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
            "Create a new document in a notebook from GFM markdown.",
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
            "Replace the content of an existing block with new markdown.",
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
            "Insert a markdown block relative to an anchor. Exactly one of previous_id, next_id, parent_id must be given.",
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
            "Append a markdown block as the last child of a parent block.",
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
            "Prepend a markdown block as the first child of a parent block.",
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
            "Move a block to a new position. Exactly one of previous_id or parent_id must be given.",
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
            "Permanently delete a block and all its children.",
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
            "Read all custom attributes of a block.",
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
            "Set one or more custom attributes on a block.",
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
            "List all notebooks in the workspace.",
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
            "Open (mount) a notebook so its documents become accessible.",
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
            "Close (unmount) a notebook.",
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
            "Create a new notebook with the given display name.",
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
            "Rename an existing notebook.",
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
            "Permanently remove a notebook and all its documents.",
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
            "Resolve a human-readable path to one or more document block ids.",
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
            "Look up the human-readable path for a document given its block id.",
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
            "Rename a document. `path` is the storage .sy path (kernel quirk), not the human hpath.",
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
            "Move one or more documents to a different notebook/path.",
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
            "Permanently remove a document. `path` is the storage .sy path.",
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
            "List all distinct tags in the workspace.",
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
            "Find all blocks that carry a specific tag.",
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
            "Full-text search via SQL LIKE on the blocks table.",
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
            "Execute a raw read-only SQL statement against the SiYuan database. WARNING: dangerous; do not issue writes.",
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
            "Upload a local file as a SiYuan asset. Returns the kernel-relative asset path.",
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
            "Compute the link-graph neighborhood around a block up to a given depth.",
            r#"{"type":"object","required":["center"],"properties":{"center":{"type":"string"},"depth":{"type":"integer","default":1},"direction":{"type":"string","enum":["outgoing","incoming","both"],"default":"both"}},"additionalProperties":true}"#,
            make_handler(move |_, args| {
                let c = Arc::clone(&c);
                async move { tools::graph::neighborhood(&c, args).await }
            })
        );
    }

    (tool_list, handlers)
}
