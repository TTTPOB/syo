use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use clap::Args;

use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;

/// Set one or more attributes on a block (partial update).
///
/// Sibling commands: `syo doc set-icon` and `syo doc set-sort` are
/// thin wrappers that set the `icon` / `sort` attribute respectively;
/// reach for them when that is all you need. There is no `get-attrs` CLI —
/// to read existing attributes use `syo block get --format json` (its
/// JSON output includes `attrs`).
///
/// Inputs:
///   --id (required): block id whose attributes to mutate.
///   --attr (repeatable): one `key=value` pair per occurrence. The flag
///     name is `--attr` (singular), supplied multiple times. Custom keys
///     MUST start with `custom-`. Setting a value to the empty string
///     deletes the key. Setting internal keys like `id` or `type` is
///     silently ignored by the kernel. Keys not listed in this call are
///     left untouched (partial update).
///
/// Prints `ok` on success.
///
/// SiYuan indexes mutations asynchronously; SQL-based reads (syo sql,
/// syo search text, syo tag search) may show stale data for ~100-500 ms
/// after this call. The kernel is immediately consistent — only the SQL
/// index lags.
///
/// Example:
///   in:  --id 20260501090000-blk0001 --attr custom-priority=high --attr custom-owner=alice
///   out: ok
#[derive(Args, Debug)]
#[command(verbatim_doc_comment)]
pub struct SetAttrsArgs {
    /// Block id whose attributes to mutate.
    #[arg(long)]
    pub id: String,

    /// Repeated `key=value` pairs. Custom attrs must be `custom-...`.
    /// Empty value deletes the key; unlisted keys are preserved.
    #[arg(long = "attr", value_name = "KEY=VALUE")]
    pub attrs: Vec<String>,
}

pub async fn run(client: &SiyuanClient, args: SetAttrsArgs) -> Result<()> {
    let id = BlockId::parse(&args.id).context("--id")?;
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for raw in &args.attrs {
        let (k, v) = raw
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("bad --attr {raw:?}; want KEY=VALUE"))?;
        if k.is_empty() {
            bail!("attr key may not be empty");
        }
        map.insert(k.into(), v.into());
    }
    client.set_block_attrs(&id, &map).await?;
    println!("ok");
    Ok(())
}
