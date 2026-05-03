use anyhow::{Context, Result, anyhow};

use siyuan_model::doc_meta::DocLookup;
use siyuan_types::{BlockId, NotebookId};

/// Build a single-document `DocLookup` from clap-parsed pieces.
///
/// Clap's `ArgGroup` already filters out partial / conflicting input, but
/// this helper is the canonical CLI-side validator so programmatic callers
/// get the same error shape as the command line.
pub(super) fn build_single_doc_lookup(
    id: Option<&str>,
    notebook: Option<&str>,
    hpath: Option<&str>,
) -> Result<DocLookup> {
    match (id, notebook, hpath) {
        (Some(id), None, None) => Ok(DocLookup::ById(BlockId::parse(id.trim()).context("--id")?)),
        (None, Some(nb), Some(hp)) => Ok(DocLookup::ByHpath {
            notebook: NotebookId::parse(nb.trim()).context("--notebook")?,
            hpath: hp.to_string(),
        }),
        (Some(_), _, _) => Err(anyhow!(
            "--id conflicts with --notebook/--hpath; pick exactly one input mode"
        )),
        _ => Err(anyhow!(
            "provide either --id, or both --notebook and --hpath"
        )),
    }
}
