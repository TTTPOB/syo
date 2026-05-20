//! Advanced block API integration tests.
//!
//! Run with:
//!   cargo test -p syo --test block_advanced -- --ignored --test-threads=1

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::{boot_with_seed, cleanup_fixture, wait_for};
use siyuan_model::{load::load_doc, pagination::PageRequest, section::populate_section_children};
use siyuan_types::BlockType;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Load doc with page_size=200, returning all blocks.
async fn load_all(f: &common::Fixture) -> anyhow::Result<Vec<siyuan_types::BlockNode>> {
    let bundle = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 200,
        },
    )
    .await?;
    Ok(bundle.blocks)
}

// ---------------------------------------------------------------------------
// Insert position: before anchor (next_id)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn insert_block_before_anchor_with_next_id() {
    let f = boot_with_seed().await.expect("boot");

    // Find the seeded "A target paragraph." block.
    let blocks = load_all(&f).await.expect("initial load");
    let anchor = blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .expect("seed contains 'A target paragraph.'")
        .clone();

    let unique_marker = "BEFORE_ANCHOR_UNIQUE_XQ1";
    f.client
        .insert_block_markdown(unique_marker, None, Some(&anchor.id), None)
        .await
        .expect("insert with next_id");

    // Poll until the new block appears in the SQL index.
    let client = &f.client;
    let doc_id = &f.doc_id;
    let anchor_id = anchor.id.clone();
    let blocks = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            if b.blocks.iter().any(|blk| blk.markdown == unique_marker) {
                Ok(Some(b.blocks))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for before-anchor insert");

    let new_idx = blocks
        .iter()
        .position(|b| b.markdown == unique_marker)
        .expect("new block present");
    let anchor_idx = blocks
        .iter()
        .position(|b| b.id == anchor_id)
        .expect("anchor present");

    // The new block must appear immediately before the anchor.
    assert!(
        new_idx < anchor_idx,
        "new block ({new_idx}) must precede anchor ({anchor_idx})"
    );
    assert_eq!(
        new_idx + 1,
        anchor_idx,
        "new block must be immediately before anchor (adjacent)"
    );
}

// ---------------------------------------------------------------------------
// Insert position: under parent (parent_id)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn insert_block_under_parent() {
    let f = boot_with_seed().await.expect("boot");

    // kernel quirk: inserting with only parent_id places the block as a child
    // of that parent; the doc root is a valid parent_id here.
    let doc_id_block = f.doc_id.clone();
    let unique_marker = "PARENT_INSERT_UNIQUE_XQ2";
    f.client
        .insert_block_markdown(unique_marker, None, None, Some(&doc_id_block))
        .await
        .expect("insert with parent_id");

    let client = &f.client;
    let doc_id = &f.doc_id;
    let blocks = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            if b.blocks.iter().any(|blk| blk.markdown == unique_marker) {
                Ok(Some(b.blocks))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for parent-insert block");

    let new_block = blocks
        .iter()
        .find(|b| b.markdown == unique_marker)
        .expect("new block present");

    // The block must exist in the doc; its root_id is the doc_id.
    assert_eq!(
        new_block.root_id.as_str(),
        doc_id_block.as_str(),
        "new block's root_id should be the doc id"
    );
    // notebook_id is referenced here to keep the Fixture field from triggering
    // dead_code when compiling this test binary in isolation.
    assert_eq!(
        new_block.notebook_id.as_str(),
        f.notebook_id.as_str(),
        "new block's notebook_id should match the seeded notebook"
    );
}

// ---------------------------------------------------------------------------
// Append to doc root
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn append_block_to_doc_root() {
    let f = boot_with_seed().await.expect("boot");

    let unique_marker = "DOC_TAIL_UNIQUE_XQ3";
    f.client
        .append_block_markdown(unique_marker, &f.doc_id)
        .await
        .expect("append_block_markdown to doc root");

    let client = &f.client;
    let doc_id = &f.doc_id;
    let blocks = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            if b.blocks.iter().any(|blk| blk.markdown == unique_marker) {
                Ok(Some(b.blocks))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for appended block at doc root");

    // The new block must be the last non-document block (exclude the doc-type node itself).
    let paragraphs_and_headings: Vec<_> = blocks
        .iter()
        .filter(|b| b.block_type != BlockType::Document)
        .collect();

    let last = paragraphs_and_headings.last().expect("at least one block");
    assert_eq!(
        last.markdown, unique_marker,
        "appended block should be the last top-level content block"
    );
}

// ---------------------------------------------------------------------------
// Prepend to doc root
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn prepend_block_to_doc_root() {
    let f = boot_with_seed().await.expect("boot");

    let unique_marker = "DOC_HEAD_UNIQUE_XQ4";
    f.client
        .prepend_block_markdown(unique_marker, &f.doc_id)
        .await
        .expect("prepend_block_markdown to doc root");

    let client = &f.client;
    let doc_id = &f.doc_id;
    let blocks = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            if b.blocks.iter().any(|blk| blk.markdown == unique_marker) {
                Ok(Some(b.blocks))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for prepended block at doc root");

    // kernel quirk: prepend to the doc inserts before the first child; the
    // document block (type=d) is always listed first, so skip it and look at
    // non-document blocks only.
    let non_doc_blocks: Vec<_> = blocks
        .iter()
        .filter(|b| b.block_type != BlockType::Document)
        .collect();

    // The prepended block should appear before all paragraphs. Some SiYuan
    // versions place it as the very first child (before even the h1 title
    // heading). We assert it appears before any paragraph that was in the
    // original seed (not including itself).
    let new_idx = non_doc_blocks
        .iter()
        .position(|b| b.markdown == unique_marker)
        .expect("prepended block present");

    // The first non-doc block should be heading or the prepended block itself;
    // either way the new block must appear before the "Goals" heading.
    let goals_idx = non_doc_blocks
        .iter()
        .position(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present");

    assert!(
        new_idx < goals_idx,
        "prepended block ({new_idx}) should appear before '## Goals' heading ({goals_idx})"
    );
}

// ---------------------------------------------------------------------------
// Append to heading section
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn append_to_heading_section() {
    let f = boot_with_seed().await.expect("boot");

    // Find the "## Goals" heading.
    let blocks = load_all(&f).await.expect("initial load");
    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();

    let unique_marker = "GOALS_TAIL_UNIQUE_XQ5";
    f.client
        .append_block_markdown(unique_marker, &goals.id)
        .await
        .expect("append to Goals heading");

    let client = &f.client;
    let doc_id = &f.doc_id;
    let goals_id = goals.id.clone();
    let (new_id, section_children) = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            let new_blk = b.blocks.iter().find(|blk| blk.markdown == unique_marker);
            let Some(new_blk) = new_blk else {
                return Ok(None);
            };
            let new_id = new_blk.id.clone();
            // After finding the block, re-run populate_section_children.
            let mut blks = b.blocks.clone();
            populate_section_children(&mut blks);
            let h = blks
                .iter()
                .find(|blk| blk.id == goals_id)
                .expect("Goals heading still present");
            if h.section_children.contains(&new_id) {
                Ok(Some((new_id, h.section_children.clone())))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for appended block to appear in Goals section_children");

    // The new block should be the LAST entry in section_children.
    let last_child = section_children.last().expect("section has children");
    assert_eq!(
        *last_child, new_id,
        "appended block should be the last child of Goals heading"
    );
}

// ---------------------------------------------------------------------------
// Prepend to heading section
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn prepend_to_heading_section() {
    let f = boot_with_seed().await.expect("boot");

    // Find the "## Goals" heading.
    let blocks = load_all(&f).await.expect("initial load");
    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();

    let unique_marker = "GOALS_HEAD_UNIQUE_XQ6";
    f.client
        .prepend_block_markdown(unique_marker, &goals.id)
        .await
        .expect("prepend to Goals heading");

    let client = &f.client;
    let doc_id = &f.doc_id;
    let goals_id = goals.id.clone();
    let (new_id, section_children) = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            let new_blk = b.blocks.iter().find(|blk| blk.markdown == unique_marker);
            let Some(new_blk) = new_blk else {
                return Ok(None);
            };
            let new_id = new_blk.id.clone();
            let mut blks = b.blocks.clone();
            populate_section_children(&mut blks);
            let h = blks
                .iter()
                .find(|blk| blk.id == goals_id)
                .expect("Goals heading still present");
            if h.section_children.contains(&new_id) {
                Ok(Some((new_id, h.section_children.clone())))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for prepended block to appear in Goals section_children");

    // The new block should be the FIRST entry in section_children.
    let first_child = section_children.first().expect("section has children");
    assert_eq!(
        *first_child, new_id,
        "prepended block should be the first child of Goals heading"
    );
}

// ---------------------------------------------------------------------------
// Move block to different parent
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn move_block_to_different_parent() {
    let f = boot_with_seed().await.expect("boot");

    let mut blocks = load_all(&f).await.expect("initial load");
    populate_section_children(&mut blocks);

    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();
    let targets = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Targets"))
        .expect("Targets heading present")
        .clone();

    let goals_para_id = goals
        .section_children
        .first()
        .expect("Goals has at least one child")
        .clone();
    let goals_para = blocks
        .iter()
        .find(|b| b.id == goals_para_id)
        .expect("paragraph block exists")
        .clone();

    f.client
        .move_block(&goals_para.id, None, Some(&targets.id))
        .await
        .expect("move_block should succeed");

    // Verify the move is reflected in the SQL index.
    let client = &f.client;
    let doc_id = &f.doc_id;
    let goals_id = goals.id.clone();
    let targets_id = targets.id.clone();
    let para_id = goals_para.id.clone();
    wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            let mut blks = b.blocks.clone();
            populate_section_children(&mut blks);
            let t_head = blks
                .iter()
                .find(|blk| blk.id == targets_id)
                .expect("Targets heading");
            let g_head = blks
                .iter()
                .find(|blk| blk.id == goals_id)
                .expect("Goals heading");
            if t_head.section_children.contains(&para_id)
                && !g_head.section_children.contains(&para_id)
            {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("kernel did execute move; timed out waiting for SQL to reflect it");
}

// ---------------------------------------------------------------------------
// Move block within same parent (reorder via previous_id)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn move_block_within_same_parent() {
    let f = boot_with_seed().await.expect("boot");

    let mut blocks = load_all(&f).await.expect("initial load");
    populate_section_children(&mut blocks);

    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();

    assert!(
        goals.section_children.len() >= 2,
        "Goals needs >=2 children to test reorder"
    );
    let first_id = goals.section_children[0].clone();
    let second_id = goals.section_children[1].clone();

    // Move first to after second; verify second_id is now first under Goals.
    f.client
        .move_block(&first_id, Some(&second_id), None)
        .await
        .expect("move_block should succeed");

    // Verify the reorder is reflected in the SQL index.
    let client = &f.client;
    let doc_id = &f.doc_id;
    let goals_id = goals.id.clone();
    wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            let mut blks = b.blocks.clone();
            populate_section_children(&mut blks);
            let g_head = blks
                .iter()
                .find(|blk| blk.id == goals_id)
                .expect("Goals heading");
            if g_head.section_children.first() == Some(&second_id) {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("kernel reordered blocks; timed out waiting for SQL to reflect it");
}

// ---------------------------------------------------------------------------
// Delete heading (empty section)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn delete_heading_cascades_section() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");
    let empty_section = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Empty Section"))
        .expect("'## Empty Section' heading present")
        .clone();
    let empty_id = empty_section.id.clone();

    f.client
        .delete_block(&empty_id)
        .await
        .expect("delete_block on Empty Section heading");

    let client = &f.client;
    let doc_id = &f.doc_id;
    wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            if !b.blocks.iter().any(|blk| blk.id == empty_id) {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for Empty Section heading deletion to appear in SQL index");
}

// ---------------------------------------------------------------------------
// Block attrs: round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn set_and_get_block_attrs_round_trip() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");
    let target = blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .expect("seed contains 'A target paragraph.'")
        .clone();

    let mut attrs = BTreeMap::new();
    attrs.insert("custom-flag".to_string(), "true".to_string());
    attrs.insert("custom-note".to_string(), "hello world".to_string());

    f.client
        .set_block_attrs(&target.id, &attrs)
        .await
        .expect("set_block_attrs");

    // get_block_attrs reads live from the kernel (not SQL), so no polling needed.
    let got = f
        .client
        .get_block_attrs(&target.id)
        .await
        .expect("get_block_attrs");

    assert_eq!(
        got.get("custom-flag").map(String::as_str),
        Some("true"),
        "custom-flag should be 'true'"
    );
    assert_eq!(
        got.get("custom-note").map(String::as_str),
        Some("hello world"),
        "custom-note should be 'hello world'"
    );
}

// ---------------------------------------------------------------------------
// Block attrs: overwrite existing
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn set_block_attrs_overwrites_existing() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");
    let target = blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .expect("seed contains 'A target paragraph.'")
        .clone();

    // Set initial value.
    let mut attrs = BTreeMap::new();
    attrs.insert("custom-flag".to_string(), "true".to_string());
    f.client
        .set_block_attrs(&target.id, &attrs)
        .await
        .expect("initial set_block_attrs");

    // Overwrite with new value.
    let mut attrs2 = BTreeMap::new();
    attrs2.insert("custom-flag".to_string(), "false".to_string());
    f.client
        .set_block_attrs(&target.id, &attrs2)
        .await
        .expect("overwrite set_block_attrs");

    let got = f
        .client
        .get_block_attrs(&target.id)
        .await
        .expect("get_block_attrs after overwrite");

    // kernel quirk: set_block_attrs is a merge/patch operation, not a replace.
    // Calling with {"custom-flag": "false"} updates that key in place; the
    // previously set key retains the new value "false".
    assert_eq!(
        got.get("custom-flag").map(String::as_str),
        Some("false"),
        "custom-flag should be overwritten to 'false'"
    );
}

// ---------------------------------------------------------------------------
// Block attrs: unicode value round-trip
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn set_block_attrs_unicode() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");
    let target = blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .expect("seed contains 'A target paragraph.'")
        .clone();

    let unicode_value = "中文 🎉 emoji";
    let mut attrs = BTreeMap::new();
    attrs.insert("custom-note".to_string(), unicode_value.to_string());

    f.client
        .set_block_attrs(&target.id, &attrs)
        .await
        .expect("set_block_attrs with unicode value");

    let got = f
        .client
        .get_block_attrs(&target.id)
        .await
        .expect("get_block_attrs after unicode set");

    assert_eq!(
        got.get("custom-note").map(String::as_str),
        Some(unicode_value),
        "unicode value should round-trip unchanged through get_block_attrs"
    );
}

// ---------------------------------------------------------------------------
// FIX 2: delete-block rejects document root, allows non-document blocks
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn delete_block_rejects_document_root() {
    let f = boot_with_seed().await.expect("boot");

    // Try to delete the document root block via delete_block — must be rejected.
    let args = syo_cli::commands::block::delete::DeleteBlockArgs {
        id: f.doc_id.to_string(),
        include_heading_section: false,
    };
    let result = syo_cli::commands::block::delete::run(&f.client, args).await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("document root"),
                "error should mention 'document root': {msg}"
            );
            assert!(
                msg.contains("doc remove"),
                "error should suggest 'doc remove': {msg}"
            );
        }
        Ok(_) => panic!("should have rejected deletion of a document root block"),
    }
}

#[tokio::test]
#[ignore]
async fn delete_block_allows_non_document_blocks() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");

    // Find a paragraph block (not a document root)
    let para = blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .expect("seed contains 'A target paragraph.'");

    let args = syo_cli::commands::block::delete::DeleteBlockArgs {
        id: para.id.to_string(),
        include_heading_section: false,
    };
    let result = syo_cli::commands::block::delete::run(&f.client, args).await;

    assert!(result.is_ok(), "should allow deletion of a paragraph block");
}
