use anyhow::{Result, bail};
use serde_json::Value;

use siyuan_client::SiyuanClient;
use siyuan_model::sql_guard;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct SqlInput {
    pub stmt: String,
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
pub async fn raw(client: &SiyuanClient, input: SqlInput) -> Result<Vec<Value>> {
    if let Err(e) = sql_guard::validate_read_only(&input.stmt) {
        bail!("{e}");
    }
    let rows = client.sql(&input.stmt).await?;
    Ok(rows)
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
    }
}
