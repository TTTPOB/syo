use anyhow::Result;
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;

use crate::output::OutputFormat;

use super::hit::{Hit, emit_hits};

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Substring to search for. Single quotes are escaped internally;
    /// LIKE meta-chars are NOT escaped — they behave as wildcards.
    #[arg(long)]
    pub query: String,

    /// Maximum hits returned. Default 50, capped by `MAX_SEARCH_LIMIT`.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Output format: `agent-md` (default; TSV `id\ttype\tmarkdown_preview`),
    /// `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let result = syo_core::search::fulltext(
        client,
        syo_core::search::FulltextInput {
            query: args.query,
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
