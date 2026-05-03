use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

pub mod backlinks;
pub mod neighborhood;
pub mod outgoing;

#[derive(Subcommand, Debug)]
pub enum GraphCmd {
    /// List blocks that reference the given block (depth=1, incoming).
    ///
    /// Sibling commands: `syo graph outgoing` is the dual (refs FROM
    /// the block); `syo graph neighborhood` walks deeper or both
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
    Backlinks(backlinks::Args),
    /// List blocks referenced BY the given block (depth=1, outgoing).
    ///
    /// Sibling commands: `syo graph backlinks` is the dual;
    /// `syo graph neighborhood` is the multi-hop generalisation.
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
    Outgoing(outgoing::Args),
    /// Walk the link graph around a block to a configurable depth.
    ///
    /// Sibling commands: `syo graph backlinks` and
    /// `syo graph outgoing` are depth-1 single-direction shortcuts.
    /// For unbounded results bypass the cap and query the `refs` table
    /// via `syo sql`.
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
    /// specific center) or fall back to `syo sql`.
    ///
    /// Output is the pretty JSON `{nodes, edges, truncated}` shape.
    ///
    /// Example:
    ///   in:  --id 20260501090000-blk0001 --depth 2 --direction both
    ///   out: {"nodes":[...],"edges":[...],"truncated":false}
    #[command(verbatim_doc_comment)]
    Neighborhood(neighborhood::Args),
}

pub async fn run(client: &SiyuanClient, cmd: GraphCmd) -> Result<()> {
    match cmd {
        GraphCmd::Backlinks(a) => backlinks::run(client, a).await,
        GraphCmd::Outgoing(a) => outgoing::run(client, a).await,
        GraphCmd::Neighborhood(a) => neighborhood::run(client, a).await,
    }
}
