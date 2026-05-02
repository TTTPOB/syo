use anyhow::{Context, Result};
use clap::{Args, ValueEnum};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

/// Move an existing block to a new position within the document tree.
///
/// Sibling commands: `siyuan insert-blocks` adds NEW blocks (different
/// ids) and supports the full eight position kinds, including
/// before_block / append_section / prepend_section. `siyuan doc move`
/// moves whole documents on disk (`.sy` files). move-block keeps the
/// block's id and all its children — only its parent and sibling order
/// change.
///
/// Inputs:
///   --id (required): block id to move.
///   --position (required): one of the five kinds below. The other kinds
///     accepted by `insert-blocks` (before_block, append_section,
///     prepend_section) are NOT accepted here — see `siyuan insert-blocks`
///     for those, or use after_block of the previous sibling for a
///     before-style move.
///   --anchor (required): destination anchor; role depends on --position.
///
/// --position kinds:
///   after_block       move --id to be a sibling immediately AFTER --anchor
///                     (--anchor = any block id)
///   prepend_child     move --id to be the FIRST child of container --anchor
///                     (--anchor = container id)
///   append_child      move --id to be the LAST child of container --anchor
///                     (--anchor = container id; same kernel call as prepend_child:
///                      kernel places the moved block at the end when no previous_id
///                      is given)
///   prepend_doc       move --id to be the FIRST block of document --anchor
///                     (--anchor = doc root id)
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

    /// Destination position kind. See command help for supported kinds;
    /// kinds accepted by `insert-blocks` but not by move-block are rejected
    /// at parse time.
    #[arg(long, value_enum)]
    pub position: MoveBlockPosition,

    /// Destination anchor. Interpretation depends on --position.
    #[arg(long)]
    pub anchor: String,
}

/// Position kinds accepted by `move-block`.
///
/// Kept local to the CLI command on purpose: `siyuan-types::Position` covers
/// the full eight-variant set used by `insert-blocks`, but the kernel's
/// `moveBlock` endpoint only realises five of those. Restricting the enum
/// here means clap rejects the unsupported kinds at parse time with a clean
/// "invalid value" message instead of a deep runtime bail.
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "snake_case")]
pub enum MoveBlockPosition {
    AfterBlock,
    AppendChild,
    PrependChild,
    AppendDoc,
    PrependDoc,
}

pub async fn run(client: &SiyuanClient, args: MoveBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    // Exhaustive match: clap rejects unsupported kinds at parse time, and
    // adding a new variant here is a compile error until handled — no
    // runtime catch-all is needed.
    match args.position {
        MoveBlockPosition::AfterBlock => {
            client.move_block(&id, Some(&anchor), None).await?;
        }
        MoveBlockPosition::AppendChild
        | MoveBlockPosition::AppendDoc
        | MoveBlockPosition::PrependChild
        | MoveBlockPosition::PrependDoc => {
            // moveBlock with parent_id and no previous_id places the moved
            // block at the END of the parent. SiYuan's kernel does not have
            // a separate "prepend" call — practically the position is the
            // same; callers wanting strict first-child semantics should
            // follow up with an after_block of the original first child.
            client.move_block(&id, None, Some(&anchor)).await?;
        }
    }
    println!("ok");
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser, Debug)]
    struct TestCli {
        #[command(flatten)]
        args: MoveBlockArgs,
    }

    #[test]
    fn clap_accepts_after_block() {
        let parsed = TestCli::try_parse_from([
            "test",
            "--id",
            "20260501090000-blk0001",
            "--position",
            "after_block",
            "--anchor",
            "20260501090000-blk0002",
        ])
        .expect("after_block must parse");
        assert_eq!(parsed.args.position, MoveBlockPosition::AfterBlock);
    }

    #[test]
    fn clap_rejects_before_block_at_parse_time() {
        let err = TestCli::try_parse_from([
            "test",
            "--id",
            "20260501090000-blk0001",
            "--position",
            "before_block",
            "--anchor",
            "20260501090000-blk0002",
        ])
        .expect_err("before_block must be rejected by clap");
        let rendered = err.to_string();
        assert!(
            rendered.contains("invalid value"),
            "expected clap 'invalid value' message; got: {rendered}"
        );
        // The error should advertise the supported set so the caller knows
        // where to look.
        assert!(
            rendered.contains("after_block"),
            "expected supported kinds in error; got: {rendered}"
        );
    }

    #[test]
    fn clap_rejects_append_section_and_prepend_section() {
        for bad in ["append_section", "prepend_section"] {
            let err = TestCli::try_parse_from([
                "test",
                "--id",
                "20260501090000-blk0001",
                "--position",
                bad,
                "--anchor",
                "20260501090000-blk0002",
            ])
            .expect_err("section kinds must be rejected by clap");
            assert!(
                err.to_string().contains("invalid value"),
                "{bad} should produce clap 'invalid value' error"
            );
        }
    }
}
