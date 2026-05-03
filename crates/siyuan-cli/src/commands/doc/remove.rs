use anyhow::Result;
use clap::{ArgGroup, Args};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::resolve_one_storage;

use super::lookup::build_single_doc_lookup;

/// Arguments for `siyuan doc remove`.
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
    let lookup = build_single_doc_lookup(
        args.id.as_deref(),
        args.notebook.as_deref(),
        args.hpath.as_deref(),
    )?;
    let (nb, storage_path) = resolve_one_storage(client, lookup).await?;
    client.remove_doc(&nb, &storage_path).await?;
    println!("ok");
    Ok(())
}
