use anyhow::{Result, bail};
use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{
    DocLookup, ResolvedDoc, resolve as resolve_doc_meta, resolve_one_storage,
};
use siyuan_model::doc_tree::{Depth, TreeNode, build_tree};
use siyuan_model::load::load_doc;
use siyuan_model::pagination::PageRequest;
use siyuan_render::agent_md::render_doc;
use siyuan_render::json_bundle::render_bundle;
use siyuan_types::{BlockId, NotebookId};

// --- get ---
pub enum DocFormat {
    AgentMd,
    Json,
    JsonPretty,
}

pub struct GetDocInput {
    pub id: BlockId,
    pub page: usize,
    pub page_size: usize,
    pub format: DocFormat,
}

pub struct GetDocOutput {
    pub content: String,
}

pub async fn get(client: &SiyuanClient, input: GetDocInput) -> Result<GetDocOutput> {
    let bundle = load_doc(
        client,
        &input.id,
        PageRequest {
            page: input.page,
            page_size: input.page_size,
        },
    )
    .await?;
    let content = match input.format {
        DocFormat::AgentMd => render_doc(&bundle),
        DocFormat::Json => render_bundle(&bundle, false)?,
        DocFormat::JsonPretty => render_bundle(&bundle, true)?,
    };
    Ok(GetDocOutput { content })
}

// --- create ---
pub struct CreateDocInput {
    pub notebook: NotebookId,
    pub hpath: String,
    pub markdown: String,
    pub force: bool,
}

pub struct CreateDocOutput {
    pub id: BlockId,
}

pub async fn create(client: &SiyuanClient, input: CreateDocInput) -> Result<CreateDocOutput> {
    if !input.force {
        let lookup = DocLookup::ByHpath {
            notebook: input.notebook.clone(),
            hpath: input.hpath.clone(),
        };
        let existing = resolve_doc_meta(client, lookup).await?;
        if !existing.is_empty() {
            bail!(
                "hpath {} already exists (id: {}). Use force to overwrite.",
                input.hpath,
                existing[0].id
            );
        }
    }
    let id = client
        .create_doc_with_md(&input.notebook, &input.hpath, &input.markdown)
        .await?;
    Ok(CreateDocOutput { id })
}

// --- resolve ---
pub struct ResolveOutput {
    pub docs: Vec<ResolvedDoc>,
}

pub async fn resolve(client: &SiyuanClient, lookup: DocLookup) -> Result<ResolveOutput> {
    let docs = resolve_doc_meta(client, lookup).await?;
    Ok(ResolveOutput { docs })
}

// --- rename ---
pub struct RenameDocInput {
    pub lookup: DocLookup,
    pub title: String,
}

pub async fn rename(client: &SiyuanClient, input: RenameDocInput) -> Result<()> {
    let (notebook, storage_path) = resolve_one_storage(client, input.lookup).await?;
    client
        .rename_doc(&notebook, &storage_path, &input.title)
        .await?;
    Ok(())
}

// --- move_docs ---
pub struct MoveDocsInput {
    pub from: Vec<DocLookup>,
    pub to_notebook: NotebookId,
    pub to_path: String,
}

pub async fn move_docs(client: &SiyuanClient, input: MoveDocsInput) -> Result<()> {
    let mut from_paths = Vec::with_capacity(input.from.len());
    for lookup in &input.from {
        let (_nb, storage_path) = resolve_one_storage(client, lookup.clone()).await?;
        from_paths.push(storage_path);
    }
    client
        .move_docs(&from_paths, &input.to_notebook, &input.to_path)
        .await?;
    Ok(())
}

// --- remove ---
pub struct RemoveDocInput {
    pub lookup: DocLookup,
}

pub async fn remove(client: &SiyuanClient, input: RemoveDocInput) -> Result<()> {
    let (notebook, storage_path) = resolve_one_storage(client, input.lookup).await?;
    client.remove_doc(&notebook, &storage_path).await?;
    Ok(())
}

// --- tree ---
pub struct TreeInput {
    pub lookup: DocLookup,
    pub depth: Depth,
}

pub struct TreeOutput {
    pub tree: TreeNode,
}

pub async fn tree(client: &SiyuanClient, input: TreeInput) -> Result<TreeOutput> {
    let tree = build_tree(client, input.lookup, input.depth).await?;
    Ok(TreeOutput { tree })
}
