use serde::{Deserialize, Serialize};

use siyuan_types::SiyuanError;

use crate::SiyuanClient;
use crate::response::SiyuanResponse;

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
        let env = self
            .post_envelope::<_, Vec<serde_json::Value>>("/api/query/sql", &SqlReq { stmt })
            .await?;
        sql_envelope_into_rows(env)
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

/// Convert a raw `/api/query/sql` envelope into rows.
///
/// `into_result()` rejects `code == 0, data == null`, but the kernel
/// can return `data: null` for queries that produce no rows (e.g. a
/// LIKE that matches nothing). This helper treats `data: null` as an
/// empty `Vec`, and preserves all other behaviour: `SqlUnavailable` for
/// read-only mode rejection and `Api` for non-zero codes.
fn sql_envelope_into_rows(
    env: SiyuanResponse<Vec<serde_json::Value>>,
) -> Result<Vec<serde_json::Value>, SiyuanError> {
    if env.code == 0 {
        Ok(env.data.unwrap_or_default())
    } else if is_read_only_message(&env.msg) {
        Err(SiyuanError::SqlUnavailable)
    } else {
        Err(SiyuanError::Api {
            code: env.code,
            msg: env.msg,
        })
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

    // -- sql_envelope_into_rows tests ------------------------------------

    #[test]
    fn null_data_treated_as_empty_vec() {
        let raw = r#"{"code":0,"msg":"","data":null}"#;
        let env: SiyuanResponse<Vec<serde_json::Value>> = serde_json::from_str(raw).unwrap();
        let rows = sql_envelope_into_rows(env).unwrap();
        assert!(rows.is_empty(), "null data must be treated as empty vec");
    }

    #[test]
    fn empty_array_returned_as_empty_vec() {
        let raw = r#"{"code":0,"msg":"","data":[]}"#;
        let env: SiyuanResponse<Vec<serde_json::Value>> = serde_json::from_str(raw).unwrap();
        let rows = sql_envelope_into_rows(env).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn populated_data_returned_as_is() {
        let raw = r#"{"code":0,"msg":"","data":[{"id":"x","type":"p"}]}"#;
        let env: SiyuanResponse<Vec<serde_json::Value>> = serde_json::from_str(raw).unwrap();
        let rows = sql_envelope_into_rows(env).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "x");
        assert_eq!(rows[0]["type"], "p");
    }

    #[test]
    fn read_only_message_converted_to_sql_unavailable() {
        let raw =
            r#"{"code":21,"msg":"This operation is not supported in read-only mode","data":null}"#;
        let env: SiyuanResponse<Vec<serde_json::Value>> = serde_json::from_str(raw).unwrap();
        let err = sql_envelope_into_rows(env).unwrap_err();
        assert!(
            matches!(err, SiyuanError::SqlUnavailable),
            "read-only message must become SqlUnavailable; got {err:?}"
        );
    }

    #[test]
    fn other_api_error_preserved() {
        let raw = r#"{"code":42,"msg":"something went wrong","data":null}"#;
        let env: SiyuanResponse<Vec<serde_json::Value>> = serde_json::from_str(raw).unwrap();
        let err = sql_envelope_into_rows(env).unwrap_err();
        match err {
            SiyuanError::Api { code, msg } => {
                assert_eq!(code, 42);
                assert_eq!(msg, "something went wrong");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }
}
