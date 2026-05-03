use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

pub mod create;
pub mod ls;
pub mod remove;
pub mod rename;

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
    Ls(ls::Args),
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
    Create(create::Args),
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
    Rename(rename::Args),
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
    Remove(remove::Args),
}

pub async fn run(client: &SiyuanClient, cmd: NotebookCmd) -> Result<()> {
    match cmd {
        NotebookCmd::Ls(a) => ls::run(client, a).await,
        NotebookCmd::Create(a) => create::run(client, a).await,
        NotebookCmd::Rename(a) => rename::run(client, a).await,
        NotebookCmd::Remove(a) => remove::run(client, a).await,
    }
}
