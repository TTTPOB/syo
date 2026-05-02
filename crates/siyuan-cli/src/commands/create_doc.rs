use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

/// Create a new document in a notebook from GFM markdown.
///
/// Sibling commands: `siyuan update-block` replaces an existing block;
/// `siyuan insert-blocks` appends/inserts blocks under an existing document.
/// Only use create-doc to mint a NEW document.
///
/// Inputs:
///   --notebook (required): notebook id from `siyuan notebook ls`.
///   --hpath (required): human path inside the notebook, e.g.
///     `/Projects/Plan`. Must start with `/`. Intermediate folders are
///     auto-created. NOT to be confused with the on-disk `.sy` storage
///     path: hpaths are titles separated by `/`, storage paths look like
///     `/20260501090000-abc1234.sy`.
///   --markdown-file (required): path to a markdown file, or `-` to read
///     from stdin.
///
/// Prints the new document's root block id to stdout.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (siyuan sql,
/// siyuan search text, siyuan tag search) may show stale data for ~100-500 ms
/// after this call. The kernel is immediately consistent — only the SQL
/// index lags.
///
/// Example:
///   in:  --notebook 20260501000000-nb00001 --hpath /Plan --markdown-file plan.md
///   out: 20260501090000-doc0001
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct CreateDocArgs {
    /// Notebook id (from `siyuan notebook ls`).
    #[arg(long)]
    pub notebook: String,

    /// Human path inside the notebook, e.g. `/Projects/Plan`. Must start with `/`.
    #[arg(long)]
    pub hpath: String,

    /// Path to a markdown file. Use `-` for stdin.
    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: CreateDocArgs) -> Result<()> {
    let notebook = NotebookId::parse(&args.notebook).context("--notebook")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    let id = client
        .create_doc_with_md(&notebook, &args.hpath, &markdown)
        .await?;
    println!("{id}");
    Ok(())
}
