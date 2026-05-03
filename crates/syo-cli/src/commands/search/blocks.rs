use anyhow::Result;
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;

use crate::output::OutputFormat;

use super::hit::{Hit, emit_hits};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Block type letter (e.g. `h`, `p`, `c`). Empty disables the filter.
    #[arg(long, default_value = "")]
    pub r#type: String,

    /// Substring to match against block content. Empty disables the filter.
    #[arg(long, default_value = "")]
    pub contains: String,

    /// Maximum hits returned. Default 50, capped by `MAX_SEARCH_LIMIT`.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Output format: `agent-md` (default; TSV `id\ttype\tmarkdown_preview`),
    /// `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let result = syo_core::search::blocks(
        client,
        syo_core::search::BlocksInput {
            block_type: args.r#type,
            contains: args.contains,
            limit: args.limit,
        },
    )
    .await?;
    let hits: Vec<Hit> = result
        .hits
        .into_iter()
        .map(|h| Hit {
            id: h.id,
            block_type: h.block_type,
            markdown: h.markdown,
        })
        .collect();
    emit_hits(hits, args.format)
}
