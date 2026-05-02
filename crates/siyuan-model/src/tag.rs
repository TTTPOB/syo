use anyhow::{Context, Result, bail};
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

const TAG_PREVIEW_LEN: usize = 160;

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

/// Find every block that has the given tag, returning at most `limit` hits.
///
/// `limit` must be non-zero; callers are responsible for capping it (the CLI
/// and MCP layers cap at `MAX_SEARCH_LIMIT`).
pub async fn search_by_tag(
    client: &SiyuanClient,
    tag: &str,
    limit: usize,
) -> Result<Vec<TagBlockHit>> {
    if limit == 0 {
        bail!("`limit` must be greater than 0");
    }
    let escaped = tag.replace('\'', "''");
    let stmt = build_search_by_tag_sql(&escaped, limit);
    let rows: Vec<Row> = client.sql_typed(&stmt).await.context("search by tag")?;
    rows.into_iter()
        .map(|r| {
            Ok(TagBlockHit {
                block_id: BlockId::parse(&r.block_id).context("parsing block id")?,
                root_id: BlockId::parse(&r.root_id).context("parsing root id")?,
                markdown_preview: truncate(r.markdown.as_str(), TAG_PREVIEW_LEN),
            })
        })
        .collect()
}

// Build the SQL for `search_by_tag`. Extracted so the LIMIT clause can be
// asserted in unit tests without needing a live kernel.
fn build_search_by_tag_sql(escaped_tag: &str, limit: usize) -> String {
    format!(
        "SELECT b.id AS block_id, b.root_id, b.markdown
         FROM blocks b
         JOIN spans s ON s.block_id = b.id
         WHERE s.type LIKE '%tag%' AND s.content = '{escaped_tag}'
         ORDER BY b.updated DESC
         LIMIT {limit}"
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let out: String = s.chars().take(max).collect();
        format!("{out}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_ascii() {
        let s = "a".repeat(TAG_PREVIEW_LEN * 2);
        let out = truncate(&s, TAG_PREVIEW_LEN);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= TAG_PREVIEW_LEN + 1); // preview chars + '…'
    }

    // The build_search_by_tag_sql helper is the single point at which the
    // caller-supplied limit is interpolated into the SQL. Pinning the LIMIT
    // suffix here means a regression that drops or hard-codes the clause
    // fails loudly without needing a live kernel.
    #[test]
    fn build_search_by_tag_sql_includes_limit_clause() {
        let stmt = build_search_by_tag_sql("alpha", 7);
        assert!(
            stmt.trim_end().ends_with("LIMIT 7"),
            "SQL must end with `LIMIT 7`; got: {stmt}"
        );
        assert!(stmt.contains("s.content = 'alpha'"));
    }

    #[test]
    fn build_search_by_tag_sql_propagates_arbitrary_limit() {
        let stmt = build_search_by_tag_sql("beta", 1);
        assert!(stmt.trim_end().ends_with("LIMIT 1"));
        let stmt = build_search_by_tag_sql("beta", 1000);
        assert!(stmt.trim_end().ends_with("LIMIT 1000"));
    }

    #[tokio::test]
    async fn search_by_tag_rejects_zero_limit() {
        // The dummy client points at an unreachable port: if the early-return
        // guard regresses, this test would fail with a network error instead
        // of the "must be greater than 0" string. Pinning the message keeps
        // the validation contract obvious.
        let client = SiyuanClient::new("http://127.0.0.1:1", "tok").expect("dummy client builds");
        let err = search_by_tag(&client, "anything", 0)
            .await
            .expect_err("limit=0 must be rejected");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("greater than 0"),
            "error must reference the limit floor; got: {msg}"
        );
    }

    #[test]
    fn truncate_cjk_no_panic() {
        let s = "中".repeat(200);
        let out = truncate(&s, 100);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 101);
    }

    #[test]
    fn truncate_emoji_no_panic() {
        let s = "😀".repeat(50);
        let out = truncate(&s, 10);
        assert!(out.ends_with('…'));
    }
}
