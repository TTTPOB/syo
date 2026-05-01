use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::section::populate_section_children;
use siyuan_types::{BlockId, BlockType, Position};

#[derive(Args, Debug)]
pub struct InsertBlocksArgs {
    /// Position kind. One of: after_block, before_block, append_child,
    /// prepend_child, append_section, prepend_section, append_doc, prepend_doc.
    #[arg(long)]
    pub position: String,

    /// Anchor block id (interpretation depends on position kind).
    #[arg(long)]
    pub anchor: String,

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
    let root_id = BlockId::parse(&root.root_id).map_err(|e| anyhow::anyhow!(e))?;

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
