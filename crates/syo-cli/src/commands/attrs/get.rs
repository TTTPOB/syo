use anyhow::{Context, Result};
use clap::Args;
use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct GetAttrsArgs {
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: GetAttrsArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let output = syo_core::attr::get(client, syo_core::attr::GetAttrsInput { id }).await?;
    println!("{}", serde_json::to_string_pretty(&output.attrs)?);
    Ok(())
}
