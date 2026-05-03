use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct SortArgs {
    /// Document root block id.
    #[arg(long)]
    pub id: String,
    /// Manual sort key (lower sorts earlier).
    #[arg(long)]
    pub sort: i64,
}

pub async fn run(client: &SiyuanClient, args: SortArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("sort".to_string(), args.sort.to_string());
    client.set_block_attrs(&id, &attrs).await?;
    println!("ok");
    Ok(())
}
