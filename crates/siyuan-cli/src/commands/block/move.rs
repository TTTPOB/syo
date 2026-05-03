use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;
use siyuan_types::position::PositionKind;

/// Move an existing block to a new position within the document tree.
///
/// Sibling commands: `siyuan block insert` adds NEW blocks (different
/// ids); `siyuan doc move` moves whole documents on disk (`.sy` files).
/// `siyuan block move` keeps the block's id and all its children — only its
/// parent and sibling order change.
///
/// Inputs:
///   --id (required): block id to move.
///   --position (required): one of the eight position kinds below.
///   --anchor (required): destination anchor; role depends on --position.
///
/// --position kinds:
///   after_block       move --id to be a sibling immediately AFTER --anchor
///                     (--anchor = any block id)
///   before_block      move --id to be a sibling immediately BEFORE --anchor
///                     (--anchor = any block id)
///   prepend_child     move --id to be the FIRST child of container --anchor
///                     (--anchor = container id; kernel places at end,
///                      follow up with after_block for strict first-child)
///   append_child      move --id to be the LAST child of container --anchor
///                     (--anchor = container id)
///   append_section    move --id to the end of the heading section owned
///                     by --anchor (--anchor MUST be a heading block)
///   prepend_section   move --id right after the heading block --anchor
///                     (--anchor MUST be a heading block)
///   prepend_doc       move --id to be the FIRST block of document --anchor
///                     (--anchor = doc root id; kernel places at end,
///                      follow up with after_block for strict first-child)
///   append_doc        move --id to the END of document --anchor
///                     (--anchor = doc root id)
///
/// The block keeps its existing id and all its children. Prints `ok` on
/// success.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (siyuan sql,
/// siyuan search text, siyuan tag search) may show stale position data for
/// ~100-500 ms after this call. The kernel is immediately consistent — only
/// the SQL index lags.
///
/// Example:
///   in:  --id 20260501090000-blk0001 --position after_block --anchor 20260501090000-blk0002
///   out: ok
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct MoveBlockArgs {
    /// Block id to move.
    #[arg(long)]
    pub id: String,

    /// Destination position kind. See command help for supported kinds:
    /// after_block, before_block, append_child, prepend_child,
    /// append_section, prepend_section, append_doc, prepend_doc.
    #[arg(long, value_parser = super::super::parse_position)]
    pub position: PositionKind,

    /// Destination anchor. Interpretation depends on --position.
    #[arg(long)]
    pub anchor: String,
}

pub async fn run(client: &SiyuanClient, args: MoveBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    match args.position {
        PositionKind::AfterBlock => {
            client.move_block(&id, Some(&anchor), None).await?;
        }
        PositionKind::BeforeBlock => {
            let prev_id = find_previous_sibling(client, &anchor).await?;
            client.move_block(&id, Some(&prev_id), None).await?;
        }
        PositionKind::AppendChild | PositionKind::AppendDoc => {
            client.move_block(&id, None, Some(&anchor)).await?;
        }
        PositionKind::PrependChild | PositionKind::PrependDoc => {
            // moveBlock with parent_id and no previous_id places the moved
            // block at the END of the parent. SiYuan's kernel does not have
            // a separate "prepend" call — practically the position is the
            // same; callers wanting strict first-child semantics should
            // follow up with an after_block of the original first child.
            client.move_block(&id, None, Some(&anchor)).await?;
        }
        PositionKind::AppendSection => {
            let section_end = super::insert::resolve_section_end(client, &anchor).await?;
            client.move_block(&id, Some(&section_end), None).await?;
        }
        PositionKind::PrependSection => {
            // Right after the heading itself.
            client.move_block(&id, Some(&anchor), None).await?;
        }
    }
    println!("ok");
    Ok(())
}

/// Find the block that comes immediately before `anchor` in its parent's
/// children list. Used by `before_block` positioning.
async fn find_previous_sibling(client: &SiyuanClient, anchor: &BlockId) -> Result<BlockId> {
    use siyuan_model::load::load_doc;
    use siyuan_model::pagination::PageRequest;

    // Query root_id for the anchor block via SQL.
    #[derive(serde::Deserialize)]
    struct R {
        root_id: String,
    }
    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT root_id FROM blocks WHERE id = '{}'",
            anchor.as_str()
        ))
        .await?;
    let root = rows
        .first()
        .ok_or_else(|| anyhow::anyhow!("anchor block not found"))?;
    let root_id = BlockId::parse(&root.root_id).context("parsing root id")?;

    let bundle = load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await?;
    let blocks = bundle.blocks;
    let idx = blocks
        .iter()
        .position(|b| &b.id == anchor)
        .ok_or_else(|| anyhow::anyhow!("anchor block not found in document"))?;
    if idx == 0 {
        bail!(
            "cannot move before first child of document; use prepend_child or prepend_doc instead"
        );
    }
    let prev = &blocks[idx - 1];
    Ok(prev.id.clone())
}

#[cfg(test)]
mod tests {
    use super::super::super::parse_position;
    use siyuan_types::position::PositionKind;

    #[test]
    fn parse_position_accepts_all_eight_kinds() {
        let cases = [
            ("after_block", PositionKind::AfterBlock),
            ("before_block", PositionKind::BeforeBlock),
            ("append_child", PositionKind::AppendChild),
            ("prepend_child", PositionKind::PrependChild),
            ("append_section", PositionKind::AppendSection),
            ("prepend_section", PositionKind::PrependSection),
            ("append_doc", PositionKind::AppendDoc),
            ("prepend_doc", PositionKind::PrependDoc),
        ];
        for (input, expected) in cases {
            let parsed = parse_position(input).expect("valid position kind must parse");
            assert_eq!(
                parsed, expected,
                "parse_position({input:?}) should return {expected:?}"
            );
        }
    }

    #[test]
    fn parse_position_rejects_invalid_kind() {
        let err = parse_position("nonsense").expect_err("invalid kind must be rejected");
        assert!(
            err.contains("invalid position kind"),
            "expected 'invalid position kind' message; got: {err}"
        );
    }
}
