use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::{BlockId, NotebookId};

/// SiYuan's first-class block kinds. Variants match the `type` column in the
/// `blocks` table: `d`, `h`, `p`, `l`, `i`, `s`, `b`, `c`, `m`, `t`, `tb`,
/// `query_embed`, `av`, `html`, `iframe`, `widget`, plus media leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Document,
    Heading,
    Paragraph,
    List,
    ListItem,
    SuperBlock,
    Blockquote,
    Code,
    Math,
    Table,
    ThematicBreak,
    QueryEmbed,
    AttributeView,
    Html,
    IFrame,
    Widget,
    Audio,
    Video,
    Unknown,
}

impl BlockType {
    /// Parse the single-letter / underscored type returned by the kernel.
    pub fn from_kernel(raw: &str) -> Self {
        match raw {
            "d" => Self::Document,
            "h" => Self::Heading,
            "p" => Self::Paragraph,
            "l" => Self::List,
            "i" => Self::ListItem,
            "s" => Self::SuperBlock,
            "b" => Self::Blockquote,
            "c" => Self::Code,
            "m" => Self::Math,
            "t" => Self::Table,
            "tb" => Self::ThematicBreak,
            "query_embed" => Self::QueryEmbed,
            "av" => Self::AttributeView,
            "html" => Self::Html,
            "iframe" => Self::IFrame,
            "widget" => Self::Widget,
            "audio" => Self::Audio,
            "video" => Self::Video,
            _ => Self::Unknown,
        }
    }

    pub fn as_kernel(&self) -> &'static str {
        match self {
            Self::Document => "d",
            Self::Heading => "h",
            Self::Paragraph => "p",
            Self::List => "l",
            Self::ListItem => "i",
            Self::SuperBlock => "s",
            Self::Blockquote => "b",
            Self::Code => "c",
            Self::Math => "m",
            Self::Table => "t",
            Self::ThematicBreak => "tb",
            Self::QueryEmbed => "query_embed",
            Self::AttributeView => "av",
            Self::Html => "html",
            Self::IFrame => "iframe",
            Self::Widget => "widget",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Unknown => "unknown",
        }
    }
}

/// Heading level / list ordering / etc. — opaque string passed through.
pub type BlockSubtype = String;

/// Semantic role: how the harness treats this block for editing/insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockRole {
    /// `d/s/l/i/b` — accepts `append_child` / `prepend_child`.
    Container,
    /// `h` — accepts `append_section` / `prepend_section`.
    HeadingSectionOwner,
    /// `p/c/m/t/tb/query_embed/...` — leaf, no child operations.
    Leaf,
}

impl BlockRole {
    pub fn for_block_type(t: BlockType) -> Self {
        match t {
            BlockType::Document
            | BlockType::SuperBlock
            | BlockType::List
            | BlockType::ListItem
            | BlockType::Blockquote => Self::Container,
            BlockType::Heading => Self::HeadingSectionOwner,
            _ => Self::Leaf,
        }
    }
}

/// One block in a document tree, with semantic annotations beyond what the
/// raw `blocks` table provides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockNode {
    pub id: BlockId,
    pub root_id: BlockId,
    pub parent_id: Option<BlockId>,
    pub notebook_id: NotebookId,

    pub block_type: BlockType,
    pub subtype: Option<BlockSubtype>,
    pub role: BlockRole,

    pub markdown: String,
    pub kramdown: Option<String>,
    pub ial: Option<String>,
    #[serde(default)]
    pub attrs: BTreeMap<String, String>,

    pub hash: Option<String>,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub sort: Option<i64>,

    /// Children whose `parent_id == self.id` (data-structure children).
    #[serde(default)]
    pub structural_children: Vec<BlockId>,

    /// Heading section content. Empty unless `block_type == Heading`.
    #[serde(default)]
    pub section_children: Vec<BlockId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_type_round_trips_through_kernel_form() {
        for t in [
            BlockType::Document,
            BlockType::Heading,
            BlockType::Paragraph,
            BlockType::List,
            BlockType::ListItem,
            BlockType::SuperBlock,
            BlockType::Blockquote,
            BlockType::Code,
            BlockType::QueryEmbed,
            BlockType::AttributeView,
            BlockType::ThematicBreak,
        ] {
            assert_eq!(BlockType::from_kernel(t.as_kernel()), t, "round trip {t:?}");
        }
    }

    #[test]
    fn unknown_kernel_type_falls_back() {
        assert_eq!(BlockType::from_kernel("xyzzy"), BlockType::Unknown);
    }

    #[test]
    fn unknown_round_trips_via_fallback() {
        // `Unknown.as_kernel()` returns "unknown"; `from_kernel("unknown")`
        // also lands on Unknown via the wildcard arm. The serialised form
        // is therefore stable even for unrecognised kernel types.
        assert_eq!(BlockType::Unknown.as_kernel(), "unknown");
        assert_eq!(BlockType::from_kernel("unknown"), BlockType::Unknown);
        assert_eq!(BlockType::from_kernel(""), BlockType::Unknown);
    }

    #[test]
    fn role_classification_covers_all_variants() {
        use BlockRole::*;
        use BlockType::*;
        let cases: &[(BlockType, BlockRole)] = &[
            (Document, Container),
            (SuperBlock, Container),
            (List, Container),
            (ListItem, Container),
            (Blockquote, Container),
            (Heading, HeadingSectionOwner),
            (Paragraph, Leaf),
            (Code, Leaf),
            (Math, Leaf),
            (Table, Leaf),
            (ThematicBreak, Leaf),
            (QueryEmbed, Leaf),
            (AttributeView, Leaf),
            (Html, Leaf),
            (IFrame, Leaf),
            (Widget, Leaf),
            (Audio, Leaf),
            (Video, Leaf),
            (Unknown, Leaf),
        ];
        for (bt, expected) in cases {
            assert_eq!(BlockRole::for_block_type(*bt), *expected, "{bt:?}");
        }
    }
}
