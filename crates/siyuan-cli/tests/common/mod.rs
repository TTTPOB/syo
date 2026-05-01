//! Shared scaffolding for cli integration tests.

use std::time::Duration;

use anyhow::{Result, bail};
use tokio::time::{Instant, sleep};

use siyuan_client::SiyuanClient;
use siyuan_model::load::load_doc;
use siyuan_model::pagination::PageRequest;
use siyuan_testkit::SiyuanContainer;
use siyuan_types::{BlockId, NotebookId};

pub struct Fixture {
    // Held to keep the podman container alive until Drop.
    #[allow(dead_code)]
    pub container: SiyuanContainer,
    pub client: SiyuanClient,
    pub notebook_id: NotebookId,
    pub doc_id: BlockId,
}

/// Poll `probe` every 100 ms until it returns `Ok(Some(value))` or `timeout` elapses.
/// Errors from `probe` propagate immediately without retrying.
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

/// Wait until SiYuan's SQL index has at least `min_blocks` blocks for `doc_id`.
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
                // Not enough blocks yet — keep polling.
                Ok(_) => Ok(None),
                // load_doc bails when the doc has no blocks at all; treat as not-ready.
                Err(_) => Ok(None),
            }
        },
        Duration::from_secs(5),
    )
    .await
}

pub async fn boot_with_seed() -> Result<Fixture> {
    siyuan_testkit::init_tracing();
    let container = SiyuanContainer::start().await?;
    let client = SiyuanClient::new(container.base_url(), container.token())?;

    let nb = client.create_notebook("integration-test").await?;
    // Newly created notebook is closed by default in some versions; open it.
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
    let doc_id = client
        .create_doc_with_md(&nb.id, "/IntegrationTestDoc", markdown)
        .await?;

    wait_for_doc_indexed(&client, &doc_id, 6).await?;

    Ok(Fixture {
        container,
        client,
        notebook_id: nb.id,
        doc_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn wait_for_resolves_immediately_on_first_some() {
        let result: Result<i32> = wait_for(|| async { Ok(Some(42)) }, Duration::from_secs(1)).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn wait_for_polls_until_some() {
        let call_count = Arc::new(Mutex::new(0u32));
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
