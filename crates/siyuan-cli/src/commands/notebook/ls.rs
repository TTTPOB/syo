use anyhow::Result;
use clap::Args as ClapArgs;
use serde::Serialize;

use siyuan_client::SiyuanClient;

use crate::output::OutputFormat;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Output format: `agent-md` (default; TSV `status\tid\tname`),
    /// `json`, or `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
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

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let nbs = client.ls_notebooks().await?;
    match args.format {
        OutputFormat::AgentMd => {
            // Preserve the legacy TSV byte shape, including the two-space
            // padding on `open  ` that aligns it visually with `closed`.
            // Padding is a TSV-formatting concern; the JSON branch emits
            // the unpadded canonical form.
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
            let s = if args.format == OutputFormat::JsonPretty {
                serde_json::to_string_pretty(&views)?
            } else {
                serde_json::to_string(&views)?
            };
            println!("{s}");
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
