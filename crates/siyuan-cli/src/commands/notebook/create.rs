use anyhow::Result;
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Display name for the new notebook.
    #[arg(long)]
    pub name: String,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let nb = client.create_notebook(&args.name).await?;
    println!("{}", nb.id);
    Ok(())
}
