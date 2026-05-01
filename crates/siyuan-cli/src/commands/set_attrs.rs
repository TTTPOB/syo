use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

#[derive(Args, Debug)]
pub struct SetAttrsArgs {
    #[arg(long)]
    pub id: String,

    /// Repeated `key=value` pairs. Custom attrs must be `custom-...`.
    #[arg(long = "attr", value_name = "KEY=VALUE")]
    pub attrs: Vec<String>,
}

pub async fn run(client: &SiyuanClient, args: SetAttrsArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for raw in &args.attrs {
        let (k, v) = raw
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("bad --attr {raw:?}; want KEY=VALUE"))?;
        if k.is_empty() {
            bail!("attr key may not be empty");
        }
        map.insert(k.into(), v.into());
    }
    client.set_block_attrs(&id, &map).await?;
    println!("ok");
    Ok(())
}
