//! Unified document-metadata lookup.
//!
//! Combines the two kernel-level facilities — id ↔ hpath conversion via the
//! filetree API and notebook enumeration via `lsNotebooks` — into a single
//! semantic call that returns a uniform `ResolvedDoc` shape regardless of
//! which direction the caller approached from. The MCP and CLI layers wrap
//! this function so agents and humans see one tool/command instead of two.
//!
//! The input is modelled as an enum (`DocLookup`) so the "exactly one of id
//! XOR (notebook+hpath)" invariant is unrepresentable in the library API;
//! validation happens at the boundary (MCP handler / CLI arg parsing) before
//! the enum is constructed.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::{BlockId, NotebookId, SiyuanError};

/// Per-document metadata returned by [`resolve`].
///
/// The serde representation maps directly to the JSON payload exposed by
/// the MCP `siyuan_doc_resolve` tool and the `siyuan doc resolve` CLI
/// subcommand — no field-level renames are needed because the snake_case
/// field names are already the desired output keys.
///
/// Named `ResolvedDoc` to avoid colliding with [`crate::bundle::DocMeta`],
/// which is a much smaller in-bundle descriptor used by the doc-bundle
/// loader and renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDoc {
    pub id: BlockId,
    pub hpath: String,
    pub notebook_id: NotebookId,
    /// Empty when the notebook id is not in the live notebook list (e.g. the
    /// notebook was removed between the SQL row being written and this
    /// lookup running). Indistinguishable from a literally-empty notebook
    /// name without cross-referencing `siyuan_notebook_ls` output.
    pub notebook_name: String,
    /// Last `/`-delimited segment of `hpath`. Empty when `hpath` is empty
    /// or exactly `/` (the kernel's canonical root).
    pub title: String,
    /// On-disk `.sy` storage path from the `blocks` table. Required by the
    /// kernel's `renameDoc`, `moveDocs`, and `removeDoc` endpoints — those
    /// take storage paths, not human-readable hpaths.
    pub storage_path: String,
}

/// Input mode for [`resolve`]. Construct via the boundary layer after
/// validating that the caller supplied exactly one variant's worth of data.
#[derive(Debug, Clone)]
pub enum DocLookup {
    ById(BlockId),
    ByHpath { notebook: NotebookId, hpath: String },
}

/// Row shape pulled from the `blocks` table for `type='d'` (document) rows.
#[derive(Debug, Deserialize)]
struct DocRow {
    id: String,
    #[serde(rename = "box")]
    box_: String,
    hpath: String,
    path: String,
}

/// Resolve a document by id or by `(notebook, hpath)` and return all matches.
///
/// Returns an empty vector when nothing matches — both branches treat
/// "no such doc" as a non-error condition so callers can distinguish
/// kernel/transport failures from a clean miss.
pub async fn resolve(
    client: &SiyuanClient,
    lookup: DocLookup,
) -> Result<Vec<ResolvedDoc>, SiyuanError> {
    // Build a notebook id → name map once; cheap call, no caching needed
    // because lookups are not in a hot loop.
    let notebooks = client.ls_notebooks().await?;
    let nb_names: HashMap<NotebookId, String> =
        notebooks.into_iter().map(|n| (n.id, n.name)).collect();

    let ids: Vec<BlockId> = match lookup {
        DocLookup::ById(id) => vec![id],
        DocLookup::ByHpath { notebook, hpath } => {
            client.get_ids_by_hpath(&notebook, &hpath).await?
        }
    };

    if ids.is_empty() {
        return Ok(Vec::new());
    }

    // Single SQL query with `IN (...)`. Block ids are validated by
    // `BlockId::parse`, so direct interpolation is safe — the regex enforces
    // `\d{14}-[0-9a-z]{7}` which contains no SQL meta-characters.
    let id_list = ids
        .iter()
        .map(|i| format!("'{}'", i.as_str()))
        .collect::<Vec<_>>()
        .join(",");
    let stmt =
        format!("SELECT id, box, hpath, path FROM blocks WHERE id IN ({id_list}) AND type = 'd'");
    let rows: Vec<DocRow> = client.sql_typed(&stmt).await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let id = BlockId::parse(r.id).map_err(|e| SiyuanError::Parse(e.to_string()))?;
        let notebook_id =
            NotebookId::parse(r.box_).map_err(|e| SiyuanError::Parse(e.to_string()))?;
        let notebook_name = nb_names.get(&notebook_id).cloned().unwrap_or_default();
        let title = title_from_hpath(&r.hpath);
        out.push(ResolvedDoc {
            id,
            hpath: r.hpath,
            notebook_id,
            notebook_name,
            title,
            storage_path: r.path,
        });
    }

    Ok(out)
}

/// Derive the document title from its hpath. The kernel stores hpaths as
/// `/Folder/Title`; the title is the last `/`-delimited segment. Edge cases
/// follow `rsplit('/').next()` mechanics:
/// - empty input or `/` (canonical root) → `""`,
/// - trailing slash (`/Foo/`) → `""` (the segment after the last `/`),
/// - double slash (`/A//B`) → `"B"` (only the final segment matters),
/// - no leading slash (`Foo`) → `"Foo"` (the whole input is one segment).
fn title_from_hpath(hpath: &str) -> String {
    hpath.rsplit('/').next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_from_nested_hpath() {
        assert_eq!(title_from_hpath("/Projects/Plan"), "Plan");
    }

    #[test]
    fn title_from_top_level_hpath() {
        assert_eq!(title_from_hpath("/Plan"), "Plan");
    }

    #[test]
    fn title_from_root_hpath_is_empty() {
        assert_eq!(title_from_hpath("/"), "");
    }

    #[test]
    fn title_from_empty_hpath_is_empty() {
        assert_eq!(title_from_hpath(""), "");
    }

    // Trailing slash: rsplit returns the empty segment after the final `/`.
    // Locks the contract: callers that pass `/Foo/` do NOT get `"Foo"`.
    #[test]
    fn title_from_trailing_slash_hpath_is_empty() {
        assert_eq!(title_from_hpath("/Foo/"), "");
    }

    // Double slash: only the segment after the LAST `/` matters, so the
    // empty middle segment is invisible.
    #[test]
    fn title_from_double_slash_hpath_uses_last_segment() {
        assert_eq!(title_from_hpath("/A//B"), "B");
    }

    // No leading slash: the entire string is one segment.
    #[test]
    fn title_from_unrooted_hpath_returns_whole_string() {
        assert_eq!(title_from_hpath("Foo"), "Foo");
    }
}
