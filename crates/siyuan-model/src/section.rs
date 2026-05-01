use siyuan_types::{BlockNode, BlockType};

/// Compute heading sections by walking the DFS-ordered block list. For each
/// heading h_n at level L, the section spans subsequent siblings until the next
/// heading whose level is <= L (or end of doc).
pub fn populate_section_children(blocks: &mut [BlockNode]) {
    // First, snapshot heading positions and levels.
    let mut headings: Vec<(usize, u8)> = Vec::new(); // (index, level)
    for (i, b) in blocks.iter().enumerate() {
        if b.block_type == BlockType::Heading {
            let level = parse_heading_level(b.subtype.as_deref());
            headings.push((i, level));
        }
    }

    // For each heading, walk forward to find section end among the same parent.
    for (h_idx, level) in headings.iter().copied() {
        let parent = blocks[h_idx].parent_id.clone();
        let mut section: Vec<_> = Vec::new();
        for j in (h_idx + 1)..blocks.len() {
            if blocks[j].parent_id != parent {
                continue;
            }
            if blocks[j].block_type == BlockType::Heading {
                let other = parse_heading_level(blocks[j].subtype.as_deref());
                if other <= level {
                    break;
                }
            }
            section.push(blocks[j].id.clone());
        }
        blocks[h_idx].section_children = section;
    }
}

fn parse_heading_level(subtype: Option<&str>) -> u8 {
    match subtype {
        Some("h1") => 1,
        Some("h2") => 2,
        Some("h3") => 3,
        Some("h4") => 4,
        Some("h5") => 5,
        Some("h6") => 6,
        _ => 6, // unknown → deepest, so it gets absorbed by anything
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
    fn h2_section_stops_at_next_h2() {
        let root = "20260501000001-doc0001";
        let mut blocks = vec![
            mk(
                "20260501000010-h2aaaaa",
                Some(root),
                root,
                BlockType::Heading,
                Some("h2"),
            ),
            mk(
                "20260501000020-paaaaaa",
                Some(root),
                root,
                BlockType::Paragraph,
                None,
            ),
            mk(
                "20260501000030-paaaaab",
                Some(root),
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
            mk(
                "20260501000050-paaaaac",
                Some(root),
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
    }

    #[test]
    fn h2_section_includes_h3_inside_it() {
        let root = "20260501000001-doc0001";
        let mut blocks = vec![
            mk(
                "20260501000010-h2aaaaa",
                Some(root),
                root,
                BlockType::Heading,
                Some("h2"),
            ),
            mk(
                "20260501000020-h3aaaaa",
                Some(root),
                root,
                BlockType::Heading,
                Some("h3"),
            ),
            mk(
                "20260501000030-paaaaab",
                Some(root),
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
        let ids: Vec<_> = blocks[0]
            .section_children
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect();
        assert_eq!(
            ids,
            vec!["20260501000020-h3aaaaa", "20260501000030-paaaaab"]
        );
    }
}
