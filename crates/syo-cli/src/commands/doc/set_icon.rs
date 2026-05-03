use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct IconArgs {
    /// Document block id.
    #[arg(long)]
    pub id: String,
    /// Icon name (e.g. emoji shortcode like ":rocket:") or empty to clear.
    #[arg(long, default_value = "")]
    pub icon: String,
}

pub async fn run(client: &SiyuanClient, args: IconArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    syo_core::attr::set_icon(
        client,
        syo_core::attr::SetIconInput {
            id,
            icon: args.icon,
        },
    )
    .await?;
    println!("ok");
    Ok(())
}
