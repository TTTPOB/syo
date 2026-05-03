use anyhow::Result;
use clap::Subcommand;
use siyuan_client::SiyuanClient;

pub mod set;

use self::set::SetAttrsArgs;

#[derive(Subcommand, Debug)]
pub enum AttrsCmd {
    /// Set one or more attributes on a block (partial update).
    Set(SetAttrsArgs),
}

pub async fn run(client: &SiyuanClient, cmd: AttrsCmd) -> Result<()> {
    match cmd {
        AttrsCmd::Set(a) => set::run(client, a).await,
    }
}
