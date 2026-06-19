use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

/// Replace the full markdown content of an existing block.
///
/// Sibling commands: `syo block insert` adds NEW blocks at a position
/// relative to an anchor; `syo block delete` removes a block; this
/// command is for in-place full overwrite of the markdown body. Partial
/// edits are NOT supported — read with `syo block get` first if you need
/// to preserve part of the existing content.
///
/// Real SiYuan container blocks (lists, list items, blockquotes, superblocks)
/// are replaced as subtrees by the kernel: children absent from the new
/// markdown are removed. Heading blocks are not real containers; by default
/// only the heading block is updated. Pass `--include-heading-children` to
/// replace the full heading section. In that mode the input markdown must
/// start with the replacement heading, followed by the new section body.
///
/// Inputs:
///   --id (required): block id to overwrite.
///   --markdown-file (required): path to a markdown file, or `-` to read
///     from stdin. The content replaces the entire block body.
///   --include-heading-children: only valid for heading blocks. Replace the
///     heading and its section children as one explicit section operation.
///
/// Prints `ok` on success.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (syo sql,
/// syo search text, syo tag search) may show stale data for ~100-500 ms
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

    /// Replace the whole heading section when --id is a heading block.
    #[arg(long)]
    pub include_heading_children: bool,
}

pub async fn run(client: &SiyuanClient, args: UpdateBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let markdown = super::super::read_markdown_input(&args.markdown_file)?;
    syo_core::block::update(
        client,
        syo_core::block::UpdateBlockInput {
            id,
            markdown,
            include_heading_children: args.include_heading_children,
        },
    )
    .await?;
    println!("ok");
    Ok(())
}
