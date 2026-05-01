use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;

#[derive(Subcommand, Debug)]
pub enum AssetCmd {
    Upload(UploadArgs),
    /// Print the markdown snippet for embedding an already-uploaded asset.
    Reference(ReferenceArgs),
}

#[derive(Args, Debug)]
pub struct UploadArgs {
    /// Local file to upload.
    #[arg(long)]
    pub file: PathBuf,
}

#[derive(Args, Debug)]
pub struct ReferenceArgs {
    /// Kernel-relative asset path (the value returned by `asset upload`).
    #[arg(long)]
    pub path: String,

    /// Alt text. For images, defaults to the file basename.
    #[arg(long, default_value = "")]
    pub alt: String,
}

pub async fn run(client: &SiyuanClient, cmd: AssetCmd) -> Result<()> {
    match cmd {
        AssetCmd::Upload(a) => {
            let path = client.upload_asset(&a.file).await?;
            println!("{path}");
        }
        AssetCmd::Reference(a) => {
            let alt = if a.alt.is_empty() {
                a.path.rsplit('/').next().unwrap_or("").to_string()
            } else {
                a.alt
            };
            // Image-style markdown reference.
            println!("![{alt}]({})", a.path);
        }
    }
    Ok(())
}
