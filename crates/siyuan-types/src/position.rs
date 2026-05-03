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

/// Bare positional variant — no block-id payload.
///
/// This decouples the "where" decision from the "what it operates on" data,
/// letting callers construct a full [`Position`] by pairing a `PositionKind`
/// with a [`BlockId`] via [`From<(PositionKind, BlockId)> for Position`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionKind {
    AfterBlock,
    BeforeBlock,
    AppendChild,
    PrependChild,
    AppendSection,
    PrependSection,
    AppendDoc,
    PrependDoc,
}

impl From<(PositionKind, BlockId)> for Position {
    fn from((kind, id): (PositionKind, BlockId)) -> Self {
        match kind {
            PositionKind::AfterBlock => Self::AfterBlock { block_id: id },
            PositionKind::BeforeBlock => Self::BeforeBlock { block_id: id },
            PositionKind::AppendChild => Self::AppendChild { container_id: id },
            PositionKind::PrependChild => Self::PrependChild { container_id: id },
            PositionKind::AppendSection => Self::AppendSection { heading_id: id },
            PositionKind::PrependSection => Self::PrependSection { heading_id: id },
            PositionKind::AppendDoc => Self::AppendDoc { doc_id: id },
            PositionKind::PrependDoc => Self::PrependDoc { doc_id: id },
        }
    }
}

impl Position {
    /// The id this position is anchored on, regardless of variant.
    pub fn anchor_id(&self) -> &BlockId {
        match self {
            Self::AfterBlock { block_id } | Self::BeforeBlock { block_id } => block_id,
            Self::AppendChild { container_id } | Self::PrependChild { container_id } => {
                container_id
            }
            Self::AppendSection { heading_id } | Self::PrependSection { heading_id } => heading_id,
            Self::AppendDoc { doc_id } | Self::PrependDoc { doc_id } => doc_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_with_kind_tag() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let pos = Position::AfterBlock {
            block_id: id.clone(),
        };
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
        let pos = Position::AppendChild {
            container_id: id.clone(),
        };
        assert_eq!(pos.anchor_id(), &id);
    }

    // --- PositionKind tests ---

    #[test]
    fn position_kind_serde_roundtrips() {
        use serde_json;
        let variants = [
            PositionKind::AfterBlock,
            PositionKind::BeforeBlock,
            PositionKind::AppendChild,
            PositionKind::PrependChild,
            PositionKind::AppendSection,
            PositionKind::PrependSection,
            PositionKind::AppendDoc,
            PositionKind::PrependDoc,
        ];
        let expected_names = [
            "\"after_block\"",
            "\"before_block\"",
            "\"append_child\"",
            "\"prepend_child\"",
            "\"append_section\"",
            "\"prepend_section\"",
            "\"append_doc\"",
            "\"prepend_doc\"",
        ];
        for (&kind, &expected_name) in variants.iter().zip(expected_names.iter()) {
            let json = serde_json::to_string(&kind).unwrap();
            assert!(
                json.contains(expected_name),
                "expected {expected_name} in {json}"
            );
            let roundtripped: PositionKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, roundtripped);
        }
    }

    #[test]
    fn from_position_kind_and_block_id_produces_correct_position() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let pairs = [
            (PositionKind::AfterBlock, "after_block"),
            (PositionKind::BeforeBlock, "before_block"),
            (PositionKind::AppendChild, "append_child"),
            (PositionKind::PrependChild, "prepend_child"),
            (PositionKind::AppendSection, "append_section"),
            (PositionKind::PrependSection, "prepend_section"),
            (PositionKind::AppendDoc, "append_doc"),
            (PositionKind::PrependDoc, "prepend_doc"),
        ];
        for (kind, expected_kind_tag) in pairs {
            let pos = Position::from((kind, id.clone()));
            let json = serde_json::to_string(&pos).unwrap();
            assert!(
                json.contains(expected_kind_tag),
                "expected kind tag {expected_kind_tag} in {json}"
            );
            assert_eq!(pos.anchor_id(), &id);
        }
    }

    #[test]
    fn position_kind_roundtrip_through_position() {
        // PositionKind -> Position via From -> extract kind via serde -> deser back
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        for kind in [
            PositionKind::AfterBlock,
            PositionKind::BeforeBlock,
            PositionKind::AppendChild,
            PositionKind::PrependChild,
            PositionKind::AppendSection,
            PositionKind::PrependSection,
            PositionKind::AppendDoc,
            PositionKind::PrependDoc,
        ] {
            let pos = Position::from((kind, id.clone()));
            // Serialize Position, then deserialize as PositionKind by extracting
            // the "kind" field. A proper roundtrip verifies the tag names match.
            let pos_json = serde_json::to_string(&pos).unwrap();
            let pos_value: serde_json::Value = serde_json::from_str(&pos_json).unwrap();
            let kind_str = pos_value["kind"].as_str().unwrap();
            let kind_json = format!("\"{kind_str}\"");
            let deser_kind: PositionKind = serde_json::from_str(&kind_json).unwrap();
            assert_eq!(kind, deser_kind);
        }
    }
}
