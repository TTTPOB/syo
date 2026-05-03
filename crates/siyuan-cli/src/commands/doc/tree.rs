use anyhow::{Context, Result, anyhow};
use clap::{ArgGroup, Args};

use siyuan_client::SiyuanClient;
use siyuan_model::doc_meta::DocLookup;
use siyuan_model::doc_tree::{Depth, build_tree, render_agent_md as render_tree_md};
use siyuan_types::{BlockId, NotebookId};

use crate::output::OutputFormat;

/// Arguments for `syo doc tree`.
///
/// Same id-XOR-(notebook+hpath) shape as `doc resolve`. `--hpath` defaults
/// to `/` when in `--notebook` mode (virtual-root behaviour). `--depth`
/// accepts an integer >= 1 or the literal string `all`.
#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("tree_lookup")
        .args(["id", "notebook"])
        .required(true)
))]
pub struct TreeArgs {
    /// Document block id. Tree root is this doc; output includes it plus
    /// `--depth` levels of descendants.
    #[arg(long, conflicts_with_all = ["notebook", "hpath"])]
    pub id: Option<String>,

    /// Notebook id. With `--hpath /` (the default in this mode) returns
    /// the notebook's top-level docs under a virtual root; with a non-`/`
    /// hpath anchors the tree at that doc.
    #[arg(long)]
    pub notebook: Option<String>,

    /// Human path inside the notebook. Defaults to `/` (virtual-root
    /// notebook listing). Required-by-association: must be supplied with
    /// `--notebook`.
    #[arg(long, requires = "notebook", default_value = "/")]
    pub hpath: String,

    /// Depth budget: integer >= 1, or the literal `all`. Default 1.
    #[arg(long, default_value = "1", value_parser = parse_depth_arg)]
    pub depth: DepthArg,

    /// Output format: `agent-md` (default; indented bullet list with
    /// `<!-- sy:doc id=... -->` markers), `json` (compact), or
    /// `json-pretty` (indented).
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

/// Wrapper around [`Depth`] for clap value-parser ergonomics.
#[derive(Debug, Clone, Copy)]
pub struct DepthArg(pub Depth);

/// Custom parser for `--depth`. Accepts `all` (case-insensitive) or a
/// non-zero positive integer.
fn parse_depth_arg(s: &str) -> Result<DepthArg, String> {
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(DepthArg(Depth::All));
    }
    let n: u32 = trimmed
        .parse()
        .map_err(|e| format!("--depth must be a positive integer or 'all': {e}"))?;
    if n == 0 {
        return Err("--depth 0 is not allowed; use 1 or higher (or 'all')".to_string());
    }
    Ok(DepthArg(Depth::N(n)))
}

pub async fn run(client: &SiyuanClient, args: TreeArgs) -> Result<()> {
    let lookup = build_tree_lookup(args.id.as_deref(), args.notebook.as_deref(), &args.hpath)?;
    let depth = args.depth.0;
    let tree = build_tree(client, lookup, depth).await?;
    let s = match args.format {
        OutputFormat::AgentMd => render_tree_md(&tree, depth),
        OutputFormat::Json => serde_json::to_string(&tree)?,
        OutputFormat::JsonPretty => serde_json::to_string_pretty(&tree)?,
    };
    // render_tree_md already terminates with a newline; the JSON branches do
    // not, so add one here for parity with the rest of the CLI output.
    print!("{s}");
    if !s.ends_with('\n') {
        println!();
    }
    Ok(())
}

/// Build a `DocLookup` for `doc tree`.
fn build_tree_lookup(id: Option<&str>, notebook: Option<&str>, hpath: &str) -> Result<DocLookup> {
    match (id, notebook) {
        (Some(id), None) => Ok(DocLookup::ById(BlockId::parse(id.trim()).context("--id")?)),
        (None, Some(nb)) => Ok(DocLookup::ByHpath {
            notebook: NotebookId::parse(nb.trim()).context("--notebook")?,
            hpath: hpath.to_string(),
        }),
        (Some(_), Some(_)) => Err(anyhow!(
            "--id conflicts with --notebook; pick exactly one input mode"
        )),
        (None, None) => Err(anyhow!(
            "provide either --id, or --notebook (with optional --hpath)"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use siyuan_model::doc_tree::Depth;

    // Locks the contract: `--depth all` (any case) yields Depth::All.
    #[test]
    fn parse_depth_arg_accepts_all_case_insensitive() {
        assert!(matches!(parse_depth_arg("all").unwrap().0, Depth::All));
        assert!(matches!(parse_depth_arg("ALL").unwrap().0, Depth::All));
        assert!(matches!(parse_depth_arg("All").unwrap().0, Depth::All));
    }

    #[test]
    fn parse_depth_arg_accepts_positive_integer() {
        match parse_depth_arg("3").unwrap().0 {
            Depth::N(n) => assert_eq!(n, 3),
            Depth::All => panic!("expected Depth::N"),
        }
    }

    #[test]
    fn parse_depth_arg_rejects_zero() {
        let err = parse_depth_arg("0").expect_err("0 must be rejected");
        assert!(
            err.contains("0 is not allowed"),
            "expected friendly error; got: {err}"
        );
    }

    #[test]
    fn parse_depth_arg_rejects_negative_or_garbage() {
        assert!(parse_depth_arg("-1").is_err());
        assert!(parse_depth_arg("everything").is_err());
        assert!(parse_depth_arg("").is_err());
    }
}
