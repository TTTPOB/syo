use anyhow::Result;
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::tag::{list_tags, search_by_tag};

#[derive(Subcommand, Debug)]
pub enum TagCmd {
    Ls,
    Search(SearchArgs),
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    #[arg(long)]
    pub tag: String,
}

pub async fn run(client: &SiyuanClient, cmd: TagCmd) -> Result<()> {
    match cmd {
        TagCmd::Ls => {
            for t in list_tags(client).await? {
                println!("{t}");
            }
        }
        TagCmd::Search(a) => {
            for hit in search_by_tag(client, &a.tag).await? {
                println!("{}\t{}", hit.block_id, hit.markdown_preview);
            }
        }
    }
    Ok(())
}
