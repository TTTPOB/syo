use anyhow::{Result, bail};

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient};

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use siyuan_model::tag::TagBlockHit;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct ListTagsOutput {
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub struct SearchByTagInput {
    pub tag: String,
    pub limit: usize,
}

#[derive(Debug)]
pub struct SearchByTagOutput {
    pub hits: Vec<TagBlockHit>,
    pub limit: usize,
    pub has_more: bool,
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// List every distinct tag string in the workspace (sorted).
pub async fn list_tags(client: &SiyuanClient) -> Result<ListTagsOutput> {
    let tags = siyuan_model::tag::list_tags(client).await?;
    Ok(ListTagsOutput { tags })
}

/// Find blocks carrying the given tag, returning at most `limit` hits.
///
/// `limit` is capped at `MAX_SEARCH_LIMIT`. A zero limit is rejected by the
/// underlying `siyuan_model::tag::search_by_tag` function.
pub async fn search_by_tag(
    client: &SiyuanClient,
    input: SearchByTagInput,
) -> Result<SearchByTagOutput> {
    if input.limit == 0 {
        bail!("{}", siyuan_model::tag::ZERO_LIMIT_ERR);
    }
    let limit = input.limit.min(MAX_SEARCH_LIMIT as usize);
    let probe_limit = limit.saturating_add(1);
    let mut hits = siyuan_model::tag::search_by_tag(client, &input.tag, probe_limit).await?;
    let has_more = hits.len() > limit;
    hits.truncate(limit);
    Ok(SearchByTagOutput {
        hits,
        limit,
        has_more,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structs_derive_debug() {
        fn _assert_debug<T: std::fmt::Debug>(_t: &T) {}

        let lto = ListTagsOutput {
            tags: vec!["alpha".into(), "beta".into()],
        };
        _assert_debug(&lto);

        let sti = SearchByTagInput {
            tag: "alpha".into(),
            limit: 10,
        };
        _assert_debug(&sti);

        let sto = SearchByTagOutput {
            hits: vec![],
            limit: 10,
            has_more: false,
        };
        _assert_debug(&sto);
    }
}
