use anyhow::Result;
use clap::Subcommand;
use siyuan_client::SiyuanClient;

use super::set_attrs::SetAttrsArgs;

#[derive(Subcommand, Debug)]
pub enum AttrsCmd {
    /// Set one or more attributes on a block (partial update).
    Set(SetAttrsArgs),
}

pub async fn run(client: &SiyuanClient, cmd: AttrsCmd) -> Result<()> {
    match cmd {
        AttrsCmd::Set(a) => super::set_attrs::run(client, a).await,
    }
}
