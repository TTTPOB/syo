use anyhow::{Result, bail};
use clap::Args as ClapArgs;

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_string};
use siyuan_model::sql_guard;

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
    let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
    // Reject blank/whitespace-only queries: an empty LIKE pattern matches
    // every block and the result is always silently trimmed by `--limit`,
    // hiding the misuse from the user.
    if args.query.trim().is_empty() {
        bail!("--query must not be empty");
    }
    // Escape single quotes for SQL string-literal safety. The SiYuan kernel's
    // SQL engine does not support ESCAPE '\' in LIKE patterns, so % and _ in
    // user input behave as LIKE wildcards.
    let needle = escape_sql_string(&args.query);
    let limit = args.limit.min(limit_cap);
    let stmt = format!(
        "SELECT id, type, markdown FROM blocks \
         WHERE markdown LIKE '%{needle}%' LIMIT {limit}"
    );
    // Defense-in-depth: validate that the assembled SQL is read-only before
    // sending to the kernel. User input is already escaped, but the AST guard
    // catches any escaping bug or future regression.
    if let Err(e) = sql_guard::validate_read_only(&stmt) {
        bail!("{e}");
    }
    let rows: Vec<Hit> = client.sql_typed(&stmt).await?;
    emit_hits(rows, args.format)
}
