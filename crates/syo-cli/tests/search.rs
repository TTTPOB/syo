//! Integration tests for search.
//!
//! Run with: `cargo test -p syo --test search -- --ignored --test-threads=1`

mod common;

use std::time::Duration;

use common::{boot_with_seed, wait_for};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Test 1: search (LIKE on markdown) finds matching blocks
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Hit {
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    markdown: String,
}

#[derive(Debug, Deserialize)]
struct CountRow {
    n: i64,
}

async fn seed_sql_limit_probe_doc(f: &common::Fixture) -> String {
    let mut md = String::from("# SQL Limit Probe\n\n");
    for i in 1..=90 {
        md.push_str(&format!("syo-limit-probe-{i:03}\n\n"));
    }

    let doc_id = f
        .client
        .create_doc_with_md(&f.notebook_id, "/SqlLimitProbe", &md)
        .await
        .expect("create SQL limit probe doc");

    let client = &f.client;
    wait_for(
        || async {
            let rows: Vec<CountRow> = client
                .sql_typed(&format!(
                    "SELECT COUNT(*) AS n FROM blocks \
                     WHERE root_id = '{}' AND content LIKE 'syo-limit-probe-%'",
                    doc_id.as_str()
                ))
                .await?;
            if rows.first().map(|r| r.n).unwrap_or(0) >= 90 {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for SQL limit probe doc to be indexed");

    doc_id.to_string()
}

#[tokio::test]
#[ignore]
async fn raw_sql_without_limit_uses_kernel_search_limit() {
    let f = boot_with_seed().await.expect("boot");
    let doc_id = seed_sql_limit_probe_doc(&f).await;

    let base = format!(
        "SELECT id, type, markdown FROM blocks \
         WHERE root_id = '{}' AND content LIKE 'syo-limit-probe-%' \
         ORDER BY content",
        doc_id
    );

    let implicit: Vec<Hit> = f.client.sql_typed(&base).await.expect("implicit limit SQL");
    assert_eq!(
        implicit.len(),
        64,
        "SiYuan /api/query/sql should apply Conf.Search.Limit to SELECT statements without LIMIT"
    );

    let explicit: Vec<Hit> = f
        .client
        .sql_typed(&format!("{base} LIMIT 80"))
        .await
        .expect("explicit limit SQL");
    assert_eq!(
        explicit.len(),
        80,
        "an explicit LIMIT should override the kernel's default search limit"
    );

    let second_page: Vec<Hit> = f
        .client
        .sql_typed(&format!("{base} LIMIT 20 OFFSET 64"))
        .await
        .expect("explicit offset SQL");
    assert_eq!(
        second_page.len(),
        20,
        "explicit LIMIT/OFFSET should fetch rows beyond the implicit 64-row page"
    );
    assert!(
        second_page
            .first()
            .map(|hit| hit.markdown.contains("syo-limit-probe-065"))
            .unwrap_or(false),
        "OFFSET 64 should start at the 65th seeded paragraph; got {second_page:?}"
    );

    let guarded = syo_core::sql::raw(&f.client, syo_core::sql::SqlInput { stmt: base })
        .await
        .expect("guarded raw SQL");
    assert_eq!(
        guarded.rows.len(),
        64,
        "guarded raw SQL should return the first page after probing for one extra row"
    );
    assert!(
        guarded.has_more,
        "guarded raw SQL should mark has_more when the probe finds row 65"
    );
    assert!(
        guarded.probe_applied,
        "guarded raw SQL should probe unlimited top-level SELECT queries"
    );
}

#[tokio::test]
#[ignore]
async fn search_finds_matching_blocks() {
    let f = boot_with_seed().await.expect("boot");

    // Append a block with distinctive content so we can search for it.
    f.client
        .append_block_markdown("RNA polymerase binds to promoter region", &f.doc_id)
        .await
        .expect("append RNA block");

    // Wait for the SQL index to catch up.
    let client = &f.client;
    let hits = wait_for(
        || async {
            let rows: Vec<Hit> = client
                .sql_typed(
                    "SELECT id, type, markdown FROM blocks \
                     WHERE markdown LIKE '%RNA%' LIMIT 50",
                )
                .await?;
            if !rows.is_empty() {
                Ok(Some(rows))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for RNA block to be indexed");

    assert_eq!(hits.len(), 1, "exactly one block should contain 'RNA'");
    assert!(
        hits[0].markdown.contains("RNA"),
        "block markdown must contain the search term; got: {:?}",
        hits[0].markdown
    );
}

// ---------------------------------------------------------------------------
// Test 2: search by type returns only blocks of that type
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_by_type_filters_correctly() {
    let f = boot_with_seed().await.expect("boot");

    // Append a heading with distinctive content.
    f.client
        .append_block_markdown("## Eukaryotic transcription factors", &f.doc_id)
        .await
        .expect("append heading block");

    // Wait for the SQL index.
    let client = &f.client;
    let hits = wait_for(
        || async {
            let rows: Vec<Hit> = client
                .sql_typed(
                    "SELECT id, type, markdown FROM blocks \
                     WHERE type = 'h' AND content LIKE '%transcription%' LIMIT 50",
                )
                .await?;
            if !rows.is_empty() {
                Ok(Some(rows))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for heading block to be indexed");

    assert!(
        !hits.is_empty(),
        "should find at least one heading containing 'transcription'"
    );
    for hit in &hits {
        assert_eq!(
            hit.block_type, "h",
            "all returned blocks must be headings; got type={} for id={}",
            hit.block_type, hit.id
        );
    }
}

// ---------------------------------------------------------------------------
// Test 3: search with no matching results returns empty (null data handling)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_no_matches_returns_empty() {
    let f = boot_with_seed().await.expect("boot");

    // Search for a string that definitely does not exist in any block.
    // The kernel may return `data: null` or `data: []` -- both are handled
    // by the `sql()` fix that treats `data: null` as empty result set.
    let rows: Vec<Hit> = f
        .client
        .sql_typed(
            "SELECT id, type, markdown FROM blocks \
             WHERE markdown LIKE '%xyznonexistent12345%' LIMIT 10",
        )
        .await
        .expect("search with no matches must succeed, not error");

    assert!(
        rows.is_empty(),
        "no rows expected for impossible search; got {rows:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 4: search by contains (LIKE on content column)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_by_content_substring() {
    let f = boot_with_seed().await.expect("boot");

    // Append a paragraph with a distinctive word in its text content.
    f.client
        .append_block_markdown("Telomerase extends chromosome ends", &f.doc_id)
        .await
        .expect("append telomerase block");

    // Wait for the SQL index.
    let client = &f.client;
    let hits = wait_for(
        || async {
            let rows: Vec<Hit> = client
                .sql_typed(
                    "SELECT id, type, markdown FROM blocks \
                     WHERE content LIKE '%Telomerase%' LIMIT 50",
                )
                .await?;
            if !rows.is_empty() {
                Ok(Some(rows))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for telomerase block to be indexed");

    assert_eq!(
        hits.len(),
        1,
        "exactly one block should contain 'Telomerase' in content"
    );
    assert!(
        hits[0].markdown.contains("Telomerase"),
        "block markdown must contain the search term; got: {:?}",
        hits[0].markdown
    );
}
