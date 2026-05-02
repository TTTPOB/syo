//! Integration tests for search text and search blocks.
//!
//! Run with: `cargo test -p siyuan-cli --test search -- --ignored --test-threads=1`

mod common;

use std::time::Duration;

use common::{boot_with_seed, wait_for};
use serde::Deserialize;

// ---------------------------------------------------------------------------
// Test 1: search text (LIKE on markdown) finds blocks containing the query
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Hit {
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    markdown: String,
}

#[tokio::test]
#[ignore]
async fn search_text_finds_matching_blocks() {
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
// Test 2: search blocks by type returns only blocks of that type
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_blocks_by_type_filters_correctly() {
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
// Test 4: search blocks by contains (LIKE on content column)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_blocks_by_content_substring() {
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
