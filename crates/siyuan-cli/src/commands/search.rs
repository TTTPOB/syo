use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use serde::Deserialize;

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_like};

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
    ///   --query (required): non-empty search string. LIKE meta-chars
    ///     (`%`, `_`, `\`) and single quotes in the input are escaped
    ///     internally before the SQL is constructed; pass plain text.
    ///     Whitespace-only inputs are rejected client-side.
    ///   --limit (optional, default 50): maximum hits, capped by
    ///     `MAX_SEARCH_LIMIT`.
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
    Text(TextArgs),
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
    ///     no content filter. LIKE meta-chars are escaped internally.
    ///   --limit (optional, default 50): maximum hits, capped by
    ///     `MAX_SEARCH_LIMIT`.
    ///
    /// Output is one hit per line: `<id>\t<type>\t<markdown-preview>`.
    /// SQL index lag (~100-500 ms) applies.
    ///
    /// Example:
    ///   in:  --type h --contains Plan --limit 5
    ///   out: 20260501090000-blk0001    h    # Plan
    #[command(verbatim_doc_comment)]
    Blocks(BlocksArgs),
}

#[derive(Args, Debug)]
pub struct TextArgs {
    /// Substring to search for. Plain text; meta-chars escaped internally.
    #[arg(long)]
    pub query: String,

    /// Maximum hits returned. Default 50, capped by `MAX_SEARCH_LIMIT`.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct BlocksArgs {
    /// Block type letter (e.g. `h`, `p`, `c`). Empty disables the filter.
    #[arg(long, default_value = "")]
    pub r#type: String,

    /// Substring to match against block content. Empty disables the filter.
    #[arg(long, default_value = "")]
    pub contains: String,

    /// Maximum hits returned. Default 50, capped by `MAX_SEARCH_LIMIT`.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Debug, Deserialize)]
struct Hit {
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    markdown: String,
}

pub async fn run(client: &SiyuanClient, cmd: SearchCmd) -> Result<()> {
    // Single source of truth for the LIMIT cap; usize-typed for CLI flag width.
    let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
    match cmd {
        SearchCmd::Text(a) => {
            // Reject blank/whitespace-only queries: an empty LIKE pattern
            // matches every block and the result is always silently trimmed
            // by `--limit`, hiding the misuse from the user.
            if a.query.trim().is_empty() {
                bail!("--query must not be empty");
            }
            // Escape LIKE meta-characters and quotes, and use ESCAPE '\' so
            // the backslashes we inject neutralise %, _ and \ in user input.
            let needle = escape_sql_like(&a.query);
            let limit = a.limit.min(limit_cap);
            let stmt = format!(
                "SELECT id, type, markdown FROM blocks \
                 WHERE markdown LIKE '%{needle}%' ESCAPE '\\' LIMIT {limit}"
            );
            let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
            for r in rows {
                println!("{}\t{}\t{}", r.id, r.block_type, oneline(&r.markdown));
            }
        }
        SearchCmd::Blocks(a) => {
            let mut conds = Vec::new();
            if !a.r#type.is_empty() {
                // type uses `=` (exact match), so only quote-escaping is needed;
                // LIKE meta-chars are not interpreted here.
                conds.push(format!("type = '{}'", a.r#type.replace('\'', "''")));
            }
            if !a.contains.is_empty() {
                // content uses LIKE, so apply the full meta-char escape and
                // pair it with ESCAPE '\' below.
                conds.push(format!(
                    "content LIKE '%{}%' ESCAPE '\\'",
                    escape_sql_like(&a.contains)
                ));
            }
            let where_clause = if conds.is_empty() {
                "1=1".into()
            } else {
                conds.join(" AND ")
            };
            let limit = a.limit.min(limit_cap);
            let stmt =
                format!("SELECT id, type, markdown FROM blocks WHERE {where_clause} LIMIT {limit}");
            let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
            for r in rows {
                println!("{}\t{}\t{}", r.id, r.block_type, oneline(&r.markdown));
            }
        }
    }
    Ok(())
}

fn oneline(s: &str) -> String {
    let one = s.replace('\n', " ");
    if one.chars().count() <= 80 {
        one
    } else {
        let truncated: String = one.chars().take(80).collect();
        format!("{truncated}\u{2026}")
    }
}
