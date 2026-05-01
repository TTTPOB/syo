use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagBlockHit {
    pub block_id: BlockId,
    pub root_id: BlockId,
    pub markdown_preview: String,
}

#[derive(Debug, Deserialize)]
struct Row {
    block_id: String,
    root_id: String,
    #[serde(default)]
    markdown: String,
}

/// List every distinct tag string in the workspace (sorted).
pub async fn list_tags(client: &SiyuanClient) -> Result<Vec<String>> {
    #[derive(Debug, Deserialize)]
    struct TagRow {
        content: String,
    }
    let rows: Vec<TagRow> = client
        .sql_typed("SELECT DISTINCT content FROM spans WHERE type LIKE '%tag%' ORDER BY content")
        .await
        .context("list tags")?;
    Ok(rows.into_iter().map(|r| r.content).collect())
}

/// Find every block that has the given tag.
pub async fn search_by_tag(client: &SiyuanClient, tag: &str) -> Result<Vec<TagBlockHit>> {
    let escaped = tag.replace('\'', "''");
    let stmt = format!(
        "SELECT b.id AS block_id, b.root_id, b.markdown
         FROM blocks b
         JOIN spans s ON s.block_id = b.id
         WHERE s.type LIKE '%tag%' AND s.content = '{escaped}'
         ORDER BY b.updated DESC
         LIMIT 200"
    );
    let rows: Vec<Row> = client.sql_typed(&stmt).await.context("search by tag")?;
    rows.into_iter()
        .map(|r| {
            Ok(TagBlockHit {
                block_id: BlockId::parse(&r.block_id).map_err(|e| anyhow::anyhow!(e))?,
                root_id: BlockId::parse(&r.root_id).map_err(|e| anyhow::anyhow!(e))?,
                markdown_preview: truncate(r.markdown.as_str(), 160),
            })
        })
        .collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
