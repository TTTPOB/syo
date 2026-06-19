use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;
use siyuan_types::position::PositionKind;

/// Insert a new markdown block (or blocks) at a position relative to an anchor.
///
/// Sibling commands: `syo block move` moves an EXISTING block (keeps id);
/// `syo block update` overwrites a block in place; `syo doc create`
/// mints a whole new document. Use `syo block insert` only to add NEW
/// blocks inside an existing document.
///
/// Inputs:
///   --position (required): one of the eight kinds below.
///   --anchor (required): a block id whose role depends on --position.
///   --markdown-file (required): markdown file, or `-` for stdin. Multiple
///     blocks can be created from a single call (one per top-level GFM
///     element).
///
/// --position kinds (each describes where the new block lands):
///   after_block       new block is a sibling immediately AFTER --anchor
///                     (--anchor = any block id; existing later siblings shift down)
///   before_block      new block is a sibling immediately BEFORE --anchor
///                     (--anchor = any block id; existing siblings stay in order)
///   append_child      new block is the LAST child of container --anchor
///                     (--anchor = container id, e.g. list item, blockquote, doc root)
///   prepend_child     new block is the FIRST child of container --anchor
///                     (--anchor = container id; existing children shift down)
///   append_section    new block is the LAST block in the heading section owned by --anchor
///                     (--anchor MUST be a heading block id; the section ends at the next
///                      same-or-higher-level heading or end-of-doc)
///   prepend_section   new block is inserted IMMEDIATELY AFTER the heading block --anchor
///                     (--anchor MUST be a heading block id; effectively the first block
///                      of the section)
///   append_doc        new block is the LAST block of document --anchor
///                     (--anchor MUST be a doc root id)
///   prepend_doc       new block is the FIRST block of document --anchor
///                     (--anchor MUST be a doc root id)
///
/// In all cases existing blocks keep their ids and their children; only
/// sibling ordering changes. Prints the new block id to stdout (when several
/// top-level blocks are created in one call, the kernel returns the id of
/// the first).
///
/// Notes on parsing: append_section walks the document via SQL + the
/// section detector to locate the section's last block. Anchor MUST be a
/// heading block (`type = 'h'`) for *_section kinds, or the call errors.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (syo sql,
/// syo search text, syo tag search) may show stale data for ~100-500 ms
/// after this call. The kernel is immediately consistent — only the SQL
/// index lags.
///
/// Example:
///   in:  --position after_block --anchor 20260501090000-blk0001 --markdown-file note.md
///   out: 20260501090500-blk0099
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct InsertBlocksArgs {
    /// Position kind. One of: after_block, before_block, append_child,
    /// prepend_child, append_section, prepend_section, append_doc, prepend_doc.
    /// See command help for the meaning of each.
    #[arg(long, value_parser = super::super::parse_position)]
    pub position: PositionKind,

    /// Anchor block id. Interpretation depends on --position (see help).
    #[arg(long)]
    pub anchor: String,

    /// Markdown file to insert. Use `-` for stdin.
    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: InsertBlocksArgs) -> Result<()> {
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    let markdown = super::super::read_markdown_input(&args.markdown_file)?;
    let result = syo_core::block::insert(
        client,
        syo_core::block::InsertBlockInput {
            markdown,
            position: args.position,
            anchor,
        },
    )
    .await?;
    println!("{}", result.id);
    Ok(())
}
