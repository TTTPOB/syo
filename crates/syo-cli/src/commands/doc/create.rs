use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;

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
    let notebook = syo_core::notebook::resolve_notebook_id(client, &args.notebook)
        .await
        .context("--notebook")?;
    let markdown = super::super::read_markdown_input(&args.markdown_file)?;
    let id = syo_core::doc::create(
        client,
        syo_core::doc::CreateDocInput {
            notebook,
            hpath: args.hpath,
            markdown,
            force: args.force,
        },
    )
    .await?
    .id;
    println!("{id}");
    Ok(())
}
