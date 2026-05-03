use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_string};
use siyuan_model::sql_guard;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single search result from a type/content block search.
///
/// Deserialized from SQL rows; the `type` column is remapped to `block_type`
/// so the struct field is idiomatic Rust.
#[derive(Debug, Deserialize, Serialize)]
pub struct SearchHit {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub markdown: String,
}

/// Input for [`search()`].
///
/// `block_type` filters by exact block type (e.g. `"h"` for heading).
/// `contains` does a SQL LIKE substring match against the `content` column.
/// When both are empty all blocks are returned up to `limit`.
#[derive(Debug)]
pub struct SearchInput {
    pub block_type: String,
    pub contains: String,
    pub limit: usize,
}

#[derive(Debug)]
pub struct SearchOutput {
    pub hits: Vec<SearchHit>,
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Search for blocks by type (`=`) and content (`LIKE`) filter.
///
/// Empty `block_type` and/or `contains` disable the corresponding filter.
/// When both are empty the result is equivalent to `SELECT ... WHERE 1=1`.
pub async fn search(client: &SiyuanClient, input: SearchInput) -> Result<SearchOutput> {
    let mut conds = Vec::new();
    if !input.block_type.is_empty() {
        conds.push(format!("type = '{}'", input.block_type.replace('\'', "''")));
    }
    if !input.contains.is_empty() {
        conds.push(format!(
            "content LIKE '%{}%'",
            escape_sql_string(&input.contains)
        ));
    }
    let where_clause = if conds.is_empty() {
        "1=1".into()
    } else {
        conds.join(" AND ")
    };
    let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
    let limit = input.limit.min(limit_cap);
    let stmt = format!("SELECT id, type, markdown FROM blocks WHERE {where_clause} LIMIT {limit}");
    if let Err(e) = sql_guard::validate_read_only(&stmt) {
        bail!("{e}");
    }
    let hits: Vec<SearchHit> = client.sql_typed(&stmt).await?;
    Ok(SearchOutput { hits })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_hit_deserializes_renamed_type_field() {
        let json = r#"{"id":"abc","type":"p","markdown":"hello"}"#;
        let hit: SearchHit = serde_json::from_str(json).unwrap();
        assert_eq!(hit.id, "abc");
        assert_eq!(hit.block_type, "p");
        assert_eq!(hit.markdown, "hello");
    }

    #[test]
    fn search_hit_deserializes_with_missing_markdown() {
        let json = r#"{"id":"abc","type":"p"}"#;
        let hit: SearchHit = serde_json::from_str(json).unwrap();
        assert_eq!(hit.markdown, "");
    }

    #[test]
    fn structs_derive_debug() {
        fn _assert_debug<T: std::fmt::Debug>(_t: &T) {}

        let si = SearchInput {
            block_type: "p".into(),
            contains: "hello".into(),
            limit: 10,
        };
        _assert_debug(&si);

        let so = SearchOutput { hits: vec![] };
        _assert_debug(&so);
    }
}
