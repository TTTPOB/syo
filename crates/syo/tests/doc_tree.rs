//! Integration tests for `syo doc tree`.
//!
//! Run with: `cargo test -p syo --test doc_tree -- --ignored --test-threads=1`
//!
//! The tests boot a fresh kernel container per scenario, seed three nested
//! docs (`/A`, `/A/B`, `/A/B/C`), and exercise the CLI binary across the
//! three address modes (id, notebook root, notebook+hpath) and the three
//! depth budgets (1, 2, all). The JSON output is parsed back through
//! `serde_json::from_str::<TreeNode>` so the on-disk schema is locked.

// `tests/common/mod.rs` is shared across every test target; the items this
// target doesn't consume show up as dead-code warnings only here.
#[allow(dead_code)]
mod common;

use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;

use common::wait_for;
use siyuan_client::SiyuanClient;
use siyuan_testkit::SiyuanContainer;
use siyuan_types::{BlockId, NotebookId};

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_syo"))
}

fn run_cli(container: &SiyuanContainer, args: &[&str]) -> std::process::Output {
    Command::new(binary_path())
        .args([
            "--base-url",
            container.base_url(),
            "--token",
            container.token(),
        ])
        .args(args)
        .output()
        .expect("spawn syo binary")
}

/// Local mirror of `siyuan_model::doc_tree::TreeNode` with `Deserialize`.
///
/// We can't use the production type directly because the CLI integration
/// crate doesn't depend on `siyuan-model` (its tests boot the binary by
/// path). Mirror the on-disk shape here so the JSON format stays pinned.
#[derive(Debug, Deserialize)]
struct TreeNode {
    id: String,
    title: String,
    hpath: String,
    has_children: bool,
    doc_count_recursive: u64,
    #[allow(dead_code)]
    created: String,
    #[allow(dead_code)]
    updated: String,
    #[allow(dead_code)]
    sort: i64,
    #[allow(dead_code)]
    icon: String,
    notebook_id: String,
    #[allow(dead_code)]
    notebook_name: String,
    storage_path: String,
    children: Vec<TreeNode>,
}

/// Boot a kernel and create three nested docs (`/A`, `/A/B`, `/A/B/C`)
/// in a fresh notebook.
async fn boot_with_nested_docs() -> Result<NestedFixture> {
    siyuan_testkit::init_tracing();
    let container = SiyuanContainer::start().await?;
    let client = SiyuanClient::new(container.base_url(), container.token())?;

    let nb = client.create_notebook("doctree-test").await?;
    let _ = client.open_notebook(&nb.id).await;

    // Each createDocWithMd takes the FULL hpath; the kernel auto-creates
    // intermediate folders. We create A first, then nested children, so
    // every level has a real `type='d'` row in the blocks table.
    let a = client.create_doc_with_md(&nb.id, "/A", "# A\n").await?;
    let b = client.create_doc_with_md(&nb.id, "/A/B", "# B\n").await?;
    let c = client.create_doc_with_md(&nb.id, "/A/B/C", "# C\n").await?;

    // Wait for the SQL index to catch up. `doc tree` runs against `blocks`,
    // so we need to see the three rows before the test queries the CLI.
    wait_for_doc_count(&client, &nb.id, 3).await?;

    Ok(NestedFixture {
        container,
        client,
        notebook_id: nb.id,
        a,
        b: b.clone(),
        c: c.clone(),
    })
}

#[allow(dead_code)]
struct NestedFixture {
    container: SiyuanContainer,
    client: SiyuanClient,
    notebook_id: NotebookId,
    a: BlockId,
    b: BlockId,
    c: BlockId,
}

/// Wait until the notebook contains at least `min_docs` rows of type='d'.
async fn wait_for_doc_count(
    client: &SiyuanClient,
    notebook_id: &NotebookId,
    min_docs: usize,
) -> Result<()> {
    let nb = notebook_id.clone();
    wait_for(
        || async {
            let stmt = format!(
                "SELECT id FROM blocks WHERE box = '{}' AND type = 'd'",
                nb.as_str()
            );
            let rows = client.sql(&stmt).await?;
            if rows.len() >= min_docs {
                Ok(Some(rows.len()))
            } else {
                Ok(None)
            }
        },
        Duration::from_secs(10),
    )
    .await
    .map(|_| ())
}

// ---------------------------------------------------------------------------
// clap parse-time guards (no kernel required)
// ---------------------------------------------------------------------------

/// Acceptance #2: `--depth 0` must be rejected at clap parse, not runtime.
#[test]
fn doc_tree_depth_zero_rejected_by_clap() {
    let out = Command::new(binary_path())
        .args([
            "doc",
            "tree",
            "--id",
            "20260501090000-doc0001",
            "--depth",
            "0",
        ])
        .output()
        .expect("spawn syo");
    assert!(
        !out.status.success(),
        "doc tree --depth 0 must error at clap parse time, but the CLI succeeded"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("0 is not allowed") || stderr.contains("depth"),
        "clap should mention the offending value; got stderr: {stderr}"
    );
}

/// Address modes are mutually exclusive — supplying both must fail at
/// parse time.
#[test]
fn doc_tree_id_and_notebook_are_mutually_exclusive() {
    let out = Command::new(binary_path())
        .args([
            "doc",
            "tree",
            "--id",
            "20260501090000-doc0001",
            "--notebook",
            "20260501000000-nb00001",
        ])
        .output()
        .expect("spawn syo");
    assert!(
        !out.status.success(),
        "doc tree --id + --notebook must error at clap parse time"
    );
}

// ---------------------------------------------------------------------------
// Live-kernel scenarios
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn tree_id_mode_depth_one_loads_immediate_children() {
    let f = boot_with_nested_docs().await.expect("boot");

    let a_str = f.a.to_string();
    let out = run_cli(
        &f.container,
        &[
            "doc", "tree", "--id", &a_str, "--depth", "1", "--format", "json",
        ],
    );
    assert!(
        out.status.success(),
        "doc tree --id --depth 1 failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let tree: TreeNode = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("output must parse as TreeNode: {e}; raw: {stdout}"));

    assert_eq!(tree.id, f.a.to_string(), "root must be /A's id");
    assert_eq!(tree.title, "A");
    assert_eq!(tree.hpath, "/A");
    assert!(tree.has_children, "/A has descendants");
    assert_eq!(tree.doc_count_recursive, 2, "B and C are descendants of A");
    assert_eq!(
        tree.children.len(),
        1,
        "depth=1 loads exactly one level under root"
    );
    let b = &tree.children[0];
    assert_eq!(b.title, "B");
    assert!(b.has_children);
    // Depth budget consumed; B's children must NOT be loaded.
    assert!(b.children.is_empty(), "depth=1 must not load /A/B/C");
    // Acceptance #7: doc_count_recursive reflects the FULL preload even
    // when the slice is partial.
    assert_eq!(b.doc_count_recursive, 1, "B has 1 descendant (C)");
}

#[tokio::test]
#[ignore]
async fn tree_id_mode_depth_two_loads_two_levels() {
    let f = boot_with_nested_docs().await.expect("boot");
    let a_str = f.a.to_string();
    let out = run_cli(
        &f.container,
        &[
            "doc", "tree", "--id", &a_str, "--depth", "2", "--format", "json",
        ],
    );
    assert!(out.status.success());
    let tree: TreeNode = serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].children.len(), 1);
    assert_eq!(tree.children[0].children[0].title, "C");
    // C is a leaf.
    assert!(tree.children[0].children[0].children.is_empty());
    assert!(!tree.children[0].children[0].has_children);
}

#[tokio::test]
#[ignore]
async fn tree_id_mode_depth_all_loads_full_subtree() {
    let f = boot_with_nested_docs().await.expect("boot");
    let a_str = f.a.to_string();
    let out = run_cli(
        &f.container,
        &[
            "doc", "tree", "--id", &a_str, "--depth", "all", "--format", "json",
        ],
    );
    assert!(out.status.success());
    let tree: TreeNode = serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].children.len(), 1);
    assert_eq!(tree.children[0].children[0].title, "C");
}

#[tokio::test]
#[ignore]
async fn tree_notebook_root_yields_virtual_root() {
    let f = boot_with_nested_docs().await.expect("boot");
    let nb_str = f.notebook_id.to_string();
    let out = run_cli(
        &f.container,
        &[
            "doc",
            "tree",
            "--notebook",
            &nb_str,
            "--hpath",
            "/",
            "--format",
            "json",
        ],
    );
    assert!(
        out.status.success(),
        "doc tree --notebook --hpath / failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree: TreeNode = serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).unwrap();

    // Virtual root: empty id/title/storage_path, hpath="/".
    assert_eq!(tree.id, "");
    assert_eq!(tree.title, "");
    assert_eq!(tree.hpath, "/");
    assert_eq!(tree.storage_path, "");
    assert_eq!(tree.notebook_id, f.notebook_id.to_string());
    // Total descendants = 3 (A + B + C).
    assert_eq!(tree.doc_count_recursive, 3);
    assert_eq!(tree.children.len(), 1, "/A is the only top-level doc");
    assert_eq!(tree.children[0].title, "A");
}

#[tokio::test]
#[ignore]
async fn tree_notebook_hpath_matches_id_mode() {
    let f = boot_with_nested_docs().await.expect("boot");
    let nb_str = f.notebook_id.to_string();
    let a_str = f.a.to_string();

    // Same depth across both invocations. Compare the trees field by
    // field — they should be identical except for the input route.
    let by_id = run_cli(
        &f.container,
        &[
            "doc", "tree", "--id", &a_str, "--depth", "1", "--format", "json",
        ],
    );
    let by_hpath = run_cli(
        &f.container,
        &[
            "doc",
            "tree",
            "--notebook",
            &nb_str,
            "--hpath",
            "/A",
            "--depth",
            "1",
            "--format",
            "json",
        ],
    );
    assert!(by_id.status.success());
    assert!(by_hpath.status.success());

    let t1: TreeNode = serde_json::from_str(String::from_utf8_lossy(&by_id.stdout).trim()).unwrap();
    let t2: TreeNode =
        serde_json::from_str(String::from_utf8_lossy(&by_hpath.stdout).trim()).unwrap();
    assert_eq!(t1.id, t2.id);
    assert_eq!(t1.hpath, t2.hpath);
    assert_eq!(t1.doc_count_recursive, t2.doc_count_recursive);
    assert_eq!(t1.children.len(), t2.children.len());
}

/// Acceptance #5: id mode against a non-doc block returns NotFound.
#[tokio::test]
#[ignore]
async fn tree_non_doc_id_is_not_found() {
    let f = boot_with_nested_docs().await.expect("boot");

    // Find any non-doc block under /A — its first paragraph (or the H1's
    // own row, but not the doc root itself).
    let a_str = f.a.to_string();
    let stmt = format!("SELECT id FROM blocks WHERE root_id = '{a_str}' AND type != 'd' LIMIT 1");
    let rows = f.client.sql(&stmt).await.expect("sql");
    let non_doc_id = rows
        .first()
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .expect("seeded doc must have at least one non-doc child block")
        .to_string();

    let out = run_cli(&f.container, &["doc", "tree", "--id", &non_doc_id]);
    assert!(
        !out.status.success(),
        "doc tree --id <non-doc> must fail with NotFound; stdout={}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not found") || stderr.contains("NotFound"),
        "stderr should signal NotFound; got: {stderr}"
    );
}

/// Acceptance #8: agent-md format produces a parseable bullet list with
/// HTML-comment markers.
#[tokio::test]
#[ignore]
async fn tree_agent_md_format_well_formed() {
    let f = boot_with_nested_docs().await.expect("boot");
    let a_str = f.a.to_string();
    let out = run_cli(
        &f.container,
        &["doc", "tree", "--id", &a_str, "--depth", "all"],
    );
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("<!-- sy:doc id="),
        "expected node markers; got:\n{s}"
    );
    assert!(s.contains("hpath=/A"), "expected /A in render; got:\n{s}");
    assert!(
        s.contains("<!-- sy:tree depth=all"),
        "expected trailing tree summary; got:\n{s}"
    );
}
