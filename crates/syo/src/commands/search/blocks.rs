use anyhow::{Result, bail};
use clap::Args as ClapArgs;

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_string};
use siyuan_model::sql_guard;

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
    let mut conds = Vec::new();
    if !args.r#type.is_empty() {
        // type uses `=` (exact match), so only quote-escaping is needed;
        // LIKE meta-chars are not interpreted here.
        conds.push(format!("type = '{}'", args.r#type.replace('\'', "''")));
    }
    if !args.contains.is_empty() {
        // content uses LIKE; only single-quote escaping is effective since
        // the SiYuan SQL engine does not support ESCAPE '\'.
        conds.push(format!(
            "content LIKE '%{}%'",
            escape_sql_string(&args.contains)
        ));
    }
    let where_clause = if conds.is_empty() {
        "1=1".into()
    } else {
        conds.join(" AND ")
    };
    let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
    let limit = args.limit.min(limit_cap);
    let stmt = format!("SELECT id, type, markdown FROM blocks WHERE {where_clause} LIMIT {limit}");
    // Defense-in-depth: validate that the assembled SQL is read-only before
    // sending to the kernel. User input is already escaped, but the AST guard
    // catches any escaping bug or future regression.
    if let Err(e) = sql_guard::validate_read_only(&stmt) {
        bail!("{e}");
    }
    let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
    emit_hits(rows, args.format)
}
