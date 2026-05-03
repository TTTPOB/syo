use anyhow::{Context, Result};
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Notebook id to rename.
    #[arg(long)]
    pub id: String,
    /// New display name.
    #[arg(long)]
    pub name: String,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let id = NotebookId::parse(&args.id).context("--id")?;
    client.rename_notebook(&id, &args.name).await?;
    println!("ok");
    Ok(())
}
