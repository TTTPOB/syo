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
    syo_core::attr::set_sort(
        client,
        syo_core::attr::SetSortInput {
            id,
            sort: args.sort,
        },
    )
    .await?;
    println!("ok");
    Ok(())
}
