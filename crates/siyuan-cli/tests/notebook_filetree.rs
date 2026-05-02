//! Integration tests for notebook + filetree APIs.
//!
//! Run with: `cargo test -p siyuan-cli --test notebook_filetree -- --ignored --test-threads=1`

mod common;

use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use common::{Fixture, boot_with_seed, wait_for};

/// Path to the compiled `siyuan` binary (cargo sets `CARGO_BIN_EXE_siyuan` for tests).
fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_siyuan"))
}

// ---------------------------------------------------------------------------
// Notebook lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn ls_notebooks_includes_seeded_notebook() {
    let f: Fixture = boot_with_seed().await.expect("boot");
    let notebooks = f.client.ls_notebooks().await.expect("ls_notebooks");
    assert!(
        notebooks.iter().any(|nb| nb.id == f.notebook_id),
        "seeded notebook id should appear in ls_notebooks"
    );
}

#[tokio::test]
#[ignore]
async fn rename_notebook_changes_name() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    f.client
        .rename_notebook(&f.notebook_id, "renamed-nb")
        .await
        .expect("rename_notebook");

    // rename_notebook is synchronous on the kernel side — no SQL lag.
    let notebooks = f
        .client
        .ls_notebooks()
        .await
        .expect("ls_notebooks after rename");
    let nb = notebooks
        .iter()
        .find(|nb| nb.id == f.notebook_id)
        .expect("seeded notebook still listed after rename");
    assert_eq!(
        nb.name, "renamed-nb",
        "notebook name should reflect the rename"
    );
}

#[tokio::test]
#[ignore]
async fn remove_notebook_drops_it_from_ls() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    // Create a fresh notebook so we don't destroy the seeded fixture.
    let extra = f
        .client
        .create_notebook("to-be-removed")
        .await
        .expect("create_notebook");

    f.client
        .remove_notebook(&extra.id)
        .await
        .expect("remove_notebook");

    let notebooks = f
        .client
        .ls_notebooks()
        .await
        .expect("ls_notebooks after remove");
    assert!(
        !notebooks.iter().any(|nb| nb.id == extra.id),
        "removed notebook should no longer appear in ls_notebooks"
    );
}

// ---------------------------------------------------------------------------
// Filetree
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn get_ids_by_hpath_resolves_seeded_doc() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    let ids = f
        .client
        .get_ids_by_hpath(&f.notebook_id, "/IntegrationTestDoc")
        .await
        .expect("get_ids_by_hpath");

    assert!(
        ids.contains(&f.doc_id),
        "get_ids_by_hpath should return the seeded doc id; got: {ids:?}"
    );
}

#[tokio::test]
#[ignore]
async fn get_hpath_by_id_round_trips() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    let hpath = f
        .client
        .get_hpath_by_id(&f.doc_id)
        .await
        .expect("get_hpath_by_id");

    // The kernel may return a leading notebook-name segment; assert on the tail.
    assert!(
        hpath.contains("IntegrationTestDoc"),
        "hpath should contain 'IntegrationTestDoc'; got: {hpath:?}"
    );
}

#[tokio::test]
#[ignore]
async fn rename_doc_changes_title() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    // The kernel's renameDoc API takes the .sy file path based on the doc id,
    // not the human-readable hpath. The actual file is /<doc_id>.sy on disk.
    let rename_path = format!("/{}.sy", f.doc_id);
    f.client
        .rename_doc(&f.notebook_id, &rename_path, "Renamed Title")
        .await
        .expect("rename_doc");

    // get_hpath_by_id reads from the kernel's in-memory filetree; poll briefly in
    // case propagation takes a moment.
    let doc_id = f.doc_id.clone();
    let hpath = wait_for(
        || async {
            let h = f.client.get_hpath_by_id(&doc_id).await?;
            if h.contains("Renamed Title") {
                Ok(Some(h))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(5),
    )
    .await
    .expect("timed out waiting for hpath to reflect renamed title");

    assert!(
        hpath.contains("Renamed Title"),
        "hpath should contain 'Renamed Title' after rename_doc; got: {hpath:?}"
    );
}

#[tokio::test]
#[ignore]
async fn move_docs_relocates_doc() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    // Create the destination notebook.
    let dest = f
        .client
        .create_notebook("dest-nb")
        .await
        .expect("create dest notebook");
    // The new notebook may start closed; open it so the kernel tracks it fully.
    let _ = f.client.open_notebook(&dest.id).await;

    // move_docs takes an on-disk .sy path: /<doc-id>.sy (no notebook segment).
    // Note: get_hpath_by_id returns only the doc title path (no notebook prefix),
    // so we cannot use hpath to verify notebook change. Instead we verify via
    // get_ids_by_hpath on the destination notebook.
    let from_path = format!("/{}.sy", f.doc_id);
    f.client
        .move_docs(&[from_path], &dest.id, "/")
        .await
        .expect("move_docs");

    // After the move, the doc should be resolvable in the destination notebook.
    let doc_id = f.doc_id.clone();
    let dest_id = dest.id.clone();
    let ids = wait_for(
        || async {
            let ids = f
                .client
                .get_ids_by_hpath(&dest_id, "/IntegrationTestDoc")
                .await?;
            if ids.contains(&doc_id) {
                Ok(Some(ids))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for moved doc to appear in dest notebook hpath lookup");

    assert!(
        ids.contains(&f.doc_id),
        "doc should be resolvable in dest notebook after move"
    );
}

#[tokio::test]
#[ignore]
async fn remove_doc_makes_lookup_empty() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    // remove_doc takes the .sy-suffixed path, not the human-readable hpath.
    // Kernel convention: the path is /<doc-id>.sy.
    let path = format!("/{}.sy", f.doc_id);
    f.client
        .remove_doc(&f.notebook_id, &path)
        .await
        .expect("remove_doc");

    // get_ids_by_hpath reads from the filetree; poll until the doc is gone.
    let nb_id = f.notebook_id.clone();
    let ids = wait_for(
        || async {
            let ids = f
                .client
                .get_ids_by_hpath(&nb_id, "/IntegrationTestDoc")
                .await?;
            if ids.is_empty() {
                Ok(Some(ids))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for removed doc to disappear from hpath lookup");

    assert!(
        ids.is_empty(),
        "get_ids_by_hpath should return empty after remove_doc"
    );
}

// ---------------------------------------------------------------------------
// `--format json` end-to-end (covers Task C)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn notebook_ls_format_json_emits_parseable_array() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    // Mirror the production CLI invocation: drive the compiled binary so we
    // exercise the actual stdout path the agent sees.
    let output = Command::new(binary_path())
        .args([
            "--base-url",
            f.container.base_url(),
            "--token",
            f.container.token(),
            "notebook",
            "ls",
            "--format",
            "json",
        ])
        .output()
        .expect("spawn siyuan notebook ls --format json");

    assert!(
        output.status.success(),
        "siyuan notebook ls --format json exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    #[derive(Debug, Deserialize)]
    struct Row {
        status: String,
        id: String,
        #[allow(dead_code)]
        name: String,
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let rows: Vec<Row> = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("output must parse as Vec<{{status,id,name}}>: {e}; raw: {stdout}")
    });

    // The seeded notebook must appear, and `status` must be the canonical
    // unpadded "open"/"closed" form (not the TSV's two-space-padded variant).
    let seeded = rows
        .iter()
        .find(|r| r.id == f.notebook_id.to_string())
        .expect("seeded notebook id must appear in --format json output");
    assert!(
        matches!(seeded.status.as_str(), "open" | "closed"),
        "status must be canonical 'open'/'closed'; got {:?}",
        seeded.status
    );
}
