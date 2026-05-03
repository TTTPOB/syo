use anyhow::{Context, Result};
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;
use siyuan_model::graph::{Direction, neighborhood};
use siyuan_types::BlockId;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Center block id.
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let g = neighborhood(client, &id, 1, Direction::Outgoing).await?;
    println!("{}", serde_json::to_string_pretty(&g)?);
    Ok(())
}
