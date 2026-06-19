//! Advanced block API integration tests.
//!
//! Run with:
//!   cargo test -p syo --test block_advanced -- --ignored --test-threads=1

mod common;

use std::collections::BTreeMap;
use std::time::Duration;

use common::{boot_with_seed, wait_for, wait_for_doc_indexed};
use siyuan_client::SiyuanClient;
use siyuan_model::{load::load_doc, pagination::PageRequest, section::populate_section_children};
use siyuan_types::{BlockId, BlockNode, BlockType, PositionKind};

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

async fn load_doc_blocks(
    client: &SiyuanClient,
    doc_id: &BlockId,
) -> anyhow::Result<Vec<BlockNode>> {
    let bundle = load_doc(
        client,
        doc_id,
        PageRequest {
            page: 1,
            page_size: 200,
        },
    )
    .await?;
    Ok(bundle.blocks)
}

fn find_block<'a>(blocks: &'a [BlockNode], markdown: &str) -> Option<&'a BlockNode> {
    blocks.iter().find(|block| block.markdown == markdown)
}

fn find_heading<'a>(blocks: &'a [BlockNode], prefix: &str) -> Option<&'a BlockNode> {
    blocks
        .iter()
        .find(|block| block.markdown.starts_with(prefix))
}

fn child_markdowns(blocks: &[BlockNode], heading: &BlockNode) -> Vec<String> {
    heading
        .section_children
        .iter()
        .filter_map(|id| {
            blocks
                .iter()
                .find(|block| block.id == *id)
                .map(|block| block.markdown.clone())
        })
        .collect()
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
// Heading section mode: get
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn get_heading_section_is_explicit() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");
    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();

    let heading_only = syo_core::block::get(
        &f.client,
        syo_core::block::GetBlockInput {
            id: goals.id.clone(),
            include_heading_children: false,
        },
    )
    .await
    .expect("get heading only");

    let meta = heading_only.meta.expect("heading metadata is present");
    assert!(!meta.heading_children_included);
    assert!(
        meta.section_child_count >= 2,
        "seeded Goals section should have body blocks"
    );
    assert!(heading_only.section_markdown.is_none());
    assert!(heading_only.kramdown.contains("Goals"));
    assert!(
        !heading_only
            .kramdown
            .contains("This is the first paragraph.")
    );

    let with_section = syo_core::block::get(
        &f.client,
        syo_core::block::GetBlockInput {
            id: goals.id.clone(),
            include_heading_children: true,
        },
    )
    .await
    .expect("get heading section");

    let meta = with_section.meta.expect("heading metadata is present");
    assert!(meta.heading_children_included);
    let section = with_section
        .section_markdown
        .expect("section markdown is included");
    assert!(section.contains("## Goals"));
    assert!(section.contains("This is the first paragraph."));
    assert!(section.contains("This paragraph references later content."));
    assert!(
        !section.contains("## Targets"),
        "section rendering must stop before the next heading"
    );
}

// ---------------------------------------------------------------------------
// Heading section mode: insert explicit section positions
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn insert_with_section_positions_targets_heading_section() {
    let f = boot_with_seed().await.expect("boot");

    let mut blocks = load_all(&f).await.expect("initial load");
    populate_section_children(&mut blocks);
    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();

    let head_marker = "INCLUDE_SECTION_HEAD_UNIQUE_XQ7";
    let tail_marker = "INCLUDE_SECTION_TAIL_UNIQUE_XQ8";

    let head_id = syo_core::block::insert(
        &f.client,
        syo_core::block::InsertBlockInput {
            markdown: head_marker.to_string(),
            position: PositionKind::PrependSection,
            anchor: goals.id.clone(),
        },
    )
    .await
    .expect("prepend to heading section")
    .id;

    let tail_id = syo_core::block::insert(
        &f.client,
        syo_core::block::InsertBlockInput {
            markdown: tail_marker.to_string(),
            position: PositionKind::AppendSection,
            anchor: goals.id.clone(),
        },
    )
    .await
    .expect("append to heading section")
    .id;

    let client = &f.client;
    let doc_id = &f.doc_id;
    let goals_id = goals.id.clone();
    let section_children = wait_for(
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
            let h = blks
                .iter()
                .find(|blk| blk.id == goals_id)
                .expect("Goals heading still present");
            if h.section_children.contains(&head_id) && h.section_children.contains(&tail_id) {
                Ok(Some(h.section_children.clone()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for section child aliases");

    assert_eq!(
        section_children.first(),
        Some(&head_id),
        "prepend_section should prepend to the section"
    );
    assert_eq!(
        section_children.last(),
        Some(&tail_id),
        "append_section should append to the section"
    );
}

#[tokio::test]
#[ignore]
async fn move_heading_with_children_keeps_section_together() {
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
    let goals_child_markdowns = child_markdowns(&blocks, &goals);
    assert!(
        goals_child_markdowns.len() >= 2,
        "Goals heading should have seeded section children"
    );

    syo_core::block::move_block(
        &f.client,
        syo_core::block::MoveBlockInput {
            id: goals.id.clone(),
            position: PositionKind::AfterBlock,
            anchor: targets.id.clone(),
            include_heading_children: true,
        },
    )
    .await
    .expect("move heading with children");

    let client = &f.client;
    let doc_id = &f.doc_id;
    let goals_id = goals.id.clone();
    let targets_id = targets.id.clone();
    let moved_child_markdowns = wait_for(
        || async {
            let b = load_doc_blocks(client, doc_id).await?;
            let ids: Vec<_> = b.iter().map(|blk| blk.id.clone()).collect();
            let Some(targets_idx) = ids.iter().position(|id| id == &targets_id) else {
                return Ok(None);
            };
            let Some(goals_idx) = ids.iter().position(|id| id == &goals_id) else {
                return Ok(None);
            };
            if goals_idx <= targets_idx {
                return Ok(None);
            }
            let mut blks = b.clone();
            populate_section_children(&mut blks);
            let Some(goals_after_move) = blks.iter().find(|blk| blk.id == goals_id) else {
                return Ok(None);
            };
            let moved_child_markdowns = child_markdowns(&blks, goals_after_move);
            if moved_child_markdowns
                .iter()
                .any(|markdown| markdown == "A target paragraph.")
            {
                Ok(Some(moved_child_markdowns))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for heading and children to move together");

    assert!(
        goals_child_markdowns
            .iter()
            .all(|markdown| moved_child_markdowns.contains(markdown)),
        "moved heading should retain its original section children; got {moved_child_markdowns:?}"
    );
    assert!(
        moved_child_markdowns
            .iter()
            .any(|markdown| markdown == "A target paragraph."),
        "same-level move after Targets should naturally absorb the old Targets body"
    );
}

#[tokio::test]
#[ignore]
async fn move_heading_with_children_reflows_target_outline_by_level() {
    let f = boot_with_seed().await.expect("boot");
    let markdown = "\
# Outline Move Target

## Orig

### Existing child

Existing child body.

### New heading moved from elsewhere

#### New heading children

New child body.
";
    let doc_id = f
        .client
        .create_doc_with_md(&f.notebook_id, "/Outline-Move-By-Level", markdown)
        .await
        .expect("create custom doc");
    wait_for_doc_indexed(&f.client, &doc_id, 7)
        .await
        .expect("custom doc indexed");

    let mut blocks = load_doc_blocks(&f.client, &doc_id)
        .await
        .expect("initial custom load");
    populate_section_children(&mut blocks);
    let orig = find_heading(&blocks, "## Orig")
        .expect("orig heading")
        .clone();
    let moved = find_heading(&blocks, "### New heading moved from elsewhere")
        .expect("moved heading")
        .clone();
    let moved_child = find_heading(&blocks, "#### New heading children")
        .expect("moved child heading")
        .clone();
    let existing = find_heading(&blocks, "### Existing child")
        .expect("existing heading")
        .clone();

    syo_core::block::move_block(
        &f.client,
        syo_core::block::MoveBlockInput {
            id: moved.id.clone(),
            position: PositionKind::AfterBlock,
            anchor: orig.id.clone(),
            include_heading_children: true,
        },
    )
    .await
    .expect("move lower-level heading into target section");

    let moved_id = moved.id.clone();
    let moved_child_id = moved_child.id.clone();
    let existing_id = existing.id.clone();
    let final_blocks = wait_for(
        || async {
            let mut blocks = load_doc_blocks(&f.client, &doc_id).await?;
            populate_section_children(&mut blocks);
            let Some(orig_after) = blocks.iter().find(|block| block.id == orig.id) else {
                return Ok(None);
            };
            let Some(moved_after) = blocks.iter().find(|block| block.id == moved_id) else {
                return Ok(None);
            };
            let Some(moved_child_after) = blocks.iter().find(|block| block.id == moved_child_id)
            else {
                return Ok(None);
            };
            let Some(existing_after) = blocks.iter().find(|block| block.id == existing_id) else {
                return Ok(None);
            };
            if orig_after.section_children.contains(&moved_id)
                && orig_after.section_children.contains(&existing_id)
                && moved_after.section_children.contains(&moved_child_id)
                && !moved_after.section_children.contains(&existing_id)
                && moved_child_after.parent_id.as_ref() == Some(&moved_id)
                && existing_after.parent_id.as_ref() == Some(&orig.id)
            {
                Ok(Some(blocks))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for outline-level reflow");

    let orig_after = final_blocks
        .iter()
        .find(|block| block.id == orig.id)
        .expect("orig after move");
    assert_eq!(
        child_markdowns(&final_blocks, orig_after),
        vec![
            "### New heading moved from elsewhere".to_string(),
            "### Existing child".to_string()
        ],
        "same-level ### headings should remain siblings under ## Orig"
    );
}

#[tokio::test]
#[ignore]
async fn move_heading_with_children_reflows_source_and_target_docs() {
    let f = boot_with_seed().await.expect("boot");
    let source_md = "\
# Source Outline

## Source parent

### Moving

Moving body.

#### Moving child

Moving child body.

### Source sibling

Source sibling body.
";
    let target_md = "\
# Target Outline

## Target anchor

Target body before move.

### Target child

Target child body.
";
    let source_doc = f
        .client
        .create_doc_with_md(&f.notebook_id, "/Outline-Move-Source", source_md)
        .await
        .expect("create source doc");
    let target_doc = f
        .client
        .create_doc_with_md(&f.notebook_id, "/Outline-Move-Target", target_md)
        .await
        .expect("create target doc");
    wait_for_doc_indexed(&f.client, &source_doc, 7)
        .await
        .expect("source indexed");
    wait_for_doc_indexed(&f.client, &target_doc, 5)
        .await
        .expect("target indexed");

    let mut source_blocks = load_doc_blocks(&f.client, &source_doc)
        .await
        .expect("source load");
    populate_section_children(&mut source_blocks);
    let mut target_blocks = load_doc_blocks(&f.client, &target_doc)
        .await
        .expect("target load");
    populate_section_children(&mut target_blocks);

    let moving = find_heading(&source_blocks, "### Moving")
        .expect("moving heading")
        .clone();
    let moving_child = find_heading(&source_blocks, "#### Moving child")
        .expect("moving child")
        .clone();
    let source_parent = find_heading(&source_blocks, "## Source parent")
        .expect("source parent")
        .clone();
    let source_sibling = find_heading(&source_blocks, "### Source sibling")
        .expect("source sibling")
        .clone();
    let target_anchor = find_heading(&target_blocks, "## Target anchor")
        .expect("target anchor")
        .clone();
    let target_body = find_block(&target_blocks, "Target body before move.")
        .expect("target body")
        .clone();
    let target_child = find_heading(&target_blocks, "### Target child")
        .expect("target child")
        .clone();

    syo_core::block::move_block(
        &f.client,
        syo_core::block::MoveBlockInput {
            id: moving.id.clone(),
            position: PositionKind::AfterBlock,
            anchor: target_anchor.id.clone(),
            include_heading_children: true,
        },
    )
    .await
    .expect("cross-doc heading section move");

    let moving_id = moving.id.clone();
    let moving_child_id = moving_child.id.clone();
    let source_parent_id = source_parent.id.clone();
    let source_sibling_id = source_sibling.id.clone();
    let target_anchor_id = target_anchor.id.clone();
    let target_body_id = target_body.id.clone();
    let target_child_id = target_child.id.clone();

    wait_for(
        || async {
            let mut source_after = load_doc_blocks(&f.client, &source_doc).await?;
            populate_section_children(&mut source_after);
            let Some(source_parent_after) = source_after
                .iter()
                .find(|block| block.id == source_parent_id)
            else {
                return Ok(None);
            };
            let Some(source_sibling_after) = source_after
                .iter()
                .find(|block| block.id == source_sibling_id)
            else {
                return Ok(None);
            };
            if source_after.iter().any(|block| block.id == moving_id)
                || !source_parent_after
                    .section_children
                    .contains(&source_sibling_id)
                || source_sibling_after.parent_id.as_ref() != Some(&source_parent_id)
            {
                return Ok(None);
            }

            let mut target_after = load_doc_blocks(&f.client, &target_doc).await?;
            populate_section_children(&mut target_after);
            let Some(moving_after) = target_after.iter().find(|block| block.id == moving_id) else {
                return Ok(None);
            };
            let Some(moving_child_after) = target_after
                .iter()
                .find(|block| block.id == moving_child_id)
            else {
                return Ok(None);
            };
            let Some(target_body_after) =
                target_after.iter().find(|block| block.id == target_body_id)
            else {
                return Ok(None);
            };
            let Some(target_child_after) = target_after
                .iter()
                .find(|block| block.id == target_child_id)
            else {
                return Ok(None);
            };
            if moving_after.parent_id.as_ref() == Some(&target_anchor_id)
                && moving_after.section_children.contains(&moving_child_id)
                && moving_child_after.parent_id.as_ref() == Some(&moving_id)
                && target_body_after.parent_id.as_ref() == Some(&moving_child_id)
                && target_child_after.parent_id.as_ref() == Some(&target_anchor_id)
                && !moving_after.section_children.contains(&target_child_id)
            {
                Ok(Some(()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for source and target outline reflow");
}

// ---------------------------------------------------------------------------
// Heading section mode: update
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn update_heading_section_replaces_body_without_crossing_next_heading() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");
    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();

    syo_core::block::update(
        &f.client,
        syo_core::block::UpdateBlockInput {
            id: goals.id.clone(),
            markdown: "## Goals Updated\n\nReplacement section paragraph.\n\nSecond replacement paragraph."
                .to_string(),
            include_heading_children: true,
        },
    )
    .await
    .expect("update heading section");

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
            let Some(updated_heading) = b.blocks.iter().find(|blk| blk.id == goals_id) else {
                return Ok(None);
            };
            let has_replacement = b
                .blocks
                .iter()
                .any(|blk| blk.markdown == "Replacement section paragraph.");
            if updated_heading.markdown.starts_with("## Goals Updated") && has_replacement {
                Ok(Some(b.blocks))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for heading section update");

    let section = syo_core::block::get(
        &f.client,
        syo_core::block::GetBlockInput {
            id: goals.id.clone(),
            include_heading_children: true,
        },
    )
    .await
    .expect("get updated heading section")
    .section_markdown
    .expect("updated section markdown");

    assert!(section.contains("## Goals Updated"));
    assert!(section.contains("Replacement section paragraph."));
    assert!(section.contains("Second replacement paragraph."));
    assert!(!section.contains("This is the first paragraph."));
    assert!(!section.contains("## Targets"));

    let blocks = load_all(&f).await.expect("reload after update");
    assert!(
        blocks
            .iter()
            .any(|blk| blk.markdown.starts_with("## Targets")),
        "next heading should survive a section update"
    );
    assert!(
        blocks
            .iter()
            .any(|blk| blk.markdown == "A target paragraph."),
        "next section body should survive a section update"
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

#[tokio::test]
#[ignore]
async fn delete_heading_section_removes_non_empty_section() {
    let f = boot_with_seed().await.expect("boot");

    let blocks = load_all(&f).await.expect("initial load");
    let goals = blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("Goals heading present")
        .clone();
    let goals_id = goals.id.clone();

    syo_core::block::delete(
        &f.client,
        syo_core::block::DeleteBlockInput {
            id: goals.id,
            include_heading_children: true,
        },
    )
    .await
    .expect("delete non-empty heading section");

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
            if !b.blocks.iter().any(|blk| blk.id == goals_id) {
                Ok(Some(b.blocks))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for heading section deletion");

    assert!(
        !blocks
            .iter()
            .any(|blk| blk.markdown == "This is the first paragraph."),
        "section child should be deleted with the heading"
    );
    assert!(
        blocks
            .iter()
            .any(|blk| blk.markdown.starts_with("## Targets")),
        "next heading should survive heading section deletion"
    );
    assert!(
        blocks
            .iter()
            .any(|blk| blk.markdown == "A target paragraph."),
        "next section body should survive heading section deletion"
    );
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
        include_heading_children: false,
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
        include_heading_children: false,
    };
    let result = syo_cli::commands::block::delete::run(&f.client, args).await;

    assert!(result.is_ok(), "should allow deletion of a paragraph block");
}
