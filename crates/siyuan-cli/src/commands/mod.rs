pub mod asset;
pub mod block;
pub mod create_doc;
pub mod delete_block;
pub mod doc;
pub mod get_block;
pub mod get_doc;
pub mod graph;
pub mod insert_blocks;
pub mod move_block;
pub mod notebook;
pub mod search;
pub mod serve_mcp;
pub mod set_attrs;
pub mod sql;
pub mod tag;
pub mod update_block;

use anyhow::Result;
use siyuan_types::position::PositionKind;

/// Read markdown content from a file path, or stdin if path is `-`.
pub fn read_markdown_input(path: &str) -> Result<String> {
    use std::io::Read;
    if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        Ok(buf)
    } else {
        Ok(std::fs::read_to_string(path)?)
    }
}

/// Parse a position kind string into PositionKind.
/// Used by both `block insert` and `block move` for clap value_parser.
pub fn parse_position(kind: &str) -> Result<PositionKind, String> {
    match kind {
        "after_block" => Ok(PositionKind::AfterBlock),
        "before_block" => Ok(PositionKind::BeforeBlock),
        "append_child" => Ok(PositionKind::AppendChild),
        "prepend_child" => Ok(PositionKind::PrependChild),
        "append_section" => Ok(PositionKind::AppendSection),
        "prepend_section" => Ok(PositionKind::PrependSection),
        "append_doc" => Ok(PositionKind::AppendDoc),
        "prepend_doc" => Ok(PositionKind::PrependDoc),
        other => Err(format!(
            "invalid position kind '{other}'. Must be one of: after_block, before_block, append_child, prepend_child, append_section, prepend_section, append_doc, prepend_doc"
        )),
    }
}
