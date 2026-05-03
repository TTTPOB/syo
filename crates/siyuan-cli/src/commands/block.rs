use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

use super::delete_block::DeleteBlockArgs;
use super::get_block::GetBlockArgs;
use super::insert_blocks::InsertBlocksArgs;
use super::move_block::MoveBlockArgs;
use super::update_block::UpdateBlockArgs;

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
        BlockCmd::Get(a) => super::get_block::run(client, a).await,
        BlockCmd::Update(a) => super::update_block::run(client, a).await,
        BlockCmd::Insert(a) => super::insert_blocks::run(client, a).await,
        BlockCmd::Move(a) => super::move_block::run(client, a).await,
        BlockCmd::Delete(a) => super::delete_block::run(client, a).await,
    }
}
