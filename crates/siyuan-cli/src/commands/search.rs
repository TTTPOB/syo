use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_string};
use siyuan_model::sql_guard;

use crate::output::OutputFormat;

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
    Blocks(BlocksArgs),
}

#[derive(Args, Debug)]
pub struct TextArgs {
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

    /// Output format: `agent-md` (default; TSV `id\ttype\tmarkdown_preview`),
    /// `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

#[derive(Debug, Deserialize)]
struct Hit {
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    markdown: String,
}

/// Serializable view of a search hit for `--format json`.
///
/// Field is named `markdown_preview` (not `markdown`) because the value is
/// passed through `oneline` — newlines are folded and the string is
/// truncated to 80 chars with a horizontal-ellipsis marker, so it is no
/// longer the verbatim markdown column.
#[derive(Debug, Serialize)]
struct HitView {
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    markdown_preview: String,
}

fn emit_hits(rows: Vec<Hit>, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::AgentMd => {
            for r in rows {
                println!("{}\t{}\t{}", r.id, r.block_type, oneline(&r.markdown));
            }
        }
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let views: Vec<HitView> = rows
                .into_iter()
                .map(|r| HitView {
                    id: r.id,
                    block_type: r.block_type,
                    markdown_preview: oneline(&r.markdown),
                })
                .collect();
            let s = if format == OutputFormat::JsonPretty {
                serde_json::to_string_pretty(&views)?
            } else {
                serde_json::to_string(&views)?
            };
            println!("{s}");
        }
    }
    Ok(())
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
            // Escape single quotes for SQL string-literal safety. The SiYuan
            // kernel's SQL engine does not support ESCAPE '\' in LIKE
            // patterns, so % and _ in user input behave as LIKE wildcards.
            let needle = escape_sql_string(&a.query);
            let limit = a.limit.min(limit_cap);
            let stmt = format!(
                "SELECT id, type, markdown FROM blocks \
                 WHERE markdown LIKE '%{needle}%' LIMIT {limit}"
            );
            // Defense-in-depth: validate that the assembled SQL is read-only
            // before sending to the kernel. User input is already escaped, but
            // the AST guard catches any escaping bug or future regression.
            if let Err(e) = sql_guard::validate_read_only(&stmt) {
                bail!("{e}");
            }
            let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
            emit_hits(rows, a.format)?;
        }
        SearchCmd::Blocks(a) => {
            let mut conds = Vec::new();
            if !a.r#type.is_empty() {
                // type uses `=` (exact match), so only quote-escaping is needed;
                // LIKE meta-chars are not interpreted here.
                conds.push(format!("type = '{}'", a.r#type.replace('\'', "''")));
            }
            if !a.contains.is_empty() {
                // content uses LIKE; only single-quote escaping is effective
                // since the SiYuan SQL engine does not support ESCAPE '\'.
                conds.push(format!(
                    "content LIKE '%{}%'",
                    escape_sql_string(&a.contains)
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
            // Defense-in-depth: validate that the assembled SQL is read-only
            // before sending to the kernel. User input is already escaped, but
            // the AST guard catches any escaping bug or future regression.
            if let Err(e) = sql_guard::validate_read_only(&stmt) {
                bail!("{e}");
            }
            let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
            emit_hits(rows, a.format)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirror of `HitView` with `Deserialize` so the round-trip test can
    /// parse the JSON we emit. Production `HitView` is `Serialize`-only —
    /// JSON is an output format, not an input format for the CLI.
    #[derive(Debug, Deserialize, PartialEq)]
    struct HitViewOwned {
        id: String,
        #[serde(rename = "type")]
        block_type: String,
        markdown_preview: String,
    }

    #[test]
    fn hit_view_serializes_with_renamed_type_field() {
        let view = HitView {
            id: "20260501090000-blk0001".to_string(),
            block_type: "p".to_string(),
            markdown_preview: "Plan kickoff for Q3".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        // `block_type` must surface as `"type"` in JSON to match the SQL
        // column name and the TSV column name.
        assert!(json.contains("\"type\":\"p\""), "got {json}");
        assert!(json.contains("\"markdown_preview\""), "got {json}");
    }

    #[test]
    fn hit_view_round_trips_through_json() {
        let view = HitView {
            id: "20260501090000-blk0001".to_string(),
            block_type: "h".to_string(),
            markdown_preview: "# Plan".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        let parsed: HitViewOwned = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed,
            HitViewOwned {
                id: "20260501090000-blk0001".to_string(),
                block_type: "h".to_string(),
                markdown_preview: "# Plan".to_string(),
            }
        );
    }
}
