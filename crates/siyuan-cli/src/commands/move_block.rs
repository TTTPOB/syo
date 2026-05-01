use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct MoveBlockArgs {
    #[arg(long)]
    pub id: String,

    /// Destination position kind: after_block | before_block | append_child | prepend_child
    /// | append_section | prepend_section | append_doc | prepend_doc.
    #[arg(long)]
    pub position: String,

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
