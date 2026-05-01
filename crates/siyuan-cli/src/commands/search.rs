use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Deserialize;

use siyuan_client::SiyuanClient;

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
    match cmd {
        SearchCmd::Text(a) => {
            let needle = a.query.replace('\'', "''");
            let stmt = format!(
                "SELECT id, type, markdown FROM blocks WHERE markdown LIKE '%{needle}%' LIMIT {}",
                a.limit
            );
            let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
            for r in rows {
                println!("{}\t{}\t{}", r.id, r.block_type, oneline(&r.markdown));
            }
        }
        SearchCmd::Blocks(a) => {
            let mut conds = Vec::new();
            if !a.r#type.is_empty() {
                conds.push(format!("type = '{}'", a.r#type.replace('\'', "''")));
            }
            if !a.contains.is_empty() {
                conds.push(format!(
                    "content LIKE '%{}%'",
                    a.contains.replace('\'', "''")
                ));
            }
            let where_clause = if conds.is_empty() {
                "1=1".into()
            } else {
                conds.join(" AND ")
            };
            let stmt = format!(
                "SELECT id, type, markdown FROM blocks WHERE {where_clause} LIMIT {}",
                a.limit
            );
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
