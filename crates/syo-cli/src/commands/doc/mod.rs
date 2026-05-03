use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

/// Hpath clarification note. Appended to the doc comment of every subcommand
/// that accepts a `notebook` + `hpath` pair, so `--help` output preempts the
/// common confusion where callers mistake the first hpath segment for a
/// notebook name.
pub(crate) const HPATH_NOTE: &str = "\
Note: the first `/`-delimited segment of an hpath is NOT a notebook name \
— it is a top-level document title INSIDE the target notebook. (SiYuan \
has no folder concept — every path segment is a document.) The notebook \
is always supplied separately via the `notebook` parameter. \
Example: notebook `expnote`, hpath `/year2026/month12` means \
`expnote:/year2026/month12` (the notebook is `expnote`, the top-level \
document is `year2026`). Even when the notebook is named `hello` and \
the hpath is `/hello/world`, the first segment is still a document \
title: `hello[notebook]:/hello/world`.";

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
    /// Sibling commands: `syo doc get` returns rendered document content;
    /// this command returns only metadata and is the canonical hpath<->id
    /// translator. Provide exactly one input mode: `--id`, or `--notebook`
    /// plus `--hpath`.
    ///
    /// Note: the first `/`-delimited segment of an hpath is NOT a notebook
    /// name — it is a top-level document title INSIDE the target notebook.
    /// (SiYuan has no folder concept — every path segment is a document.)
    /// The notebook is always supplied separately via `--notebook`.
    /// Example: notebook `expnote`, hpath `/year2026/month12` means
    /// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
    /// `/hello/world`, the first segment is still a document title:
    /// `hello[notebook]:/hello/world`.
    #[command(verbatim_doc_comment)]
    Resolve(ResolveArgs),
    /// Rename a document by changing its display title.
    ///
    /// Provide exactly one locator mode (`--id`, or `--notebook` + `--hpath`)
    /// plus `--title`. Storage `.sy` paths are not accepted as input.
    ///
    /// Note: the first `/`-delimited segment of an hpath is NOT a notebook
    /// name — it is a top-level document title INSIDE the target notebook.
    /// (SiYuan has no folder concept — every path segment is a document.)
    /// The notebook is always supplied separately via `--notebook`.
    /// Example: notebook `expnote`, hpath `/year2026/month12` means
    /// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
    /// `/hello/world`, the first segment is still a document title:
    /// `hello[notebook]:/hello/world`.
    #[command(verbatim_doc_comment)]
    Rename(RenameArgs),
    /// Move one or more documents to a different notebook/folder.
    ///
    /// Source addressing supports either `--from-ids` or `--notebook` plus
    /// `--from-hpaths`. The destination is `--to-notebook` plus `--to-path`.
    ///
    /// Note: the first `/`-delimited segment of an hpath is NOT a notebook
    /// name — it is a top-level document title INSIDE the target notebook.
    /// (SiYuan has no folder concept — every path segment is a document.)
    /// The notebook is always supplied separately via `--notebook`.
    /// Example: notebook `expnote`, hpath `/year2026/month12` means
    /// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
    /// `/hello/world`, the first segment is still a document title:
    /// `hello[notebook]:/hello/world`.
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
    ///
    /// Note: the first `/`-delimited segment of an hpath is NOT a notebook
    /// name — it is a top-level document title INSIDE the target notebook.
    /// (SiYuan has no folder concept — every path segment is a document.)
    /// The notebook is always supplied separately via `--notebook`.
    /// Example: notebook `expnote`, hpath `/year2026/month12` means
    /// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
    /// `/hello/world`, the first segment is still a document title:
    /// `hello[notebook]:/hello/world`.
    #[command(verbatim_doc_comment)]
    Remove(RemoveArgs),
    /// List documents under a notebook/folder root as a tree.
    ///
    /// Address by `--id`, or by `--notebook` with optional `--hpath` (default
    /// `/`). Use `--depth N` or `--depth all` to control descendants.
    ///
    /// Note: the first `/`-delimited segment of an hpath is NOT a notebook
    /// name — it is a top-level document title INSIDE the target notebook.
    /// (SiYuan has no folder concept — every path segment is a document.)
    /// The notebook is always supplied separately via `--notebook`.
    /// Example: notebook `expnote`, hpath `/year2026/month12` means
    /// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
    /// `/hello/world`, the first segment is still a document title:
    /// `hello[notebook]:/hello/world`.
    #[command(verbatim_doc_comment)]
    Tree(TreeArgs),
    /// Get the rendered content of a document (agent-md, json, or json-bundle).
    Get(GetDocArgs),
    /// Create a new document in a notebook from markdown input.
    ///
    /// Sibling commands: `syo block update` replaces an existing block;
    /// `syo block insert` appends/inserts blocks under an existing document.
    /// Only use create-doc to mint a NEW document.
    ///
    /// Inputs:
    ///   --notebook (required): notebook id from `syo notebook ls`.
    ///   --hpath (required): human path inside the notebook, e.g.
    ///     `/Projects/Plan`. Must start with `/`. Intermediate folders are
    ///     auto-created. NOT to be confused with the on-disk `.sy` storage
    ///     path: hpaths are titles separated by `/`, storage paths look like
    ///     `/20260501090000-abc1234.sy`.
    ///   --markdown-file (required): path to a markdown file, or `-` to read
    ///     from stdin.
    ///   --force (optional): skip the hpath-conflict check. By default,
    ///     create-doc rejects duplicate hpaths with a clear error.
    ///
    /// Prints the new document's root block id to stdout.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads (syo sql,
    /// syo search text, syo tag search) may show stale data for ~100-500 ms
    /// after this call. The kernel is immediately consistent — only the SQL
    /// index lags.
    ///
    /// Example:
    ///   in:  --notebook 20260501000000-nb00001 --hpath /Plan --markdown-file plan.md
    ///   out: 20260501090000-doc0001
    ///
    /// Note: the first `/`-delimited segment of an hpath is NOT a notebook
    /// name — it is a top-level document title INSIDE the target notebook.
    /// (SiYuan has no folder concept — every path segment is a document.)
    /// The notebook is always supplied separately via `--notebook`.
    /// Example: notebook `expnote`, hpath `/year2026/month12` means
    /// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
    /// `/hello/world`, the first segment is still a document title:
    /// `hello[notebook]:/hello/world`.
    #[command(verbatim_doc_comment)]
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
