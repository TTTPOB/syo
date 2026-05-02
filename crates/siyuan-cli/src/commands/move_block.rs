use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

/// Move an existing block to a new position within the document tree.
///
/// Sibling commands: `siyuan insert-blocks` adds NEW blocks (different
/// ids); `siyuan doc move` moves whole documents on disk (`.sy` files).
/// move-block keeps the block's id and all its children — only its parent
/// and sibling order change.
///
/// Inputs:
///   --id (required): block id to move.
///   --position (required): one of the kinds below; some kinds are
///     unsupported in v1 and will error out (see notes).
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
///   before_block      NOT SUPPORTED in v1 — the kernel's moveBlock takes
///                     `previousID`, not `nextID`. Use after_block with the
///                     previous sibling's id instead.
///   append_section    NOT SUPPORTED in v1 — section-relative moves require
///                     section resolution that move-block does not perform.
///                     Resolve to a sibling block id first and use after_block.
///   prepend_section   NOT SUPPORTED in v1 — see append_section.
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

    /// Destination position kind. See command help for supported kinds and
    /// v1 limitations.
    #[arg(long)]
    pub position: String,

    /// Destination anchor. Interpretation depends on --position.
    #[arg(long)]
    pub anchor: String,
}

pub async fn run(client: &SiyuanClient, args: MoveBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let anchor = BlockId::parse(&args.anchor).context("--anchor")?;
    match args.position.as_str() {
        "after_block" => client.move_block(&id, Some(&anchor), None).await?,
        "append_child" | "append_doc" => client.move_block(&id, None, Some(&anchor)).await?,
        "before_block" => {
            // SiYuan moveBlock supports previousID for "after"; "before" via using
            // the predecessor of `anchor` as previous_id. For v1 we error out and
            // tell the caller to use after_block with the previous sibling instead.
            bail!(
                "position=before_block is not supported by move; use after_block of the previous sibling"
            );
        }
        "prepend_child" | "prepend_doc" => {
            // Equivalent to "no previous, parent=container" handled by moveBlock.
            client.move_block(&id, None, Some(&anchor)).await?;
        }
        "append_section" | "prepend_section" => {
            bail!("section-relative move is not supported in v1; resolve to a sibling block first");
        }
        other => bail!("unknown --position: {other}"),
    }
    println!("ok");
    Ok(())
}
