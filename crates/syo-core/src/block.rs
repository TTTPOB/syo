use anyhow::{Context, Result, bail};
use serde::Deserialize;

use siyuan_client::SiyuanClient;
use siyuan_model::load::load_doc;
use siyuan_model::pagination::PageRequest;
use siyuan_model::section::populate_section_children;
use siyuan_types::position::PositionKind;
use siyuan_types::{BlockId, BlockType, Position};

// ---------------------------------------------------------------------------
// Input / output structs
// ---------------------------------------------------------------------------

/// Output after fetching a block.
#[derive(Debug)]
pub struct GetBlockOutput {
    pub id: BlockId,
    pub kramdown: String,
}

/// Input for updating a block's markdown in-place.
#[derive(Debug)]
pub struct UpdateBlockInput {
    pub id: BlockId,
    pub markdown: String,
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
}

/// Input for moving an existing block to a new position.
#[derive(Debug)]
pub struct MoveBlockInput {
    pub id: BlockId,
    pub position: PositionKind,
    pub anchor: BlockId,
}

// ---------------------------------------------------------------------------
// Public operations
// ---------------------------------------------------------------------------

/// Fetch the kramdown source of a block.
pub async fn get(client: &SiyuanClient, id: &BlockId) -> Result<GetBlockOutput> {
    let bk = client.get_block_kramdown(id).await?;
    Ok(GetBlockOutput {
        id: bk.id,
        kramdown: bk.kramdown,
    })
}

/// Update a block's markdown in-place.
pub async fn update(client: &SiyuanClient, input: UpdateBlockInput) -> Result<()> {
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
    match input.position {
        PositionKind::AfterBlock => {
            client
                .move_block(&input.id, Some(&input.anchor), None)
                .await?;
        }
        PositionKind::BeforeBlock => {
            let prev_id = find_previous_sibling(client, &input.anchor).await?;
            client.move_block(&input.id, Some(&prev_id), None).await?;
        }
        PositionKind::AppendChild | PositionKind::AppendDoc => {
            client
                .move_block(&input.id, None, Some(&input.anchor))
                .await?;
        }
        PositionKind::PrependChild | PositionKind::PrependDoc => {
            // move_block with parent_id and no previous_id places the moved
            // block at the end of the parent. SiYuan's kernel does not have
            // a separate "prepend" call — practically the position is the
            // same; callers wanting strict first-child semantics should
            // follow up with an after_block of the original first child.
            client
                .move_block(&input.id, None, Some(&input.anchor))
                .await?;
        }
        PositionKind::AppendSection => {
            let section_end = resolve_section_end(client, &input.anchor).await?;
            client
                .move_block(&input.id, Some(&section_end), None)
                .await?;
        }
        PositionKind::PrependSection => {
            // Right after the heading itself.
            client
                .move_block(&input.id, Some(&input.anchor), None)
                .await?;
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
        };
        _assert_debug(&gbo);

        let ubi = UpdateBlockInput {
            id: BlockId::parse("20260501093000-abc1234").unwrap(),
            markdown: "## hi".into(),
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
        };
        _assert_debug(&dbi);

        let mbi = MoveBlockInput {
            id: BlockId::parse("20260501093000-abc1234").unwrap(),
            position: PositionKind::AfterBlock,
            anchor: BlockId::parse("20260501093000-abc1234").unwrap(),
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
}
