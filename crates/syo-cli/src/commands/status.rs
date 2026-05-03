use anyhow::Result;
use tracing::info;

use siyuan_client::SiyuanClient;

pub async fn run(client: &SiyuanClient) -> Result<()> {
    let v = syo_core::system::status(client).await?.version;
    info!(%v, "siyuan ok");
    println!("{v}");
    Ok(())
}
