use serde::{Deserialize, Serialize};

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct SqlReq<'a> {
    stmt: &'a str,
}

impl SiyuanClient {
    /// `/api/query/sql` — read-only SQL. Returns rows as JSON objects.
    /// Note: in publish mode the kernel disables this endpoint and returns a
    /// non-zero code; callers should handle `SiyuanError::Api` and surface
    /// `SqlUnavailable` if recognised.
    pub async fn sql(&self, stmt: &str) -> Result<Vec<serde_json::Value>, SiyuanError> {
        match self
            .post::<_, Vec<serde_json::Value>>("/api/query/sql", &SqlReq { stmt })
            .await
        {
            Ok(rows) => Ok(rows),
            Err(SiyuanError::Api { code, msg }) if msg.to_lowercase().contains("publish") => {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_mode_detected_from_message() {
        let err = SiyuanError::Api {
            code: -1,
            msg: "sql is disabled in publish mode".into(),
        };
        // Simulate the match logic
        match err {
            SiyuanError::Api { msg, .. } if msg.to_lowercase().contains("publish") => {}
            _ => panic!("should have matched publish"),
        }
    }

    #[test]
    fn publish_mode_not_matched_for_other_errors() {
        let err = SiyuanError::Api {
            code: 500,
            msg: "internal server error".into(),
        };
        match err {
            SiyuanError::Api { msg, .. } if msg.to_lowercase().contains("publish") => {
                panic!("should not match non-publish message")
            }
            _ => {} // expected
        }
    }
}
