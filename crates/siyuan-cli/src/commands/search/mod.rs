use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

pub mod blocks;
mod hit;
pub mod text;

#[derive(Subcommand, Debug)]
pub enum SearchCmd {
    /// Full-text search across all blocks (LIKE `%query%` on `markdown`).
    ///
    /// Sibling commands: `siyuan search blocks` filters by type+content
    /// (uses the `content` column instead of `markdown`); `siyuan tag
    /// search` is exact tag match; `siyuan sql` is the raw escape hatch
    /// for arbitrary queries.
    ///
    /// Inputs:
    ///   --query (required): non-empty search string. Single quotes are
    ///     escaped internally; LIKE meta-chars (`%`, `_`, `\\`) are NOT
    ///     escaped — they behave as wildcards. Pass plain text.
    ///     Whitespace-only inputs are rejected client-side.
    ///   --limit (optional, default 50): maximum hits, capped by
    ///     `MAX_SEARCH_LIMIT`.
    ///   --format (default agent-md): one of `agent-md` (the TSV form
    ///     described above), `json` (compact array of
    ///     `{id, type, markdown_preview}`), or `json-pretty` (indented).
    ///
    /// Output is one hit per line: `<id>\t<type>\t<markdown-preview>`
    /// (preview truncated to 80 chars on a single line). The SQL index
    /// lags writes by ~100-500 ms — freshly-written blocks may not show
    /// up immediately even though the kernel has them.
    ///
    /// Example:
    ///   in:  --query kickoff --limit 10
    ///   out: 20260501090000-blk0001    p    Plan kickoff for Q3
    #[command(verbatim_doc_comment)]
    Text(text::Args),
    /// Filter blocks by type and/or content substring.
    ///
    /// Sibling commands: `siyuan search text` searches the `markdown`
    /// column (includes inline syntax markers); this command searches
    /// `content` (visible text) and adds an exact-match `type` filter.
    /// Use `siyuan sql` for joins or projections this command does not
    /// expose.
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
    #[command(verbatim_doc_comment)]
    Blocks(blocks::Args),
}

pub async fn run(client: &SiyuanClient, cmd: SearchCmd) -> Result<()> {
    match cmd {
        SearchCmd::Text(a) => text::run(client, a).await,
        SearchCmd::Blocks(a) => blocks::run(client, a).await,
    }
}
