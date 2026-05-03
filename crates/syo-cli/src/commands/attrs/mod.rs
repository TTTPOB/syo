use anyhow::Result;
use clap::Subcommand;
use siyuan_client::SiyuanClient;

pub mod get;
pub mod set;

use self::set::SetAttrsArgs;
pub use get::GetAttrsArgs;

#[derive(Subcommand, Debug)]
pub enum AttrsCmd {
    /// Read all attributes of a block as a JSON object.
    Get(GetAttrsArgs),
    /// Set one or more attributes on a block (partial update).
    Set(SetAttrsArgs),
}

pub async fn run(client: &SiyuanClient, cmd: AttrsCmd) -> Result<()> {
    match cmd {
        AttrsCmd::Get(a) => get::run(client, a).await,
        AttrsCmd::Set(a) => set::run(client, a).await,
    }
}
