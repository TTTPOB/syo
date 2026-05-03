use anyhow::{Context, Result};
use clap::{ArgGroup, Args};

use siyuan_client::SiyuanClient;

use super::lookup::build_single_doc_lookup;

/// Arguments for `syo doc remove`.
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
    ArgGroup::new("remove_lookup")
        .args(["id", "hpath"])
        .required(true)
))]
pub struct RemoveArgs {
    /// Document block id. Use to address by id directly.
    #[arg(long, conflicts_with_all = ["notebook", "hpath"])]
    pub id: Option<String>,

    /// Notebook id (use together with --hpath to address by human path).
    #[arg(long, requires = "hpath")]
    pub notebook: Option<String>,

    /// Human path inside the notebook, e.g. `/Projects/Plan`. NOT a `.sy`
    /// storage path.
    #[arg(long, requires = "notebook")]
    pub hpath: Option<String>,
}

pub async fn run(client: &SiyuanClient, args: RemoveArgs) -> Result<()> {
    let notebook = match &args.notebook {
        Some(nb) => Some(
            syo_core::notebook::resolve_notebook_id(client, nb)
                .await
                .context("--notebook")?,
        ),
        None => None,
    };
    let lookup = build_single_doc_lookup(args.id.as_deref(), notebook, args.hpath.as_deref())?;
    syo_core::doc::remove(client, syo_core::doc::RemoveDocInput { lookup }).await?;
    println!("ok");
    Ok(())
}
