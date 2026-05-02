//! Integration tests for asset upload/embedding and graph neighborhood traversal.
//!
//! Run with: `cargo test -p siyuan-cli --test asset_graph -- --ignored --test-threads=1`

mod common;

use std::io::Write as _;
use std::time::Duration;

use common::{boot_with_seed, wait_for};
use serde::Deserialize;
use siyuan_model::graph::{Direction, neighborhood};
use siyuan_model::{load::load_doc, pagination::PageRequest};

// Minimal valid PNG: 8-byte signature + IHDR chunk (13 bytes of data) + IEND chunk.
// This is accepted by most file-type validators.
const TINY_PNG: &[u8] = &[
    // PNG signature
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // IHDR chunk: length=13
    0x00, 0x00, 0x00, 0x0d, // Chunk type "IHDR"
    0x49, 0x48, 0x44, 0x52,
    // Width=1, Height=1, BitDepth=8, ColorType=2 (RGB), Compression=0, Filter=0, Interlace=0
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00,
    // CRC of IHDR chunk data
    0x90, 0x77, 0x53, 0xde, // IEND chunk: length=0
    0x00, 0x00, 0x00, 0x00, // Chunk type "IEND"
    0x49, 0x45, 0x4e, 0x44, // CRC of IEND
    0xae, 0x42, 0x60, 0x82,
];

// ---------------------------------------------------------------------------
// Asset test 1: upload returns an assets/ path with correct extension
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn upload_asset_returns_assets_path() {
    let f = boot_with_seed().await.expect("boot");

    let mut tmp = tempfile::Builder::new()
        .prefix("tiny")
        .suffix(".png")
        .tempfile()
        .expect("create tempfile");
    tmp.write_all(TINY_PNG).expect("write PNG bytes");
    tmp.flush().expect("flush");

    let path = f
        .client
        .upload_asset(tmp.path())
        .await
        .expect("upload_asset");

    assert!(
        path.starts_with("assets/"),
        "returned path must start with 'assets/'; got: {path}"
    );
    assert!(
        path.ends_with(".png"),
        "returned path must end with '.png'; got: {path}"
    );
}

// ---------------------------------------------------------------------------
// Asset test 2: two uploads of the same filename return distinct paths
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn upload_asset_returns_unique_paths_for_same_filename() {
    let f = boot_with_seed().await.expect("boot");

    // Write the same bytes to two tempfiles with the same prefix/suffix so the
    // kernel receives requests with the same original filename.
    let mut tmp1 = tempfile::Builder::new()
        .prefix("samenam")
        .suffix(".png")
        .tempfile()
        .expect("create tempfile 1");
    tmp1.write_all(TINY_PNG).expect("write PNG 1");
    tmp1.flush().expect("flush 1");

    let mut tmp2 = tempfile::Builder::new()
        .prefix("samenam")
        .suffix(".png")
        .tempfile()
        .expect("create tempfile 2");
    tmp2.write_all(TINY_PNG).expect("write PNG 2");
    tmp2.flush().expect("flush 2");

    let path1 = f
        .client
        .upload_asset(tmp1.path())
        .await
        .expect("first upload");
    let path2 = f
        .client
        .upload_asset(tmp2.path())
        .await
        .expect("second upload");

    // The kernel appends a unique timestamp/id suffix per upload so paths differ.
    assert_ne!(
        path1, path2,
        "each upload should produce a unique kernel path; got the same: {path1}"
    );
}

// ---------------------------------------------------------------------------
// Asset test 3: uploaded asset can be embedded and found in the block index
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn embed_uploaded_asset_in_block() {
    let f = boot_with_seed().await.expect("boot");

    let mut tmp = tempfile::Builder::new()
        .prefix("embed")
        .suffix(".png")
        .tempfile()
        .expect("create tempfile");
    tmp.write_all(TINY_PNG).expect("write PNG bytes");
    tmp.flush().expect("flush");

    let asset_path = f
        .client
        .upload_asset(tmp.path())
        .await
        .expect("upload_asset");

    // Embed the asset by appending a markdown image reference.
    f.client
        .append_block_markdown(&format!("![tiny]({asset_path})"), &f.doc_id)
        .await
        .expect("append image block");

    // Poll until the new block appears in the SQL-backed block index.
    let client = &f.client;
    let doc_id = &f.doc_id;
    let ap = asset_path.clone();
    let block = wait_for(
        || async {
            let bundle = load_doc(
                client,
                doc_id,
                PageRequest {
                    page: 1,
                    page_size: 200,
                },
            )
            .await?;
            let found = bundle.blocks.into_iter().find(|b| b.markdown.contains(&ap));
            Ok(found)
        },
        Duration::from_secs(15),
    )
    .await
    .expect("timed out waiting for image block to appear in SQL index");

    assert!(
        block.markdown.contains(&asset_path),
        "block markdown must contain the asset path; got: {:?}",
        block.markdown
    );
}

// ---------------------------------------------------------------------------
// Graph helpers
// ---------------------------------------------------------------------------

/// Wait until the refs table has at least one row linking `from_id` → `to_id`.
async fn wait_for_ref(
    client: &siyuan_client::SiyuanClient,
    from_id: &siyuan_types::BlockId,
    to_id: &siyuan_types::BlockId,
) {
    #[derive(Deserialize)]
    struct CountRow {
        n: i64,
    }
    let a = from_id.as_str().to_string();
    let b = to_id.as_str().to_string();
    wait_for(
        || async {
            let stmt = format!(
                "SELECT COUNT(*) AS n FROM refs WHERE block_id = '{a}' AND def_block_id = '{b}'"
            );
            let rows: Vec<CountRow> = client.sql_typed(&stmt).await?;
            let count = rows.first().map(|r| r.n).unwrap_or(0);
            if count >= 1 { Ok(Some(())) } else { Ok(None) }
        },
        Duration::from_secs(15),
    )
    .await
    .expect("timed out waiting for refs table to index the block reference");
}

// ---------------------------------------------------------------------------
// Graph test 1: outgoing traversal finds the referenced block
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn neighborhood_outgoing_finds_target() {
    let f = boot_with_seed().await.expect("boot");

    // Append B first to capture its id, then A with a ref to B.
    let b_id = f
        .client
        .append_block_markdown("Graph target block B", &f.doc_id)
        .await
        .expect("append block B");

    let a_id = f
        .client
        .append_block_markdown(
            &format!("Graph source A (({} 'B preview'))", b_id.as_str()),
            &f.doc_id,
        )
        .await
        .expect("append block A with ref");

    wait_for_ref(&f.client, &a_id, &b_id).await;

    let graph = neighborhood(&f.client, &a_id, 1, Direction::Outgoing)
        .await
        .expect("neighborhood outgoing");

    // There must be an edge A → B.
    let edge = graph
        .edges
        .iter()
        .find(|e| e.source == a_id && e.target == b_id);
    assert!(
        edge.is_some(),
        "outgoing graph from A must contain edge A→B; edges: {:?}",
        graph.edges
    );

    // Both nodes must appear.
    let node_ids: Vec<_> = graph.nodes.iter().map(|n| &n.id).collect();
    assert!(
        node_ids.contains(&&a_id),
        "nodes must include A; nodes: {node_ids:?}"
    );
    assert!(
        node_ids.contains(&&b_id),
        "nodes must include B; nodes: {node_ids:?}"
    );
}

// ---------------------------------------------------------------------------
// Graph test 2: incoming traversal finds the backlink
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn neighborhood_incoming_finds_backlink() {
    let f = boot_with_seed().await.expect("boot");

    let b_id = f
        .client
        .append_block_markdown("Incoming target B", &f.doc_id)
        .await
        .expect("append B");

    let a_id = f
        .client
        .append_block_markdown(
            &format!("Incoming source A (({} 'anchor'))", b_id.as_str()),
            &f.doc_id,
        )
        .await
        .expect("append A");

    wait_for_ref(&f.client, &a_id, &b_id).await;

    // Query from B's perspective: incoming links should include A.
    let graph = neighborhood(&f.client, &b_id, 1, Direction::Incoming)
        .await
        .expect("neighborhood incoming");

    let edge = graph
        .edges
        .iter()
        .find(|e| e.source == a_id && e.target == b_id);
    assert!(
        edge.is_some(),
        "incoming graph for B must contain edge A→B; edges: {:?}",
        graph.edges
    );

    let node_ids: Vec<_> = graph.nodes.iter().map(|n| &n.id).collect();
    assert!(
        node_ids.contains(&&a_id),
        "nodes must include A; nodes: {node_ids:?}"
    );
    assert!(
        node_ids.contains(&&b_id),
        "nodes must include B; nodes: {node_ids:?}"
    );
}

// ---------------------------------------------------------------------------
// Graph test 3: Direction::Both is symmetric (contains the same edge)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn neighborhood_both_is_symmetric() {
    let f = boot_with_seed().await.expect("boot");

    let b_id = f
        .client
        .append_block_markdown("Both target B", &f.doc_id)
        .await
        .expect("append B");

    let a_id = f
        .client
        .append_block_markdown(
            &format!("Both source A (({} 'anchor'))", b_id.as_str()),
            &f.doc_id,
        )
        .await
        .expect("append A");

    wait_for_ref(&f.client, &a_id, &b_id).await;

    let graph = neighborhood(&f.client, &a_id, 1, Direction::Both)
        .await
        .expect("neighborhood both");

    // With dedup, Direction::Both must produce exactly one A→B edge (the
    // outgoing pass discovers it; the incoming pass must not push it again).
    assert_eq!(
        graph.edges.len(),
        1,
        "Both graph from A must contain exactly one edge (no duplicates); edges: {:?}",
        graph.edges
    );
    let edge = &graph.edges[0];
    assert_eq!(edge.source, a_id, "edge source must be A");
    assert_eq!(edge.target, b_id, "edge target must be B");

    let node_ids: Vec<_> = graph.nodes.iter().map(|n| &n.id).collect();
    assert!(
        node_ids.contains(&&a_id),
        "nodes must include A; nodes: {node_ids:?}"
    );
    assert!(
        node_ids.contains(&&b_id),
        "nodes must include B; nodes: {node_ids:?}"
    );
}

// ---------------------------------------------------------------------------
// Graph test 4: depth limits traversal (depth=1 excludes transitive targets)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn neighborhood_depth_limits_traversal() {
    let f = boot_with_seed().await.expect("boot");

    // Chain: A → B → C
    let c_id = f
        .client
        .append_block_markdown("Depth chain C (leaf)", &f.doc_id)
        .await
        .expect("append C");

    let b_id = f
        .client
        .append_block_markdown(
            &format!("Depth chain B refs C (({} 'C'))", c_id.as_str()),
            &f.doc_id,
        )
        .await
        .expect("append B");

    let a_id = f
        .client
        .append_block_markdown(
            &format!("Depth chain A refs B (({} 'B'))", b_id.as_str()),
            &f.doc_id,
        )
        .await
        .expect("append A");

    // Wait for both ref rows to appear.
    wait_for_ref(&f.client, &a_id, &b_id).await;
    wait_for_ref(&f.client, &b_id, &c_id).await;

    // depth=1: A→B is reachable; B→C is NOT (requires depth 2).
    let graph1 = neighborhood(&f.client, &a_id, 1, Direction::Outgoing)
        .await
        .expect("neighborhood depth=1");

    let node_ids1: Vec<_> = graph1.nodes.iter().map(|n| n.id.clone()).collect();
    assert!(
        node_ids1.contains(&b_id),
        "depth=1 nodes must include B; nodes: {node_ids1:?}"
    );
    assert!(
        !node_ids1.contains(&c_id),
        "depth=1 nodes must NOT include C; nodes: {node_ids1:?}"
    );

    // depth=2: C should now appear.
    let graph2 = neighborhood(&f.client, &a_id, 2, Direction::Outgoing)
        .await
        .expect("neighborhood depth=2");

    let node_ids2: Vec<_> = graph2.nodes.iter().map(|n| n.id.clone()).collect();
    assert!(
        node_ids2.contains(&c_id),
        "depth=2 nodes must include C; nodes: {node_ids2:?}"
    );
}

// ---------------------------------------------------------------------------
// Graph test 5: isolated block yields empty edges, single-node result
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn neighborhood_isolated_block_has_no_edges() {
    let f = boot_with_seed().await.expect("boot");

    // Append a fresh block with no refs in or out.
    let isolated_id = f
        .client
        .append_block_markdown("Isolated block — no refs", &f.doc_id)
        .await
        .expect("append isolated block");

    // The SQL index may not have the block yet; wait until it appears in `blocks`.
    let client = &f.client;
    let iso_clone = isolated_id.clone();
    wait_for(
        || async {
            #[derive(Deserialize)]
            struct Row {
                #[allow(dead_code)]
                id: String,
            }
            let stmt = format!("SELECT id FROM blocks WHERE id = '{}'", iso_clone.as_str());
            let rows: Vec<Row> = client.sql_typed(&stmt).await?;
            if rows.is_empty() {
                Ok(None)
            } else {
                Ok(Some(()))
            }
        },
        Duration::from_secs(10),
    )
    .await
    .expect("timed out waiting for isolated block to appear in SQL blocks table");

    let graph = neighborhood(client, &isolated_id, 1, Direction::Both)
        .await
        .expect("neighborhood isolated");

    assert!(
        graph.edges.is_empty(),
        "isolated block must have no edges; got: {:?}",
        graph.edges
    );

    let node_ids: Vec<_> = graph.nodes.iter().map(|n| &n.id).collect();
    assert!(
        node_ids.contains(&&isolated_id),
        "nodes must include the center block; nodes: {node_ids:?}"
    );
    assert_eq!(
        graph.nodes.len(),
        1,
        "isolated block should yield exactly one node; got: {:?}",
        graph.nodes
    );
}
