use anyhow::{Context, Result, bail};
use clap::{ArgGroup, Args};

use siyuan_client::SiyuanClient;

use crate::output::OutputFormat;

use super::lookup::build_single_doc_lookup;

/// Arguments for `syo doc resolve`.
///
/// Note: the first `/`-delimited segment of an hpath is NOT a notebook
/// name — it is a top-level document title INSIDE the target notebook.
/// (SiYuan has no folder concept — every path segment is a document.)
/// The notebook is always supplied separately via `--notebook`.
/// Example: notebook `expnote`, hpath `/year2026/month12` means
/// `expnote:/year2026/month12`. Even when notebook `hello`, hpath
/// `/hello/world`, the first segment is still a document title:
/// `hello[notebook]:/hello/world`.
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

    /// Notebook id or display name (use together with --hpath to look up by human path).
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
    let notebook = match &args.notebook {
        Some(nb) => Some(
            syo_core::notebook::resolve_notebook_id(client, nb)
                .await
                .context("--notebook")?,
        ),
        None => None,
    };
    let lookup = build_single_doc_lookup(args.id.as_deref(), notebook, args.hpath.as_deref())?;
    let docs = syo_core::doc::resolve(client, lookup).await?.docs;
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
