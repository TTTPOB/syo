use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, resolve};
use siyuan_types::NotebookId;

/// Create a new document in a notebook from GFM markdown.
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
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct CreateDocArgs {
    /// Notebook id (from `syo notebook ls`).
    #[arg(long)]
    pub notebook: String,

    /// Human path inside the notebook, e.g. `/Projects/Plan`. Must start with `/`.
    #[arg(long)]
    pub hpath: String,

    /// Path to a markdown file. Use `-` for stdin.
    #[arg(long)]
    pub markdown_file: String,

    /// Skip the hpath-conflict check.
    #[arg(long)]
    pub force: bool,
}

pub async fn run(client: &SiyuanClient, args: CreateDocArgs) -> Result<()> {
    let notebook = NotebookId::parse(&args.notebook).context("--notebook")?;

    // Check for hpath conflicts unless --force
    if !args.force {
        let lookup = DocLookup::ByHpath {
            notebook: notebook.clone(),
            hpath: args.hpath.clone(),
        };
        let existing = resolve(client, lookup).await?;
        if !existing.is_empty() {
            let existing_id = &existing[0].id;
            bail!(
                "hpath {} already exists (id: {}). Use --force to overwrite.",
                args.hpath,
                existing_id
            );
        }
    }

    let markdown = super::super::read_markdown_input(&args.markdown_file)?;
    let id = client
        .create_doc_with_md(&notebook, &args.hpath, &markdown)
        .await?;
    println!("{id}");
    Ok(())
}
