use anyhow::{Context, Result};
use clap::{Args, Subcommand};

use siyuan_client::SiyuanClient;
use siyuan_types::{BlockId, NotebookId};

#[derive(Subcommand, Debug)]
pub enum DocCmd {
    Resolve(ResolveArgs),
    Rename(RenameArgs),
    Move(MoveArgs),
    SetIcon(IconArgs),
    SetSort(SortArgs),
    Remove(RemoveArgs),
}

#[derive(Args, Debug)]
pub struct ResolveArgs {
    #[arg(long)]
    pub notebook: String,
    #[arg(long)]
    pub hpath: String,
}

#[derive(Args, Debug)]
pub struct RenameArgs {
    #[arg(long)]
    pub notebook: String,
    /// Storage path (e.g. `/20260501090000-abc1234.sy`). Get via `doc resolve` then look up.
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub title: String,
}

#[derive(Args, Debug)]
pub struct MoveArgs {
    #[arg(long, num_args = 1.., value_name = "STORAGE_PATH")]
    pub from_paths: Vec<String>,
    #[arg(long)]
    pub to_notebook: String,
    #[arg(long)]
    pub to_path: String,
}

#[derive(Args, Debug)]
pub struct IconArgs {
    /// Document block id.
    #[arg(long)]
    pub id: String,
    /// Icon name (e.g. emoji shortcode like ":rocket:") or empty to clear.
    #[arg(long, default_value = "")]
    pub icon: String,
}

#[derive(Args, Debug)]
pub struct SortArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub sort: i64,
}

#[derive(Args, Debug)]
pub struct RemoveArgs {
    #[arg(long)]
    pub notebook: String,
    #[arg(long)]
    pub path: String,
}

pub async fn run(client: &SiyuanClient, cmd: DocCmd) -> Result<()> {
    match cmd {
        DocCmd::Resolve(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            let ids = client.get_ids_by_hpath(&nb, &a.hpath).await?;
            for id in ids {
                println!("{id}");
            }
        }
        DocCmd::Rename(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            client.rename_doc(&nb, &a.path, &a.title).await?;
            println!("ok");
        }
        DocCmd::Move(a) => {
            let to_nb = NotebookId::parse(&a.to_notebook).context("--to-notebook")?;
            client.move_docs(&a.from_paths, &to_nb, &a.to_path).await?;
            println!("ok");
        }
        DocCmd::SetIcon(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("icon".to_string(), a.icon);
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::SetSort(a) => {
            let id = BlockId::parse(&a.id).context("--id")?;
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("sort".to_string(), a.sort.to_string());
            client.set_block_attrs(&id, &attrs).await?;
            println!("ok");
        }
        DocCmd::Remove(a) => {
            let nb = NotebookId::parse(&a.notebook).context("--notebook")?;
            client.remove_doc(&nb, &a.path).await?;
            println!("ok");
        }
    }
    Ok(())
}
