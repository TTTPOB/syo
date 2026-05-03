use anyhow::Result;
use siyuan_client::SiyuanClient;

pub struct StatusOutput {
    pub version: String,
}

pub async fn status(client: &SiyuanClient) -> Result<StatusOutput> {
    let version = client.system_version().await?;
    Ok(StatusOutput { version })
}
