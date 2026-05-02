use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

/// Permanently delete a block and all of its children.
///
/// Sibling commands: `siyuan update-block` with empty content clears a block
/// in place but keeps it; `siyuan doc remove` deletes an entire document
/// (use it instead when the target is a document root and you also want to
/// drop the `.sy` file). This command removes the block and its subtree
/// irreversibly.
///
/// Inputs:
///   --id (required): block id to delete. Any block type is accepted,
///     including a document root (in which case the document is removed).
///
/// Prints `ok` on success.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (siyuan sql,
/// siyuan search text, siyuan tag search) may show stale data for ~100-500 ms
/// after this call. The kernel is immediately consistent — only the SQL
/// index lags.
///
/// Example:
///   in:  --id 20260501090000-blk0001
///   out: ok
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct DeleteBlockArgs {
    /// Block id to delete.
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: DeleteBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    client.delete_block(&id).await?;
    println!("ok");
    Ok(())
}
