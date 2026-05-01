use siyuan_types::{BlockNode, BlockType};

/// Mark each container's `structural_children` field. Assumes `blocks` is in
/// canonical DFS order with `parent_id` set on every non-doc block.
pub fn populate_structural_children(blocks: &mut [BlockNode]) {
    use std::collections::HashMap;
    let mut map: HashMap<_, Vec<_>> = HashMap::new();
    for b in blocks.iter() {
        if let Some(parent) = b.parent_id.clone() {
            map.entry(parent).or_default().push(b.id.clone());
        }
    }
    for b in blocks.iter_mut() {
        if matches!(
            b.block_type,
            BlockType::Document
                | BlockType::SuperBlock
                | BlockType::List
                | BlockType::ListItem
                | BlockType::Blockquote
        ) {
            if let Some(children) = map.remove(&b.id) {
                b.structural_children = children;
            }
        }
    }
}
