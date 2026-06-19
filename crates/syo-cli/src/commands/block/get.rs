use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use tracing::warn;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

use crate::output::OutputFormat;

/// Fetch the raw kramdown source of a single block plus its attributes.
///
/// Sibling commands: `syo doc get` returns the rendered document tree —
/// use this only when you need the storage syntax of ONE block (e.g. to
/// inspect attributes embedded in kramdown braces). `syo search text`
/// finds candidate ids when you do not have one yet.
///
/// Heading blocks are section owners, not real SiYuan containers. By default
/// this command returns only the heading block itself and annotates whether a
/// section was omitted. Pass `--include-heading-children` to include the full
/// heading section before editing or replacing that section.
///
/// Inputs:
///   --id (required): block id (14-digit timestamp + 7-char suffix). Any
///     block id is accepted — paragraph, heading, list item, document root,
///     etc. If the id does not exist, the kernel returns NotFound.
///   --format (default agent-md): one of `agent-md` (an HTML-comment header
///     plus the kramdown body), `json`, or `json-pretty`. JSON outputs an
///     object with `id`, `kramdown`, `attrs`, and heading `meta` when relevant.
///   --include-heading-children: only valid for heading blocks. Include the
///     recursively annotated heading section in `section_markdown`.
///
/// Example:
///   in:  --id 20260501090000-doc0001 --format json
///   out: {"id":"20260501090000-doc0001","kramdown":"# Heading\n\nBody\n","attrs":{"title":"Plan"}}
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct GetBlockArgs {
    /// Block id to fetch (any block type).
    #[arg(long)]
    pub id: String,

    /// Output format: `agent-md` (default), `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,

    /// Include the full heading section when --id is a heading block.
    #[arg(long)]
    pub include_heading_children: bool,
}

#[derive(Debug, Serialize)]
struct BlockView {
    id: String,
    kramdown: String,
    attrs: std::collections::BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<syo_core::block::HeadingSectionMeta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section_markdown: Option<String>,
}

pub async fn run(client: &SiyuanClient, args: GetBlockArgs) -> Result<()> {
    let id = BlockId::parse(args.id).context("--id is not a valid block id")?;
    let kr = syo_core::block::get(
        client,
        syo_core::block::GetBlockInput {
            id: id.clone(),
            include_heading_children: args.include_heading_children,
        },
    )
    .await?;
    let attrs = match client.get_block_attrs(&id).await {
        Ok(a) => a,
        Err(e) => {
            warn!(%id, %e, "failed to fetch block attrs, continuing with empty attrs");
            std::collections::BTreeMap::new()
        }
    };

    let view = BlockView {
        id: kr.id.to_string(),
        kramdown: kr.kramdown.clone(),
        attrs,
        meta: kr.meta.clone(),
        section_markdown: kr.section_markdown.clone(),
    };
    let s = match args.format {
        OutputFormat::AgentMd => syo_core::block::render_block_get_agent_md(&kr),
        OutputFormat::Json => serde_json::to_string(&view)?,
        OutputFormat::JsonPretty => serde_json::to_string_pretty(&view)?,
    };
    println!("{s}");
    Ok(())
}
