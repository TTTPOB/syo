//! Shared scaffolding for cli integration tests.

use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Result, bail};
use serde::Deserialize;
use tokio::sync::OnceCell;
use tokio::time::{Instant, sleep};

use siyuan_client::SiyuanClient;
use siyuan_model::load::load_doc;
use siyuan_model::pagination::PageRequest;
use siyuan_testkit::SiyuanContainer;
use siyuan_types::{BlockId, NotebookId, SiyuanError};

// ── Shared container ──────────────────────────────────────────────────────

/// Booted once per test binary via `OnceCell::get_or_init` (async-safe).
static CONTAINER: OnceCell<SiyuanContainer> = OnceCell::const_new();
/// Stored separately so the atexit handler can clean up without taking
/// ownership of the container.
static CID: OnceLock<String> = OnceLock::new();
static WORKSPACE: OnceLock<String> = OnceLock::new();

/// Ensure the shared container is running (idempotent, concurrency-safe).
pub async fn ensure_booted() -> &'static SiyuanContainer {
    let c = CONTAINER
        .get_or_init(|| async {
            siyuan_testkit::init_tracing();
            SiyuanContainer::start()
                .await
                .expect("boot shared container")
        })
        .await;
    stash_cleanup_info(c);
    c
}

fn stash_cleanup_info(c: &SiyuanContainer) {
    let _ = CID.set(c.container_id().to_string());
    if let Some(p) = c.workspace_path() {
        let _ = WORKSPACE.set(p.to_string_lossy().into_owned());
    }
    register_atexit();
}

/// Synchronous accessors.  Only call after `ensure_booted().await`.
pub fn base_url() -> &'static str {
    CONTAINER
        .get()
        .expect("shared container not booted")
        .base_url()
}

pub fn token() -> &'static str {
    CONTAINER
        .get()
        .expect("shared container not booted")
        .token()
}

/// Create a new client connected to the shared container.
pub async fn shared_client() -> SiyuanClient {
    ensure_booted().await;
    SiyuanClient::new(base_url(), token()).expect("shared client")
}

// ── atexit cleanup ────────────────────────────────────────────────────────

/// Rust's libtest calls `std::process::exit()` which skips static destructors,
/// so the container and workspace would leak.  Register a POSIX `atexit`
/// handler to stop the container and clean up the workspace.
fn register_atexit() {
    static REGISTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if REGISTERED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    unsafe extern "C" {
        fn atexit(cb: unsafe extern "C" fn()) -> std::ffi::c_int;
    }
    unsafe extern "C" fn cleanup() {
        if let Some(id) = CID.get() {
            let _ = std::process::Command::new("podman")
                .args(["stop", "-t", "5", id])
                .status();
            let _ = std::process::Command::new("podman")
                .args(["rm", "-f", id])
                .status();
        }
        if let Some(ws) = WORKSPACE.get() {
            let _ = std::process::Command::new("podman")
                .args(["unshare", "rm", "-rf", ws])
                .status();
        }
    }
    unsafe {
        atexit(cleanup);
    }
}

// ── Fixture ───────────────────────────────────────────────────────────────

// Some test binaries don't read every field; keep them on the struct anyway.
#[allow(dead_code)]
pub struct Fixture {
    pub client: SiyuanClient,
    pub notebook_id: NotebookId,
    pub doc_id: BlockId,
    pub doc_hpath: String,
}

/// Create a fixture using the shared container: unique notebook, seeded doc,
/// and wait for the SQL index to converge.
pub async fn boot_with_seed() -> Result<Fixture> {
    let client = shared_client().await;
    let suffix = unique_suffix();
    let nb_name = format!("it-{suffix}");
    let nb = client.create_notebook(&nb_name).await?;
    let _ = client.open_notebook(&nb.id).await;

    let markdown = "\
# Integration Test Doc

## Goals

This is the first paragraph.

This paragraph references later content.

## Targets

A target paragraph.

- bullet one
- bullet two

## Empty Section
";
    let doc_hpath = format!("/Doc-{suffix}");
    let doc_id = client
        .create_doc_with_md(&nb.id, &doc_hpath, markdown)
        .await?;

    wait_for_doc_indexed(&client, &doc_id, 6).await?;

    Ok(Fixture {
        client,
        notebook_id: nb.id,
        doc_id,
        doc_hpath,
    })
}

/// Remove the fixture's notebook and wait for the kernel to confirm it is gone.
pub async fn cleanup_fixture(f: Fixture) -> Result<()> {
    f.client.remove_notebook(&f.notebook_id).await?;
    wait_for_notebook_cleaned_up(&f.client, &f.notebook_id).await
}

/// A short unique suffix so notebook/doc names don't collide across tests.
fn unique_suffix() -> String {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{ms:016x}")
}

// ── Generic poll primitive ────────────────────────────────────────────────

/// Poll `probe` every 100 ms until it returns `Ok(Some(value))` or `timeout`.
pub async fn wait_for<F, Fut, T>(mut probe: F, timeout: Duration) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Option<T>>>,
{
    let deadline = Instant::now() + timeout;
    loop {
        match probe().await? {
            Some(value) => return Ok(value),
            None => {
                if Instant::now() >= deadline {
                    bail!("wait_for: timed out after {:?}", timeout);
                }
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

// ── High-level wait helpers ───────────────────────────────────────────────

/// Wait until the SQL index has at least `min_blocks` blocks for `doc_id`.
pub async fn wait_for_doc_indexed(
    client: &SiyuanClient,
    doc_id: &BlockId,
    min_blocks: usize,
) -> Result<()> {
    wait_for(
        || async {
            let bundle = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await;
            match bundle {
                Ok(b) if b.page.total_blocks >= min_blocks => Ok(Some(())),
                Ok(_) => Ok(None),
                Err(e) => match e.downcast_ref::<SiyuanError>() {
                    Some(SiyuanError::NotFound(_)) => Ok(None),
                    _ => Err(e),
                },
            }
        },
        Duration::from_secs(5),
    )
    .await
}

/// Run a SQL query and wait until it returns non-empty results.
pub async fn wait_for_sql<T: for<'de> Deserialize<'de>>(
    client: &SiyuanClient,
    stmt: &str,
    timeout: Duration,
) -> Result<Vec<T>> {
    let stmt = stmt.to_string();
    wait_for(
        || async {
            let rows: Vec<T> = client.sql_typed(&stmt).await?;
            if rows.is_empty() {
                Ok(None)
            } else {
                Ok(Some(rows))
            }
        },
        timeout,
    )
    .await
}

/// Wait until a block whose markdown contains `needle` appears in the doc's
/// SQL index. Returns the full block list.
pub async fn wait_for_block_with_content(
    client: &SiyuanClient,
    doc_id: &BlockId,
    needle: &str,
    timeout: Duration,
) -> Result<Vec<siyuan_types::BlockNode>> {
    let needle = needle.to_string();
    let doc_id = doc_id.clone();
    wait_for(
        || async {
            let b = load_doc(
                client,
                &doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            if b.blocks.iter().any(|blk| blk.markdown.contains(&needle)) {
                Ok(Some(b.blocks))
            } else {
                Ok(None)
            }
        },
        timeout,
    )
    .await
}

/// Wait until a ref edge from `from_id` to `to_id` appears in the `refs` table.
pub async fn wait_for_ref(client: &SiyuanClient, from_id: &BlockId, to_id: &BlockId) {
    #[derive(Deserialize)]
    struct CountRow {
        n: i64,
    }
    let a = from_id.as_str();
    let b = to_id.as_str();
    wait_for(
        || async {
            let rows: Vec<CountRow> = client
                .sql_typed(&format!(
                    "SELECT COUNT(*) AS n FROM refs WHERE block_id = '{a}' AND def_block_id = '{b}'"
                ))
                .await?;
            let count = rows.first().map(|r| r.n).unwrap_or(0);
            if count >= 1 { Ok(Some(())) } else { Ok(None) }
        },
        Duration::from_secs(15),
    )
    .await
    .expect("timed out waiting for refs table edge");
}

/// Wait for a notebook to disappear from `ls_notebooks`.
pub async fn wait_for_notebook_cleaned_up(
    client: &SiyuanClient,
    notebook_id: &NotebookId,
) -> Result<()> {
    let nb_id = notebook_id.clone();
    wait_for(
        || async {
            let notebooks = client.ls_notebooks().await?;
            if notebooks.iter().any(|nb| nb.id == nb_id) {
                Ok(None)
            } else {
                Ok(Some(()))
            }
        },
        Duration::from_secs(5),
    )
    .await
}

// ── Unit tests for the helpers ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};

    #[tokio::test]
    async fn wait_for_resolves_immediately_on_first_some() {
        let result: Result<i32> = wait_for(|| async { Ok(Some(42)) }, Duration::from_secs(1)).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn wait_for_polls_until_some() {
        let call_count = Arc::new(StdMutex::new(0u32));
        let cc = Arc::clone(&call_count);

        let result: Result<&str> = wait_for(
            move || {
                let cc = Arc::clone(&cc);
                async move {
                    let mut n = cc.lock().unwrap();
                    *n += 1;
                    if *n >= 3 { Ok(Some("done")) } else { Ok(None) }
                }
            },
            Duration::from_secs(5),
        )
        .await;

        assert_eq!(result.unwrap(), "done");
        assert!(*call_count.lock().unwrap() >= 3);
    }

    #[tokio::test]
    async fn wait_for_bails_after_timeout() {
        let result: Result<i32> = wait_for(|| async { Ok(None) }, Duration::from_millis(150)).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("timed out"), "unexpected error: {msg}");
    }

    #[tokio::test]
    async fn wait_for_propagates_probe_error() {
        let result: Result<i32> = wait_for(
            || async { Err(anyhow::anyhow!("probe failed")) },
            Duration::from_secs(5),
        )
        .await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("probe failed"), "unexpected error: {msg}");
    }
}
