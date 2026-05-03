use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

pub mod create;
pub mod get;
mod lookup;
pub mod r#move;
pub mod remove;
pub mod rename;
pub mod resolve;
pub mod set_icon;
pub mod set_sort;
pub mod tree;

pub use create::CreateDocArgs;
pub use get::GetDocArgs;
pub use r#move::MoveArgs;
pub use remove::RemoveArgs;
pub use rename::RenameArgs;
pub use resolve::ResolveArgs;
pub use set_icon::IconArgs;
pub use set_sort::SortArgs;
pub use tree::TreeArgs;

/// Manage documents: resolve, rename, move, set icon/sort, remove, tree,
/// get rendered content, and create new documents.
#[derive(Subcommand, Debug)]
pub enum DocCmd {
    /// Look up document metadata by id OR by (notebook + hpath).
    ///
    /// Sibling commands: `siyuan doc get` returns rendered document content;
    /// this command returns only metadata and is the canonical hpath<->id
    /// translator. Provide exactly one input mode: `--id`, or `--notebook`
    /// plus `--hpath`.
    #[command(verbatim_doc_comment)]
    Resolve(ResolveArgs),
    /// Rename a document by changing its display title.
    ///
    /// Provide exactly one locator mode (`--id`, or `--notebook` + `--hpath`)
    /// plus `--title`. Storage `.sy` paths are not accepted as input.
    #[command(verbatim_doc_comment)]
    Rename(RenameArgs),
    /// Move one or more documents to a different notebook/folder.
    ///
    /// Source addressing supports either `--from-ids` or `--notebook` plus
    /// `--from-hpaths`. The destination is `--to-notebook` plus `--to-path`.
    #[command(verbatim_doc_comment)]
    Move(MoveArgs),
    /// Set the document's `icon` attribute (or clear it with empty value).
    #[command(verbatim_doc_comment)]
    SetIcon(IconArgs),
    /// Set the document's `sort` attribute (manual ordering hint).
    #[command(verbatim_doc_comment)]
    SetSort(SortArgs),
    /// Permanently remove a document and all its child blocks.
    ///
    /// Provide exactly one locator mode: `--id`, or `--notebook` + `--hpath`.
    /// Storage `.sy` paths are not accepted.
    #[command(verbatim_doc_comment)]
    Remove(RemoveArgs),
    /// List documents under a notebook/folder root as a tree.
    ///
    /// Address by `--id`, or by `--notebook` with optional `--hpath` (default
    /// `/`). Use `--depth N` or `--depth all` to control descendants.
    #[command(verbatim_doc_comment)]
    Tree(TreeArgs),
    /// Get the rendered content of a document (agent-md, json, or json-bundle).
    Get(GetDocArgs),
    /// Create a new document in a notebook from markdown input.
    Create(CreateDocArgs),
}

pub async fn run(client: &SiyuanClient, cmd: DocCmd) -> Result<()> {
    match cmd {
        DocCmd::Resolve(a) => resolve::run(client, a).await,
        DocCmd::Rename(a) => rename::run(client, a).await,
        DocCmd::Move(a) => r#move::run(client, a).await,
        DocCmd::SetIcon(a) => set_icon::run(client, a).await,
        DocCmd::SetSort(a) => set_sort::run(client, a).await,
        DocCmd::Remove(a) => remove::run(client, a).await,
        DocCmd::Tree(a) => tree::run(client, a).await,
        DocCmd::Get(a) => get::run(client, a).await,
        DocCmd::Create(a) => create::run(client, a).await,
    }
}
