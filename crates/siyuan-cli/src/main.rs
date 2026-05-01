mod commands;
mod config;
mod output;

use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "siyuan", version, about = "Agent harness for SiYuan")]
struct Cli {
    /// Base URL of the SiYuan kernel HTTP API.
    #[arg(long, env = "SIYUAN_BASE_URL", global = true)]
    base_url: Option<String>,

    /// API token (Authorization: Token <value>).
    #[arg(long, env = "SIYUAN_TOKEN", global = true)]
    token: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print kernel version (smoke test).
    Status,
    GetDoc(commands::get_doc::GetDocArgs),
    GetBlock(commands::get_block::GetBlockArgs),
    CreateDoc(commands::create_doc::CreateDocArgs),
    UpdateBlock(commands::update_block::UpdateBlockArgs),
    InsertBlocks(commands::insert_blocks::InsertBlocksArgs),
    MoveBlock(commands::move_block::MoveBlockArgs),
    DeleteBlock(commands::delete_block::DeleteBlockArgs),
    SetAttrs(commands::set_attrs::SetAttrsArgs),
    Notebook {
        #[command(subcommand)]
        cmd: commands::notebook::NotebookCmd,
    },
    Doc {
        #[command(subcommand)]
        cmd: commands::doc::DocCmd,
    },
    Tag {
        #[command(subcommand)]
        cmd: commands::tag::TagCmd,
    },
    Asset {
        #[command(subcommand)]
        cmd: commands::asset::AssetCmd,
    },
    Graph {
        #[command(subcommand)]
        cmd: commands::graph::GraphCmd,
    },
    Search {
        #[command(subcommand)]
        cmd: commands::search::SearchCmd,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let cfg = config::Config::resolve(cli.base_url, cli.token)?;
    let client = cfg.into_client()?;

    match cli.cmd {
        Cmd::Status => {
            let v = client.system_version().await?;
            info!(%v, "siyuan ok");
            println!("{v}");
        }
        Cmd::GetDoc(a) => commands::get_doc::run(&client, a).await?,
        Cmd::GetBlock(a) => commands::get_block::run(&client, a).await?,
        Cmd::CreateDoc(a) => commands::create_doc::run(&client, a).await?,
        Cmd::UpdateBlock(a) => commands::update_block::run(&client, a).await?,
        Cmd::InsertBlocks(a) => commands::insert_blocks::run(&client, a).await?,
        Cmd::MoveBlock(a) => commands::move_block::run(&client, a).await?,
        Cmd::DeleteBlock(a) => commands::delete_block::run(&client, a).await?,
        Cmd::SetAttrs(a) => commands::set_attrs::run(&client, a).await?,
        Cmd::Notebook { cmd } => commands::notebook::run(&client, cmd).await?,
        Cmd::Doc { cmd } => commands::doc::run(&client, cmd).await?,
        Cmd::Tag { cmd } => commands::tag::run(&client, cmd).await?,
        Cmd::Asset { cmd } => commands::asset::run(&client, cmd).await?,
        Cmd::Graph { cmd } => commands::graph::run(&client, cmd).await?,
        Cmd::Search { cmd } => commands::search::run(&client, cmd).await?,
    }
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
}
