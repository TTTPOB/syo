//! Doc-tree listing: enumerate documents under a notebook/folder root.
//!
//! Single-source backend for `syo doc tree` (CLI) and `syo_siyuan_doc_tree`
//! (MCP). The kernel exposes no list-children endpoint at the filetree HTTP
//! API, so this module pulls the relevant subset of `blocks` rows via a
//! single SQL query and assembles the tree in memory.
//!
//! Address modes mirror [`crate::doc_meta::DocLookup`]:
//! - `--id <doc>` → tree root is that doc itself, plus depth levels of
//!   descendants.
//! - `--notebook <nb> --hpath /` → virtual root, output is the notebook's
//!   top-level docs.
//! - `--notebook <nb> --hpath /Foo` → tree root is the doc at `/Foo`.
//!
//! `doc_count_recursive` is computed from the FULL preload regardless of
//! depth slice, so even a `--depth 1` view tells the caller how many
//! descendants exist further down.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_types::{NotebookId, SiyuanError};

use crate::doc_meta::{DocLookup, resolve as resolve_doc_meta};

/// Maximum depth interpreted as "all". Chosen to be deeper than any
/// realistic SiYuan workspace nesting; serialized to JSON via `Depth::All`.
const ALL_DEPTH_SENTINEL: u32 = u32::MAX;
const INTERNAL_SQL_LIMIT: usize = 100_000;

/// Tree-traversal depth budget.
///
/// `--depth all` becomes [`Depth::All`]; integer inputs become [`Depth::N`].
/// `0` is rejected by the boundary parsers (clap / MCP arg validator) so
/// every constructed value here is meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// Numeric budget (>= 1). Slice the tree to this many levels below
    /// the tree root.
    N(u32),
    /// Unlimited (full subtree).
    All,
}

impl Depth {
    /// Internal helper: convert to a saturating integer for arithmetic.
    fn as_budget(self) -> u32 {
        match self {
            Depth::N(n) => n,
            Depth::All => ALL_DEPTH_SENTINEL,
        }
    }
}

/// One node in the rendered tree.
///
/// Field order matches the spec; all fields are always present for
/// uniformity (the agent-md formatter suppresses noisy defaults). The
/// virtual-root case sets `id`/`title`/`storage_path`/timestamps to empty
/// strings and `sort=0`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    pub title: String,
    pub hpath: String,
    pub has_children: bool,
    pub doc_count_recursive: u64,
    pub created: String,
    pub updated: String,
    pub sort: i64,
    pub icon: String,
    pub notebook_id: String,
    pub notebook_name: String,
    pub storage_path: String,
    /// Children present at this depth slice. Empty when:
    /// - the node has no children at all (`has_children=false`), OR
    /// - the depth budget cut off below this node.
    ///
    /// Use `has_children` to disambiguate.
    pub children: Vec<TreeNode>,
}

/// Row shape pulled from the `blocks` table for `type='d'` (document) rows.
#[derive(Debug, Clone, Deserialize)]
struct DocRow {
    id: String,
    hpath: String,
    path: String,
    #[serde(default)]
    sort: i64,
    #[serde(default)]
    created: String,
    #[serde(default)]
    updated: String,
    #[serde(default)]
    ial: String,
}

/// Build a doc tree from a [`DocLookup`] address.
///
/// The function performs three round trips:
/// 1. `lsNotebooks` — for the notebook-name lookup map.
/// 2. (id mode only) `resolve_doc_meta` — to turn the id into a known root.
///    For notebook-root mode this is skipped; for `notebook+hpath` mode
///    `resolve_doc_meta` is also used because the kernel's `getIDsByHpath`
///    path is the only way to translate a hpath to an id.
/// 3. A single `SELECT ... FROM blocks` for the subtree.
pub async fn build_tree(
    client: &SiyuanClient,
    lookup: DocLookup,
    depth: Depth,
) -> Result<TreeNode, SiyuanError> {
    // Build notebook id → name map once. Cheap call; not worth caching.
    let notebooks = client.ls_notebooks().await?;
    let nb_names: HashMap<NotebookId, String> =
        notebooks.into_iter().map(|n| (n.id, n.name)).collect();

    // Determine the (notebook_id, root_path) anchor. `root_path` is the
    // storage path that prefixes every descendant; the empty string is the
    // sentinel used by `notebook root` mode and means "all docs in the
    // notebook".
    let (notebook_id, root_path) = anchor(client, lookup).await?;

    // Pull the subtree rows. `root_path` empty → whole notebook;
    // otherwise root row plus everything under `<root_path-without-suffix>/`.
    let rows = fetch_rows(client, &notebook_id, &root_path).await?;

    let nb_name = nb_names.get(&notebook_id).cloned().unwrap_or_default();

    Ok(assemble(
        &rows,
        &notebook_id,
        &nb_name,
        if root_path.is_empty() {
            None
        } else {
            Some(&root_path)
        },
        depth,
    ))
}

/// Anchor resolution: turn a [`DocLookup`] into a `(notebook_id, root_path)`
/// pair where `root_path` is either the storage path of the tree root or
/// the empty string for the notebook-root virtual-root case.
async fn anchor(
    client: &SiyuanClient,
    lookup: DocLookup,
) -> Result<(NotebookId, String), SiyuanError> {
    // Special-case: --notebook X --hpath / means "virtual root".
    // We don't translate that to a doc resolve — there is no doc with
    // hpath="/".
    if let DocLookup::ByHpath { notebook, hpath } = &lookup
        && (hpath == "/" || hpath.is_empty())
    {
        return Ok((notebook.clone(), String::new()));
    }

    // Capture a human-readable identifier before moving the lookup.
    let descriptor = match &lookup {
        DocLookup::ById(id) => format!("id:{}", id.as_str()),
        DocLookup::ByHpath { hpath, .. } => hpath.clone(),
    };

    let docs = resolve_doc_meta(client, lookup).await?;
    match docs.len() {
        0 => Err(SiyuanError::NotFound(descriptor)),
        1 => {
            let d = docs.into_iter().next().expect("len==1");
            Ok((d.notebook_id, d.storage_path))
        }
        _ => {
            let candidates = docs.into_iter().map(|d| d.id).collect();
            Err(SiyuanError::AmbiguousPath {
                hpath: descriptor,
                candidates,
            })
        }
    }
}

/// Pull rows for the requested subtree.
///
/// `root_path` empty means "all docs in the notebook" (notebook-root mode).
/// `root_path` non-empty (e.g. `/<id>.sy`) means "this row plus every doc
/// whose path starts with `<root_path-without-trailing-.sy>/`". The
/// LIKE prefix is the storage path with `.sy` stripped — the kernel stores
/// children of doc X under `/<X-without-.sy>/<child>.sy`.
async fn fetch_rows(
    client: &SiyuanClient,
    notebook: &NotebookId,
    root_path: &str,
) -> Result<Vec<DocRow>, SiyuanError> {
    // Notebook ids are validated by NotebookId::parse, so direct
    // interpolation is safe (regex `\d{14}-[0-9a-z]{7}` has no SQL meta-chars).
    // Storage paths come from the `blocks` table itself (round-tripped via
    // resolve_doc_meta) — they are not user free-text — so direct
    // interpolation is also acceptable.
    let stmt = if root_path.is_empty() {
        format!(
            "SELECT id, hpath, path, sort, created, updated, ial \
             FROM blocks \
             WHERE box = '{}' AND type = 'd' \
             ORDER BY path \
             LIMIT {}",
            notebook.as_str(),
            INTERNAL_SQL_LIMIT
        )
    } else {
        let prefix = strip_sy_suffix(root_path);
        format!(
            "SELECT id, hpath, path, sort, created, updated, ial \
             FROM blocks \
             WHERE box = '{}' AND type = 'd' \
               AND (path = '{}' OR path LIKE '{}/%') \
             ORDER BY path \
             LIMIT {}",
            notebook.as_str(),
            root_path,
            prefix,
            INTERNAL_SQL_LIMIT
        )
    };
    client.sql_typed(&stmt).await
}

/// Strip a trailing `.sy` from a storage path, so `/abc.sy` becomes `/abc`
/// suitable for use as a parent-prefix in a LIKE `<prefix>/%` clause.
fn strip_sy_suffix(path: &str) -> String {
    if let Some(stripped) = path.strip_suffix(".sy") {
        stripped.to_string()
    } else {
        // Defensive: kernel always emits `.sy` for doc rows, but if a row
        // ever lacks it, fall through with the path as-is so the LIKE still
        // produces a meaningful prefix.
        path.to_string()
    }
}

/// In-memory tree assembly from a flat row list.
///
/// `root_path` is the storage path of the tree root, or `None` for the
/// virtual-root (notebook-root) case. The function:
/// 1. Indexes rows by their storage path → DocRow.
/// 2. Computes parent-child relationships by stripping the last
///    `/<id>.sy` segment (or `/<id>` for parents that are themselves docs
///    — see `parent_path_of` for the exact rule).
/// 3. Walks parent-child relationships from the root, computing
///    `doc_count_recursive` from the FULL load and slicing children to
///    `depth` levels.
fn assemble(
    rows: &[DocRow],
    notebook_id: &NotebookId,
    notebook_name: &str,
    root_path: Option<&str>,
    depth: Depth,
) -> TreeNode {
    use std::collections::BTreeMap;

    // Index rows by storage path for O(1) parent lookup.
    let by_path: HashMap<&str, &DocRow> = rows.iter().map(|r| (r.path.as_str(), r)).collect();

    // Build parent → children map. Children are kept in `path` lexical
    // order (which matches the SQL `ORDER BY path` output) so the tree
    // walk is deterministic. Use BTreeMap for the inner storage so within
    // a parent the children stay sorted by their full storage path.
    let mut children_of: HashMap<String, BTreeMap<String, &DocRow>> = HashMap::new();
    for r in rows {
        let parent = parent_path_of(&r.path);
        children_of
            .entry(parent)
            .or_default()
            .insert(r.path.clone(), r);
    }

    // Compute doc_count_recursive for every row from the FULL load.
    // Walk: count(node) = 1 if !is_root else 0, summed over self and
    // descendants. We subtract 1 at the end because the count is
    // descendants only, not the node itself.
    let counts = compute_counts(rows, &children_of);

    match root_path {
        // Notebook-root virtual node: synthesize a placeholder and slice
        // depth from there.
        None => {
            let depth_budget = depth.as_budget();
            // Top-level docs are those whose `path` has no `/<parent>/`
            // structure — i.e. their parent path is empty.
            let top_level = children_of
                .get("")
                .map(|m| m.values().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            let mut total_descendants: u64 = 0;
            let mut children = Vec::new();
            for r in &top_level {
                total_descendants += 1 + counts.get(r.path.as_str()).copied().unwrap_or(0);
                if depth_budget >= 1 {
                    children.push(build_node(
                        r,
                        notebook_id,
                        notebook_name,
                        &children_of,
                        &counts,
                        depth_budget - 1,
                    ));
                }
            }
            TreeNode {
                id: String::new(),
                title: String::new(),
                hpath: "/".to_string(),
                has_children: !top_level.is_empty(),
                doc_count_recursive: total_descendants,
                created: String::new(),
                updated: String::new(),
                sort: 0,
                icon: String::new(),
                notebook_id: notebook_id.to_string(),
                notebook_name: notebook_name.to_string(),
                storage_path: String::new(),
                children,
            }
        }
        // Doc-rooted tree: locate the root row and recurse.
        Some(rp) => {
            let root_row = by_path
                .get(rp)
                .copied()
                .expect("root row must be present in fetch_rows result");
            build_node(
                root_row,
                notebook_id,
                notebook_name,
                &children_of,
                &counts,
                depth.as_budget(),
            )
        }
    }
}

/// Build a single node (with its sliced children) from a row.
fn build_node(
    row: &DocRow,
    notebook_id: &NotebookId,
    notebook_name: &str,
    children_of: &HashMap<String, std::collections::BTreeMap<String, &DocRow>>,
    counts: &HashMap<String, u64>,
    remaining_depth: u32,
) -> TreeNode {
    // The "children container" key for this node is the path with .sy
    // stripped — that's the prefix kernel uses for the next level.
    let kids_key = strip_sy_suffix(&row.path);
    let kid_rows: Vec<&DocRow> = children_of
        .get(&kids_key)
        .map(|m| m.values().copied().collect())
        .unwrap_or_default();
    let has_children = !kid_rows.is_empty();

    let children = if remaining_depth == 0 {
        Vec::new()
    } else {
        kid_rows
            .iter()
            .map(|r| {
                build_node(
                    r,
                    notebook_id,
                    notebook_name,
                    children_of,
                    counts,
                    remaining_depth - 1,
                )
            })
            .collect()
    };

    TreeNode {
        id: row.id.clone(),
        title: title_from_hpath(&row.hpath),
        hpath: row.hpath.clone(),
        has_children,
        doc_count_recursive: counts.get(row.path.as_str()).copied().unwrap_or(0),
        created: row.created.clone(),
        updated: row.updated.clone(),
        sort: row.sort,
        icon: extract_icon(&row.ial),
        notebook_id: notebook_id.to_string(),
        notebook_name: notebook_name.to_string(),
        storage_path: row.path.clone(),
        children,
    }
}

/// Compute `doc_count_recursive` for every row by post-order walk.
///
/// Returns a path → descendant-count map. The root's own count is excluded
/// (matches the spec: "count of all DESCENDANT docs under this node").
fn compute_counts(
    rows: &[DocRow],
    children_of: &HashMap<String, std::collections::BTreeMap<String, &DocRow>>,
) -> HashMap<String, u64> {
    // Iterative DFS keyed by path so we can populate the map post-order.
    let mut out: HashMap<String, u64> = HashMap::with_capacity(rows.len());

    fn visit(
        path: &str,
        children_of: &HashMap<String, std::collections::BTreeMap<String, &DocRow>>,
        out: &mut HashMap<String, u64>,
    ) -> u64 {
        let kids_key = strip_sy_suffix(path);
        let kids: Vec<&DocRow> = children_of
            .get(&kids_key)
            .map(|m| m.values().copied().collect())
            .unwrap_or_default();
        let mut total: u64 = 0;
        for k in kids {
            total += 1 + visit(&k.path, children_of, out);
        }
        out.insert(path.to_string(), total);
        total
    }

    for r in rows {
        if !out.contains_key(r.path.as_str()) {
            visit(&r.path, children_of, &mut out);
        }
    }

    out
}

/// Storage-path → parent-path. The kernel layout:
/// - top-level doc: `/<id>.sy` → parent = ""
/// - nested doc: `/<parent>/<id>.sy` → parent = "/<parent>"
///   (NOTE: the parent's storage path itself is `/<parent>.sy` — but in
///   `path` strings the `.sy` is dropped from intermediate segments.)
///
/// The function returns the parent's storage-path KEY used in the
/// `children_of` map. For top-level docs this is the empty string; for
/// nested docs it is the parent's path-without-`.sy` (i.e. what
/// [`strip_sy_suffix`] produces for the parent's row).
fn parent_path_of(path: &str) -> String {
    // Find the last `/`. Everything before it is the parent prefix.
    match path.rfind('/') {
        // Top-level: only one `/`, before the doc's own `<id>.sy`.
        Some(0) => String::new(),
        Some(pos) => path[..pos].to_string(),
        // Pathological — kernel always starts paths with `/`.
        None => String::new(),
    }
}

/// Derive the document title from its hpath. Mirrors the same helper in
/// `doc_meta.rs` — duplicated because the function there is private and
/// inlining a trivial `rsplit` keeps both modules self-contained.
fn title_from_hpath(hpath: &str) -> String {
    hpath.rsplit('/').next().unwrap_or("").to_string()
}

/// Extract the `icon` value from an IAL (Inline Attribute List) string.
///
/// SiYuan stores IAL inside `{:` ... `}` braces with `key="value"`
/// attributes, e.g. `{: id="20260501090000-doc0001" icon=":rocket:"}`.
/// This is a minimal parser: it locates `icon="..."` and returns the
/// quoted value, ignoring escaping (the kernel does not emit escaped
/// quotes inside icon values in practice). Returns empty string if
/// absent or unparseable.
fn extract_icon(ial: &str) -> String {
    // Look for the literal `icon="` and the next `"` after it.
    let key = "icon=\"";
    let start = match ial.find(key) {
        Some(i) => i + key.len(),
        None => return String::new(),
    };
    let rest = &ial[start..];
    match rest.find('"') {
        Some(end) => rest[..end].to_string(),
        None => String::new(),
    }
}

/// Render a [`TreeNode`] as agent-markdown.
///
/// Indented bullet list per level with `<!-- sy:doc id=... -->` markers.
/// The trailing `<!-- sy:tree depth=... total_loaded=... truncated=... -->`
/// summary lets jq/grep callers reason about whether they are looking at
/// a partial slice without re-deriving it from indentation.
pub fn render_agent_md(root: &TreeNode, depth: Depth) -> String {
    let mut buf = String::new();
    let total_loaded = count_rendered(root);
    let truncated = is_truncated(root);

    if root.id.is_empty() {
        // Virtual root: skip the placeholder line; emit only its children.
        for c in &root.children {
            render_node(c, 0, &mut buf);
        }
    } else {
        render_node(root, 0, &mut buf);
    }

    let depth_label = match depth {
        Depth::All => "all".to_string(),
        Depth::N(n) => n.to_string(),
    };
    let truncated_str = if truncated {
        match depth {
            Depth::N(n) => format!("true (depth limit: {n}, use --depth all to see full tree)"),
            Depth::All => "true (node limit reached)".to_string(),
        }
    } else {
        "false".to_string()
    };
    buf.push_str(&format!(
        "<!-- sy:tree depth={depth_label} total_loaded={total_loaded} truncated={truncated_str} -->\n"
    ));
    buf
}

/// Count the nodes actually emitted in the agent-md slice (i.e. the loaded
/// portion of the tree, excluding any virtual root).
fn count_rendered(node: &TreeNode) -> u64 {
    let mut n = if node.id.is_empty() { 0 } else { 1 };
    for c in &node.children {
        n += count_rendered(c);
    }
    n
}

/// True if any node in the rendered slice has `has_children=true` but no
/// loaded `children` — indicating the depth budget cut off below it.
fn is_truncated(node: &TreeNode) -> bool {
    if node.has_children && node.children.is_empty() && !node.id.is_empty() {
        return true;
    }
    node.children.iter().any(is_truncated)
}

fn render_node(node: &TreeNode, level: usize, buf: &mut String) {
    let indent = "  ".repeat(level);
    // Node marker (HTML comment): id, hpath, title, sort.
    buf.push_str(&format!(
        "{indent}<!-- sy:doc id={} hpath={} title={:?} sort={} -->\n",
        node.id, node.hpath, node.title, node.sort
    ));
    // One-line summary: hpath + descendant count + truncated marker.
    let trunc = if node.has_children && node.children.is_empty() {
        " (truncated)"
    } else {
        ""
    };
    buf.push_str(&format!(
        "{indent}- {} ({} subdoc{}){}\n",
        node.hpath,
        node.doc_count_recursive,
        if node.doc_count_recursive == 1 {
            ""
        } else {
            "s"
        },
        trunc,
    ));
    for c in &node.children {
        render_node(c, level + 1, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nb() -> NotebookId {
        NotebookId::parse("20260501000000-nb00001").unwrap()
    }

    /// Build a synthetic row.
    fn row(id: &str, hpath: &str, path: &str, sort: i64, ial: &str) -> DocRow {
        DocRow {
            id: id.to_string(),
            hpath: hpath.to_string(),
            path: path.to_string(),
            sort,
            created: "20260501090000".to_string(),
            updated: "20260501090000".to_string(),
            ial: ial.to_string(),
        }
    }

    /// Synthetic /A → /A/B → /A/B/C tree rows. Storage paths follow the
    /// kernel convention: `/<root>.sy`, `/<root>/<child>.sy`, etc.
    fn nested_rows() -> Vec<DocRow> {
        vec![
            row(
                "20260501090000-doc0001",
                "/A",
                "/20260501090000-doc0001.sy",
                10,
                "{: id=\"20260501090000-doc0001\" icon=\":rocket:\"}",
            ),
            row(
                "20260501090000-doc0002",
                "/A/B",
                "/20260501090000-doc0001/20260501090000-doc0002.sy",
                20,
                "",
            ),
            row(
                "20260501090000-doc0003",
                "/A/B/C",
                "/20260501090000-doc0001/20260501090000-doc0002/20260501090000-doc0003.sy",
                30,
                "",
            ),
        ]
    }

    #[test]
    fn title_from_hpath_handles_root() {
        assert_eq!(title_from_hpath("/"), "");
        assert_eq!(title_from_hpath("/A"), "A");
        assert_eq!(title_from_hpath("/A/B/C"), "C");
    }

    #[test]
    fn extract_icon_pulls_quoted_value() {
        let ial = "{: id=\"20260501090000-doc0001\" icon=\":rocket:\"}";
        assert_eq!(extract_icon(ial), ":rocket:");
    }

    #[test]
    fn extract_icon_returns_empty_when_absent() {
        let ial = "{: id=\"20260501090000-doc0001\"}";
        assert_eq!(extract_icon(ial), "");
    }

    #[test]
    fn extract_icon_handles_empty_string() {
        assert_eq!(extract_icon(""), "");
    }

    #[test]
    fn strip_sy_suffix_removes_dot_sy() {
        assert_eq!(strip_sy_suffix("/abc.sy"), "/abc");
        assert_eq!(strip_sy_suffix("/abc/def.sy"), "/abc/def");
    }

    #[test]
    fn strip_sy_suffix_passes_through_when_missing() {
        // Defensive: should not panic on a malformed input.
        assert_eq!(strip_sy_suffix("/abc"), "/abc");
    }

    #[test]
    fn parent_path_of_top_level_is_empty() {
        assert_eq!(parent_path_of("/20260501090000-doc0001.sy"), "");
    }

    #[test]
    fn parent_path_of_nested_strips_last_segment() {
        // The parent storage key is the prefix without `.sy` — matches what
        // `strip_sy_suffix` produces for the parent's own row.
        assert_eq!(
            parent_path_of("/20260501090000-doc0001/20260501090000-doc0002.sy"),
            "/20260501090000-doc0001"
        );
    }

    // Exercises the synthetic tree at depth=1 from the /A root. The root
    // node should report 2 descendants total (B + C) but only load B.
    #[test]
    fn assemble_id_mode_depth_one_loads_immediate_child() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::N(1),
        );
        assert_eq!(tree.id, "20260501090000-doc0001");
        assert_eq!(tree.title, "A");
        assert!(tree.has_children);
        assert_eq!(tree.doc_count_recursive, 2);
        assert_eq!(tree.children.len(), 1, "depth=1 loads exactly one level");
        let b = &tree.children[0];
        assert_eq!(b.title, "B");
        assert!(b.has_children);
        // depth budget consumed at /A → /B; /C is NOT loaded.
        assert!(b.children.is_empty());
        // But doc_count_recursive on /B still reflects /C from full preload.
        assert_eq!(b.doc_count_recursive, 1);
    }

    #[test]
    fn assemble_id_mode_depth_two_loads_two_levels() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::N(2),
        );
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].title, "C");
        assert!(tree.children[0].children[0].children.is_empty());
        assert!(!tree.children[0].children[0].has_children);
        assert_eq!(tree.children[0].children[0].doc_count_recursive, 0);
    }

    #[test]
    fn assemble_id_mode_depth_all_loads_full_subtree() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::All,
        );
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].children.len(), 1);
        assert_eq!(tree.children[0].children[0].title, "C");
    }

    // Notebook-root mode: the synthesized virtual root has empty id/title
    // and `hpath="/"`, with the top-level docs as its children.
    #[test]
    fn assemble_notebook_root_yields_virtual_root() {
        let rows = nested_rows();
        let tree = assemble(&rows, &nb(), "Inbox", None, Depth::N(1));
        assert_eq!(tree.id, "");
        assert_eq!(tree.title, "");
        assert_eq!(tree.hpath, "/");
        assert_eq!(tree.storage_path, "");
        assert_eq!(tree.notebook_id, "20260501000000-nb00001");
        assert_eq!(tree.notebook_name, "Inbox");
        assert!(tree.has_children);
        // Total descendants = 3 (A + B + C); A is the only top-level.
        assert_eq!(tree.doc_count_recursive, 3);
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].title, "A");
    }

    // Sort and icon roundtrip: assemble must surface the row's `sort` and
    // the parsed `icon` from IAL.
    #[test]
    fn assemble_propagates_sort_and_icon() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::N(0),
        );
        assert_eq!(tree.sort, 10);
        assert_eq!(tree.icon, ":rocket:");
    }

    // Empty notebook: virtual root has 0 children and 0 descendants.
    #[test]
    fn assemble_notebook_root_empty_notebook() {
        let rows: Vec<DocRow> = Vec::new();
        let tree = assemble(&rows, &nb(), "Inbox", None, Depth::N(1));
        assert!(!tree.has_children);
        assert_eq!(tree.doc_count_recursive, 0);
        assert!(tree.children.is_empty());
    }

    // Render check: virtual-root variant must NOT emit an empty
    // `<!-- sy:doc id= -->` line.
    #[test]
    fn render_agent_md_skips_virtual_root_marker() {
        let rows = nested_rows();
        let tree = assemble(&rows, &nb(), "Inbox", None, Depth::N(1));
        let s = render_agent_md(&tree, Depth::N(1));
        assert!(
            !s.contains("sy:doc id= "),
            "virtual root must not emit an empty id marker; got:\n{s}"
        );
        assert!(s.contains("hpath=/A"), "expected /A child in render: {s}");
        assert!(
            s.contains("<!-- sy:tree depth=1"),
            "expected trailing tree summary; got:\n{s}"
        );
    }

    // Render check: depth=1 with descendants → truncated=true marker.
    #[test]
    fn render_agent_md_marks_truncated_when_partial() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::N(1),
        );
        let s = render_agent_md(&tree, Depth::N(1));
        assert!(
            s.contains("truncated=true"),
            "expected truncated=true; got:\n{s}"
        );
    }

    // Render check: depth=all on the same data → truncated=false.
    #[test]
    fn render_agent_md_marks_not_truncated_when_full() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::All,
        );
        let s = render_agent_md(&tree, Depth::All);
        assert!(
            s.contains("truncated=false"),
            "expected truncated=false; got:\n{s}"
        );
        assert!(s.contains("<!-- sy:tree depth=all"), "got: {s}");
    }

    // Render check: when truncated=true and depth=N, footer explains the depth
    // limit and suggests --depth all as remedy.
    #[test]
    fn render_agent_md_truncated_depth_limit_message() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::N(1),
        );
        let s = render_agent_md(&tree, Depth::N(1));
        assert!(
            s.contains("truncated=true (depth limit: 1, use --depth all to see full tree)"),
            "expected actionable depth-limit message; got:\n{s}"
        );
    }

    // Render check: depth=all but truncated=true (defensive path; tree was
    // assembled with a depth budget so nodes are missing even though the
    // renderer was told "all").  Footer should not suggest --depth all
    // since it is already in use.
    #[test]
    fn render_agent_md_truncated_all_no_depth_hint() {
        let rows = nested_rows();
        let tree = assemble(
            &rows,
            &nb(),
            "Inbox",
            Some("/20260501090000-doc0001.sy"),
            Depth::N(1),
        );
        // Render with Depth::All even though the tree is truncated.
        let s = render_agent_md(&tree, Depth::All);
        assert!(
            s.contains("truncated=true (node limit reached)"),
            "expected defensive truncated message; got:\n{s}"
        );
    }
}
