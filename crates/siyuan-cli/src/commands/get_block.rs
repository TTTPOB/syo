use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

use crate::output::OutputFormat;

#[derive(Args, Debug)]
pub struct GetBlockArgs {
    #[arg(long)]
    pub id: String,

    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

#[derive(Debug, Serialize)]
struct BlockView {
    id: String,
    kramdown: String,
    attrs: std::collections::BTreeMap<String, String>,
}

pub async fn run(client: &SiyuanClient, args: GetBlockArgs) -> Result<()> {
    let id = BlockId::parse(args.id).context("--id is not a valid block id")?;
    let kr = client.get_block_kramdown(&id).await?;
    let attrs = client.get_block_attrs(&id).await.unwrap_or_default();

    let view = BlockView {
        id: kr.id.to_string(),
        kramdown: kr.kramdown,
        attrs,
    };
    let s = match args.format {
        OutputFormat::AgentMd => format!("<!-- sy:block id={} -->\n{}", view.id, view.kramdown),
        OutputFormat::Json => serde_json::to_string(&view)?,
        OutputFormat::JsonPretty => serde_json::to_string_pretty(&view)?,
    };
    println!("{s}");
    Ok(())
}
