use anyhow::Result;
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_model::tag::{list_tags, search_by_tag};

#[derive(Subcommand, Debug)]
pub enum TagCmd {
    /// List all distinct tags used anywhere in the workspace.
    ///
    /// Sibling commands: `siyuan tag search` finds blocks tagged with one
    /// specific tag; `siyuan search blocks --type t` lists tag blocks
    /// directly. Output of this command is one tag per line, WITHOUT the
    /// surrounding `#` characters.
    ///
    /// Tag list is derived from the SQL index, so newly-added tags may
    /// take ~100-500 ms to appear (the kernel itself is consistent
    /// immediately).
    ///
    /// Example:
    ///   out: project
    ///        urgent
    ///        idea
    #[command(verbatim_doc_comment)]
    Ls,
    /// Find all blocks that carry a specific tag.
    ///
    /// Sibling commands: `siyuan tag ls` enumerates available tags;
    /// `siyuan search text` does free-text search instead of tag-exact
    /// match.
    ///
    /// Inputs:
    ///   --tag (required): tag content WITHOUT the surrounding `#`
    ///     characters (pass `project` to find blocks tagged `#project`).
    ///     Match is exact on the tag value.
    ///
    /// Output is one block per line: `<block-id>\t<markdown-preview>`.
    /// Results are eventually consistent with the SQL index — freshly
    /// tagged blocks may take ~100-500 ms to appear.
    ///
    /// Example:
    ///   in:  --tag project
    ///   out: 20260501090000-blk0001    Plan kickoff #project
    #[command(verbatim_doc_comment)]
    Search(SearchArgs),
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Tag content WITHOUT the leading `#` (e.g. `project`, not `#project`).
    #[arg(long)]
    pub tag: String,
}

pub async fn run(client: &SiyuanClient, cmd: TagCmd) -> Result<()> {
    match cmd {
        TagCmd::Ls => {
            for t in list_tags(client).await? {
                println!("{t}");
            }
        }
        TagCmd::Search(a) => {
            for hit in search_by_tag(client, &a.tag).await? {
                println!("{}\t{}", hit.block_id, hit.markdown_preview);
            }
        }
    }
    Ok(())
}
