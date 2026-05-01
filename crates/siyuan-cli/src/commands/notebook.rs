use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

#[derive(Subcommand, Debug)]
pub enum NotebookCmd {
    Ls,
    Open(IdArgs),
    Close(IdArgs),
    Create(NameArgs),
    Rename(RenameArgs),
    Remove(IdArgs),
}

#[derive(Args, Debug)]
pub struct IdArgs {
    #[arg(long)]
    pub id: String,
}

#[derive(Args, Debug)]
pub struct NameArgs {
    #[arg(long)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct RenameArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub name: String,
}

pub async fn run(client: &SiyuanClient, cmd: NotebookCmd) -> Result<()> {
    match cmd {
        NotebookCmd::Ls => {
            let nbs = client.ls_notebooks().await?;
            for nb in nbs {
                let status = if nb.closed { "closed" } else { "open  " };
                println!("{}\t{}\t{}", status, nb.id, nb.name);
            }
        }
        NotebookCmd::Open(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.open_notebook(&id).await?;
            println!("ok");
        }
        NotebookCmd::Close(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.close_notebook(&id).await?;
            println!("ok");
        }
        NotebookCmd::Create(a) => {
            let nb = client.create_notebook(&a.name).await?;
            println!("{}", nb.id);
        }
        NotebookCmd::Rename(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.rename_notebook(&id, &a.name).await?;
            println!("ok");
        }
        NotebookCmd::Remove(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.remove_notebook(&id).await?;
            println!("ok");
        }
    }
    Ok(())
}
