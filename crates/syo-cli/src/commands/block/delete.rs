use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Deserialize;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

/// Permanently delete a block and all of its children.
///
/// Sibling commands: `syo block update` with empty content clears a block
/// in place but keeps it; `syo doc remove` deletes an entire document
/// (use it instead when the target is a document root and you also want to
/// drop the `.sy` file). This command removes the block and its subtree
/// irreversibly.
///
/// Heading blocks are not real containers. By default, deleting a heading
/// deletes that block via the kernel. Pass `--include-heading-children` to
/// explicitly delete the heading plus its section children.
///
/// Inputs:
///   --id (required): block id to delete. Document root blocks (type='d')
///     are REJECTED — use `syo doc remove --id <id>` instead. All other
///     block types are accepted.
///   --include-heading-children: only valid for heading blocks. Delete the
///     heading and its section children.
///
/// Prints `ok` on success.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (syo sql,
/// syo search text, syo tag search) may show stale data for ~100-500 ms
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

    /// Delete the whole heading section when --id is a heading block.
    #[arg(long)]
    pub include_heading_children: bool,
}

pub async fn run(client: &SiyuanClient, args: DeleteBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;

    // Check if block is a document root — those must be deleted via doc remove.
    #[derive(Deserialize)]
    struct Row {
        #[serde(rename = "type")]
        ty: String,
    }
    let rows: Vec<Row> = client
        .sql_typed(&format!(
            "SELECT type FROM blocks WHERE id = '{}'",
            id.as_str()
        ))
        .await
        .context("checking block type")?;
    if rows.first().map(|r| r.ty.as_str()) == Some("d") {
        bail!(
            "{} is a document root block. delete-block cannot delete entire documents.\n\
             Use `syo doc remove --id {}` instead.",
            id.as_str(),
            id.as_str()
        );
    }

    syo_core::block::delete(
        client,
        syo_core::block::DeleteBlockInput {
            id,
            include_heading_children: args.include_heading_children,
        },
    )
    .await?;
    println!("ok");
    Ok(())
}
