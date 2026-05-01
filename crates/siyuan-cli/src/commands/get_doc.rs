use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::{
    load::load_doc,
    pagination::{DEFAULT_PAGE_SIZE, PageRequest},
};
use siyuan_render::agent_md::render_doc;
use siyuan_render::json_bundle::render_bundle;
use siyuan_types::BlockId;

use crate::output::OutputFormat;

#[derive(Args, Debug)]
pub struct GetDocArgs {
    /// Document block id.
    #[arg(long)]
    pub id: String,

    #[arg(long, default_value_t = 1)]
    pub page: usize,

    #[arg(long, default_value_t = DEFAULT_PAGE_SIZE)]
    pub page_size: usize,

    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: GetDocArgs) -> Result<()> {
    let id = BlockId::parse(args.id).context("--id is not a valid block id")?;
    let bundle = load_doc(
        client,
        &id,
        PageRequest {
            page: args.page,
            page_size: args.page_size,
        },
    )
    .await?;
    let s = match args.format {
        OutputFormat::AgentMd => render_doc(&bundle),
        OutputFormat::Json => render_bundle(&bundle, false)?,
        OutputFormat::JsonPretty => render_bundle(&bundle, true)?,
    };
    println!("{s}");
    Ok(())
}
