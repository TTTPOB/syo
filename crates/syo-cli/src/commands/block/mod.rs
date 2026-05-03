use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

pub mod delete;
pub mod get;
pub mod insert;
pub mod r#move;
pub mod update;

use self::delete::DeleteBlockArgs;
use self::get::GetBlockArgs;
use self::insert::InsertBlocksArgs;
use self::r#move::MoveBlockArgs;
use self::update::UpdateBlockArgs;

#[derive(Subcommand, Debug)]
pub enum BlockCmd {
    /// Fetch the raw kramdown source of a single block plus its attributes.
    Get(GetBlockArgs),
    /// Replace the full markdown content of an existing block.
    Update(UpdateBlockArgs),
    /// Insert a new markdown block (or blocks) at a position relative to an anchor.
    Insert(InsertBlocksArgs),
    /// Move an existing block to a new position within the document tree.
    Move(MoveBlockArgs),
    /// Permanently delete a block and all of its children.
    Delete(DeleteBlockArgs),
}

pub async fn run(client: &SiyuanClient, cmd: BlockCmd) -> Result<()> {
    match cmd {
        BlockCmd::Get(a) => get::run(client, a).await,
        BlockCmd::Update(a) => update::run(client, a).await,
        BlockCmd::Insert(a) => insert::run(client, a).await,
        BlockCmd::Move(a) => r#move::run(client, a).await,
        BlockCmd::Delete(a) => delete::run(client, a).await,
    }
}
