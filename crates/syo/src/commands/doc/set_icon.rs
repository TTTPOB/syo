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
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("icon".to_string(), args.icon);
    client.set_block_attrs(&id, &attrs).await?;
    println!("ok");
    Ok(())
}
