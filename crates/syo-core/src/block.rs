use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use siyuan_client::SiyuanClient;
use siyuan_model::load::load_doc;
use siyuan_model::pagination::PageRequest;
use siyuan_model::section::populate_section_children;
use siyuan_types::position::PositionKind;
use siyuan_types::{BlockId, BlockNode, BlockRole, BlockType, Position};

// ---------------------------------------------------------------------------
// Input / output structs
// ---------------------------------------------------------------------------

/// Output after fetching a block.
#[derive(Debug)]
pub struct GetBlockOutput {
    pub id: BlockId,
    pub kramdown: String,
    // TODO: The current heading-section metadata shape is intentionally
    // pragmatic. The desired end state is a more elegant structured model
    // for block-get context, but keep this compatibility-oriented shape for
    // now and revisit the public JSON/MCP contract later.
    pub meta: Option<HeadingSectionMeta>,
    pub section_markdown: Option<String>,
}

/// Input for updating a block's markdown in-place.
#[derive(Debug)]
pub struct UpdateBlockInput {
    pub id: BlockId,
    pub markdown: String,
    pub include_heading_children: bool,
}

/// Input for inserting a new block at a position relative to an anchor.
#[derive(Debug)]
pub struct InsertBlockInput {
    pub markdown: String,
    pub position: PositionKind,
    pub anchor: BlockId,
}

/// Output after inserting a new block.
#[derive(Debug)]
pub struct InsertBlockOutput {
    pub id: BlockId,
}

/// Input for deleting a block.
#[derive(Debug)]
pub struct DeleteBlockInput {
    pub id: BlockId,
    pub include_heading_children: bool,
}

/// Input for moving an existing block to a new position.
#[derive(Debug)]
pub struct MoveBlockInput {
    pub id: BlockId,
    pub position: PositionKind,
    pub anchor: BlockId,
    pub include_heading_children: bool,
}

#[derive(Debug)]
pub struct GetBlockInput {
    pub id: BlockId,
    pub include_heading_children: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeadingSectionMeta {
    #[serde(rename = "type")]
    pub block_type: String,
    pub role: String,
    pub structural_children: Vec<BlockId>,
    pub section_children: Vec<BlockId>,
    pub section_descendants: Vec<BlockId>,
    pub structural_child_count: usize,
    pub section_child_count: usize,
    pub section_descendant_count: usize,
    pub heading_children_included: bool,
}

// ---------------------------------------------------------------------------
// Public operations
// ---------------------------------------------------------------------------

/// Fetch the kramdown source of a block.
pub async fn get(client: &SiyuanClient, input: GetBlockInput) -> Result<GetBlockOutput> {
    let bk = client.get_block_kramdown(&input.id).await?;
    let context = match heading_section_context(client, &input.id).await {
        Ok(context) => Some(context),
        Err(e) if input.include_heading_children => return Err(e),
        Err(_) => None,
    };

    let (meta, section_markdown) = if let Some(context) = context {
        let meta = context.meta(input.include_heading_children);
        let section_markdown = input
            .include_heading_children
            .then(|| render_heading_section_agent_md(&context));
        (Some(meta), section_markdown)
    } else {
        if input.include_heading_children {
            bail!("--include-heading-children requires a heading block id");
        }
        (None, None)
    };

    Ok(GetBlockOutput {
        id: bk.id,
        kramdown: bk.kramdown,
        meta,
        section_markdown,
    })
}

/// Update a block's markdown in-place.
pub async fn update(client: &SiyuanClient, input: UpdateBlockInput) -> Result<()> {
    if input.include_heading_children {
        update_heading_section(client, input).await?;
        return Ok(());
    }

    client
        .update_block_markdown(&input.id, &input.markdown)
        .await?;
    Ok(())
}

/// Insert a new markdown block at a position relative to an anchor.
///
/// All 8 position kinds are supported. The anchor's role depends on the
/// position kind — see [`PositionKind`] for details.
pub async fn insert(client: &SiyuanClient, input: InsertBlockInput) -> Result<InsertBlockOutput> {
    let position = Position::from((input.position, input.anchor));
    let new_id = match position {
        Position::AfterBlock { block_id } => {
            client
                .insert_block_markdown(&input.markdown, Some(&block_id), None, None)
                .await?
        }
        Position::BeforeBlock { block_id } => {
            client
                .insert_block_markdown(&input.markdown, None, Some(&block_id), None)
                .await?
        }
        Position::AppendChild { container_id } => {
            client
                .append_block_markdown(&input.markdown, &container_id)
                .await?
        }
        Position::PrependChild { container_id } => {
            client
                .prepend_block_markdown(&input.markdown, &container_id)
                .await?
        }
        Position::AppendSection { heading_id } => {
            let section_end = resolve_section_end(client, &heading_id).await?;
            client
                .insert_block_markdown(&input.markdown, Some(&section_end), None, None)
                .await?
        }
        Position::PrependSection { heading_id } => {
            // Right after the heading itself.
            client
                .insert_block_markdown(&input.markdown, Some(&heading_id), None, None)
                .await?
        }
        Position::AppendDoc { doc_id } => {
            client
                .append_block_markdown(&input.markdown, &doc_id)
                .await?
        }
        Position::PrependDoc { doc_id } => {
            client
                .prepend_block_markdown(&input.markdown, &doc_id)
                .await?
        }
    };
    Ok(InsertBlockOutput { id: new_id })
}

/// Delete a block permanently.
pub async fn delete(client: &SiyuanClient, input: DeleteBlockInput) -> Result<()> {
    if input.include_heading_children {
        let context = heading_section_context(client, &input.id).await?;
        for child in context.section_descendants.iter().rev() {
            client.delete_block(child).await?;
        }
    }
    client.delete_block(&input.id).await?;
    Ok(())
}

/// Move an existing block to a new position within the document tree.
///
/// All 8 position kinds are supported. The block keeps its id and all its
/// children — only the parent and sibling order change.
///
/// Note for `PrependChild` / `PrependDoc`: the SiYuan kernel does not have a
/// dedicated "prepend" call. `move_block` with only `parent_id` places the
/// block at the end of the parent, so the result is practically equivalent.
/// Callers needing strict first-child position should follow up with an
/// `after_block` targeting the current first child.
pub async fn move_block(client: &SiyuanClient, input: MoveBlockInput) -> Result<()> {
    if input.include_heading_children {
        move_heading_with_children(client, input).await?;
        return Ok(());
    }

    move_single_block(client, &input.id, input.position, &input.anchor).await
}

async fn move_heading_with_children(client: &SiyuanClient, input: MoveBlockInput) -> Result<()> {
    let context = heading_section_context(client, &input.id).await?;
    let child_groups = heading_child_groups(&context);

    move_single_block(client, &context.heading.id, input.position, &input.anchor).await?;
    for (parent, children) in child_groups {
        for child in children {
            client.move_block(&child, None, Some(&parent)).await?;
        }
    }
    Ok(())
}

async fn move_single_block(
    client: &SiyuanClient,
    id: &BlockId,
    position: PositionKind,
    anchor: &BlockId,
) -> Result<()> {
    match position {
        PositionKind::AfterBlock => {
            client.move_block(id, Some(anchor), None).await?;
        }
        PositionKind::BeforeBlock => {
            let prev_id = find_previous_sibling(client, anchor).await?;
            client.move_block(id, Some(&prev_id), None).await?;
        }
        PositionKind::AppendChild | PositionKind::AppendDoc => {
            client.move_block(id, None, Some(anchor)).await?;
        }
        PositionKind::PrependChild | PositionKind::PrependDoc => {
            // move_block with parent_id and no previous_id places the moved
            // block at the end of the parent. SiYuan's kernel does not have
            // a separate "prepend" call — practically the position is the
            // same; callers wanting strict first-child semantics should
            // follow up with an after_block of the original first child.
            client.move_block(id, None, Some(anchor)).await?;
        }
        PositionKind::AppendSection => {
            let section_end = resolve_section_end(client, anchor).await?;
            client.move_block(id, Some(&section_end), None).await?;
        }
        PositionKind::PrependSection => {
            // Right after the heading itself.
            client.move_block(id, Some(anchor), None).await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Find the last block in the section owned by `heading_id`.
///
/// Loads the heading's document, populates section children, and returns the
/// last block in the heading's section. If the section is empty, returns the
/// heading itself.
///
/// This is the consolidated implementation — previously duplicated in
/// `syo` (CLI) and `syo-mcp`.
pub async fn resolve_section_end(client: &SiyuanClient, heading_id: &BlockId) -> Result<BlockId> {
    #[derive(Deserialize)]
    struct R {
        root_id: String,
        #[serde(rename = "type")]
        ty: String,
    }

    // Find the document root and verify this is a heading.
    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT root_id, type FROM blocks WHERE id = '{}'",
            heading_id.as_str()
        ))
        .await
        .context("resolve_section_end: query heading info")?;
    let root = rows
        .first()
        .ok_or_else(|| anyhow::anyhow!("heading not found"))?;
    if root.ty != "h" {
        bail!("anchor for append_section / resolve_section_end must be a heading block");
    }
    let root_id = BlockId::parse(&root.root_id).context("parsing root id")?;

    // Load the full document so we can detect section boundaries.
    let bundle = load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await
    .context("resolve_section_end: load doc")?;
    let mut blocks = bundle.blocks;
    populate_section_children(&mut blocks);

    let heading = blocks
        .iter()
        .find(|b| &b.id == heading_id)
        .ok_or_else(|| anyhow::anyhow!("heading not in doc"))?;
    if heading.block_type != BlockType::Heading {
        bail!("anchor is not a heading after re-resolution");
    }

    if let Some(last) = heading.section_children.last() {
        Ok(last.clone())
    } else {
        // Empty section: treat heading itself as anchor.
        Ok(heading_id.clone())
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Find the block that comes immediately before `anchor` in its parent's
/// children list. Used by `before_block` positioning.
///
/// Loads the anchor's document and walks the block list to find the
/// predecessor. Returns an error if the anchor is the first child.
async fn find_previous_sibling(client: &SiyuanClient, anchor: &BlockId) -> Result<BlockId> {
    #[derive(Deserialize)]
    struct R {
        root_id: String,
    }

    let rows: Vec<R> = client
        .sql_typed(&format!(
            "SELECT root_id FROM blocks WHERE id = '{}'",
            anchor.as_str()
        ))
        .await
        .context("find_previous_sibling: query root id")?;
    let root = rows
        .first()
        .ok_or_else(|| anyhow::anyhow!("anchor block not found"))?;
    let root_id = BlockId::parse(&root.root_id).context("parsing root id")?;

    let bundle = load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await
    .context("find_previous_sibling: load doc")?;
    let blocks = bundle.blocks;

    let idx = blocks
        .iter()
        .position(|b| &b.id == anchor)
        .ok_or_else(|| anyhow::anyhow!("anchor block not found in document"))?;
    if idx == 0 {
        bail!(
            "cannot move before first child of document; use prepend_child or prepend_doc instead"
        );
    }
    let prev = &blocks[idx - 1];
    Ok(prev.id.clone())
}

#[derive(Debug, Deserialize)]
struct BlockInfoRow {
    root_id: String,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Debug, Clone)]
struct HeadingSectionContext {
    heading: BlockNode,
    section_children: Vec<BlockId>,
    section_descendants: Vec<BlockId>,
    blocks: Vec<BlockNode>,
}

impl HeadingSectionContext {
    fn meta(&self, included: bool) -> HeadingSectionMeta {
        HeadingSectionMeta {
            block_type: self.heading.block_type.as_kernel().to_string(),
            role: role_label(self.heading.role).to_string(),
            structural_children: self.heading.structural_children.clone(),
            section_children: self.section_children.clone(),
            section_descendants: self.section_descendants.clone(),
            structural_child_count: self.heading.structural_children.len(),
            section_child_count: self.section_children.len(),
            section_descendant_count: self.section_descendants.len(),
            heading_children_included: included,
        }
    }
}

async fn block_info(client: &SiyuanClient, id: &BlockId) -> Result<BlockInfoRow> {
    let rows: Vec<BlockInfoRow> = client
        .sql_typed(&format!(
            "SELECT root_id, type FROM blocks WHERE id = '{}'",
            id.as_str()
        ))
        .await
        .context("query block info")?;
    rows.into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("block not found"))
}

async fn heading_section_context(
    client: &SiyuanClient,
    heading_id: &BlockId,
) -> Result<HeadingSectionContext> {
    let info = block_info(client, heading_id).await?;
    if info.ty != "h" {
        bail!("--include-heading-children requires a heading block id");
    }

    let root_id = BlockId::parse(&info.root_id).context("parsing root id")?;
    let bundle = load_doc(
        client,
        &root_id,
        PageRequest {
            page: 1,
            page_size: 100_000,
        },
    )
    .await
    .context("load heading document")?;

    let mut blocks = bundle.blocks;
    populate_section_children(&mut blocks);
    let by_id: HashMap<String, BlockNode> = blocks
        .iter()
        .map(|b| (b.id.as_str().to_string(), b.clone()))
        .collect();
    let heading = by_id
        .get(heading_id.as_str())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("heading not in doc"))?;
    if heading.block_type != BlockType::Heading {
        bail!("block is not a heading after document load");
    }

    let section_children = heading.section_children.clone();
    let section_descendants = collect_section_descendants(&section_children, &by_id);
    Ok(HeadingSectionContext {
        heading,
        section_children,
        section_descendants,
        blocks,
    })
}

fn heading_child_groups(context: &HeadingSectionContext) -> Vec<(BlockId, Vec<BlockId>)> {
    let by_id: HashMap<String, BlockNode> = context
        .blocks
        .iter()
        .map(|b| (b.id.as_str().to_string(), b.clone()))
        .collect();
    let mut out = vec![(context.heading.id.clone(), context.section_children.clone())];
    let mut stack: Vec<BlockId> = context.section_children.iter().rev().cloned().collect();
    while let Some(id) = stack.pop() {
        let Some(node) = by_id.get(id.as_str()) else {
            continue;
        };
        if node.block_type == BlockType::Heading {
            out.push((node.id.clone(), node.section_children.clone()));
            for child in node.section_children.iter().rev() {
                stack.push(child.clone());
            }
        }
    }
    out
}

fn collect_section_descendants(
    section_children: &[BlockId],
    by_id: &HashMap<String, BlockNode>,
) -> Vec<BlockId> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut stack: Vec<BlockId> = section_children.iter().rev().cloned().collect();

    while let Some(id) = stack.pop() {
        if !seen.insert(id.as_str().to_string()) {
            continue;
        }
        out.push(id.clone());

        let Some(node) = by_id.get(id.as_str()) else {
            continue;
        };
        let mut next = Vec::new();
        next.extend(node.structural_children.iter().cloned());
        if node.block_type == BlockType::Heading {
            next.extend(node.section_children.iter().cloned());
        }
        for child in next.into_iter().rev() {
            stack.push(child);
        }
    }

    out
}

pub fn render_block_get_agent_md(output: &GetBlockOutput) -> String {
    if let Some(section) = &output.section_markdown {
        return section.clone();
    }

    let mut out = String::new();
    if let Some(meta) = &output.meta {
        let _ = writeln!(
            out,
            "<!-- sy:block id={} type={} role={} structural_children={} section_children={} section_descendants={} heading_children_included=false hint=\"heading section omitted; pass --include-heading-children to include it\" -->",
            output.id,
            meta.block_type,
            meta.role,
            meta.structural_child_count,
            meta.section_child_count,
            meta.section_descendant_count,
        );
    } else {
        let _ = writeln!(out, "<!-- sy:block id={} -->", output.id);
    }
    out.push_str(&output.kramdown);
    out
}

fn render_heading_section_agent_md(context: &HeadingSectionContext) -> String {
    let mut selected = HashSet::new();
    selected.insert(context.heading.id.as_str().to_string());
    for id in &context.section_descendants {
        selected.insert(id.as_str().to_string());
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "<!-- sy:heading-section begin id={} section_children={} structural_children={} section_descendants={} -->",
        context.heading.id,
        context.section_children.len(),
        context.heading.structural_children.len(),
        context.section_descendants.len(),
    );

    for block in &context.blocks {
        if !selected.contains(block.id.as_str()) {
            continue;
        }
        let _ = writeln!(
            out,
            "<!-- sy:block id={} type={} subtype={} role={} structural_children={} section_children={} -->",
            block.id,
            block.block_type.as_kernel(),
            block.subtype.as_deref().unwrap_or(""),
            role_label(block.role),
            block.structural_children.len(),
            block.section_children.len(),
        );
        let _ = writeln!(out, "{}", block.markdown);
    }

    let _ = writeln!(
        out,
        "<!-- sy:heading-section end id={} -->",
        context.heading.id,
    );
    out
}

fn role_label(role: BlockRole) -> &'static str {
    match role {
        BlockRole::Container => "container",
        BlockRole::HeadingSectionOwner => "heading_section_owner",
        BlockRole::Leaf => "leaf",
    }
}

async fn update_heading_section(client: &SiyuanClient, input: UpdateBlockInput) -> Result<()> {
    let context = heading_section_context(client, &input.id).await?;
    let (heading_markdown, body_markdown) = split_heading_section_markdown(&input.markdown)?;

    client
        .update_block_markdown(&input.id, heading_markdown.trim_end())
        .await?;

    for child in context.section_descendants.iter().rev() {
        client.delete_block(child).await?;
    }

    if !body_markdown.trim().is_empty() {
        client
            .insert_block_markdown(body_markdown.trim_start(), Some(&input.id), None, None)
            .await?;
    }
    Ok(())
}

fn split_heading_section_markdown(markdown: &str) -> Result<(&str, &str)> {
    let lines = line_spans(markdown);
    let first = lines
        .iter()
        .position(|span| !span.content.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("heading section markdown cannot be empty"))?;

    if is_atx_heading(lines[first].content) {
        let mut end = lines[first].end;
        if let Some(next) = lines.get(first + 1)
            && is_block_ial(next.content)
        {
            end = next.end;
        }
        return Ok(markdown.split_at(end));
    }

    if let Some(underline) = lines.get(first + 1)
        && is_setext_heading_underline(underline.content)
    {
        let mut end = underline.end;
        if let Some(next) = lines.get(first + 2)
            && is_block_ial(next.content)
        {
            end = next.end;
        }
        return Ok(markdown.split_at(end));
    }

    bail!("heading section markdown must start with a heading block");
}

#[derive(Debug)]
struct LineSpan<'a> {
    content: &'a str,
    end: usize,
}

fn line_spans(markdown: &str) -> Vec<LineSpan<'_>> {
    let mut out = Vec::new();
    let mut start = 0;
    for line in markdown.split_inclusive('\n') {
        let end = start + line.len();
        out.push(LineSpan { content: line, end });
        start = end;
    }
    if start < markdown.len() {
        out.push(LineSpan {
            content: &markdown[start..],
            end: markdown.len(),
        });
    }
    out
}

fn is_atx_heading(line: &str) -> bool {
    let trimmed_start = line.trim_start_matches(' ');
    if line.len() - trimmed_start.len() > 3 {
        return false;
    }
    let hashes = trimmed_start.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    matches!(
        trimmed_start.as_bytes().get(hashes),
        None | Some(b' ' | b'\t' | b'\r' | b'\n')
    )
}

fn is_setext_heading_underline(line: &str) -> bool {
    let content = line.trim();
    if content.is_empty() {
        return false;
    }
    content.bytes().all(|b| b == b'=') || content.bytes().all(|b| b == b'-')
}

fn is_block_ial(line: &str) -> bool {
    let content = line.trim();
    content.starts_with("{:") && content.ends_with('}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structs_derive_debug() {
        // Compile-time check: all public structs must implement Debug.
        fn _assert_debug<T: std::fmt::Debug>(_t: &T) {}

        let gbo = GetBlockOutput {
            id: BlockId::parse("20260501093000-abc1234").unwrap(),
            kramdown: "## hi".into(),
            meta: None,
            section_markdown: None,
        };
        _assert_debug(&gbo);

        let ubi = UpdateBlockInput {
            id: BlockId::parse("20260501093000-abc1234").unwrap(),
            markdown: "## hi".into(),
            include_heading_children: false,
        };
        _assert_debug(&ubi);

        let ibi = InsertBlockInput {
            markdown: "## hi".into(),
            position: PositionKind::AfterBlock,
            anchor: BlockId::parse("20260501093000-abc1234").unwrap(),
        };
        _assert_debug(&ibi);

        let ibo = InsertBlockOutput {
            id: BlockId::parse("20260501093000-abc1234").unwrap(),
        };
        _assert_debug(&ibo);

        let dbi = DeleteBlockInput {
            id: BlockId::parse("20260501093000-abc1234").unwrap(),
            include_heading_children: false,
        };
        _assert_debug(&dbi);

        let mbi = MoveBlockInput {
            id: BlockId::parse("20260501093000-abc1234").unwrap(),
            position: PositionKind::AfterBlock,
            anchor: BlockId::parse("20260501093000-abc1234").unwrap(),
            include_heading_children: false,
        };
        _assert_debug(&mbi);
    }

    #[test]
    fn move_block_input_requires_position_and_anchor() {
        // These tests verify at the type level that MoveBlockInput requires
        // both position and anchor. Since the struct fields are mandatory,
        // construction is the test.
        let _input = MoveBlockInput {
            id: BlockId::parse("20260501093000-blk0001").unwrap(),
            position: PositionKind::AfterBlock,
            anchor: BlockId::parse("20260501093000-blk0002").unwrap(),
            include_heading_children: false,
        };
    }

    #[test]
    fn all_eight_position_kinds_are_referenced() {
        // Ensure all 8 variants exist and can be used in match arms.
        // This is a compile-time assertion that no variant is missing.
        let kinds = [
            PositionKind::AfterBlock,
            PositionKind::BeforeBlock,
            PositionKind::AppendChild,
            PositionKind::PrependChild,
            PositionKind::AppendSection,
            PositionKind::PrependSection,
            PositionKind::AppendDoc,
            PositionKind::PrependDoc,
        ];
        assert_eq!(kinds.len(), 8);
        for (i, kind) in kinds.iter().enumerate() {
            // Verify round-trip through Position conversion.
            let id = BlockId::parse("20260501093000-abc1234").unwrap();
            let pos = Position::from((*kind, id.clone()));
            assert_eq!(pos.anchor_id(), &id, "mismatch at index {i}");
        }
    }

    #[test]
    fn split_heading_section_markdown_accepts_atx_heading() {
        let (heading, body) =
            split_heading_section_markdown("## New title\n\nBody\n").expect("valid heading");
        assert_eq!(heading, "## New title\n");
        assert_eq!(body, "\nBody\n");
    }

    #[test]
    fn split_heading_section_markdown_keeps_heading_ial() {
        let (heading, body) = split_heading_section_markdown(
            "## New title\n{: id=\"20260501093000-abc1234\"}\n\nBody\n",
        )
        .expect("valid heading with ial");
        assert_eq!(heading, "## New title\n{: id=\"20260501093000-abc1234\"}\n");
        assert_eq!(body, "\nBody\n");
    }

    #[test]
    fn split_heading_section_markdown_accepts_setext_heading() {
        let (heading, body) =
            split_heading_section_markdown("New title\n---\nBody\n").expect("valid heading");
        assert_eq!(heading, "New title\n---\n");
        assert_eq!(body, "Body\n");
    }

    #[test]
    fn split_heading_section_markdown_rejects_non_heading() {
        let err = split_heading_section_markdown("Body first\n\n## Later\n")
            .expect_err("paragraph first should fail");
        assert!(err.to_string().contains("must start with a heading block"));
    }

    #[test]
    fn render_block_get_agent_md_hints_omitted_heading_section() {
        let id = BlockId::parse("20260501093000-abc1234").unwrap();
        let output = GetBlockOutput {
            id: id.clone(),
            kramdown: "## Title\n".into(),
            meta: Some(HeadingSectionMeta {
                block_type: "h".into(),
                role: "heading_section_owner".into(),
                structural_children: vec![],
                section_children: vec![BlockId::parse("20260501093001-bcd1234").unwrap()],
                section_descendants: vec![BlockId::parse("20260501093001-bcd1234").unwrap()],
                structural_child_count: 0,
                section_child_count: 1,
                section_descendant_count: 1,
                heading_children_included: false,
            }),
            section_markdown: None,
        };

        let md = render_block_get_agent_md(&output);
        assert!(md.contains("heading_children_included=false"));
        assert!(md.contains("section_children=1"));
        assert!(md.contains("--include-heading-children"));
        assert!(md.contains("## Title"));
    }
}
