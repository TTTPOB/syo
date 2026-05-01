use siyuan_types::{BlockNode, BlockType};

/// Compute heading sections. In SiYuan's block tree, a heading's content is
/// stored as its direct structural children (parent_id == heading.id), so
/// section_children simply mirrors those direct children ordered by sort.
pub fn populate_section_children(blocks: &mut [BlockNode]) {
    // Snapshot heading indices so we can mutate the slice afterwards.
    let heading_indices: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.block_type == BlockType::Heading)
        .map(|(i, _)| i)
        .collect();

    for h_idx in heading_indices {
        let heading_id = blocks[h_idx].id.clone();
        // Collect ids of blocks that are direct children of this heading.
        let section: Vec<_> = blocks
            .iter()
            .filter(|b| b.parent_id.as_ref() == Some(&heading_id))
            .map(|b| b.id.clone())
            .collect();
        blocks[h_idx].section_children = section;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use siyuan_types::{BlockId, BlockRole, BlockType, NotebookId};
    use std::collections::BTreeMap;

    fn mk(
        id: &str,
        parent: Option<&str>,
        root: &str,
        ty: BlockType,
        sub: Option<&str>,
    ) -> BlockNode {
        BlockNode {
            id: BlockId::parse(id).unwrap(),
            root_id: BlockId::parse(root).unwrap(),
            parent_id: parent.map(|p| BlockId::parse(p).unwrap()),
            notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
            block_type: ty,
            subtype: sub.map(String::from),
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
    fn h2_section_contains_its_direct_children() {
        // In SiYuan's actual tree, paragraphs are children of the heading
        // (parent_id == heading.id), not siblings.
        let root = "20260501000001-doc0001";
        let h2a_id = "20260501000010-h2aaaaa";
        let h2b_id = "20260501000040-h2bbbbb";
        let mut blocks = vec![
            mk(h2a_id, Some(root), root, BlockType::Heading, Some("h2")),
            // paragraphs are children of h2a
            mk(
                "20260501000020-paaaaaa",
                Some(h2a_id),
                root,
                BlockType::Paragraph,
                None,
            ),
            mk(
                "20260501000030-paaaaab",
                Some(h2a_id),
                root,
                BlockType::Paragraph,
                None,
            ),
            mk(h2b_id, Some(root), root, BlockType::Heading, Some("h2")),
            // paragraph child of h2b
            mk(
                "20260501000050-paaaaac",
                Some(h2b_id),
                root,
                BlockType::Paragraph,
                None,
            ),
        ];
        populate_section_children(&mut blocks);
        let h2a_section: Vec<_> = blocks[0]
            .section_children
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        assert_eq!(
            h2a_section,
            vec!["20260501000020-paaaaaa", "20260501000030-paaaaab"]
        );
        // h2b's section contains only its own child
        let h2b_section: Vec<_> = blocks[3]
            .section_children
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        assert_eq!(h2b_section, vec!["20260501000050-paaaaac"]);
    }

    #[test]
    fn h2_section_includes_nested_h3() {
        // h3 and its paragraph are children of h2; h3's paragraph is child of h3.
        let root = "20260501000001-doc0001";
        let h2a_id = "20260501000010-h2aaaaa";
        let h3a_id = "20260501000020-h3aaaaa";
        let mut blocks = vec![
            mk(h2a_id, Some(root), root, BlockType::Heading, Some("h2")),
            // h3 is a direct child of h2
            mk(h3a_id, Some(h2a_id), root, BlockType::Heading, Some("h3")),
            // paragraph is a child of h3
            mk(
                "20260501000030-paaaaab",
                Some(h3a_id),
                root,
                BlockType::Paragraph,
                None,
            ),
            mk(
                "20260501000040-h2bbbbb",
                Some(root),
                root,
                BlockType::Heading,
                Some("h2"),
            ),
        ];
        populate_section_children(&mut blocks);
        // h2's direct children include only h3 (paragraph is child of h3, not h2)
        let ids: Vec<_> = blocks[0]
            .section_children
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        assert_eq!(ids, vec!["20260501000020-h3aaaaa"]);
        // h3's section contains its paragraph
        let h3_ids: Vec<_> = blocks[1]
            .section_children
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        assert_eq!(h3_ids, vec!["20260501000030-paaaaab"]);
    }
}
