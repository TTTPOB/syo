use anyhow::{Result, bail};
use serde_json::Value;
use sqlparser::ast::Statement;
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

use siyuan_client::SiyuanClient;
use siyuan_model::sql_guard;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct SqlInput {
    pub stmt: String,
}

#[derive(Debug)]
pub struct SqlOutput {
    pub rows: Vec<Value>,
    pub limit: usize,
    pub has_more: bool,
    pub probe_applied: bool,
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Execute a raw read-only SQL statement against the SiYuan SQLite kernel.
///
/// The statement is validated via [`sql_guard::validate_read_only`] before
/// any kernel round trip — only single SELECT / WITH / VALUES / EXPLAIN
/// statements are accepted.
///
/// Returns rows as `serde_json::Value` objects, one per row.
pub async fn raw(client: &SiyuanClient, input: SqlInput) -> Result<SqlOutput> {
    if let Err(e) = sql_guard::validate_read_only(&input.stmt) {
        bail!("{e}");
    }
    let limit = 64;
    let probe_limit = limit + 1;
    let probe_stmt = probe_stmt_if_unlimited(&input.stmt, probe_limit)?;
    let probe_applied = probe_stmt.is_some();
    let stmt = probe_stmt.as_deref().unwrap_or(&input.stmt);

    let mut rows = client.sql(stmt).await?;
    let has_more = probe_applied && rows.len() > limit;
    if has_more {
        rows.truncate(limit);
    }

    Ok(SqlOutput {
        rows,
        limit,
        has_more,
        probe_applied,
    })
}

fn probe_stmt_if_unlimited(stmt: &str, limit: usize) -> Result<Option<String>> {
    let dialect = SQLiteDialect {};
    let statements = Parser::parse_sql(&dialect, stmt)?;
    let Some(statement) = statements.into_iter().next() else {
        return Ok(None);
    };
    match statement {
        Statement::Query(query) if query.limit_clause.is_none() && query.fetch.is_none() => {
            Ok(Some(format!(
                "SELECT * FROM ({query}) AS syo_raw_sql_probe LIMIT {limit}"
            )))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structs_derive_debug() {
        fn _assert_debug<T: std::fmt::Debug>(_t: &T) {}

        let si = SqlInput {
            stmt: "SELECT 1".into(),
        };
        _assert_debug(&si);
        let so = SqlOutput {
            rows: vec![],
            limit: 64,
            has_more: false,
            probe_applied: true,
        };
        _assert_debug(&so);
    }

    #[test]
    fn probe_wraps_unlimited_select() {
        let stmt = probe_stmt_if_unlimited("SELECT id FROM blocks ORDER BY id", 65)
            .unwrap()
            .expect("unlimited SELECT should be wrapped");
        assert!(stmt.starts_with("SELECT * FROM (SELECT id FROM blocks"));
        assert!(stmt.ends_with("LIMIT 65"));
    }

    #[test]
    fn probe_skips_explicit_limit() {
        assert!(
            probe_stmt_if_unlimited("SELECT id FROM blocks LIMIT 5", 65)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn probe_skips_explain() {
        assert!(
            probe_stmt_if_unlimited("EXPLAIN SELECT id FROM blocks", 65)
                .unwrap()
                .is_none()
        );
    }
}
