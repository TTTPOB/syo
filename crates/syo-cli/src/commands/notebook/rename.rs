use anyhow::{Context, Result};
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Notebook id or display name to rename.
    #[arg(long)]
    pub id: String,
    /// New display name.
    #[arg(long)]
    pub name: String,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let id = syo_core::notebook::resolve_notebook_id(client, &args.id)
        .await
        .context("--id")?;
    syo_core::notebook::rename(
        client,
        syo_core::notebook::RenameInput {
            id,
            name: args.name,
        },
    )
    .await?;
    println!("ok");
    Ok(())
}
