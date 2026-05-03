//! Integration tests for resolve_notebook_id.
//!
//! Run with: `cargo test -p syo --test notebook_resolve -- --ignored --test-threads=1`

mod common;

use common::{boot_with_seed, cleanup_fixture, shared_client};
use siyuan_types::SiyuanError;

/// resolve_notebook_id returns the same id when given a valid notebook id.
#[tokio::test]
#[ignore]
async fn resolve_by_id_returns_immediately() {
    let f = boot_with_seed().await.expect("boot");
    let result = syo_core::notebook::resolve_notebook_id(&f.client, f.notebook_id.as_str())
        .await
        .expect("resolve by id should succeed");
    assert_eq!(result, f.notebook_id);
    cleanup_fixture(f).await.expect("cleanup");
}

/// resolve_notebook_id finds a notebook by its display name.
#[tokio::test]
#[ignore]
async fn resolve_by_name_finds_notebook() {
    let f = boot_with_seed().await.expect("boot");

    // Reconstruct the notebook name from boot_with_seed — it's "it-{suffix}"
    // We get the name from ls_notebooks
    let notebooks = f.client.ls_notebooks().await.expect("ls notebooks");
    let nb = notebooks
        .iter()
        .find(|n| n.id == f.notebook_id)
        .expect("our notebook should be in the list");

    let resolved = syo_core::notebook::resolve_notebook_id(&f.client, &nb.name)
        .await
        .expect("resolve by name should succeed");
    assert_eq!(resolved, f.notebook_id);

    cleanup_fixture(f).await.expect("cleanup");
}

/// resolve_notebook_id returns NotebookNotFound for a non-existent name.
#[tokio::test]
#[ignore]
async fn resolve_unknown_name_returns_not_found() {
    let client = shared_client().await;
    let result =
        syo_core::notebook::resolve_notebook_id(&client, "this-notebook-does-not-exist-xyz").await;
    match result {
        Err(SiyuanError::NotebookNotFound { name }) => {
            assert!(name.contains("this-notebook-does-not-exist-xyz"));
        }
        other => panic!("expected NotebookNotFound, got {other:?}"),
    }
}
