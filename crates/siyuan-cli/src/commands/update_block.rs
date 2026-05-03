use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

/// Replace the full markdown content of an existing block.
///
/// Sibling commands: `siyuan block insert` adds NEW blocks at a position
/// relative to an anchor; `siyuan block delete` removes a block; this
/// command is for in-place full overwrite of the markdown body. Partial
/// edits are NOT supported — read with `siyuan block get` first if you need
/// to preserve part of the existing content.
///
/// Inputs:
///   --id (required): block id to overwrite.
///   --markdown-file (required): path to a markdown file, or `-` to read
///     from stdin. The content replaces the entire block body.
///
/// Prints `ok` on success.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (siyuan sql,
/// siyuan search text, siyuan tag search) may show stale data for ~100-500 ms
/// after this call. The kernel is immediately consistent — only the SQL
/// index lags.
///
/// Example:
///   in:  --id 20260501090000-blk0001 --markdown-file new.md
///   out: ok
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct UpdateBlockArgs {
    /// Block id to overwrite.
    #[arg(long)]
    pub id: String,

    /// Markdown file replacing the block body. Use `-` for stdin.
    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: UpdateBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    client.update_block_markdown(&id, &markdown).await?;
    println!("ok");
    Ok(())
}
