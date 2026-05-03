use anyhow::Result;
use clap::Args as ClapArgs;
use serde::Serialize;

use siyuan_client::SiyuanClient;

use crate::output::OutputFormat;

#[derive(ClapArgs, Debug)]
pub struct Args {
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

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let hits = syo_core::tag::search_by_tag(
        client,
        syo_core::tag::SearchByTagInput {
            tag: args.tag,
            limit: args.limit,
        },
    )
    .await?
    .hits;
    println!("{}", format_search_results(&hits, args.format)?);
    Ok(())
}

/// Render tag-search hits into a string for the requested output format.
///
/// Agent-md prints a human-readable message when the result set is empty
/// so the user can tell the command executed successfully (JSON already
/// emits `[]`, which is unambiguous).
fn format_search_results(
    hits: &[syo_core::tag::TagBlockHit],
    format: OutputFormat,
) -> Result<String> {
    match format {
        OutputFormat::AgentMd => {
            if hits.is_empty() {
                return Ok("No blocks found".to_string());
            }
            let lines: Vec<String> = hits
                .iter()
                .map(|h| format!("{}\t{}", h.block_id, h.markdown_preview))
                .collect();
            Ok(lines.join("\n"))
        }
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let views: Vec<TagSearchView> = hits
                .iter()
                .map(|h| TagSearchView {
                    block_id: h.block_id.to_string(),
                    markdown_preview: h.markdown_preview.clone(),
                })
                .collect();
            if format == OutputFormat::JsonPretty {
                Ok(serde_json::to_string_pretty(&views)?)
            } else {
                Ok(serde_json::to_string(&views)?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use syo_core::tag::TagBlockHit;

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

    fn make_hit(id: &str, preview: &str) -> TagBlockHit {
        TagBlockHit {
            block_id: siyuan_types::BlockId::parse(id).expect("valid test block id"),
            root_id: siyuan_types::BlockId::parse("20260501090000-7ab0001").expect("valid root id"),
            markdown_preview: preview.to_string(),
        }
    }

    #[test]
    fn format_empty_agent_md_shows_message() {
        let hits: Vec<TagBlockHit> = vec![];
        let output = format_search_results(&hits, OutputFormat::AgentMd).unwrap();
        assert!(
            output.contains("No blocks found"),
            "empty agent-md must print a user-facing message; got: {output:?}"
        );
        assert!(!output.is_empty());
    }

    #[test]
    fn format_empty_json_emits_empty_array() {
        let hits: Vec<TagBlockHit> = vec![];
        let output = format_search_results(&hits, OutputFormat::Json).unwrap();
        assert_eq!(output, "[]", "empty JSON must be `[]`");
    }

    #[test]
    fn format_empty_json_pretty_emits_empty_array() {
        let hits: Vec<TagBlockHit> = vec![];
        let output = format_search_results(&hits, OutputFormat::JsonPretty).unwrap();
        assert_eq!(output, "[]", "empty json-pretty must be `[]`");
    }

    #[test]
    fn format_non_empty_agent_md_uses_tsv() {
        let hits = vec![
            make_hit("20260501090000-blk0001", "hello world"),
            make_hit("20260501090000-blk0002", "foo bar"),
        ];
        let output = format_search_results(&hits, OutputFormat::AgentMd).unwrap();
        assert!(output.contains("20260501090000-blk0001"));
        assert!(output.contains("hello world"));
        assert!(output.contains("20260501090000-blk0002"));
        assert!(output.contains("foo bar"));
        assert!(output.contains('\t'), "agent-md must be TSV");
        assert_eq!(output.lines().count(), 2);
    }

    #[test]
    fn format_non_empty_json_round_trips() {
        let hits = vec![make_hit("20260501090000-blk0001", "preview")];
        let output = format_search_results(&hits, OutputFormat::Json).unwrap();
        let parsed: Vec<TagSearchViewOwned> = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].block_id, "20260501090000-blk0001");
        assert_eq!(parsed[0].markdown_preview, "preview");
    }
}
