use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;
use siyuan_types::position::PositionKind;

/// Move an existing block to a new position within the document tree.
///
/// Sibling commands: `syo block insert` adds NEW blocks (different
/// ids); `syo doc move` moves whole documents on disk (`.sy` files).
/// `syo block move` keeps the block's id and all its children — only its
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
/// Headings are section owners, not real containers. Pass
/// `--include-heading-section` with append_child or prepend_child to treat a
/// heading anchor as a virtual section container.
///
/// The block keeps its existing id and all its children. Prints `ok` on
/// success.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (syo sql,
/// syo search text, syo tag search) may show stale position data for
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

    /// Treat a heading anchor as a virtual section container for child positions.
    #[arg(long)]
    pub include_heading_section: bool,
}

pub async fn run(client: &SiyuanClient, args: MoveBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    syo_core::block::move_block(
        client,
        syo_core::block::MoveBlockInput {
            id,
            position: args.position,
            anchor,
            include_heading_section: args.include_heading_section,
        },
    )
    .await?;
    println!("ok");
    Ok(())
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
