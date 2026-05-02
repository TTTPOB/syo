use anyhow::Result;
use clap::{Args, Subcommand};
use serde::Serialize;

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient};
use siyuan_model::tag::{list_tags, search_by_tag};

use crate::output::OutputFormat;

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
    Ls(LsArgs),
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
    Search(SearchArgs),
}

#[derive(Args, Debug)]
pub struct LsArgs {
    /// Output format: `agent-md` (default; one tag per line), `json`, or
    /// `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Tag content WITHOUT the leading `#` (e.g. `project`, not `#project`).
    #[arg(long)]
    pub tag: String,

    /// Maximum hits returned. Default 50, capped by `MAX_SEARCH_LIMIT`.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    /// Output format: `agent-md` (default; TSV `block_id\tmarkdown_preview`),
    /// `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

/// Serializable view of a tag-search hit for `tag search --format json`.
#[derive(Debug, Serialize)]
struct TagSearchView {
    block_id: String,
    markdown_preview: String,
}

pub async fn run(client: &SiyuanClient, cmd: TagCmd) -> Result<()> {
    match cmd {
        TagCmd::Ls(a) => {
            let tags = list_tags(client).await?;
            match a.format {
                OutputFormat::AgentMd => {
                    for t in tags {
                        println!("{t}");
                    }
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&tags)?);
                }
                OutputFormat::JsonPretty => {
                    println!("{}", serde_json::to_string_pretty(&tags)?);
                }
            }
        }
        TagCmd::Search(a) => {
            // Mirror `search blocks`/`search text`: usize-typed CLI flag
            // capped at MAX_SEARCH_LIMIT so a pathological caller cannot
            // ask the kernel for an unbounded result set. `limit == 0` is
            // intentionally NOT promoted to 1 — the model layer rejects it
            // with a typed bail!, and surfacing that error makes misuse
            // visible instead of papering over it.
            let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
            let limit = a.limit.min(limit_cap);
            let hits = search_by_tag(client, &a.tag, limit).await?;
            match a.format {
                OutputFormat::AgentMd => {
                    for hit in hits {
                        println!("{}\t{}", hit.block_id, hit.markdown_preview);
                    }
                }
                OutputFormat::Json | OutputFormat::JsonPretty => {
                    let views: Vec<TagSearchView> = hits
                        .into_iter()
                        .map(|h| TagSearchView {
                            block_id: h.block_id.to_string(),
                            markdown_preview: h.markdown_preview,
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
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct TagSearchViewOwned {
        block_id: String,
        markdown_preview: String,
    }

    #[test]
    fn tag_search_view_round_trips_through_json() {
        let view = TagSearchView {
            block_id: "20260501090000-blk0001".to_string(),
            markdown_preview: "Plan kickoff #project".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        let parsed: TagSearchViewOwned = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed,
            TagSearchViewOwned {
                block_id: "20260501090000-blk0001".to_string(),
                markdown_preview: "Plan kickoff #project".to_string(),
            }
        );
    }
}
