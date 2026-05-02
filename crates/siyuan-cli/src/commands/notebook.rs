use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

use crate::output::OutputFormat;

#[derive(Subcommand, Debug)]
pub enum NotebookCmd {
    /// List all notebooks (open AND closed) with status, id and name.
    ///
    /// Output is one notebook per line, three tab-separated columns:
    /// status (`open  ` or `closed`), notebook id, name. Closed notebooks
    /// are listed too — they cannot be reopened from this CLI (open/close
    /// is a UI-only action), but their ids are still useful for
    /// `siyuan doc resolve` queries against the closed corpus, which may
    /// or may not return data depending on kernel version.
    ///
    /// Sibling commands: `siyuan doc resolve` looks up a single document
    /// by id or hpath; this command enumerates whole notebooks.
    ///
    /// Inputs:
    ///   --format (default agent-md): one of `agent-md` (the TSV form
    ///     described above), `json` (compact JSON array of
    ///     `{status, id, name}`; status is `"open"` or `"closed"` without
    ///     padding), or `json-pretty` (the same shape, indented).
    ///
    /// Example:
    ///   out: open    20260501000000-nb00001    Inbox
    ///        closed  20250812000000-archived   Archive
    #[command(verbatim_doc_comment)]
    Ls(LsArgs),
    /// Create a new notebook with the given display name.
    ///
    /// Sibling commands: `siyuan notebook rename` only changes the name
    /// of an existing notebook. There is no programmatic open/close —
    /// the user opens or closes notebooks in the SiYuan UI.
    ///
    /// Inputs:
    ///   --name (required): display name. Any non-empty UTF-8 string;
    ///     duplicates are allowed (the kernel disambiguates by id).
    ///
    /// Prints the new notebook id to stdout.
    ///
    /// Some kernel versions create the notebook in a CLOSED state. The
    /// id is usable immediately for `siyuan doc resolve` (which queries
    /// the kernel directly), but reads via `siyuan sql` /
    /// `siyuan search text` may return empty until the user opens the
    /// notebook in the SiYuan UI.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --name Inbox
    ///   out: 20260501000000-nb00001
    #[command(verbatim_doc_comment)]
    Create(NameArgs),
    /// Rename an existing notebook.
    ///
    /// Sibling commands: `siyuan notebook create` mints a new notebook;
    /// `siyuan notebook remove` destroys one and all its documents.
    /// `siyuan notebook rename` changes the display name only — the
    /// on-disk folder and the notebook id remain stable, so storage
    /// paths inside it are unaffected.
    ///
    /// The on-disk folder is NOT renamed; only the display name changes.
    /// Storage paths and the notebook id remain stable.
    ///
    /// Inputs:
    ///   --id (required): notebook id from `siyuan notebook ls`.
    ///   --name (required): new display name.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501000000-nb00001 --name Triage
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Rename(RenameArgs),
    /// Permanently remove a notebook AND every document it contains.
    ///
    /// Sibling commands: `siyuan doc remove` removes a single document
    /// by storage path; this destroys the whole notebook and is
    /// irreversible. Verify the notebook id from `siyuan notebook ls`
    /// before calling.
    ///
    /// Inputs:
    ///   --id (required): notebook id.
    ///
    /// Prints `ok` on success.
    ///
    /// SiYuan indexes mutations asynchronously; SQL-based reads
    /// (siyuan sql, siyuan search text, siyuan tag search) may show stale
    /// data for ~100-500 ms after this call. The kernel is immediately
    /// consistent — only the SQL index lags.
    ///
    /// Example:
    ///   in:  --id 20260501000000-nb00001
    ///   out: ok
    #[command(verbatim_doc_comment)]
    Remove(IdArgs),
}

#[derive(Args, Debug)]
pub struct LsArgs {
    /// Output format: `agent-md` (default; TSV `status\tid\tname`),
    /// `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct IdArgs {
    /// Notebook id (from `siyuan notebook ls`).
    #[arg(long)]
    pub id: String,
}

/// Serializable view of a notebook entry for `notebook ls --format json`.
///
/// The `status` field is the unpadded canonical form (`"open"` /
/// `"closed"`); the TSV branch keeps the legacy padded `"open  "` for
/// byte-identical column alignment.
#[derive(Debug, Serialize)]
struct NotebookView<'a> {
    status: &'a str,
    id: String,
    name: String,
}

#[derive(Args, Debug)]
pub struct NameArgs {
    /// Display name for the new notebook.
    #[arg(long)]
    pub name: String,
}

#[derive(Args, Debug)]
pub struct RenameArgs {
    /// Notebook id to rename.
    #[arg(long)]
    pub id: String,
    /// New display name.
    #[arg(long)]
    pub name: String,
}

pub async fn run(client: &SiyuanClient, cmd: NotebookCmd) -> Result<()> {
    match cmd {
        NotebookCmd::Ls(a) => {
            let nbs = client.ls_notebooks().await?;
            match a.format {
                OutputFormat::AgentMd => {
                    // Preserve the legacy TSV byte shape, including the
                    // two-space padding on `open  ` that aligns it visually
                    // with `closed`. Padding is a TSV-formatting concern;
                    // the JSON branch emits the unpadded canonical form.
                    for nb in nbs {
                        let status = if nb.closed { "closed" } else { "open  " };
                        println!("{}\t{}\t{}", status, nb.id, nb.name);
                    }
                }
                OutputFormat::Json | OutputFormat::JsonPretty => {
                    let views: Vec<NotebookView<'_>> = nbs
                        .iter()
                        .map(|nb| NotebookView {
                            status: if nb.closed { "closed" } else { "open" },
                            id: nb.id.to_string(),
                            name: nb.name.clone(),
                        })
                        .collect();
                    let s = if a.format == OutputFormat::JsonPretty {
                        serde_json::to_string_pretty(&views)?
                    } else {
                        serde_json::to_string(&views)?
                    };
                    println!("{s}");
                }
            }
        }
        NotebookCmd::Create(a) => {
            let nb = client.create_notebook(&a.name).await?;
            println!("{}", nb.id);
        }
        NotebookCmd::Rename(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.rename_notebook(&id, &a.name).await?;
            println!("ok");
        }
        NotebookCmd::Remove(a) => {
            let id = NotebookId::parse(&a.id).context("--id")?;
            client.remove_notebook(&id).await?;
            println!("ok");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    /// Mirror of `NotebookView` with `Deserialize` so the round-trip test can
    /// parse the JSON we emit. The production view is intentionally
    /// `Serialize`-only — JSON is an output, never an input.
    #[derive(Debug, Deserialize, PartialEq)]
    struct NotebookViewOwned {
        status: String,
        id: String,
        name: String,
    }

    #[test]
    fn notebook_view_serializes_open_status_without_padding() {
        let view = NotebookView {
            status: "open",
            id: "20260501000000-nb00001".to_string(),
            name: "Inbox".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        // Status MUST be the canonical "open" string; the TSV's two-space
        // padding is a column-alignment concern that does not belong in JSON.
        assert!(json.contains("\"status\":\"open\""), "got {json}");
        assert!(
            !json.contains("\"open  \""),
            "padding leaked into JSON: {json}"
        );
    }

    #[test]
    fn notebook_view_round_trips_through_json() {
        let view = NotebookView {
            status: "closed",
            id: "20250812000000-archived".to_string(),
            name: "Archive".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        let parsed: NotebookViewOwned = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed,
            NotebookViewOwned {
                status: "closed".to_string(),
                id: "20250812000000-archived".to_string(),
                name: "Archive".to_string(),
            }
        );
    }
}
