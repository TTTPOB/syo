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
    let nb =
        syo_core::notebook::create(client, syo_core::notebook::CreateInput { name: args.name })
            .await?
            .notebook;
    println!("{}", nb.id);
    Ok(())
}
