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
    let result = syo_core::asset::upload(
        client,
        syo_core::asset::UploadInput {
            file_path: args.file.to_string_lossy().to_string(),
        },
    )
    .await?;
    println!("{}", result.asset_path);
    Ok(())
}
