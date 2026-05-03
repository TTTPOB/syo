//! Integration tests for notebook + filetree APIs.
//!
//! Run with: `cargo test -p syo --test notebook_filetree -- --ignored --test-threads=1`

mod common;

use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use common::{Fixture, boot_with_seed, cleanup_fixture, wait_for};

/// Path to the compiled `syo` binary (cargo sets `CARGO_BIN_EXE_syo` for tests).
fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_syo"))
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
        .get_ids_by_hpath(&f.notebook_id, &f.doc_hpath)
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
    let expected = f.doc_hpath.trim_start_matches('/');
    assert!(
        hpath.contains(expected),
        "hpath should contain '{expected}'; got: {hpath:?}"
    );
}

/// Wait for an hpath inside `notebook` to contain `needle`. Used by the
/// rename / move tests that drive the CLI binary and then verify via the
/// kernel's in-memory filetree.
async fn wait_for_hpath_containing(
    client: &siyuan_client::SiyuanClient,
    doc_id: &siyuan_types::BlockId,
    needle: &str,
) -> anyhow::Result<String> {
    let needle = needle.to_string();
    let id = doc_id.clone();
    wait_for(
        || async {
            let h = client.get_hpath_by_id(&id).await?;
            if h.contains(&needle) {
                Ok(Some(h))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(5),
    )
    .await
}

/// Drive the compiled `syo` binary so the test exercises the same clap
/// parse path the user/agent sees. Returns stdout on success.
fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .args(["--base-url", common::base_url(), "--token", common::token()])
        .args(args)
        .output()
        .expect("spawn syo binary")
}

#[tokio::test]
#[ignore]
async fn rename_doc_by_id_changes_title() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    let id_str = f.doc_id.to_string();
    let out = run_cli(&["doc", "rename", "--id", &id_str, "--title", "Renamed By Id"]);
    assert!(
        out.status.success(),
        "doc rename --id failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let hpath = wait_for_hpath_containing(&f.client, &f.doc_id, "Renamed By Id")
        .await
        .expect("timed out waiting for hpath to reflect renamed title");
    assert!(
        hpath.contains("Renamed By Id"),
        "hpath should contain 'Renamed By Id' after rename; got: {hpath:?}"
    );
}

#[tokio::test]
#[ignore]
async fn rename_doc_by_hpath_changes_title() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    let nb_str = f.notebook_id.to_string();
    let out = run_cli(&[
        "doc",
        "rename",
        "--notebook",
        &nb_str,
        "--hpath",
        &f.doc_hpath,
        "--title",
        "Renamed By Hpath",
    ]);
    assert!(
        out.status.success(),
        "doc rename --notebook --hpath failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let hpath = wait_for_hpath_containing(&f.client, &f.doc_id, "Renamed By Hpath")
        .await
        .expect("timed out waiting for hpath to reflect renamed title");
    assert!(
        hpath.contains("Renamed By Hpath"),
        "hpath should contain 'Renamed By Hpath' after rename; got: {hpath:?}"
    );
}

/// The legacy `--path` flag is gone; clap must reject it at parse time.
/// Locking this guard prevents anyone from accidentally re-introducing the
/// old surface during a refactor.
#[test]
fn rename_doc_legacy_path_flag_is_rejected_by_clap() {
    let out = Command::new(binary_path())
        .args([
            "doc",
            "rename",
            "--notebook",
            "20260501000000-nb00001",
            "--path",
            "/20260501090000-doc0001.sy",
            "--title",
            "X",
        ])
        .output()
        .expect("spawn syo");
    assert!(
        !out.status.success(),
        "doc rename --path must error at clap parse time, but the CLI succeeded"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--path") || stderr.contains("unexpected"),
        "clap should mention the offending flag; got stderr: {stderr}"
    );
}

#[tokio::test]
#[ignore]
async fn move_docs_by_from_ids_relocates_doc() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    // Create the destination notebook.
    let dest = f
        .client
        .create_notebook("dest-nb-from-ids")
        .await
        .expect("create dest notebook");
    let _ = f.client.open_notebook(&dest.id).await;

    let id_str = f.doc_id.to_string();
    let dest_str = dest.id.to_string();
    let out = run_cli(&[
        "doc",
        "move",
        "--from-ids",
        &id_str,
        "--to-notebook",
        &dest_str,
        "--to-path",
        "/",
    ]);
    assert!(
        out.status.success(),
        "doc move --from-ids failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // After the move, the doc should be resolvable in the destination notebook.
    let doc_id = f.doc_id.clone();
    let dest_id = dest.id.clone();
    let ids = wait_for(
        || async {
            let ids = f.client.get_ids_by_hpath(&dest_id, &f.doc_hpath).await?;
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
    assert!(ids.contains(&f.doc_id));
}

#[tokio::test]
#[ignore]
async fn move_docs_by_from_hpaths_relocates_doc() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    let dest = f
        .client
        .create_notebook("dest-nb-from-hpaths")
        .await
        .expect("create dest notebook");
    let _ = f.client.open_notebook(&dest.id).await;

    let nb_str = f.notebook_id.to_string();
    let dest_str = dest.id.to_string();
    let out = run_cli(&[
        "doc",
        "move",
        "--notebook",
        &nb_str,
        "--from-hpaths",
        &f.doc_hpath,
        "--to-notebook",
        &dest_str,
        "--to-path",
        "/",
    ]);
    assert!(
        out.status.success(),
        "doc move --from-hpaths failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let doc_id = f.doc_id.clone();
    let dest_id = dest.id.clone();
    let ids = wait_for(
        || async {
            let ids = f.client.get_ids_by_hpath(&dest_id, &f.doc_hpath).await?;
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
    assert!(ids.contains(&f.doc_id));
}

/// Legacy `--from-paths` flag is gone; clap must reject it at parse time.
#[test]
fn move_docs_legacy_from_paths_flag_is_rejected_by_clap() {
    let out = Command::new(binary_path())
        .args([
            "doc",
            "move",
            "--from-paths",
            "/20260501090000-doc0001.sy",
            "--to-notebook",
            "20260501000000-nb00002",
            "--to-path",
            "/",
        ])
        .output()
        .expect("spawn syo");
    assert!(
        !out.status.success(),
        "doc move --from-paths must error at clap parse time, but the CLI succeeded"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--from-paths") || stderr.contains("unexpected"),
        "clap should mention the offending flag; got stderr: {stderr}"
    );
}

#[tokio::test]
#[ignore]
async fn remove_doc_by_id_makes_lookup_empty() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    let id_str = f.doc_id.to_string();
    let out = run_cli(&["doc", "remove", "--id", &id_str]);
    assert!(
        out.status.success(),
        "doc remove --id failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let nb_id = f.notebook_id.clone();
    let ids = wait_for(
        || async {
            let ids = f.client.get_ids_by_hpath(&nb_id, &f.doc_hpath).await?;
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
    assert!(ids.is_empty());
}

#[tokio::test]
#[ignore]
async fn remove_doc_by_hpath_makes_lookup_empty() {
    let f: Fixture = boot_with_seed().await.expect("boot");

    let nb_str = f.notebook_id.to_string();
    let out = run_cli(&[
        "doc",
        "remove",
        "--notebook",
        &nb_str,
        "--hpath",
        &f.doc_hpath,
    ]);
    assert!(
        out.status.success(),
        "doc remove --notebook --hpath failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let nb_id = f.notebook_id.clone();
    let ids = wait_for(
        || async {
            let ids = f.client.get_ids_by_hpath(&nb_id, &f.doc_hpath).await?;
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
    assert!(ids.is_empty());
}

/// Legacy `--path` flag is gone; clap must reject it at parse time.
#[test]
fn remove_doc_legacy_path_flag_is_rejected_by_clap() {
    let out = Command::new(binary_path())
        .args([
            "doc",
            "remove",
            "--notebook",
            "20260501000000-nb00001",
            "--path",
            "/20260501090000-doc0001.sy",
        ])
        .output()
        .expect("spawn syo");
    assert!(
        !out.status.success(),
        "doc remove --path must error at clap parse time, but the CLI succeeded"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--path") || stderr.contains("unexpected"),
        "clap should mention the offending flag; got stderr: {stderr}"
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
            common::base_url(),
            "--token",
            common::token(),
            "notebook",
            "ls",
            "--format",
            "json",
        ])
        .output()
        .expect("spawn syo notebook ls --format json");

    assert!(
        output.status.success(),
        "syo notebook ls --format json exited non-zero: stderr={}",
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
