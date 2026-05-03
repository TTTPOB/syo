use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_client::api::notebook::Notebook;
use siyuan_types::{NotebookId, SiyuanError};

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

/// Resolve a user-supplied string to a [`NotebookId`].
///
/// If `input` matches the notebook-id format it is returned immediately —
/// no network call is made. Otherwise `ls_notebooks()` is called and the
/// input is matched by exact display name. Duplicate names are rejected
/// with a diagnostic listing all matching ids.
pub async fn resolve_notebook_id(
    client: &SiyuanClient,
    input: &str,
) -> std::result::Result<NotebookId, SiyuanError> {
    // If it parses as a valid notebook id, return it directly.
    if let Ok(id) = NotebookId::parse(input) {
        return Ok(id);
    }

    let notebooks = client.ls_notebooks().await?;
    let mut matches: Vec<&Notebook> = notebooks.iter().filter(|n| n.name == input).collect();

    match matches.len() {
        0 => Err(SiyuanError::NotebookNotFound {
            name: input.to_string(),
        }),
        1 => Ok(matches.pop().unwrap().id.clone()),
        _ => {
            let ids: Vec<String> = matches
                .iter()
                .map(|n| format!("{} ({})", n.id.as_str(), n.name))
                .collect();
            Err(SiyuanError::AmbiguousNotebook {
                name: input.to_string(),
                candidates: ids.join(", "),
            })
        }
    }
}
