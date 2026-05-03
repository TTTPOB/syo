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
/// the MCP `syo_siyuan_doc_resolve` tool and the `syo doc resolve` CLI
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
    /// name without cross-referencing `syo_siyuan_notebook_ls` output.
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

/// Resolve a [`DocLookup`] to exactly one `(notebook_id, storage_path)` pair.
///
/// Wraps [`resolve`] for the rename/move/remove call sites that need a single
/// storage path and refuse to silently pick one when the lookup is ambiguous:
/// - 0 hits → [`SiyuanError::NotFound`] (the kernel would otherwise return a
///   confusing API error like "file does not exist" with the still-unresolved
///   hpath in the message);
/// - 1 hit → `(notebook_id, storage_path)`;
/// - more than 1 hit → [`SiyuanError::AmbiguousPath`] with all candidate ids
///   so the caller can disambiguate by id.
///
/// The hpath surfaced in the `NotFound` / `AmbiguousPath` errors is the one the
/// caller passed in (for `ByHpath`) or a synthetic `id:<...>` marker (for
/// `ById`) so the message is always actionable.
pub async fn resolve_one_storage(
    client: &SiyuanClient,
    lookup: DocLookup,
) -> Result<(NotebookId, String), SiyuanError> {
    // Capture a human-readable identifier BEFORE moving `lookup` into
    // `resolve`, so the error path below can quote what the caller asked for
    // without needing to reconstruct it.
    let descriptor = match &lookup {
        DocLookup::ById(id) => format!("id:{}", id.as_str()),
        DocLookup::ByHpath { hpath, .. } => hpath.clone(),
    };

    let docs = resolve(client, lookup).await?;
    pick_one_storage(descriptor, docs)
}

/// Pure dispatch over `resolve`'s output. Split out so it is unit-testable
/// without a live `SiyuanClient` — the `>1` and `0` branches in particular
/// can't be reproduced through the dummy-client pattern used elsewhere.
fn pick_one_storage(
    descriptor: String,
    docs: Vec<ResolvedDoc>,
) -> Result<(NotebookId, String), SiyuanError> {
    match docs.len() {
        0 => Err(SiyuanError::NotFound(descriptor)),
        1 => {
            let d = docs.into_iter().next().expect("len==1 implies one element");
            Ok((d.notebook_id, d.storage_path))
        }
        // >1 hits: surface ALL candidate ids so the caller can disambiguate
        // by id rather than guessing. The kernel allows duplicate hpaths in
        // rare edge cases (e.g. concurrent doc creation races) — we do not
        // pick a winner here.
        _ => {
            let candidates = docs.into_iter().map(|d| d.id).collect();
            Err(SiyuanError::AmbiguousPath {
                hpath: descriptor,
                candidates,
            })
        }
    }
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

    // ---- pick_one_storage --------------------------------------------------
    //
    // The `resolve_one_storage` wrapper composes a network call with
    // `pick_one_storage`. The pure dispatch is the interesting failure-mode
    // surface (0/1/>1 hits → which error variant?), so we test the dispatch
    // directly. Exercising `resolve_one_storage` through a live kernel is
    // covered by the integration tests in `crates/syo-cli/tests/`.

    fn nb() -> NotebookId {
        NotebookId::parse("20260501000000-nb00001").unwrap()
    }

    fn doc(id: &str, hpath: &str, storage_path: &str) -> ResolvedDoc {
        ResolvedDoc {
            id: BlockId::parse(id).unwrap(),
            hpath: hpath.to_string(),
            notebook_id: nb(),
            notebook_name: "Inbox".to_string(),
            title: hpath.rsplit('/').next().unwrap_or("").to_string(),
            storage_path: storage_path.to_string(),
        }
    }

    #[test]
    fn pick_one_storage_zero_hits_is_not_found() {
        let err = pick_one_storage("/Missing".into(), Vec::new()).expect_err("0 hits must error");
        match err {
            SiyuanError::NotFound(s) => assert_eq!(s, "/Missing"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn pick_one_storage_one_hit_returns_pair() {
        let docs = vec![doc(
            "20260501090000-doc0001",
            "/Plan",
            "/20260501090000-doc0001.sy",
        )];
        let (nb_id, path) =
            pick_one_storage("/Plan".into(), docs).expect("single hit must succeed");
        assert_eq!(nb_id, nb());
        assert_eq!(path, "/20260501090000-doc0001.sy");
    }

    // Locks the contract for the >1 hits case: the kernel can return more
    // than one document at the same hpath in rare race conditions, and we
    // want callers to see ALL candidate ids so they can disambiguate by id.
    #[test]
    fn pick_one_storage_multiple_hits_is_ambiguous_with_all_candidates() {
        let docs = vec![
            doc(
                "20260501090000-doc0001",
                "/Plan",
                "/20260501090000-doc0001.sy",
            ),
            doc(
                "20260501090000-doc0002",
                "/Plan",
                "/20260501090000-doc0002.sy",
            ),
        ];
        let err = pick_one_storage("/Plan".into(), docs).expect_err(">1 hits must error");
        match err {
            SiyuanError::AmbiguousPath { hpath, candidates } => {
                assert_eq!(hpath, "/Plan");
                assert_eq!(candidates.len(), 2);
                assert_eq!(candidates[0].as_str(), "20260501090000-doc0001");
                assert_eq!(candidates[1].as_str(), "20260501090000-doc0002");
            }
            other => panic!("expected AmbiguousPath, got {other:?}"),
        }
    }
}
