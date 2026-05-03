use anyhow::{Context, Result};
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Notebook id or display name (from `syo notebook ls`).
    #[arg(long)]
    pub id: String,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let id = syo_core::notebook::resolve_notebook_id(client, &args.id)
        .await
        .context("--id")?;
    syo_core::notebook::remove(client, syo_core::notebook::RemoveInput { id }).await?;
    println!("ok");
    Ok(())
}
