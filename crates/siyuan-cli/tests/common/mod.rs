//! Shared scaffolding for cli integration tests.

use anyhow::Result;

use siyuan_client::SiyuanClient;
use siyuan_testkit::SiyuanContainer;
use siyuan_types::{BlockId, NotebookId};

pub struct Fixture {
    pub container: SiyuanContainer,
    pub client: SiyuanClient,
    pub notebook_id: NotebookId,
    pub doc_id: BlockId,
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

    Ok(Fixture { container, client, notebook_id: nb.id, doc_id })
}
