use std::fmt;
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

static BLOCK_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\d{14}-[0-9a-z]{7}$").expect("compile-time-valid regex"));

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdError {
    #[error("invalid SiYuan block id: {0:?} (expected 14-digit timestamp + '-' + 7 lowercase alnum)")]
    Invalid(String),
}

/// A SiYuan block identifier.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(String);

impl BlockId {
    pub fn parse(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if BLOCK_ID_RE.is_match(&s) {
            Ok(Self(s))
        } else {
            Err(IdError::Invalid(s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BlockId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// A SiYuan notebook identifier. Same shape as a block id.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NotebookId(String);

impl NotebookId {
    /// Shares `BLOCK_ID_RE` because notebook ids currently have the same
    /// lexical shape as block ids. Tighten if/when the kernel diverges.
    pub fn parse(s: impl Into<String>) -> Result<Self, IdError> {
        let s = s.into();
        if BLOCK_ID_RE.is_match(&s) {
            Ok(Self(s))
        } else {
            Err(IdError::Invalid(s))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NotebookId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for NotebookId {
    type Err = IdError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_block_id() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        assert_eq!(id.as_str(), "20260501093000-abc1234");
    }

    #[test]
    fn rejects_uppercase() {
        assert!(BlockId::parse("20260501093000-ABC1234").is_err());
    }

    #[test]
    fn rejects_short_suffix() {
        assert!(BlockId::parse("20260501093000-abc123").is_err());
    }

    #[test]
    fn rejects_missing_dash() {
        assert!(BlockId::parse("20260501093000abc1234").is_err());
    }

    #[test]
    fn from_str_works() {
        let id: BlockId = "20260501093000-abc1234".parse().unwrap();
        assert_eq!(id.to_string(), "20260501093000-abc1234");
    }

    #[test]
    fn serde_round_trip() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"20260501093000-abc1234\"");
        let back: BlockId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }
}
