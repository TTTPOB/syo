use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::graph::{Direction, neighborhood};
use siyuan_types::BlockId;

#[derive(Subcommand, Debug)]
pub enum GraphCmd {
    /// List blocks that reference the given block (depth=1, incoming).
    ///
    /// Sibling commands: `siyuan graph outgoing` is the dual (refs FROM
    /// the block); `siyuan graph neighborhood` walks deeper or both
    /// directions. This command is a shorthand for
    /// `neighborhood --depth 1 --direction incoming`.
    ///
    /// Inputs:
    ///   --id (required): center block id.
    ///
    /// Output is the pretty JSON `{nodes, edges, truncated}` shape.
    ///
    /// Example:
    ///   in:  --id 20260501090000-blk0001
    ///   out: {"nodes":[...],"edges":[...],"truncated":false}
    #[command(verbatim_doc_comment)]
    Backlinks(IdArgs),
    /// List blocks referenced BY the given block (depth=1, outgoing).
    ///
    /// Sibling commands: `siyuan graph backlinks` is the dual;
    /// `siyuan graph neighborhood` is the multi-hop generalisation.
    ///
    /// Inputs:
    ///   --id (required): center block id.
    ///
    /// Output is the pretty JSON `{nodes, edges, truncated}` shape.
    ///
    /// Example:
    ///   in:  --id 20260501090000-blk0001
    ///   out: {"nodes":[...],"edges":[...],"truncated":false}
    #[command(verbatim_doc_comment)]
    Outgoing(IdArgs),
    /// Walk the link graph around a block to a configurable depth.
    ///
    /// Sibling commands: `siyuan graph backlinks` and
    /// `siyuan graph outgoing` are depth-1 single-direction shortcuts.
    /// For unbounded results bypass the cap and query the `refs` table
    /// via `siyuan sql`.
    ///
    /// Inputs:
    ///   --id (required): center block id.
    ///   --depth (optional, default 2): hop count; capped at 8 by the
    ///     model layer.
    ///   --direction (optional, default both): `in`/`incoming`,
    ///     `out`/`outgoing`, or anything else (e.g. `both`) for both
    ///     directions.
    ///
    /// Traversal stops at 500 nodes or 1000 edges. When either cap is
    /// hit, `truncated` is `true` in the output and the result is
    /// partial — narrow the query (lower depth, single direction, more
    /// specific center) or fall back to `siyuan sql`.
    ///
    /// Output is the pretty JSON `{nodes, edges, truncated}` shape.
    ///
    /// Example:
    ///   in:  --id 20260501090000-blk0001 --depth 2 --direction both
    ///   out: {"nodes":[...],"edges":[...],"truncated":false}
    #[command(verbatim_doc_comment)]
    Neighborhood(NeighborhoodArgs),
}

#[derive(Args, Debug)]
pub struct IdArgs {
    /// Center block id.
    #[arg(long)]
    pub id: String,
}

#[derive(Args, Debug)]
pub struct NeighborhoodArgs {
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
