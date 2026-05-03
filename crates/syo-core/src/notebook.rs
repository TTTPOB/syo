use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_client::api::notebook::Notebook;
use siyuan_types::NotebookId;

// --- ls ---
pub struct LsOutput {
    pub notebooks: Vec<Notebook>,
}

pub async fn ls(client: &SiyuanClient) -> Result<LsOutput> {
    let notebooks = client.ls_notebooks().await?;
    Ok(LsOutput { notebooks })
}

// --- create ---
pub struct CreateInput {
    pub name: String,
}

pub struct CreateOutput {
    pub notebook: Notebook,
}

pub async fn create(client: &SiyuanClient, input: CreateInput) -> Result<CreateOutput> {
    let notebook = client.create_notebook(&input.name).await?;
    Ok(CreateOutput { notebook })
}

// --- rename ---
pub struct RenameInput {
    pub id: NotebookId,
    pub name: String,
}

pub async fn rename(client: &SiyuanClient, input: RenameInput) -> Result<()> {
    client.rename_notebook(&input.id, &input.name).await?;
    Ok(())
}

// --- remove ---
pub struct RemoveInput {
    pub id: NotebookId,
}

pub async fn remove(client: &SiyuanClient, input: RemoveInput) -> Result<()> {
    client.remove_notebook(&input.id).await?;
    Ok(())
}
