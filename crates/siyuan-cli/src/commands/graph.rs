use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::graph::{Direction, neighborhood};
use siyuan_types::BlockId;

#[derive(Subcommand, Debug)]
pub enum GraphCmd {
    Backlinks(IdArgs),
    Outgoing(IdArgs),
    Neighborhood(NeighborhoodArgs),
}

#[derive(Args, Debug)]
pub struct IdArgs {
    #[arg(long)]
    pub id: String,
}

#[derive(Args, Debug)]
pub struct NeighborhoodArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long, default_value_t = 2)]
    pub depth: usize,
    #[arg(long, default_value = "both")]
    pub direction: String,
}

pub async fn run(client: &SiyuanClient, cmd: GraphCmd) -> Result<()> {
    match cmd {
        GraphCmd::Backlinks(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let g = neighborhood(client, &id, 1, Direction::Incoming).await?;
            println!("{}", serde_json::to_string_pretty(&g)?);
        }
        GraphCmd::Outgoing(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let g = neighborhood(client, &id, 1, Direction::Outgoing).await?;
            println!("{}", serde_json::to_string_pretty(&g)?);
        }
        GraphCmd::Neighborhood(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let dir = match a.direction.as_str() {
                "in" | "incoming" => Direction::Incoming,
                "out" | "outgoing" => Direction::Outgoing,
                _ => Direction::Both,
            };
            let g = neighborhood(client, &id, a.depth, dir).await?;
            println!("{}", serde_json::to_string_pretty(&g)?);
        }
    }
    Ok(())
}
