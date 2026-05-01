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

#[cfg(test)]
mod tests {
    use super::*;
    use siyuan_types::{BlockId, BlockRole, NotebookId};
    use std::collections::BTreeMap;

    fn mk(id: &str, parent: Option<&str>, ty: BlockType) -> BlockNode {
        BlockNode {
            id: BlockId::parse(id).unwrap(),
            root_id: BlockId::parse("20260501000001-doc0001").unwrap(),
            parent_id: parent.map(|p| BlockId::parse(p).unwrap()),
            notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
            block_type: ty,
            subtype: None,
            role: BlockRole::for_block_type(ty),
            markdown: String::new(),
            kramdown: None,
            ial: None,
            attrs: BTreeMap::new(),
            hash: None,
            created: None,
            updated: None,
            sort: None,
            structural_children: vec![],
            section_children: vec![],
        }
    }

    #[test]
    fn document_gets_structural_children() {
        let root = "20260501000001-doc0001";
        let mut blocks = vec![
            mk(root, None, BlockType::Document),
            mk("20260501000020-paaaaaa", Some(root), BlockType::Paragraph),
            mk("20260501000030-paaaaab", Some(root), BlockType::Paragraph),
        ];
        populate_structural_children(&mut blocks);
        assert_eq!(blocks[0].structural_children.len(), 2);
        assert!(blocks[1].structural_children.is_empty());
    }

    #[test]
    fn superblock_gets_children() {
        let root = "20260501000001-doc0001";
        let sb = "20260501000010-sb00001";
        let mut blocks = vec![
            mk(root, None, BlockType::Document),
            mk(sb, Some(root), BlockType::SuperBlock),
            mk("20260501000030-paaaaaa", Some(sb), BlockType::Paragraph),
        ];
        populate_structural_children(&mut blocks);
        assert_eq!(blocks[0].structural_children.len(), 1); // doc has sb as child
        assert_eq!(blocks[1].structural_children.len(), 1); // sb has paragraph as child
    }

    #[test]
    fn heading_does_not_get_structural_children() {
        let root = "20260501000001-doc0001";
        let mut blocks = vec![
            mk(root, None, BlockType::Document),
            mk("20260501000020-h2aaaaa", Some(root), BlockType::Heading),
            mk("20260501000030-paaaaaa", Some(root), BlockType::Paragraph),
        ];
        populate_structural_children(&mut blocks);
        assert!(blocks[1].structural_children.is_empty());
    }
}
