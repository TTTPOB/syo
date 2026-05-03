use anyhow::{Context, Result};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_model::{
    load::load_doc,
    pagination::{DEFAULT_PAGE_SIZE, PageRequest},
};
use siyuan_render::agent_md::render_doc;
use siyuan_render::json_bundle::render_bundle;
use siyuan_types::BlockId;

use crate::output::OutputFormat;

/// Fetch a document by id and render as agent-markdown or JSON.
///
/// Sibling commands: `syo block get` returns one block's raw kramdown;
/// `syo doc resolve` converts an hpath to an id (this command requires an
/// id, not an hpath). For full-text scans across many docs use
/// `syo search text` instead of paging through every document.
///
/// Inputs:
///   --id (required): document block id (14-digit timestamp + 7-char suffix,
///     e.g. `20260501090000-doc0001`). Must be the ROOT block of a document.
///   --page (default 1): 1-indexed page number in DFS document order.
///   --page-size (default 50, capped at 1000): blocks per page.
///   --format (default agent-md): one of `agent-md` (compact markdown with
///     `<!-- sy:* -->` HTML-comment block markers), `json` (raw structured
///     bundle), or `json-pretty` (the same bundle, indented).
///
/// When `total_pages > page` the JSON output wraps the bundle with a `_hint`
/// telling you to fetch the next page; agent-md emits the same hint as a
/// trailing comment.
///
/// Example:
///   in:  --id 20260501090000-doc0001 --format json
///   out: {"doc":{"id":"20260501090000-doc0001","hpath":"/Plan",...},"blocks":[...],"page":1,"total_pages":1}
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct GetDocArgs {
    /// Document block id (root block; not an hpath).
    #[arg(long)]
    pub id: String,

    /// 1-indexed page number; pages are sliced in DFS document order.
    #[arg(long, default_value_t = 1)]
    pub page: usize,

    /// Blocks per page. Default 50, capped at 1000 by the model layer.
    #[arg(long, default_value_t = DEFAULT_PAGE_SIZE)]
    pub page_size: usize,

    /// Output format: `agent-md` (default), `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: GetDocArgs) -> Result<()> {
    let id = BlockId::parse(args.id).context("--id is not a valid block id")?;
    let bundle = load_doc(
        client,
        &id,
        PageRequest {
            page: args.page,
            page_size: args.page_size,
        },
    )
    .await?;
    let s = match args.format {
        OutputFormat::AgentMd => render_doc(&bundle),
        OutputFormat::Json => render_bundle(&bundle, false)?,
        OutputFormat::JsonPretty => render_bundle(&bundle, true)?,
    };
    println!("{s}");
    Ok(())
}
