use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgGroup, Args};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::DocLookup;
use siyuan_types::{BlockId, NotebookId};

/// Arguments for `syo doc move`.
///
/// Source addressing has two mutually exclusive modes:
/// - `--from-ids` (one-or-more): each source is addressed by its block id.
/// - `--notebook` + `--from-hpaths` (one-or-more): each source is addressed
///   by its human path inside the SOURCE notebook.
#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("move_source")
        .args(["from_ids", "from_hpaths"])
        .required(true)
))]
pub struct MoveArgs {
    /// One or more source documents addressed by block id.
    #[arg(
        long,
        num_args = 1..,
        value_name = "BLOCK_ID",
        conflicts_with_all = ["notebook", "from_hpaths"],
    )]
    pub from_ids: Vec<String>,

    /// SOURCE notebook id (used only with --from-hpaths). Distinct from
    /// --to-notebook (the destination).
    #[arg(long, requires = "from_hpaths")]
    pub notebook: Option<String>,

    /// One or more source documents addressed by human path inside
    /// `--notebook`. NOT `.sy` storage paths.
    #[arg(
        long,
        num_args = 1..,
        value_name = "HPATH",
        requires = "notebook",
    )]
    pub from_hpaths: Vec<String>,

    /// Destination notebook id.
    #[arg(long)]
    pub to_notebook: String,

    /// Destination FOLDER as an hpath (e.g. `/Projects` or `/`). For
    /// folders the hpath and storage path coincide because folders carry
    /// no `.sy` suffix.
    #[arg(long)]
    pub to_path: String,
}

pub async fn run(client: &SiyuanClient, args: MoveArgs) -> Result<()> {
    let to_nb = NotebookId::parse(&args.to_notebook).context("--to-notebook")?;
    validate_target_parent_exists(client, &to_nb, &args.to_path).await?;

    let source_lookups =
        build_move_source_lookups(&args.from_ids, args.notebook.as_deref(), &args.from_hpaths)?;

    syo_core::doc::move_docs(
        client,
        syo_core::doc::MoveDocsInput {
            from: source_lookups,
            to_notebook: to_nb,
            to_path: args.to_path,
        },
    )
    .await?;
    println!("ok");
    Ok(())
}

/// Build a vector of source-document `DocLookup`s for `doc move`.
fn build_move_source_lookups(
    from_ids: &[String],
    notebook: Option<&str>,
    from_hpaths: &[String],
) -> Result<Vec<DocLookup>> {
    let id_mode = !from_ids.is_empty();
    let hpath_mode = !from_hpaths.is_empty();

    if id_mode && (hpath_mode || notebook.is_some()) {
        bail!("--from-ids conflicts with --notebook/--from-hpaths; pick exactly one source mode");
    }
    if !id_mode && !hpath_mode {
        bail!("provide either --from-ids, or both --notebook and --from-hpaths");
    }

    if id_mode {
        let mut lookups = Vec::with_capacity(from_ids.len());
        for raw in from_ids {
            let id = BlockId::parse(raw.trim()).context("--from-ids")?;
            lookups.push(DocLookup::ById(id));
        }
        return Ok(lookups);
    }

    // Hpath batch mode: --notebook is the SOURCE notebook for ALL hpaths in
    // this batch. The kernel's `getIDsByHpath` is per-notebook, so a
    // multi-source-notebook batch would need multiple resolves — we keep the
    // surface simple by requiring a single source notebook per invocation.
    let nb =
        notebook.ok_or_else(|| anyhow!("--notebook is required when --from-hpaths is supplied"))?;
    let nb = NotebookId::parse(nb.trim()).context("--notebook")?;
    let mut lookups = Vec::with_capacity(from_hpaths.len());
    for hp in from_hpaths {
        lookups.push(DocLookup::ByHpath {
            notebook: nb.clone(),
            hpath: hp.clone(),
        });
    }
    Ok(lookups)
}

/// Validate that the target parent folder exists in the destination notebook
/// before attempting a doc move. The kernel's `moveDocs` returns a cryptic
/// "not found" error when the target folder is missing; this check produces a
/// clear, actionable error message instead.
async fn validate_target_parent_exists(
    client: &SiyuanClient,
    notebook: &NotebookId,
    to_path: &str,
) -> Result<()> {
    // Get parent path. If to_path is "/Foo/Bar", parent is "/Foo".
    // If parent is "/" or to_path is "/Something" (depth 1), parent is "/",
    // which always exists.
    let parent = match to_path.rfind('/') {
        Some(idx) if idx > 0 => &to_path[..idx],
        _ => return Ok(()),
    };
    if parent.is_empty() || parent == "/" {
        return Ok(());
    }

    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct R {
        id: String,
    }
    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT id FROM blocks WHERE box = '{}' AND type = 'd' AND (hpath = '{}' OR hpath LIKE '{}/%') LIMIT 1",
            notebook.as_str(),
            parent,
            parent
        ))
        .await?;

    if rows.is_empty() {
        bail!(
            "target parent folder \"{}\" does not exist in notebook {}. \
             create-doc auto-creates intermediate folders, but doc move requires \
             the target folder to exist first.",
            parent,
            notebook.as_str()
        );
    }
    Ok(())
}
