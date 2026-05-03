use anyhow::Result;
use clap::Args as ClapArgs;

use siyuan_client::SiyuanClient;
use siyuan_model::tag::list_tags;

use crate::output::OutputFormat;

#[derive(ClapArgs, Debug)]
pub struct Args {
    /// Output format: `agent-md` (default; one tag per line), `json`, or
    /// `json-pretty`.
    #[arg(long, value_enum, default_value_t = OutputFormat::AgentMd)]
    pub format: OutputFormat,
}

pub async fn run(client: &SiyuanClient, args: Args) -> Result<()> {
    let tags = list_tags(client).await?;
    match args.format {
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
    Ok(())
}
