use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

#[derive(Args, Debug)]
pub struct CreateDocArgs {
    #[arg(long)]
    pub notebook: String,

    /// Human path, e.g. "/Projects/New Page".
    #[arg(long)]
    pub hpath: String,

    /// Path to a markdown file. Use `-` for stdin.
    #[arg(long)]
    pub markdown_file: String,
}

pub async fn run(client: &SiyuanClient, args: CreateDocArgs) -> Result<()> {
    let notebook = NotebookId::parse(&args.notebook).context("--notebook")?;
    let markdown = super::read_markdown_input(&args.markdown_file)?;
    let id = client
        .create_doc_with_md(&notebook, &args.hpath, &markdown)
        .await?;
    println!("{id}");
    Ok(())
}
