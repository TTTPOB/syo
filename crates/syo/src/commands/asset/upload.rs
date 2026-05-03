use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use siyuan_client::SiyuanClient;

#[derive(Args, Debug)]
pub struct UploadArgs {
    /// Local file to upload.
    #[arg(long)]
    pub file: PathBuf,
}

pub async fn run(client: &SiyuanClient, args: UploadArgs) -> Result<()> {
    let path = client.upload_asset(&args.file).await?;
    println!("{path}");
    Ok(())
}
