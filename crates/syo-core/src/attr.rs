use anyhow::Result;
use siyuan_client::SiyuanClient;
use siyuan_types::BlockId;
use std::collections::BTreeMap;

// --- get ---
pub struct GetAttrsInput {
    pub id: BlockId,
}

pub struct GetAttrsOutput {
    pub id: BlockId,
    pub attrs: BTreeMap<String, String>,
}

pub async fn get(client: &SiyuanClient, input: GetAttrsInput) -> Result<GetAttrsOutput> {
    let attrs = client.get_block_attrs(&input.id).await?;
    Ok(GetAttrsOutput {
        id: input.id,
        attrs,
    })
}

// --- set ---
pub struct SetAttrsInput {
    pub id: BlockId,
    pub attrs: BTreeMap<String, String>,
}

pub async fn set(client: &SiyuanClient, input: SetAttrsInput) -> Result<()> {
    client.set_block_attrs(&input.id, &input.attrs).await?;
    Ok(())
}

// --- set_icon convenience ---
pub struct SetIconInput {
    pub id: BlockId,
    pub icon: String,
}

pub async fn set_icon(client: &SiyuanClient, input: SetIconInput) -> Result<()> {
    let mut attrs = BTreeMap::new();
    attrs.insert("icon".to_string(), input.icon);
    client.set_block_attrs(&input.id, &attrs).await?;
    Ok(())
}

// --- set_sort convenience ---
pub struct SetSortInput {
    pub id: BlockId,
    pub sort: i64,
}

pub async fn set_sort(client: &SiyuanClient, input: SetSortInput) -> Result<()> {
    let mut attrs = BTreeMap::new();
    attrs.insert("sort".to_string(), input.sort.to_string());
    client.set_block_attrs(&input.id, &attrs).await?;
    Ok(())
}
