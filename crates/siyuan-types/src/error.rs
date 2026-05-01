use thiserror::Error;

use crate::id::BlockId;

/// Categorised harness error. `kind()` gives a stable enum-shaped value for
/// programmatic handling; `Display` gives the human message.
#[derive(Debug, Error)]
pub enum SiyuanError {
    #[error("HTTP transport error: {0}")]
    Http(String),

    #[error("authentication missing or invalid")]
    Auth,

    #[error("SiYuan API returned code {code}: {msg}")]
    Api { code: i32, msg: String },

    #[error("block not found: {0}")]
    NotFound(String),

    #[error("path is ambiguous: {hpath:?} resolves to multiple ids: {candidates:?}")]
    AmbiguousPath { hpath: String, candidates: Vec<BlockId> },

    #[error("operation {op:?} is not supported on block {id} of type {block_type}")]
    UnsupportedOp { id: BlockId, block_type: String, op: String },

    #[error("SQL query unavailable (publish mode disables /api/query/sql)")]
    SqlUnavailable,

    #[error("graph result exceeded limit ({limit}); refine query")]
    GraphLimit { limit: usize },

    #[error("parse error: {0}")]
    Parse(String),

    #[error("verification after write failed: {0}")]
    VerifyFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Http,
    Auth,
    Api,
    NotFound,
    AmbiguousPath,
    UnsupportedOp,
    SqlUnavailable,
    GraphLimit,
    Parse,
    VerifyFailed,
}

impl SiyuanError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Http(_) => ErrorKind::Http,
            Self::Auth => ErrorKind::Auth,
            Self::Api { .. } => ErrorKind::Api,
            Self::NotFound(_) => ErrorKind::NotFound,
            Self::AmbiguousPath { .. } => ErrorKind::AmbiguousPath,
            Self::UnsupportedOp { .. } => ErrorKind::UnsupportedOp,
            Self::SqlUnavailable => ErrorKind::SqlUnavailable,
            Self::GraphLimit { .. } => ErrorKind::GraphLimit,
            Self::Parse(_) => ErrorKind::Parse,
            Self::VerifyFailed(_) => ErrorKind::VerifyFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_matches_variant() {
        let err = SiyuanError::Auth;
        assert_eq!(err.kind(), ErrorKind::Auth);
    }

    #[test]
    fn api_error_displays_code_and_msg() {
        let err = SiyuanError::Api { code: 21, msg: "Bad token".into() };
        let s = err.to_string();
        assert!(s.contains("21"));
        assert!(s.contains("Bad token"));
    }
}
