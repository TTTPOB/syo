use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::output::OutputFormat;

#[derive(Debug, Deserialize)]
pub(super) struct Hit {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub markdown: String,
}

pub(super) struct HitSet {
    pub rows: Vec<Hit>,
    pub limit: usize,
    pub has_more: bool,
}

/// Serializable view of a search hit for `--format json`.
///
/// Field is named `markdown_preview` (not `markdown`) because the value is
/// passed through `oneline` — newlines are folded and the string is
/// truncated to 80 chars with a horizontal-ellipsis marker, so it is no
/// longer the verbatim markdown column.
#[derive(Debug, Serialize)]
struct HitView {
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    markdown_preview: String,
}

pub(super) fn emit_hits(result: HitSet, format: OutputFormat) -> Result<()> {
    let HitSet {
        rows,
        limit,
        has_more,
    } = result;
    match format {
        OutputFormat::AgentMd => {
            for r in rows {
                println!("{}\t{}\t{}", r.id, r.block_type, oneline(&r.markdown));
            }
            if has_more {
                eprintln!(
                    "Hint: more search hits exist. Re-run with --limit greater than {limit}."
                );
            }
        }
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let views: Vec<HitView> = rows
                .into_iter()
                .map(|r| HitView {
                    id: r.id,
                    block_type: r.block_type,
                    markdown_preview: oneline(&r.markdown),
                })
                .collect();
            let s = if format == OutputFormat::JsonPretty {
                serde_json::to_string_pretty(&serde_json::json!({
                    "hits": views,
                    "limit": limit,
                    "has_more": has_more,
                }))?
            } else {
                serde_json::to_string(&serde_json::json!({
                    "hits": views,
                    "limit": limit,
                    "has_more": has_more,
                }))?
            };
            println!("{s}");
        }
    }
    Ok(())
}

fn oneline(s: &str) -> String {
    let one = s.replace('\n', " ");
    if one.chars().count() <= 80 {
        one
    } else {
        let truncated: String = one.chars().take(80).collect();
        format!("{truncated}\u{2026}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirror of `HitView` with `Deserialize` so the round-trip test can
    /// parse the JSON we emit. Production `HitView` is `Serialize`-only —
    /// JSON is an output format, not an input format for the CLI.
    #[derive(Debug, Deserialize, PartialEq)]
    struct HitViewOwned {
        id: String,
        #[serde(rename = "type")]
        block_type: String,
        markdown_preview: String,
    }

    #[test]
    fn hit_view_serializes_with_renamed_type_field() {
        let view = HitView {
            id: "20260501090000-blk0001".to_string(),
            block_type: "p".to_string(),
            markdown_preview: "Plan kickoff for Q3".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        // `block_type` must surface as `"type"` in JSON to match the SQL
        // column name and the TSV column name.
        assert!(json.contains("\"type\":\"p\""), "got {json}");
        assert!(json.contains("\"markdown_preview\""), "got {json}");
    }

    #[test]
    fn hit_view_round_trips_through_json() {
        let view = HitView {
            id: "20260501090000-blk0001".to_string(),
            block_type: "h".to_string(),
            markdown_preview: "# Plan".to_string(),
        };
        let json = serde_json::to_string(&view).unwrap();
        let parsed: HitViewOwned = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed,
            HitViewOwned {
                id: "20260501090000-blk0001".to_string(),
                block_type: "h".to_string(),
                markdown_preview: "# Plan".to_string(),
            }
        );
    }
}
