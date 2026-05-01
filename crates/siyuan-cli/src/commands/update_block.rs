use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct UpdateBlockArgs {
    #[arg(long)]
    pub id: String,

    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: UpdateBlockArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    client.update_block_markdown(&id, &markdown).await?;
    println!("ok");
    Ok(())
}
