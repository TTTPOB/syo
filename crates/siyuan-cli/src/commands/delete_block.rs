use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct DeleteBlockArgs {
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: DeleteBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    client.delete_block(&id).await?;
    println!("ok");
    Ok(())
}
