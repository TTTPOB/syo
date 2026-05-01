//! End-to-end CLI integration tests.
//!
//! Run with: `cargo test -p siyuan-cli --test cli_integration -- --ignored --nocapture`

mod common;

use common::boot_with_seed;
use siyuan_model::{load::load_doc, pagination::PageRequest};
use siyuan_render::agent_md::render_doc;
use siyuan_types::BlockId;

#[tokio::test]
#[ignore]
async fn get_doc_returns_seeded_content() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(&f.client, &f.doc_id, PageRequest::default())
        .await
        .expect("load_doc");
    assert!(
        bundle.blocks.iter().any(|b| b.markdown.contains("Goals")),
        "should contain heading 'Goals'"
    );
    let md = render_doc(&bundle);
    assert!(md.contains("<!-- sy:doc"));
    assert!(md.contains("Goals"));
}

#[tokio::test]
#[ignore]
async fn update_block_then_reload_reflects_change() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .expect("load_doc");

    let target = bundle
        .blocks
        .iter()
        .find(|b| b.markdown == "This is the first paragraph.")
        .expect("seed contains the first paragraph");

    f.client
        .update_block_markdown(&target.id, "Replaced text.")
        .await
        .expect("update");

    let reloaded = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .unwrap();
    let updated = reloaded.blocks.iter().find(|b| b.id == target.id).unwrap();
    assert_eq!(updated.markdown, "Replaced text.");
}

#[tokio::test]
#[ignore]
async fn insert_blocks_after_anchor_preserves_order() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .expect("load_doc");

    let anchor = bundle
        .blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .expect("seed contains target paragraph");

    let new_md = "Inserted A.\n\nInserted B.\n\nInserted C.";
    f.client
        .insert_block_markdown(new_md, Some(&anchor.id), None, None)
        .await
        .expect("insert");

    let reloaded = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .unwrap();
    let positions: Vec<_> = reloaded
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.markdown.starts_with("Inserted "))
        .map(|(i, b)| (i, b.markdown.clone()))
        .collect();
    assert_eq!(
        positions.len(),
        3,
        "all three inserted blocks should be present"
    );
    let texts: Vec<_> = positions.iter().map(|(_, m)| m.clone()).collect();
    assert_eq!(texts, vec!["Inserted A.", "Inserted B.", "Inserted C."]);
}

#[tokio::test]
#[ignore]
async fn create_doc_returns_resolvable_id() {
    let f = boot_with_seed().await.expect("boot");
    let id = f
        .client
        .create_doc_with_md(&f.notebook_id, "/AnotherDoc", "# Another\n\nHello.")
        .await
        .expect("create");
    assert!(BlockId::parse(id.as_str()).is_ok());
    let bundle = load_doc(&f.client, &id, PageRequest::default())
        .await
        .unwrap();
    assert!(bundle.blocks.iter().any(|b| b.markdown == "Hello."));
}

#[tokio::test]
#[ignore]
async fn delete_block_removes_it() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .unwrap();
    let target = bundle
        .blocks
        .iter()
        .find(|b| b.markdown == "A target paragraph.")
        .unwrap();
    let target_id = target.id.clone();

    f.client.delete_block(&target_id).await.expect("delete");

    let reloaded = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .unwrap();
    assert!(
        !reloaded.blocks.iter().any(|b| b.id == target_id),
        "deleted block should not appear in reload"
    );
}

#[tokio::test]
#[ignore]
async fn append_section_inserts_at_section_end() {
    let f = boot_with_seed().await.expect("boot");
    let bundle = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .unwrap();

    let goals_heading = bundle
        .blocks
        .iter()
        .find(|b| b.markdown.starts_with("## Goals"))
        .expect("seed contains Goals heading");

    // Resolve section end via the same helper the cli uses.
    use siyuan_model::section::populate_section_children;
    let mut blocks = bundle.blocks.clone();
    populate_section_children(&mut blocks);
    let h = blocks.iter().find(|b| b.id == goals_heading.id).unwrap();
    let section_end = h
        .section_children
        .last()
        .expect("Goals section has content")
        .clone();

    let new = "End-of-section content.";
    f.client
        .insert_block_markdown(new, Some(&section_end), None, None)
        .await
        .expect("insert");

    let reloaded = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 1,
            page_size: 100,
        },
    )
    .await
    .unwrap();

    // Find the new block and the next heading; new must precede next heading.
    let new_idx = reloaded
        .blocks
        .iter()
        .position(|b| b.markdown == "End-of-section content.")
        .expect("new block present");
    let next_heading_idx = reloaded
        .blocks
        .iter()
        .position(|b| b.markdown.starts_with("## Targets"))
        .expect("next heading present");
    assert!(
        new_idx < next_heading_idx,
        "inserted block ({new_idx}) must come before next h2 ({next_heading_idx})"
    );
}
