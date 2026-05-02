//! Integration tests for tag listing, tag search, raw SQL, and relations.
//!
//! Run with: `cargo test -p siyuan-cli --test tag_search -- --ignored --test-threads=1`

mod common;

use std::time::Duration;

use common::{boot_with_seed, wait_for};
use serde::Deserialize;
use siyuan_model::{
    relations::relations_for,
    tag::{list_tags, search_by_tag},
};

// ---------------------------------------------------------------------------
// Test 1: list_tags returns seeded tags sorted
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn list_tags_includes_seeded_tags() {
    let f = boot_with_seed().await.expect("boot");

    // Append two separate paragraphs so each gets its own block id.
    f.client
        .append_block_markdown("#alpha# first tagged paragraph", &f.doc_id)
        .await
        .expect("append alpha block");
    f.client
        .append_block_markdown("#beta# second tagged paragraph", &f.doc_id)
        .await
        .expect("append beta block");

    // Tag indexing into `spans` is async; poll until both tags appear.
    let client = &f.client;
    let tags = wait_for(
        || async {
            let t = list_tags(client).await?;
            if t.contains(&"alpha".to_string()) && t.contains(&"beta".to_string()) {
                Ok(Some(t))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for alpha and beta tags to appear in spans");

    // The list must be lexicographically sorted (alpha before beta).
    let alpha_pos = tags
        .iter()
        .position(|t| t == "alpha")
        .expect("alpha present");
    let beta_pos = tags.iter().position(|t| t == "beta").expect("beta present");
    assert!(
        alpha_pos < beta_pos,
        "list_tags must return tags in ascending order; got {tags:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: search_by_tag returns the correct block
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_by_tag_returns_tagged_blocks() {
    let f = boot_with_seed().await.expect("boot");

    f.client
        .append_block_markdown("#alpha# search-target paragraph", &f.doc_id)
        .await
        .expect("append alpha block");

    // Wait for the spans table to index the new tag before querying.
    let client = &f.client;
    let hits = wait_for(
        || async {
            let h = search_by_tag(client, "alpha", 50).await?;
            if !h.is_empty() { Ok(Some(h)) } else { Ok(None) }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for alpha tag to be searchable");

    assert_eq!(hits.len(), 1, "exactly one block tagged alpha");
    assert!(
        hits[0].markdown_preview.contains("#alpha#"),
        "preview should contain the raw tag markup; got: {:?}",
        hits[0].markdown_preview
    );
}

// ---------------------------------------------------------------------------
// Test 3: search_by_tag with unknown tag returns empty, no error
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_by_tag_handles_unknown_tag() {
    let f = boot_with_seed().await.expect("boot");

    let hits = search_by_tag(&f.client, "nonexistent", 50)
        .await
        .expect("search_by_tag should not error for missing tag");

    assert!(
        hits.is_empty(),
        "no blocks should match an unknown tag; got {hits:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3b: search_by_tag honours the caller-supplied LIMIT
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn search_by_tag_respects_limit() {
    let f = boot_with_seed().await.expect("boot");

    // Append three paragraphs that all carry the same `#cap` tag.
    for body in ["#cap# row one", "#cap# row two", "#cap# row three"] {
        f.client
            .append_block_markdown(body, &f.doc_id)
            .await
            .expect("append cap block");
    }

    // Wait for all three blocks to be indexed by querying with a generous
    // limit; only after we see >=3 hits do we know the index has caught up
    // and the limit-respect assertion becomes meaningful.
    let client = &f.client;
    let _ = wait_for(
        || async {
            let h = search_by_tag(client, "cap", 50).await?;
            if h.len() >= 3 { Ok(Some(h)) } else { Ok(None) }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for three cap blocks to index");

    let limited = search_by_tag(&f.client, "cap", 2)
        .await
        .expect("search_by_tag with limit=2 should succeed");
    assert!(
        limited.len() <= 2,
        "limit=2 must cap result count; got {} hits",
        limited.len()
    );
}

// ---------------------------------------------------------------------------
// Test 4: sql_typed round-trip with a typed struct
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn sql_typed_round_trip() {
    #[derive(Debug, Deserialize)]
    struct MyRow {
        id: String,
        markdown: String,
    }

    let f = boot_with_seed().await.expect("boot");

    // ORDER BY (type='d') DESC pins the doc-root row to the top of the slice
    // so the title assertion below is independent of kernel-side row order.
    // The id tiebreaker keeps the non-doc rows deterministic across runs.
    let stmt = format!(
        "SELECT id, markdown FROM blocks WHERE root_id = '{}' \
         ORDER BY (type = 'd') DESC, id LIMIT 5",
        f.doc_id.as_str()
    );
    let rows: Vec<MyRow> = f
        .client
        .sql_typed(&stmt)
        .await
        .expect("sql_typed should succeed");

    assert!(
        !rows.is_empty(),
        "must return at least one row for the seeded doc"
    );

    // The doc-level block (type=d) has the document title as its markdown.
    assert!(
        rows.iter()
            .any(|r| r.markdown.contains("Integration Test Doc")),
        "at least one row must contain the doc title; rows: {rows:?}"
    );

    // Every returned id must be parseable as a BlockId.
    for row in &rows {
        siyuan_types::BlockId::parse(&row.id)
            .unwrap_or_else(|_| panic!("id {:?} must be a valid BlockId", row.id));
    }
}

// ---------------------------------------------------------------------------
// Test 5: sql raw with no matching rows returns Ok(empty)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn sql_raw_handles_empty_result() {
    let f = boot_with_seed().await.expect("boot");

    let rows = f
        .client
        .sql("SELECT * FROM blocks WHERE id = 'no-such-id'")
        .await
        .expect("sql must not error for a valid query with zero results");

    assert!(
        rows.is_empty(),
        "no rows expected for impossible predicate; got {rows:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 6: relations_for populates tags per block
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn relations_for_populates_tags() {
    let f = boot_with_seed().await.expect("boot");

    let alpha_id = f
        .client
        .append_block_markdown("#alpha# relations-tag-test alpha", &f.doc_id)
        .await
        .expect("append alpha block");
    let beta_id = f
        .client
        .append_block_markdown("#beta# relations-tag-test beta", &f.doc_id)
        .await
        .expect("append beta block");

    // Wait until the spans table has indexed both tags.
    let client = &f.client;
    let (a_clone, b_clone) = (alpha_id.clone(), beta_id.clone());
    wait_for(
        || async {
            let tags = list_tags(client).await?;
            if tags.contains(&"alpha".to_string()) && tags.contains(&"beta".to_string()) {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for spans to index both tags");

    let map = relations_for(client, &[a_clone.clone(), b_clone.clone()])
        .await
        .expect("relations_for should succeed");

    let alpha_summary = map.get(&a_clone).expect("alpha block in relations map");
    assert!(
        alpha_summary.tags.contains(&"alpha".to_string()),
        "alpha block summary must contain tag 'alpha'; got: {:?}",
        alpha_summary.tags
    );

    let beta_summary = map.get(&b_clone).expect("beta block in relations map");
    assert!(
        beta_summary.tags.contains(&"beta".to_string()),
        "beta block summary must contain tag 'beta'; got: {:?}",
        beta_summary.tags
    );
}

// ---------------------------------------------------------------------------
// Test 7: relations_for counts incoming and outgoing refs
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn relations_for_counts_incoming_and_outgoing_refs() {
    let f = boot_with_seed().await.expect("boot");

    // Append block B first so we have its id to embed in A's markdown.
    let b_id = f
        .client
        .append_block_markdown("Block B — ref target", &f.doc_id)
        .await
        .expect("append block B");

    // Append block A with a siyuan block-ref to B.
    // SiYuan block-ref syntax: ((id 'anchor text'))
    let a_md = format!(
        "Block A — references B (({} 'B preview text'))",
        b_id.as_str()
    );
    let a_id = f
        .client
        .append_block_markdown(&a_md, &f.doc_id)
        .await
        .expect("append block A with ref");

    // The `refs` table is populated asynchronously; poll until the outgoing ref
    // from A to B appears before asserting relation counts.
    let client = &f.client;
    let (a_clone, b_clone) = (a_id.clone(), b_id.clone());
    wait_for(
        || async {
            let stmt = format!(
                "SELECT COUNT(*) AS n FROM refs WHERE block_id = '{}' AND def_block_id = '{}'",
                a_clone.as_str(),
                b_clone.as_str()
            );
            #[derive(Deserialize)]
            struct CountRow {
                n: i64,
            }
            let rows: Vec<CountRow> = client.sql_typed(&stmt).await?;
            let count = rows.first().map(|r| r.n).unwrap_or(0);
            if count >= 1 { Ok(Some(())) } else { Ok(None) }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for refs table to index A→B reference");

    let map = relations_for(client, &[a_id.clone(), b_id.clone()])
        .await
        .expect("relations_for should succeed");

    let a_summary = map.get(&a_id).expect("block A in relations map");
    assert_eq!(
        a_summary.outgoing_refs.len(),
        1,
        "block A should have exactly one outgoing ref; got: {:?}",
        a_summary.outgoing_refs
    );
    assert_eq!(
        a_summary.outgoing_refs[0].target_id, b_id,
        "outgoing ref from A must point to B"
    );

    let b_summary = map.get(&b_id).expect("block B in relations map");
    assert_eq!(
        b_summary.incoming_refs_count, 1,
        "block B should have exactly one incoming ref; got: {:?}",
        b_summary.incoming_refs_count
    );
}
