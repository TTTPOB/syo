use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Incoming,
    Outgoing,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: BlockId,
    pub root_id: BlockId,
    pub block_type: String,
    pub markdown_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: BlockId,
    pub target: BlockId,
    pub anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub schema: String, // "siyuan-agent.graph.v1"
    pub center: BlockId,
    pub depth: usize,
    pub direction: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
struct EdgeRow {
    block_id: String,
    def_block_id: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize)]
struct NodeRow {
    id: String,
    root_id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    markdown: String,
}

const NODE_LIMIT: usize = 500;
const EDGE_LIMIT: usize = 1000;

pub async fn neighborhood(
    client: &SiyuanClient,
    center: &BlockId,
    depth: usize,
    direction: Direction,
) -> Result<Graph> {
    let mut visited: BTreeSet<BlockId> = BTreeSet::new();
    visited.insert(center.clone());
    let mut frontier: VecDeque<BlockId> = VecDeque::new();
    frontier.push_back(center.clone());

    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut seen_edges: BTreeSet<(BlockId, BlockId, String)> = BTreeSet::new();
    let mut truncated = false;

    for _ in 0..depth {
        let current: Vec<BlockId> = std::mem::take(&mut frontier).into_iter().collect();
        if current.is_empty() {
            break;
        }
        let id_list = current
            .iter()
            .map(|i| format!("'{}'", i.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        let mut next_ids: BTreeSet<BlockId> = BTreeSet::new();

        if matches!(direction, Direction::Outgoing | Direction::Both) {
            let rows: Vec<EdgeRow> = client
                .sql_typed(&format!(
                    "SELECT block_id, def_block_id, content FROM refs WHERE block_id IN ({id_list})"
                ))
                .await
                .context("graph outgoing")?;
            for r in rows {
                if edges.len() >= EDGE_LIMIT {
                    truncated = true;
                    break;
                }
                let (src, tgt) =
                    match (BlockId::parse(&r.block_id), BlockId::parse(&r.def_block_id)) {
                        (Ok(s), Ok(t)) => (s, t),
                        _ => continue,
                    };
                let edge_key = (src.clone(), tgt.clone(), r.content.clone());
                if seen_edges.insert(edge_key) {
                    edges.push(GraphEdge {
                        source: src,
                        target: tgt.clone(),
                        anchor: r.content,
                    });
                }
                if !visited.contains(&tgt) {
                    next_ids.insert(tgt);
                }
            }
        }
        if matches!(direction, Direction::Incoming | Direction::Both) && edges.len() < EDGE_LIMIT {
            let rows: Vec<EdgeRow> = client
                .sql_typed(&format!(
                    "SELECT block_id, def_block_id, content FROM refs WHERE def_block_id IN ({id_list})"
                ))
                .await
                .context("graph incoming")?;
            for r in rows {
                if edges.len() >= EDGE_LIMIT {
                    truncated = true;
                    break;
                }
                let (src, tgt) =
                    match (BlockId::parse(&r.block_id), BlockId::parse(&r.def_block_id)) {
                        (Ok(s), Ok(t)) => (s, t),
                        _ => continue,
                    };
                let edge_key = (src.clone(), tgt.clone(), r.content.clone());
                if seen_edges.insert(edge_key) {
                    edges.push(GraphEdge {
                        source: src.clone(),
                        target: tgt,
                        anchor: r.content,
                    });
                }
                if !visited.contains(&src) {
                    next_ids.insert(src);
                }
            }
        }

        for id in next_ids {
            if visited.len() >= NODE_LIMIT {
                truncated = true;
                break;
            }
            visited.insert(id.clone());
            frontier.push_back(id);
        }
    }

    // Fetch node metadata for everyone in `visited`.
    let id_list = visited
        .iter()
        .map(|i| format!("'{}'", i.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let stmt = format!("SELECT id, root_id, type, markdown FROM blocks WHERE id IN ({id_list})");
    let rows: Vec<NodeRow> = client.sql_typed(&stmt).await.context("graph nodes")?;
    let mut node_map: BTreeMap<BlockId, GraphNode> = BTreeMap::new();
    for r in rows {
        if let (Ok(id), Ok(root)) = (BlockId::parse(&r.id), BlockId::parse(&r.root_id)) {
            let preview = if r.markdown.chars().count() <= 100 {
                r.markdown
            } else {
                let out: String = r.markdown.chars().take(100).collect();
                format!("{out}…")
            };
            node_map.insert(
                id.clone(),
                GraphNode {
                    id,
                    root_id: root,
                    block_type: r.block_type,
                    markdown_preview: preview,
                },
            );
        }
    }

    let direction_s = match direction {
        Direction::Incoming => "incoming",
        Direction::Outgoing => "outgoing",
        Direction::Both => "both",
    };

    Ok(Graph {
        schema: "siyuan-agent.graph.v1".to_string(),
        center: center.clone(),
        depth,
        direction: direction_s.to_string(),
        nodes: node_map.into_values().collect(),
        edges,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_preview_cjk_safe() {
        // Construct a minimal GraphNode via the neighborhood path would need a
        // mock client, so we test the inline truncation logic directly.
        let md = "中".repeat(200);
        let preview = if md.chars().count() <= 100 {
            md
        } else {
            let out: String = md.chars().take(100).collect();
            format!("{out}…")
        };
        assert!(preview.ends_with('…'));
        assert!(preview.chars().count() <= 101);
    }

    #[test]
    fn edge_dedup_prevents_duplicate_push() {
        // Simulates the BFS edge-collection logic: the same (source, target,
        // anchor) triple must only appear once in the edge list.
        let mut seen: BTreeSet<(BlockId, BlockId, String)> = BTreeSet::new();
        let mut edges: Vec<GraphEdge> = Vec::new();

        let a = BlockId::parse("20260501093000-aaa0001").unwrap();
        let b = BlockId::parse("20260501093000-bbb0002").unwrap();

        // helper: push edge only if not already seen
        fn push_edge(
            seen: &mut BTreeSet<(BlockId, BlockId, String)>,
            edges: &mut Vec<GraphEdge>,
            src: &BlockId,
            tgt: &BlockId,
            anchor: &str,
        ) {
            let key = (src.clone(), tgt.clone(), anchor.to_string());
            if seen.insert(key) {
                edges.push(GraphEdge {
                    source: src.clone(),
                    target: tgt.clone(),
                    anchor: anchor.to_string(),
                });
            }
        }

        // Outgoing pass finds A→B
        push_edge(&mut seen, &mut edges, &a, &b, "anchor");
        assert_eq!(edges.len(), 1);

        // Incoming pass re-discovers the same A→B — must be deduplicated
        push_edge(&mut seen, &mut edges, &a, &b, "anchor");
        assert_eq!(edges.len(), 1, "duplicate edge must not be added");

        // Different anchor is a distinct edge
        push_edge(&mut seen, &mut edges, &a, &b, "other-anchor");
        assert_eq!(edges.len(), 2, "different anchor means different edge");

        // Reverse direction is distinct
        push_edge(&mut seen, &mut edges, &b, &a, "backlink");
        assert_eq!(edges.len(), 3, "B→A is distinct from A→B");
    }
}
