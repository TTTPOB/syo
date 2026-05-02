use serde::{Deserialize, Serialize};

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct SqlReq<'a> {
    stmt: &'a str,
}

impl SiyuanClient {
    /// `/api/query/sql` — read-only SQL. Returns rows as JSON objects.
    ///
    /// Note: in read-only / publish mode the kernel disables mutating and
    /// (effectively) the SQL endpoint via Lang(34). The English message is
    /// "This operation is not supported in read-only mode" and the zh_CN
    /// variant uses the literal "只读". When detected, this method surfaces
    /// `SiyuanError::SqlUnavailable` so callers can react with a typed error.
    pub async fn sql(&self, stmt: &str) -> Result<Vec<serde_json::Value>, SiyuanError> {
        match self
            .post::<_, Vec<serde_json::Value>>("/api/query/sql", &SqlReq { stmt })
            .await
        {
            Ok(rows) => Ok(rows),
            Err(SiyuanError::Api { code, msg }) if is_read_only_message(&msg) => {
                let _ = code;
                Err(SiyuanError::SqlUnavailable)
            }
            Err(e) => Err(e),
        }
    }

    /// Typed convenience: deserialise rows into `T`.
    pub async fn sql_typed<T: for<'de> Deserialize<'de>>(
        &self,
        stmt: &str,
    ) -> Result<Vec<T>, SiyuanError> {
        let rows = self.sql(stmt).await?;
        rows.into_iter()
            .map(|v| serde_json::from_value::<T>(v).map_err(|e| SiyuanError::Parse(e.to_string())))
            .collect()
    }
}

/// Recognise the kernel's read-only / publish-mode rejection message.
///
/// SiYuan signals SQL-disabled (and other mutating-call rejections in
/// publish mode) via Lang(34). The English text is "This operation is
/// not supported in read-only mode"; the zh_CN variant contains the
/// literal "只读". We accept both spellings of "read only" (with or
/// without a hyphen) for resilience against minor wording changes.
fn is_read_only_message(msg: &str) -> bool {
    let lowered = msg.to_lowercase();
    lowered.contains("read-only") || lowered.contains("read only") || msg.contains("只读")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_english_message_detected() {
        // Lang(34) English text emitted by the kernel.
        assert!(is_read_only_message(
            "This operation is not supported in read-only mode"
        ));
    }

    #[test]
    fn read_only_zh_cn_message_detected() {
        // zh_CN Lang(34) variant; literal "只读" must match without lowercasing.
        assert!(is_read_only_message("此操作在只读模式下不被支持"));
    }

    #[test]
    fn read_only_alt_spelling_detected() {
        // Defensive against the un-hyphenated variant.
        assert!(is_read_only_message(
            "operation not supported in read only mode"
        ));
    }

    #[test]
    fn unrelated_error_not_detected() {
        assert!(!is_read_only_message("internal server error"));
    }
}
