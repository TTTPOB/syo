//! End-to-end CLI integration tests.
//!
//! Run with: `cargo test -p siyuan-cli --test cli_integration -- --ignored --nocapture`

mod common;

use std::time::Duration;

use common::{boot_with_seed, wait_for};
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

    // Poll until SQL index reflects the update; the index converges asynchronously.
    let target_id = target.id.clone();
    let client = &f.client;
    let doc_id = &f.doc_id;
    let reloaded = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 100,
                },
            )
            .await?;
            if b.blocks
                .iter()
                .any(|blk| blk.id == target_id && blk.markdown == "Replaced text.")
            {
                Ok(Some(b))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(5),
    )
    .await
    .expect("timed out waiting for update to appear in SQL index");

    let updated = reloaded.blocks.iter().find(|b| b.id == target_id).unwrap();
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

    // Poll until all three inserted blocks appear in the SQL index.
    let client = &f.client;
    let doc_id = &f.doc_id;
    let reloaded = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 100,
                },
            )
            .await?;
            let count = b
                .blocks
                .iter()
                .filter(|blk| blk.markdown.starts_with("Inserted "))
                .count();
            if count == 3 { Ok(Some(b)) } else { Ok(None) }
        },
        Duration::from_secs(5),
    )
    .await
    .expect("timed out waiting for inserted blocks to appear in SQL index");

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

    // Poll until the deletion is reflected in the SQL index.
    let client = &f.client;
    let doc_id = &f.doc_id;
    let reloaded = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 100,
                },
            )
            .await?;
            if !b.blocks.iter().any(|blk| blk.id == target_id) {
                Ok(Some(b))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(5),
    )
    .await
    .expect("timed out waiting for deletion to appear in SQL index");

    assert!(
        !reloaded.blocks.iter().any(|b| b.id == target_id),
        "deleted block should not appear in reload"
    );
}

#[tokio::test]
#[ignore]
async fn append_section_inserts_at_section_end() {
    use siyuan_model::section::populate_section_children;

    let f = boot_with_seed().await.expect("boot");

    // Poll until the Goals section has indexed children in the SQL store.
    let client = &f.client;
    let doc_id = &f.doc_id;
    // Poll until the Goals heading has indexed children; use populate_section_children
    // (the same helper the CLI uses) to find the last child id.
    let section_end = wait_for(
        || async {
            let b = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 100,
                },
            )
            .await?;
            let goals_heading = b
                .blocks
                .iter()
                .find(|blk| blk.markdown.starts_with("## Goals"));
            let Some(h) = goals_heading else {
                return Ok(None);
            };
            let goals_id = h.id.clone();
            let mut blocks = b.blocks.clone();
            populate_section_children(&mut blocks);
            let h2 = blocks.iter().find(|blk| blk.id == goals_id).unwrap();
            Ok(h2.section_children.last().cloned())
        },
        Duration::from_secs(5),
    )
    .await
    .expect("timed out waiting for Goals section children to appear in SQL index");

    let new = "End-of-section content.";
    f.client
        .insert_block_markdown(new, Some(&section_end), None, None)
        .await
        .expect("insert");

    // Poll until the inserted block appears in the SQL index.
    let reloaded = wait_for(
        || async {
            let b = load_doc(
                &f.client,
                &f.doc_id,
                PageRequest {
                    page: 1,
                    page_size: 100,
                },
            )
            .await?;
            if b.blocks
                .iter()
                .any(|blk| blk.markdown == "End-of-section content.")
            {
                Ok(Some(b))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(5),
    )
    .await
    .expect("timed out waiting for inserted block to appear in SQL index");

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

// ---------------------------------------------------------------------------
// FIX 4: doc move gives clear error for missing target folder
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn doc_move_rejects_missing_target_folder() {
    let f = boot_with_seed().await.expect("boot");

    // Create a doc to move
    let doc_id = f
        .client
        .create_doc_with_md(&f.notebook_id, "/MoveSource", "# Source")
        .await
        .expect("create source");

    // Try to move it to a path where the parent folder doesn't exist.
    // The CLI handler should give a clear error, not a cryptic "block not found".
    let args = siyuan_cli::commands::doc::MoveArgs {
        from_ids: vec![doc_id.to_string()],
        notebook: None,
        from_hpaths: vec![],
        to_notebook: f.notebook_id.to_string(),
        to_path: "/NonExistentFolder/Target".to_string(),
    };

    let cmd = siyuan_cli::commands::doc::DocCmd::Move(args);
    let result = siyuan_cli::commands::doc::run(&f.client, cmd).await;

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("parent folder"),
                "error should mention parent folder: {msg}"
            );
            assert!(
                msg.contains("NonExistentFolder"),
                "error should name the missing folder: {msg}"
            );
        }
        Ok(_) => panic!("should have returned an error for missing target folder"),
    }
}
