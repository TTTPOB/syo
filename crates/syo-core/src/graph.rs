use anyhow::Result;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use siyuan_model::graph::{Direction, Graph};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct NeighborhoodInput {
    pub center: BlockId,
    pub depth: usize,
    pub direction: Direction,
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Walk the link graph up to `depth` hops from `center` in the given direction.
///
/// Returns a `Graph` containing nodes, edges, and metadata.
pub async fn neighborhood(client: &SiyuanClient, input: NeighborhoodInput) -> Result<Graph> {
    siyuan_model::graph::neighborhood(client, &input.center, input.depth, input.direction).await
}

/// Convenience: fetch a single-hop `incoming` backlink graph.
pub async fn backlinks(client: &SiyuanClient, center: &BlockId) -> Result<Graph> {
    siyuan_model::graph::neighborhood(client, center, 1, Direction::Incoming).await
}

/// Convenience: fetch a single-hop `outgoing` link graph.
pub async fn outgoing(client: &SiyuanClient, center: &BlockId) -> Result<Graph> {
    siyuan_model::graph::neighborhood(client, center, 1, Direction::Outgoing).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structs_derive_debug() {
        fn _assert_debug<T: std::fmt::Debug>(_t: &T) {}

        let ni = NeighborhoodInput {
            center: BlockId::parse("20260501093000-abc1234").unwrap(),
            depth: 1,
            direction: Direction::Both,
        };
        _assert_debug(&ni);
    }
}
