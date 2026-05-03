use anyhow::{Context, Result};
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;
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
        "in" | "incoming" => syo_core::graph::Direction::Incoming,
        "out" | "outgoing" => syo_core::graph::Direction::Outgoing,
        _ => syo_core::graph::Direction::Both,
    };
    let g = syo_core::graph::neighborhood(
        client,
        syo_core::graph::NeighborhoodInput {
            center: id,
            depth: args.depth,
            direction: dir,
        },
    )
    .await?;
    println!("{}", serde_json::to_string_pretty(&g)?);
    Ok(())
}
