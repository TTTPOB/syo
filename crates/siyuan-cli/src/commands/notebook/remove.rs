use anyhow::{Context, Result};
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Notebook id (from `syo notebook ls`).
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let id = NotebookId::parse(&args.id).context("--id")?;
    client.remove_notebook(&id).await?;
    println!("ok");
    Ok(())
}
