use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::section::populate_section_children;
use siyuan_types::{BlockId, BlockType, Position};

/// Insert a new markdown block (or blocks) at a position relative to an anchor.
///
/// Sibling commands: `siyuan move-block` moves an EXISTING block (keeps id);
/// `siyuan update-block` overwrites a block in place; `siyuan create-doc`
/// mints a whole new document. Use insert-blocks only to add NEW blocks
/// inside an existing document.
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
/// SiYuan indexes mutations asynchronously; SQL-based reads (siyuan sql,
/// siyuan search text, siyuan tag search) may show stale data for ~100-500 ms
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
    #[arg(long)]
    pub position: String,

    /// Anchor block id. Interpretation depends on --position (see help).
    #[arg(long)]
    pub anchor: String,

    /// Markdown file to insert. Use `-` for stdin.
    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: InsertBlocksArgs) -> Result<()> {
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    let position = parse_position(&args.position, anchor.clone())?;

    let new_id = match position {
        Position::AfterBlock { block_id } => {
            client
                .insert_block_markdown(&markdown, Some(&block_id), None, None)
                .await?
        }
        Position::BeforeBlock { block_id } => {
            client
                .insert_block_markdown(&markdown, None, Some(&block_id), None)
                .await?
        }
        Position::AppendChild { container_id } => {
            client
                .append_block_markdown(&markdown, &container_id)
                .await?
        }
        Position::PrependChild { container_id } => {
            client
                .prepend_block_markdown(&markdown, &container_id)
                .await?
        }
        Position::AppendSection { heading_id } => {
            let section_end = resolve_section_end(client, &heading_id).await?;
            client
                .insert_block_markdown(&markdown, Some(&section_end), None, None)
                .await?
        }
        Position::PrependSection { heading_id } => {
            // Right after the heading itself.
            client
                .insert_block_markdown(&markdown, Some(&heading_id), None, None)
                .await?
        }
        Position::AppendDoc { doc_id } => client.append_block_markdown(&markdown, &doc_id).await?,
        Position::PrependDoc { doc_id } => {
            client.prepend_block_markdown(&markdown, &doc_id).await?
        }
    };
    println!("{new_id}");
    Ok(())
}

fn parse_position(kind: &str, anchor: BlockId) -> Result<Position> {
    Ok(match kind {
        "after_block" => Position::AfterBlock { block_id: anchor },
        "before_block" => Position::BeforeBlock { block_id: anchor },
        "append_child" => Position::AppendChild {
            container_id: anchor,
        },
        "prepend_child" => Position::PrependChild {
            container_id: anchor,
        },
        "append_section" => Position::AppendSection { heading_id: anchor },
        "prepend_section" => Position::PrependSection { heading_id: anchor },
        "append_doc" => Position::AppendDoc { doc_id: anchor },
        "prepend_doc" => Position::PrependDoc { doc_id: anchor },
        other => bail!("unknown --position kind: {other}"),
    })
}

/// Find the last block in the section owned by `heading_id`. We do this by
/// loading the heading's doc and running our section detector — sufficient for
/// v1 (small docs). For huge docs this should be optimised by querying SQL
/// directly for the heading's section range.
async fn resolve_section_end(client: &SiyuanClient, heading_id: &BlockId) -> Result<BlockId> {
    use siyuan_model::load::load_doc;
    use siyuan_model::pagination::PageRequest;

    // Need root_id for the heading. SQL it.
    #[derive(serde::Deserialize)]
    struct R {
        root_id: String,
        #[serde(rename = "type")]
        ty: String,
    }
    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT root_id, type FROM blocks WHERE id = '{}'",
            heading_id.as_str()
        ))
        .await?;
    let root = rows
        .first()
        .ok_or_else(|| anyhow::anyhow!("heading not found"))?;
    if root.ty != "h" {
        bail!("--anchor for append_section must be a heading block");
    }
    let root_id = BlockId::parse(&root.root_id).context("parsing root id")?;

    // For simplicity load the whole doc; v1 docs are bounded by 50/page but we
    // need full range for section detection. Issue a single-page big request.
    let bundle = load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await?;
    let mut blocks = bundle.blocks;
    populate_section_children(&mut blocks);
    let heading = blocks
        .iter()
        .find(|b| &b.id == heading_id)
        .ok_or_else(|| anyhow::anyhow!("heading not in doc"))?;
    if heading.block_type != BlockType::Heading {
        bail!("anchor is not a heading after re-resolution");
    }
    if let Some(last) = heading.section_children.last() {
        Ok(last.clone())
    } else {
        // Empty section: treat heading itself as anchor.
        Ok(heading_id.clone())
    }
}
