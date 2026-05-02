use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Deserialize;

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_like};

#[derive(Subcommand, Debug)]
pub enum SearchCmd {
    Text(TextArgs),
    Blocks(BlocksArgs),
}

#[derive(Args, Debug)]
pub struct TextArgs {
    #[arg(long)]
    pub query: String,

    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct BlocksArgs {
    /// Block type letter (e.g. `h`, `p`, `c`).
    #[arg(long, default_value = "")]
    pub r#type: String,

    /// Substring to match against block content.
    #[arg(long, default_value = "")]
    pub contains: String,

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
