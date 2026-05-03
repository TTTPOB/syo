mod commands;
mod config;
mod output;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "syo", version, about = "Agent harness for SiYuan")]
struct Cli {
    /// Base URL of the SiYuan kernel HTTP API.
    ///
    /// Falls back to the `SIYUAN_BASE_URL` env var, then to
    /// `http://127.0.0.1:6806` (the kernel's default loopback bind). Format:
    /// scheme://host:port with no trailing slash, e.g. `http://127.0.0.1:6806`
    /// or `https://siyuan.example.com`.
    #[arg(long, env = "SIYUAN_BASE_URL", global = true)]
    base_url: Option<String>,

    /// API token (Authorization: Token <value>).
    ///
    /// Falls back to the `SIYUAN_TOKEN` env var. Required for every
    /// subcommand EXCEPT `serve-mcp`: if neither flag nor env is set, the CLI
    /// errors before dispatching the subcommand. `serve-mcp` tolerates a
    /// missing token (it logs a warning and defers auth failures to per-tool
    /// kernel calls), so the MCP server can boot first and the user can wire
    /// the token in later via env.
    #[arg(long, env = "SIYUAN_TOKEN", global = true)]
    token: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print the SiYuan kernel version and confirm the server is reachable.
    Status,
    /// Read, write, move, and delete individual blocks.
    Block {
        #[command(subcommand)]
        cmd: commands::block::BlockCmd,
    },
    /// Manage notebooks (list, create, rename, remove).
    Notebook {
        #[command(subcommand)]
        cmd: commands::notebook::NotebookCmd,
    },
    /// Manage documents (get, create, resolve, rename, move, set icon/sort, remove, tree).
    Doc {
        #[command(subcommand)]
        cmd: commands::doc::DocCmd,
    },
    /// Manage block attributes (set).
    Attrs {
        #[command(subcommand)]
        cmd: commands::attrs::AttrsCmd,
    },
    /// List or search blocks by tag.
    Tag {
        #[command(subcommand)]
        cmd: commands::tag::TagCmd,
    },
    /// Upload local files as assets and emit markdown references for them.
    Asset {
        #[command(subcommand)]
        cmd: commands::asset::AssetCmd,
    },
    /// Inspect the link graph: backlinks, outgoing refs, neighborhood walks.
    Graph {
        #[command(subcommand)]
        cmd: commands::graph::GraphCmd,
    },
    /// Filter blocks by type and/or content substring.
    Search(commands::search::SearchArgs),
    Sql(commands::sql::SqlArgs),
    ServeMcp(commands::serve_mcp::ServeMcpArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();

    // ServeMcp tolerates a missing token and builds its own client with a
    // configurable timeout; every other subcommand requires a token up front
    // and uses the default client.
    if let Cmd::ServeMcp(args) = cli.cmd {
        let cfg = config::Config::resolve_optional_token(cli.base_url, cli.token);
        return commands::serve_mcp::run(cfg, args).await;
    }

    let cfg = config::Config::resolve(cli.base_url, cli.token)?;
    let client = cfg.into_client()?;

    match cli.cmd {
        Cmd::Status => {
            commands::status::run(&client).await?;
        }
        Cmd::Block { cmd } => commands::block::run(&client, cmd).await?,
        Cmd::Notebook { cmd } => commands::notebook::run(&client, cmd).await?,
        Cmd::Attrs { cmd } => commands::attrs::run(&client, cmd).await?,
        Cmd::Doc { cmd } => commands::doc::run(&client, cmd).await?,
        Cmd::Tag { cmd } => commands::tag::run(&client, cmd).await?,
        Cmd::Asset { cmd } => commands::asset::run(&client, cmd).await?,
        Cmd::Graph { cmd } => commands::graph::run(&client, cmd).await?,
        Cmd::Search(a) => commands::search::run(&client, a).await?,
        Cmd::Sql(a) => commands::sql::run(&client, a).await?,
        Cmd::ServeMcp(_) => unreachable!("serve-mcp dispatched above"),
    }
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    // Always write tracing to stderr: stdout is reserved for user-facing
    // command output (println!) and, under `serve-mcp`, for JSON-RPC framing.
    let _ = fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .try_init();
    // try_init fails only when a global subscriber is already set (e.g., in tests).
}
