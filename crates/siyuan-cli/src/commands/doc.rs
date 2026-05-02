use anyhow::{Context, Result, anyhow};
use clap::{ArgGroup, Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, resolve as resolve_doc_meta};
use siyuan_types::{BlockId, NotebookId};

#[derive(Subcommand, Debug)]
pub enum DocCmd {
    /// Look up document metadata by id OR by (notebook + hpath).
    ///
    /// Provide EXACTLY ONE input mode: either `--id` to look up a document
    /// by its block id (recovers the hpath after a move/rename), or
    /// `--notebook` and `--hpath` together to locate a document by its
    /// human-readable path (gives you the id and storage_path needed by
    /// other commands). Output is a JSON array of matches; an empty array
    /// means no such document. The kernel allows duplicate hpaths in rare
    /// edge cases, so a hpath lookup may return multiple entries.
    Resolve(ResolveArgs),
    Rename(RenameArgs),
    Move(MoveArgs),
    SetIcon(IconArgs),
    SetSort(SortArgs),
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
    /// Document block id. Use this OR `--notebook` + `--hpath`, not both.
    /// Pick this direction when you have an id (e.g. from search results)
    /// and want to recover its human-readable hpath.
    #[arg(long, conflicts_with_all = ["notebook", "hpath"])]
    pub id: Option<String>,

    /// Notebook id. Required together with `--hpath`; conflicts with `--id`.
    #[arg(long, requires = "hpath")]
    pub notebook: Option<String>,

    /// Human-readable path like `/Folder/Title`. Required together with
    /// `--notebook`; conflicts with `--id`. Pick this direction when you
    /// know the document's title/path and need its id or storage path.
    #[arg(long, requires = "notebook")]
    pub hpath: Option<String>,
}

#[derive(Args, Debug)]
pub struct RenameArgs {
    #[arg(long)]
    pub notebook: String,
    /// Storage path (e.g. `/20260501090000-abc1234.sy`). Get via `doc resolve` then look up.
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub title: String,
}

#[derive(Args, Debug)]
pub struct MoveArgs {
    #[arg(long, num_args = 1.., value_name = "STORAGE_PATH")]
    pub from_paths: Vec<String>,
    #[arg(long)]
    pub to_notebook: String,
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
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub sort: i64,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    #[arg(long)]
    pub notebook: String,
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
            // Emit a single pretty JSON array so downstream tooling can pipe
            // through `jq` without a per-line shimmying.
            println!("{}", serde_json::to_string_pretty(&docs)?);
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
