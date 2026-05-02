use std::fmt::Write;

use siyuan_model::bundle::DocBundle;
use siyuan_types::{BlockNode, BlockType};

pub fn render_doc(bundle: &DocBundle) -> String {
    let mut out = String::new();

    // Use JSON encoding for the hpath so non-ASCII paths (CJK, emoji, etc.)
    // survive as printable codepoints instead of `\u{...}` escapes that the
    // Rust `Debug` formatter emits. JSON quoting only escapes characters
    // JSON requires (control chars, `"`, `\`), preserving readability and
    // letting agents round-trip the marker back into a string.
    let hpath_json = serde_json::to_string(&bundle.doc.hpath).unwrap_or_default();
    let _ = writeln!(
        out,
        "<!-- sy:doc id={} hpath={} page={} of {} -->",
        bundle.doc.id, hpath_json, bundle.page.page, bundle.page.total_pages,
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

    #[test]
    fn renders_superblock_with_fence() {
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
                mk_block("20260501000001-doc0001", BlockType::Document, ""),
                mk_block(
                    "20260501000010-sb00001",
                    BlockType::SuperBlock,
                    "# Hello superblock",
                ),
            ],
        };
        let md = render_doc(&bundle);
        insta::assert_snapshot!(md, @r###"
        <!-- sy:doc id=20260501000001-doc0001 hpath="/Demo" page=1 of 1 -->

        <!-- sy:block id=20260501000001-doc0001 type=d subtype= -->


        <!-- sy:block id=20260501000010-sb00001 type=s subtype= -->
        :::sy-superblock id=20260501000010-sb00001
        # Hello superblock
        :::
        "###);
    }

    #[test]
    fn renders_subtype_in_annotation() {
        let mut block = mk_block("20260501000010-h2aaaaa", BlockType::Heading, "## Hello");
        block.subtype = Some("h2".into());
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
                total_blocks: 1,
                total_pages: 1,
            },
            blocks: vec![block],
        };
        let md = render_doc(&bundle);
        insta::assert_snapshot!(md, @r###"
        <!-- sy:doc id=20260501000001-doc0001 hpath="/Demo" page=1 of 1 -->

        <!-- sy:block id=20260501000010-h2aaaaa type=h subtype=h2 -->
        ## Hello
        "###);
    }

    #[test]
    fn ascii_hpath_renders_as_plain_quoted_string() {
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
                total_blocks: 0,
                total_pages: 1,
            },
            blocks: vec![],
        };
        let md = render_doc(&bundle);
        assert!(
            md.contains("hpath=\"/Demo\""),
            "ASCII hpath should appear as a plain JSON string; got: {md}"
        );
    }

    #[test]
    fn cjk_hpath_renders_as_printable_codepoints() {
        // The Rust `Debug` formatter would emit `\u{7B14}\u{8BB0}` for "笔记";
        // JSON encoding preserves the printable codepoints.
        let bundle = DocBundle {
            schema: DocBundle::SCHEMA.into(),
            doc: DocMeta {
                id: BlockId::parse("20260501000001-doc0001").unwrap(),
                notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
                hpath: "/笔记".into(),
                title: "笔记".into(),
            },
            page: PageInfo {
                page: 1,
                page_size: 50,
                total_blocks: 0,
                total_pages: 1,
            },
            blocks: vec![],
        };
        let md = render_doc(&bundle);
        assert!(
            md.contains("hpath=\"/笔记\""),
            "CJK hpath should appear as printable codepoints; got: {md}"
        );
        assert!(
            !md.contains("\\u{"),
            "Rust-style `\\u{{..}}` escapes must not leak into the marker; got: {md}"
        );
    }

    #[test]
    fn renders_empty_doc_with_header_only() {
        let bundle = DocBundle {
            schema: DocBundle::SCHEMA.into(),
            doc: DocMeta {
                id: BlockId::parse("20260501000001-doc0001").unwrap(),
                notebook_id: NotebookId::parse("20260501000000-nb00001").unwrap(),
                hpath: "/Empty".into(),
                title: "Empty".into(),
            },
            page: PageInfo {
                page: 1,
                page_size: 50,
                total_blocks: 0,
                total_pages: 1,
            },
            blocks: vec![],
        };
        let md = render_doc(&bundle);
        insta::assert_snapshot!(md, @r###"
        <!-- sy:doc id=20260501000001-doc0001 hpath="/Empty" page=1 of 1 -->

        "###);
    }
}
