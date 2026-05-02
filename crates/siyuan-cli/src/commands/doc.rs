use anyhow::{Context, Result, anyhow, bail};
use clap::{ArgGroup, Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::{DocLookup, resolve as resolve_doc_meta, resolve_one_storage};
use siyuan_types::{BlockId, NotebookId};

use crate::output::OutputFormat;

#[derive(Subcommand, Debug)]
pub enum DocCmd {
    /// Look up document metadata by id OR by (notebook + hpath).
    ///
    /// Sibling commands: `siyuan get-doc` returns the rendered document
    /// content (requires id); this command returns ONLY the metadata
    /// (id, hpath, notebook_id, notebook_name, title, storage_path) and
    /// is the canonical hpath<->id translator. `siyuan notebook ls`
    /// enumerates whole notebooks.
    ///
    /// Provide EXACTLY ONE input mode: either `--id` to recover the
    /// hpath/notebook from a known id (e.g. after a move or rename, or
    /// when only an id is in hand from SQL/search results), or
    /// `--notebook` plus `--hpath` together to look up by human path
    /// (when you only know the title/path).
    ///
    /// Output is a JSON array of matches (`docs`); an empty array means
    /// no such document — this is NOT an error. The kernel allows
    /// duplicate hpaths in rare edge cases, so a hpath lookup may return
    /// multiple entries. Each entry has six fields: `id`, `hpath`,
    /// `notebook_id`, `notebook_name`, `title`, and `storage_path`. The
    /// `storage_path` (`.sy`-suffixed) is an internal kernel detail
    /// surfaced here for diagnostics — `siyuan doc rename`, `siyuan doc
    /// move`, and `siyuan doc remove` accept the same id-or-hpath locator
    /// as this command and resolve the storage path internally.
    ///
    /// Inputs:
    ///   --format (default json-pretty): `json-pretty` (the indented form
    ///     shown above), or `json` (the same array, compact). `agent-md`
    ///     is rejected — this output is structured metadata, not prose.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001
    ///   out: [{"id":"20260501090000-doc0001","hpath":"/Plan","notebook_id":"20260501000000-nb00001","notebook_name":"Inbox","title":"Plan","storage_path":"/20260501090000-doc0001.sy"}]
    ///
    ///   in:  --notebook 20260501000000-nb00001 --hpath /Plan
    ///   out: [{"id":"20260501090000-doc0001","hpath":"/Plan","notebook_id":"20260501000000-nb00001","notebook_name":"Inbox","title":"Plan","storage_path":"/20260501090000-doc0001.sy"}]
    #[command(verbatim_doc_comment)]
    Resolve(ResolveArgs),
    /// Rename a document by changing its display title.
    ///
    /// Sibling commands: `siyuan doc move` changes the parent folder of a
    /// document; this changes only its title (the last hpath segment).
    /// `siyuan doc set-icon` sets the icon attribute alongside the title.
    /// Use `siyuan doc resolve` if you need to look up an id from an hpath
    /// before calling this command — but note that resolve is no longer
    /// REQUIRED: this command accepts the same dual-mode locator natively.
    ///
    /// Inputs: provide EXACTLY ONE locator mode plus `--title`.
    ///   --id <BLOCK_ID>: document root block id.
    ///   --notebook <NB_ID> --hpath <HPATH>: locate by human path.
    ///   --title (required): new human-readable display title.
    /// Storage `.sy` paths are NOT accepted as input — they are an internal
    /// implementation detail and the CLI resolves them for you.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001 --title "Q3 Plan"
    ///   out: ok
    ///
    ///   in:  --notebook 20260501000000-nb00001 --hpath /Plan --title "Q3 Plan"
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Rename(RenameArgs),
    /// Move one or more documents to a different notebook/folder.
    ///
    /// Sibling commands: `siyuan move-block` moves a block within the
    /// document tree (block-level); `siyuan doc rename` only retitles a
    /// document. doc move relocates whole `.sy` files in the file tree.
    ///
    /// Inputs: source addressing has TWO mutually exclusive modes; the
    /// destination is the same shape in both.
    ///   --from-ids <ID> [<ID> ...]: source documents addressed by id.
    ///   --notebook <SRC_NB> --from-hpaths <HPATH> [<HPATH> ...]: sources
    ///     addressed by human path inside SRC_NB. The two flags must be
    ///     supplied together. SRC_NB is the SOURCE notebook (distinct from
    ///     `--to-notebook`).
    ///   --to-notebook (required): DESTINATION notebook id.
    ///   --to-path (required): destination FOLDER as an hpath (e.g.
    ///     `/Projects` or `/`). For folders the hpath and storage path
    ///     coincide because folders have no `.sy` suffix. Each source's
    ///     own `.sy` filename is preserved at the target.
    /// Storage `.sy` paths are NOT accepted as source input — the CLI
    /// resolves them internally before calling the kernel.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --from-ids 20260501090000-doc0001 --to-notebook 20260501000000-nb00002 --to-path /
    ///   out: ok
    ///
    ///   in:  --notebook 20260501000000-nb00001 --from-hpaths /Plan /Notes \
    ///        --to-notebook 20260501000000-nb00002 --to-path /Archive
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Move(MoveArgs),
    /// Set the document's `icon` attribute (or clear it with empty value).
    ///
    /// Sibling commands: `siyuan set-attrs --attr icon=...` does the
    /// same thing for any block; this is just a convenience wrapper for
    /// document roots. Use `siyuan doc set-sort` to change ordering.
    ///
    /// Inputs:
    ///   --id (required): document root block id.
    ///   --icon (optional, default empty): icon name (e.g. emoji
    ///     shortcode `:rocket:`) or empty string to clear.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001 --icon :rocket:
    ///   out: ok
    #[command(verbatim_doc_comment)]
    SetIcon(IconArgs),
    /// Set the document's `sort` attribute (manual ordering hint).
    ///
    /// Sibling commands: `siyuan set-attrs --attr sort=N` is the generic
    /// equivalent. SiYuan uses `sort` as the sibling-ordering key when the
    /// notebook is configured for manual sort.
    ///
    /// Inputs:
    ///   --id (required): document root block id.
    ///   --sort (required): integer; lower values sort earlier.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001 --sort 100
    ///   out: ok
    #[command(verbatim_doc_comment)]
    SetSort(SortArgs),
    /// Permanently remove a document and all its child blocks.
    ///
    /// Sibling commands: `siyuan delete-block --id <doc-root-id>` is a
    /// block-level delete that also drops the document; `siyuan doc move`
    /// relocates instead of deleting; `siyuan notebook remove` destroys
    /// the entire notebook. doc remove is the per-document destroyer.
    ///
    /// Inputs: provide EXACTLY ONE locator mode.
    ///   --id <BLOCK_ID>: document root block id.
    ///   --notebook <NB_ID> --hpath <HPATH>: locate by human path.
    /// Storage `.sy` paths are NOT accepted — the CLI resolves them for you.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501090000-doc0001
    ///   out: ok
    ///
    ///   in:  --notebook 20260501000000-nb00001 --hpath /Plan
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Remove(RemoveArgs),
}

/// Arguments for `siyuan doc resolve`.
///
/// Mutual exclusion is enforced both by clap (via `ArgGroup` so partial
/// supply produces a friendly clap error) and again at runtime when the
/// `DocLookup` enum is constructed — the model layer is the canonical
/// validator and the CLI layer is the user-facing one.
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

/// Arguments for `siyuan doc rename`.
///
/// Mirrors `ResolveArgs`'s id-XOR-(notebook+hpath) shape — clap's `ArgGroup`
/// produces a friendly error on partial supply, and the runtime match arms
/// reconstruct the same invariant when building the `DocLookup` enum so the
/// model layer stays the canonical validator.
#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("rename_lookup")
        .args(["id", "hpath"])
        .required(true)
))]
pub struct RenameArgs {
    /// Document block id. Use to address by id directly.
    #[arg(long, conflicts_with_all = ["notebook", "hpath"])]
    pub id: Option<String>,

    /// Notebook id (use together with --hpath to address by human path).
    #[arg(long, requires = "hpath")]
    pub notebook: Option<String>,

    /// Human path inside the notebook, e.g. `/Projects/Plan`. NOT a `.sy`
    /// storage path — the CLI resolves the storage path for you.
    #[arg(long, requires = "notebook")]
    pub hpath: Option<String>,

    /// New display title.
    #[arg(long)]
    pub title: String,
}

/// Arguments for `siyuan doc move`.
///
/// Source addressing has two mutually exclusive modes:
/// - `--from-ids` (one-or-more): each source is addressed by its block id.
/// - `--notebook` + `--from-hpaths` (one-or-more): each source is addressed
///   by its human path inside the SOURCE notebook.
///
/// `--notebook` here names the SOURCE notebook (only used together with
/// `--from-hpaths`); the destination notebook is `--to-notebook`. Clap's
/// `requires` constraint links `--notebook`/`--from-hpaths` together, and the
/// `ArgGroup` ensures exactly one source-address mode is supplied.
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

#[derive(Args, Debug)]
pub struct IconArgs {
    /// Document block id.
    #[arg(long)]
    pub id: String,
    /// Icon name (e.g. emoji shortcode like ":rocket:") or empty to clear.
    #[arg(long, default_value = "")]
    pub icon: String,
}

#[derive(Args, Debug)]
pub struct SortArgs {
    /// Document root block id.
    #[arg(long)]
    pub id: String,
    /// Manual sort key (lower sorts earlier).
    #[arg(long)]
    pub sort: i64,
}

/// Arguments for `siyuan doc remove`.
///
/// Same id-XOR-(notebook+hpath) shape as `RenameArgs`. Storage `.sy` paths
/// are not accepted; the CLI resolves them internally.
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

pub async fn run(client: &SiyuanClient, cmd: DocCmd) -> Result<()> {
    match cmd {
        DocCmd::Resolve(a) => {
            // Same shape as the rename/remove locator. Clap's ArgGroup
            // already prevents most invalid combinations; the helper re-checks
            // here so the model layer remains the canonical gate — anything
            // that builds a DocLookup goes through the same door regardless
            // of caller.
            let lookup = build_single_doc_lookup(
                a.id.as_deref(),
                a.notebook.as_deref(),
                a.hpath.as_deref(),
            )?;
            let docs = resolve_doc_meta(client, lookup).await?;
            // `resolve` output is structured metadata; the `agent-md`
            // variant has no sensible mapping (we'd be inventing prose
            // around already-structured fields). Reject it loudly so the
            // user picks a JSON variant rather than getting a silent
            // pretty-printed default that masks the misuse.
            let s = match a.format {
                OutputFormat::AgentMd => {
                    bail!(
                        "doc resolve does not support --format agent-md; use json or json-pretty"
                    );
                }
                OutputFormat::Json => serde_json::to_string(&docs)?,
                OutputFormat::JsonPretty => serde_json::to_string_pretty(&docs)?,
            };
            println!("{s}");
        }
        DocCmd::Rename(a) => {
            let lookup = build_single_doc_lookup(
                a.id.as_deref(),
                a.notebook.as_deref(),
                a.hpath.as_deref(),
            )?;
            let (nb, storage_path) = resolve_one_storage(client, lookup).await?;
            client.rename_doc(&nb, &storage_path, &a.title).await?;
            println!("ok");
        }
        DocCmd::Move(a) => {
            let to_nb = NotebookId::parse(&a.to_notebook).context("--to-notebook")?;

            // Build one DocLookup per source. The clap `ArgGroup` already
            // enforces that exactly one of `from_ids` / `from_hpaths` is
            // populated; we re-validate here so the runtime path is
            // self-contained and a future caller that constructs MoveArgs
            // programmatically still gets a clean error.
            let source_lookups =
                build_move_source_lookups(&a.from_ids, a.notebook.as_deref(), &a.from_hpaths)?;

            // Resolve each source to its storage path sequentially. The
            // kernel's `moveDocs` is a single transaction that takes a Vec
            // of paths, so we need them all up front. Sequential is fine
            // for typical batch sizes (<10) — the resolve() call internally
            // hits `lsNotebooks` + a single SQL `IN` query, both cheap.
            let mut from_paths = Vec::with_capacity(source_lookups.len());
            for lookup in source_lookups {
                let (_nb, storage_path) = resolve_one_storage(client, lookup).await?;
                from_paths.push(storage_path);
            }

            client.move_docs(&from_paths, &to_nb, &a.to_path).await?;
            println!("ok");
        }
        DocCmd::SetIcon(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("icon".to_string(), a.icon);
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::SetSort(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("sort".to_string(), a.sort.to_string());
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::Remove(a) => {
            let lookup = build_single_doc_lookup(
                a.id.as_deref(),
                a.notebook.as_deref(),
                a.hpath.as_deref(),
            )?;
            let (nb, storage_path) = resolve_one_storage(client, lookup).await?;
            client.remove_doc(&nb, &storage_path).await?;
            println!("ok");
        }
    }
    Ok(())
}

/// Build a single-document `DocLookup` from clap-parsed pieces.
///
/// Clap's `ArgGroup` already filters out partial / conflicting input, but
/// we keep this helper as the canonical CLI-side validator so:
/// 1. The model layer's `DocLookup` invariant ("exactly one variant's
///    worth of data") is enforced at the boundary regardless of caller.
/// 2. Programmatic callers (tests, future scripting) get a uniform error.
/// 3. The same helper is reused by `doc resolve`, `doc rename`, and
///    `doc remove` — keeping the user-facing error messages consistent.
fn build_single_doc_lookup(
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

/// Build a vector of source-document `DocLookup`s for `doc move`.
///
/// Mirrors `build_single_doc_lookup` but for the batch case. Callers must
/// supply EITHER `from_ids` (non-empty) OR (`notebook` + `from_hpaths`,
/// both non-empty), never both, never neither. Empty arrays in the
/// supplied mode are rejected so a misconfigured invocation surfaces as
/// a usage error rather than a silent kernel no-op.
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
