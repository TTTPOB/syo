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
    /// Hop count. Default 2, capped at 8.
    #[arg(long, default_value_t = 2)]
    pub depth: usize,
    /// Direction: `in`/`incoming`, `out`/`outgoing`, or `both` (default).
    #[arg(long, default_value = "both")]
    pub direction: String,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let dir = match args.direction.as_str() {
        "in" | "incoming" => Direction::Incoming,
        "out" | "outgoing" => Direction::Outgoing,
        _ => Direction::Both,
    };
    let g = neighborhood(client, &id, args.depth, dir).await?;
    println!("{}", serde_json::to_string_pretty(&g)?);
    Ok(())
}
