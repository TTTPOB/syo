use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

pub mod ls;
pub mod search;

#[derive(Subcommand, Debug)]
pub enum TagCmd {
    /// List all distinct tags used anywhere in the workspace.
    ///
    /// Sibling commands: `syo tag search` finds blocks tagged with one
    /// specific tag; `syo search blocks --type t` lists tag blocks
    /// directly. Output of this command is one tag per line, WITHOUT the
    /// surrounding `#` characters.
    ///
    /// Tag list is derived from the SQL index, so newly-added tags may
    /// take ~100-500 ms to appear (the kernel itself is consistent
    /// immediately).
    ///
    /// Inputs:
    ///   --format (default agent-md): one of `agent-md` (one tag per line,
    ///     legacy behaviour), `json` (compact JSON array of strings), or
    ///     `json-pretty` (the same array, indented).
    ///
    /// Example:
    ///   out: project
    ///        urgent
    ///        idea
    #[command(verbatim_doc_comment)]
    Ls(ls::Args),
    /// Find all blocks that carry a specific tag.
    ///
    /// Sibling commands: `syo tag ls` enumerates available tags;
    /// `syo search text` does free-text search instead of tag-exact
    /// match.
    ///
    /// Inputs:
    ///   --tag (required): tag content WITHOUT the surrounding `#`
    ///     characters (pass `project` to find blocks tagged `#project`).
    ///     Match is exact on the tag value.
    ///   --limit (optional, default 50): maximum hits, capped by
    ///     `MAX_SEARCH_LIMIT`.
    ///   --format (default agent-md): one of `agent-md` (the TSV form
    ///     described above), `json` (compact JSON array of
    ///     `{block_id, markdown_preview}`), or `json-pretty` (indented).
    ///
    /// Output is one block per line: `<block-id>\t<markdown-preview>`.
    /// Results are eventually consistent with the SQL index — freshly
    /// tagged blocks may take ~100-500 ms to appear.
    ///
    /// Example:
    ///   in:  --tag project --limit 10
    ///   out: 20260501090000-blk0001    Plan kickoff #project
    #[command(verbatim_doc_comment)]
    Search(search::Args),
}

pub async fn run(client: &SiyuanClient, cmd: TagCmd) -> Result<()> {
    match cmd {
        TagCmd::Ls(a) => ls::run(client, a).await,
        TagCmd::Search(a) => search::run(client, a).await,
    }
}
