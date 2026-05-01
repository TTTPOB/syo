use serde::{Deserialize, Serialize};

use crate::id::BlockId;

/// Where to drop blocks for `insert_blocks` / `move_block`.
///
/// Variants are deliberately distinct so the harness — not the agent — picks
/// the correct combination of SiYuan's `previousID` / `nextID` / `parentID`
/// arguments. In particular, `AppendSection` (heading) and `AppendChild`
/// (container) are different ops with different semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Position {
    AfterBlock { block_id: BlockId },
    BeforeBlock { block_id: BlockId },
    AppendChild { container_id: BlockId },
    PrependChild { container_id: BlockId },
    AppendSection { heading_id: BlockId },
    PrependSection { heading_id: BlockId },
    AppendDoc { doc_id: BlockId },
    PrependDoc { doc_id: BlockId },
}

impl Position {
    /// The id this position is anchored on, regardless of variant.
    pub fn anchor_id(&self) -> &BlockId {
        match self {
            Self::AfterBlock { block_id }
            | Self::BeforeBlock { block_id } => block_id,
            Self::AppendChild { container_id }
            | Self::PrependChild { container_id } => container_id,
            Self::AppendSection { heading_id }
            | Self::PrependSection { heading_id } => heading_id,
            Self::AppendDoc { doc_id }
            | Self::PrependDoc { doc_id } => doc_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_with_kind_tag() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let pos = Position::AfterBlock { block_id: id.clone() };
        let json = serde_json::to_string(&pos).unwrap();
        assert!(json.contains("\"kind\":\"after_block\""));
        assert!(json.contains("\"block_id\":\"20260501093000-abc1234\""));
    }

    #[test]
    fn deserialises_section_position() {
        let raw = r#"{"kind":"append_section","heading_id":"20260501093000-abc1234"}"#;
        let pos: Position = serde_json::from_str(raw).unwrap();
        match pos {
            Position::AppendSection { heading_id } => {
                assert_eq!(heading_id.as_str(), "20260501093000-abc1234");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn anchor_id_extracts_underlying_id() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let pos = Position::AppendChild { container_id: id.clone() };
        assert_eq!(pos.anchor_id(), &id);
    }
}
