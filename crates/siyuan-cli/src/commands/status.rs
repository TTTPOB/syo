use anyhow::Result;
use tracing::info;

use siyuan_client::SiyuanClient;

pub async fn run(client: &SiyuanClient) -> Result<()> {
    let v = client.system_version().await?;
    info!(%v, "siyuan ok");
    println!("{v}");
    Ok(())
}
