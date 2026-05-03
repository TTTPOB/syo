use anyhow::{Result, bail};
use clap::{ArgGroup, Args};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::resolve as resolve_doc_meta;

use crate::output::OutputFormat;

use super::lookup::build_single_doc_lookup;

/// Arguments for `syo doc resolve`.
#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("doc_lookup")
        .args(["id", "hpath"])
        .required(true)
))]
pub struct ResolveArgs {
    /// Document block id. Use to recover hpath/notebook from a known id.
    #[arg(long, conflicts_with_all = ["notebook", "hpath"])]
    pub id: Option<String>,

    /// Notebook id (use together with --hpath to look up by human path).
    #[arg(long, requires = "hpath")]
    pub notebook: Option<String>,

    /// Human path inside the notebook, e.g. `/Projects/Plan`.
    #[arg(long, requires = "notebook")]
    pub hpath: Option<String>,

    /// Output format: `json-pretty` (default), or `json` (compact).
    /// `agent-md` is not supported for resolve — the output is structured
    /// metadata, not prose.
    #[arg(long, value_enum, default_value_t = OutputFormat::JsonPretty)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: ResolveArgs) -> Result<()> {
    let lookup = build_single_doc_lookup(
        args.id.as_deref(),
        args.notebook.as_deref(),
        args.hpath.as_deref(),
    )?;
    let docs = resolve_doc_meta(client, lookup).await?;
    let s = match args.format {
        OutputFormat::AgentMd => {
            bail!("doc resolve does not support --format agent-md; use json or json-pretty");
        }
        OutputFormat::Json => serde_json::to_string(&docs)?,
        OutputFormat::JsonPretty => serde_json::to_string_pretty(&docs)?,
    };
    println!("{s}");
    Ok(())
}
