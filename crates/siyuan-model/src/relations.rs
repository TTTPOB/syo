use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefHint {
    pub source_id: BlockId,
    pub target_id: BlockId,
    pub anchor: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlockRelationSummary {
    pub outgoing_refs: Vec<RefHint>,
    pub incoming_refs_count: usize,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OutgoingRow {
    block_id: String,
    def_block_id: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize)]
struct IncomingRow {
    def_block_id: String,
    n: i64,
}

#[derive(Debug, Deserialize)]
struct TagRow {
    block_id: String,
    #[serde(default)]
    content: String,
}

/// Build a per-block relation summary for every id in `block_ids`.
pub async fn relations_for(
    client: &SiyuanClient,
    block_ids: &[BlockId],
) -> Result<BTreeMap<BlockId, BlockRelationSummary>> {
    if block_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    let id_list = block_ids
        .iter()
        .map(|i| format!("'{}'", i.as_str()))
        .collect::<Vec<_>>()
        .join(",");

    // Outgoing refs.
    let outgoing: Vec<OutgoingRow> = client
        .sql_typed(&format!(
            "SELECT block_id, def_block_id, content FROM refs WHERE block_id IN ({id_list})"
        ))
        .await
        .context("query outgoing refs")?;

    // Incoming counts.
    let incoming: Vec<IncomingRow> = client
        .sql_typed(&format!(
            "SELECT def_block_id, COUNT(*) AS n FROM refs WHERE def_block_id IN ({id_list}) GROUP BY def_block_id"
        ))
        .await
        .context("query incoming refs")?;

    // Tag spans.
    let tags: Vec<TagRow> = client
        .sql_typed(&format!(
            "SELECT block_id, content FROM spans WHERE type LIKE '%tag%' AND block_id IN ({id_list})"
        ))
        .await
        .context("query tags")?;

    let mut map: BTreeMap<BlockId, BlockRelationSummary> = BTreeMap::new();
    for id in block_ids {
        map.entry(id.clone()).or_default();
    }

    for r in outgoing {
        if let (Ok(src), Ok(tgt)) = (BlockId::parse(&r.block_id), BlockId::parse(&r.def_block_id)) {
            map.entry(src.clone())
                .or_default()
                .outgoing_refs
                .push(RefHint {
                    source_id: src,
                    target_id: tgt,
                    anchor: r.content,
                });
        }
    }

    for r in incoming {
        if let Ok(id) = BlockId::parse(&r.def_block_id) {
            map.entry(id).or_default().incoming_refs_count = r.n as usize;
        }
    }

    for r in tags {
        if let Ok(id) = BlockId::parse(&r.block_id) {
            map.entry(id).or_default().tags.push(r.content);
        }
    }

    Ok(map)
}
