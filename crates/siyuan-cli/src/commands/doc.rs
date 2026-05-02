use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgGroup, Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, resolve as resolve_doc_meta};
use siyuan_types::{BlockId, NotebookId};

use crate::output::OutputFormat;

#[derive(Subcommand, Debug)]
pub enum DocCmd {
    /// Look up document metadata by id OR by (notebook + hpath).
    ///
    /// Sibling commands: `siyuan get-doc` returns the rendered document
    /// content (requires id); this command returns ONLY the metadata
    /// (id, hpath, notebook_id, notebook_name, title, storage_path) and
    /// is the canonical hpath<->id translator. `siyuan notebook ls`
    /// enumerates whole notebooks.
    ///
    /// Provide EXACTLY ONE input mode: either `--id` to recover the
    /// hpath/notebook from a known id (e.g. after a move or rename, or
    /// when only an id is in hand from SQL/search results), or
    /// `--notebook` plus `--hpath` together to look up by human path
    /// (when you only know the title/path).
    ///
    /// Output is a JSON array of matches (`docs`); an empty array means
    /// no such document — this is NOT an error. The kernel allows
    /// duplicate hpaths in rare edge cases, so a hpath lookup may return
    /// multiple entries. Each entry has six fields: `id`, `hpath`,
    /// `notebook_id`, `notebook_name`, `title`, and `storage_path`. The
    /// `storage_path` (`.sy`-suffixed) is what the rename/move/remove
    /// commands take as their `--path` / `--from-paths` argument — those
    /// commands take STORAGE paths, not hpaths.
    ///
    /// Inputs:
    ///   --format (default json-pretty): `json-pretty` (the indented form
    ///     shown above), or `json` (the same array, compact). `agent-md`
    ///     is rejected — this output is structured metadata, not prose.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001
    ///   out: [{"id":"20260501090000-doc0001","hpath":"/Plan","notebook_id":"20260501000000-nb00001","notebook_name":"Inbox","title":"Plan","storage_path":"/20260501090000-doc0001.sy"}]
    ///
    ///   in:  --notebook 20260501000000-nb00001 --hpath /Plan
    ///   out: [{"id":"20260501090000-doc0001","hpath":"/Plan","notebook_id":"20260501000000-nb00001","notebook_name":"Inbox","title":"Plan","storage_path":"/20260501090000-doc0001.sy"}]
    #[command(verbatim_doc_comment)]
    Resolve(ResolveArgs),
    /// Rename a document by changing its display title.
    ///
    /// Sibling commands: `siyuan doc move` changes the parent folder of a
    /// document; this changes only its title (the last hpath segment).
    /// `siyuan doc set-icon` sets the icon attribute alongside the title.
    ///
    /// Inputs:
    ///   --notebook (required): notebook id.
    ///   --path (required): on-disk STORAGE path with `.sy` suffix (e.g.
    ///     `/20260501090000-doc0001.sy`). NOT the hpath. Get it from
    ///     `siyuan doc resolve` (the `storage_path` field).
    ///   --title (required): new human-readable display title.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --notebook 20260501000000-nb00001 --path /20260501090000-doc0001.sy --title "Q3 Plan"
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Rename(RenameArgs),
    /// Move one or more documents to a different notebook/folder.
    ///
    /// Sibling commands: `siyuan move-block` moves a block within the
    /// document tree (block-level); `siyuan doc rename` only retitles a
    /// document. doc move relocates whole `.sy` files in the file tree.
    ///
    /// Inputs:
    ///   --from-paths (required, one-or-more): STORAGE paths (`.sy`
    ///     suffix), space-separated. NOT hpaths. Each must exist.
    ///   --to-notebook (required): destination notebook id.
    ///   --to-path (required): destination FOLDER as a storage path
    ///     (e.g. `/Projects` or `/`). NOT an hpath, although in practice
    ///     for folders the two often coincide because folders have no
    ///     `.sy` suffix. The trailing target inherits each source's own
    ///     `.sy` filename.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --from-paths /20260501090000-doc0001.sy --to-notebook 20260501000000-nb00002 --to-path /
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Move(MoveArgs),
    /// Set the document's `icon` attribute (or clear it with empty value).
    ///
    /// Sibling commands: `siyuan set-attrs --attr icon=...` does the
    /// same thing for any block; this is just a convenience wrapper for
    /// document roots. Use `siyuan doc set-sort` to change ordering.
    ///
    /// Inputs:
    ///   --id (required): document root block id.
    ///   --icon (optional, default empty): icon name (e.g. emoji
    ///     shortcode `:rocket:`) or empty string to clear.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001 --icon :rocket:
    ///   out: ok
    #[command(verbatim_doc_comment)]
    SetIcon(IconArgs),
    /// Set the document's `sort` attribute (manual ordering hint).
    ///
    /// Sibling commands: `siyuan set-attrs --attr sort=N` is the generic
    /// equivalent. SiYuan uses `sort` as the sibling-ordering key when the
    /// notebook is configured for manual sort.
    ///
    /// Inputs:
    ///   --id (required): document root block id.
    ///   --sort (required): integer; lower values sort earlier.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001 --sort 100
    ///   out: ok
    #[command(verbatim_doc_comment)]
    SetSort(SortArgs),
    /// Permanently remove a document and all its child blocks.
    ///
    /// Sibling commands: `siyuan delete-block --id <doc-root-id>` is a
    /// block-level delete that also drops the document; `siyuan doc move`
    /// relocates instead of deleting; `siyuan notebook remove` destroys
    /// the entire notebook. doc remove is the per-document destroyer.
    ///
    /// Inputs:
    ///   --notebook (required): notebook id.
    ///   --path (required): STORAGE path (`.sy` suffix) — NOT an hpath.
    ///     Get it from `siyuan doc resolve` (the `storage_path` field).
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --notebook 20260501000000-nb00001 --path /20260501090000-doc0001.sy
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Remove(RemoveArgs),
}

/// Arguments for `siyuan doc resolve`.
///
/// Mutual exclusion is enforced both by clap (via `ArgGroup` so partial
/// supply produces a friendly clap error) and again at runtime when the
/// `DocLookup` enum is constructed — the model layer is the canonical
/// validator and the CLI layer is the user-facing one.
#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("doc_lookup")
        .args(["id", "hpath"])
        .required(true)
))]
pub struct ResolveArgs {
    /// Document block id. Use to recover hpath/notebook from a known id.
    #[arg(long, conflicts_with_all = ["notebook", "hpath"])]
    pub id: Option<String>,

    /// Notebook id (use together with --hpath to look up by human path).
    #[arg(long, requires = "hpath")]
    pub notebook: Option<String>,

    /// Human path inside the notebook, e.g. `/Projects/Plan`.
    #[arg(long, requires = "notebook")]
    pub hpath: Option<String>,

    /// Output format: `json-pretty` (default), or `json` (compact).
    /// `agent-md` is not supported for resolve — the output is structured
    /// metadata, not prose.
    #[arg(long, value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct RenameArgs {
    /// Notebook id (from `siyuan notebook ls`).
    #[arg(long)]
    pub notebook: String,
    /// Storage path with `.sy` suffix (e.g. `/20260501090000-abc1234.sy`).
    /// NOT an hpath. Get via `siyuan doc resolve`.
    #[arg(long)]
    pub path: String,
    /// New display title.
    #[arg(long)]
    pub title: String,
}

#[derive(Args, Debug)]
pub struct MoveArgs {
    /// One or more source documents as storage `.sy` paths. NOT hpaths.
    #[arg(long, num_args = 1.., value_name = "STORAGE_PATH")]
    pub from_paths: Vec<String>,
    /// Destination notebook id.
    #[arg(long)]
    pub to_notebook: String,
    /// Destination FOLDER as a storage path (e.g. `/Projects` or `/`).
    #[arg(long)]
    pub to_path: String,
}

#[derive(Args, Debug)]
pub struct IconArgs {
    /// Document block id.
    #[arg(long)]
    pub id: String,
    /// Icon name (e.g. emoji shortcode like ":rocket:") or empty to clear.
    #[arg(long, default_value = "")]
    pub icon: String,
}

#[derive(Args, Debug)]
pub struct SortArgs {
    /// Document root block id.
    #[arg(long)]
    pub id: String,
    /// Manual sort key (lower sorts earlier).
    #[arg(long)]
    pub sort: i64,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    /// Notebook id (from `siyuan notebook ls`).
    #[arg(long)]
    pub notebook: String,
    /// Storage `.sy` path of the document. NOT an hpath. Get via `siyuan doc resolve`.
    #[arg(long)]
    pub path: String,
}

pub async fn run(client: &SiyuanClient, cmd: DocCmd) -> Result<()> {
    match cmd {
        DocCmd::Resolve(a) => {
            // Build DocLookup with the same exclusivity rule as the MCP layer.
            // Clap's ArgGroup already prevents most invalid combinations, but
            // we re-check here so the model layer remains the canonical
            // gate — anything that builds a DocLookup goes through the same
            // door regardless of caller.
            let lookup = match (a.id.as_deref(), a.notebook.as_deref(), a.hpath.as_deref()) {
                (Some(id), None, None) => {
                    DocLookup::ById(BlockId::parse(id.trim()).context("--id")?)
                }
                (None, Some(nb), Some(hp)) => DocLookup::ByHpath {
                    notebook: NotebookId::parse(nb.trim()).context("--notebook")?,
                    hpath: hp.to_string(),
                },
                (Some(_), _, _) => {
                    return Err(anyhow!(
                        "--id conflicts with --notebook/--hpath; pick exactly one input mode"
                    ));
                }
                _ => {
                    return Err(anyhow!(
                        "provide either --id, or both --notebook and --hpath"
                    ));
                }
            };
            let docs = resolve_doc_meta(client, lookup).await?;
            // `resolve` output is structured metadata; the `agent-md`
            // variant has no sensible mapping (we'd be inventing prose
            // around already-structured fields). Reject it loudly so the
            // user picks a JSON variant rather than getting a silent
            // pretty-printed default that masks the misuse.
            let s = match a.format {
                OutputFormat::AgentMd => {
                    bail!(
                        "doc resolve does not support --format agent-md; use json or json-pretty"
                    );
                }
                OutputFormat::Json => serde_json::to_string(&docs)?,
                OutputFormat::JsonPretty => serde_json::to_string_pretty(&docs)?,
            };
            println!("{s}");
        }
        DocCmd::Rename(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            client.rename_doc(&nb, &a.path, &a.title).await?;
            println!("ok");
        }
        DocCmd::Move(a) => {
            let to_nb = NotebookId::parse(&a.to_notebook).context("--to-notebook")?;
            client.move_docs(&a.from_paths, &to_nb, &a.to_path).await?;
            println!("ok");
        }
        DocCmd::SetIcon(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("icon".to_string(), a.icon);
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::SetSort(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("sort".to_string(), a.sort.to_string());
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::Remove(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            client.remove_doc(&nb, &a.path).await?;
            println!("ok");
        }
    }
    Ok(())
}
