use anyhow::Result;
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;

use crate::output::OutputFormat;

mod hit;
use hit::{Hit, emit_hits};

/// Filter blocks by type and/or content substring.
///
/// Sibling commands: `syo tag search` is exact tag match; `syo sql` is
/// the raw escape hatch for arbitrary queries (joins, aggregates, LIKE on
/// markdown).
///
/// Inputs:
///   --type (optional): block type letter — common values:
///     `d` document, `h` heading, `p` paragraph, `l` list, `i` list
///     item, `c` code, `t` table, `b` blockquote, `m` math,
///     `s` super-block. Empty (default) means no type filter.
///   --contains (optional): substring matched against block `content`
///     (visible text, no markdown formatting). Empty (default) means
///     no content filter. LIKE meta-chars (`%`, `_`) are NOT
///     escaped — they behave as wildcards.
///   --limit (optional, default 50): maximum hits, capped by
///     `MAX_SEARCH_LIMIT`.
///   --format (default agent-md): one of `agent-md` (the TSV form
///     described above), `json` (compact array of
///     `{id, type, markdown_preview}`), or `json-pretty` (indented).
///
/// Output is one hit per line: `<id>\t<type>\t<markdown-preview>`.
/// SQL index lag (~100-500 ms) applies.
///
/// Example:
///   in:  --type h --contains Plan --limit 5
///   out: 20260501090000-blk0001    h    # Plan
#[derive(ClapArgs, Debug)]
#[command(verbatim_doc_comment)]
pub struct SearchArgs {
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

pub async fn run(client: &SiyuanClient, args: SearchArgs) -> Result<()> {
    let result = syo_core::search::search(
        client,
        syo_core::search::SearchInput {
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
