use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_types::NotebookId;

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
    /// Example:
    ///   out: open    20260501000000-nb00001    Inbox
    ///        closed  20250812000000-archived   Archive
    #[command(verbatim_doc_comment)]
    Ls,
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
pub struct IdArgs {
    /// Notebook id (from `siyuan notebook ls`).
    #[arg(long)]
    pub id: String,
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
        NotebookCmd::Ls => {
            let nbs = client.ls_notebooks().await?;
            for nb in nbs {
                let status = if nb.closed { "closed" } else { "open  " };
                println!("{}\t{}\t{}", status, nb.id, nb.name);
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
