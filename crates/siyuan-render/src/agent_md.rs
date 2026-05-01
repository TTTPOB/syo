use std::fmt::Write;

use siyuan_model::bundle::DocBundle;
use siyuan_types::{BlockNode, BlockType};

pub fn render_doc(bundle: &DocBundle) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "<!-- sy:doc id={} hpath={:?} page={} of {} -->",
        bundle.doc.id, bundle.doc.hpath, bundle.page.page, bundle.page.total_pages,
    );
    let _ = writeln!(out);

    for b in &bundle.blocks {
        render_block(&mut out, b);
        let _ = writeln!(out);
    }

    out
}

pub fn render_block(out: &mut String, b: &BlockNode) {
    let _ = writeln!(
        out,
        "<!-- sy:block id={} type={} subtype={} -->",
        b.id,
        b.block_type.as_kernel(),
        b.subtype.as_deref().unwrap_or(""),
    );
    if b.block_type == BlockType::SuperBlock {
        // Read-only superblock: wrap in a fence so the agent can see boundaries.
        let _ = writeln!(out, ":::sy-superblock id={}", b.id);
        let _ = writeln!(out, "{}", b.markdown);
        let _ = writeln!(out, ":::");
    } else {
        let _ = writeln!(out, "{}", b.markdown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use siyuan_model::bundle::{DocBundle, DocMeta, PageInfo};
    use siyuan_types::{BlockId, BlockRole, BlockType, NotebookId};
    use std::collections::BTreeMap;

    fn mk_block(id: &str, ty: BlockType, md: &str) -> BlockNode {
        BlockNode {
            id: BlockId::parse(id).unwrap(),
            root_id: BlockId::parse("20260501000001-doc0001").unwrap(),
            parent_id: Some(BlockId::parse("20260501000001-doc0001").unwrap()),
            notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
            block_type: ty,
            subtype: None,
            role: BlockRole::for_block_type(ty),
            markdown: md.into(),
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
    fn renders_doc_header_and_blocks() {
        let bundle = DocBundle {
            schema: DocBundle::SCHEMA.into(),
            doc: DocMeta {
                id: BlockId::parse("20260501000001-doc0001").unwrap(),
                notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
                hpath: "/Demo".into(),
                title: "Demo".into(),
            },
            page: PageInfo {
                page: 1,
                page_size: 50,
                total_blocks: 2,
                total_pages: 1,
            },
            blocks: vec![
                mk_block("20260501000010-h2aaaaa", BlockType::Heading, "## Hello"),
                mk_block("20260501000020-paaaaaa", BlockType::Paragraph, "World."),
            ],
        };
        let md = render_doc(&bundle);
        insta::assert_snapshot!(md, @r###"
        <!-- sy:doc id=20260501000001-doc0001 hpath="/Demo" page=1 of 1 -->

        <!-- sy:block id=20260501000010-h2aaaaa type=h subtype= -->
        ## Hello

        <!-- sy:block id=20260501000020-paaaaaa type=p subtype= -->
        World.
        "###);
    }
}
