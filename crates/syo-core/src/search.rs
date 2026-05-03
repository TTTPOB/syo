use anyhow::{Result, bail};
use serde::Deserialize;

use siyuan_client::{MAX_SEARCH_LIMIT, SiyuanClient, escape_sql_string};
use siyuan_model::sql_guard;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single search result from a fulltext or blocks query.
///
/// Deserialized from SQL rows; the `type` column is remapped to `block_type`
/// so the struct field is idiomatic Rust.
#[derive(Debug, Deserialize)]
pub struct SearchHit {
    pub id: String,
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub markdown: String,
}

#[derive(Debug)]
pub struct FulltextInput {
    pub query: String,
    pub limit: usize,
}

#[derive(Debug)]
pub struct BlocksInput {
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

/// Run a fulltext (LIKE) search against the `markdown` column.
///
/// The query is escaped for SQL string-literal safety. `%` and `_` in the
/// user input behave as LIKE wildcards (the SiYuan SQL engine does not
/// support `ESCAPE '\'`).
pub async fn fulltext(client: &SiyuanClient, input: FulltextInput) -> Result<SearchOutput> {
    let limit_cap: usize = MAX_SEARCH_LIMIT as usize;
    if input.query.trim().is_empty() {
        bail!("--query must not be empty");
    }
    let needle = escape_sql_string(&input.query);
    let limit = input.limit.min(limit_cap);
    let stmt = format!(
        "SELECT id, type, markdown FROM blocks \
         WHERE markdown LIKE '%{needle}%' LIMIT {limit}"
    );
    if let Err(e) = sql_guard::validate_read_only(&stmt) {
        bail!("{e}");
    }
    let hits: Vec<SearchHit> = client.sql_typed(&stmt).await?;
    Ok(SearchOutput { hits })
}

/// Search for blocks by type (`=`) and content (`LIKE`) filter.
///
/// Empty `block_type` and/or `contains` disable the corresponding filter.
/// When both are empty the result is equivalent to `SELECT ... WHERE 1=1`.
pub async fn blocks(client: &SiyuanClient, input: BlocksInput) -> Result<SearchOutput> {
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

        let fi = FulltextInput {
            query: "hello".into(),
            limit: 10,
        };
        _assert_debug(&fi);

        let bi = BlocksInput {
            block_type: "p".into(),
            contains: "hello".into(),
            limit: 10,
        };
        _assert_debug(&bi);

        let so = SearchOutput { hits: vec![] };
        _assert_debug(&so);
    }
}
