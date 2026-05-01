pub mod asset;
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
pub mod set_attrs;
pub mod tag;
pub mod update_block;

use anyhow::Result;

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
