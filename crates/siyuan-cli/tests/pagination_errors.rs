//! Integration tests for pagination and error paths.
//!
//! Run with: `cargo test -p siyuan-cli --test pagination_errors -- --ignored --test-threads=1`
//!
//! Test #9 (parse_error_when_block_id_malformed) is skipped because BlockId::parse
//! validation is already exhaustively covered by unit tests in siyuan-types/src/id.rs.

mod common;

use std::time::Duration;

use common::{boot_with_seed, wait_for, wait_for_doc_indexed};
use siyuan_client::SiyuanClient;
use siyuan_model::{load::load_doc, pagination::PageRequest};
use siyuan_types::{BlockId, ErrorKind, SiyuanError};

// ---------------------------------------------------------------------------
// Helper: seed a document containing many paragraphs to force multi-page output.
//
// Creates a new doc (separate from f.doc_id) with 60 numbered paragraphs so
// that the default page_size of 50 splits the content across pages.
// Returns the BlockId of the newly created doc.
// ---------------------------------------------------------------------------

async fn seed_large_doc(f: &common::Fixture) -> BlockId {
    // Build a markdown body with 60 numbered paragraphs via a single API call.
    // This avoids 60 individual HTTP round-trips and gives the SQL indexer a
    // single flush boundary to converge from.
    let mut md = String::from("# Large Document\n\n## Section One\n\n");
    for i in 1..=60 {
        md.push_str(&format!("Paragraph {i}\n\n"));
    }

    let doc_id = f
        .client
        .create_doc_with_md(&f.notebook_id, "/LargeDoc", &md)
        .await
        .expect("create large doc");

    // Wait until the SQL index has at least 60 content blocks plus doc/heading blocks.
    wait_for_doc_indexed(&f.client, &doc_id, 60)
        .await
        .expect("timed out waiting for large doc to be indexed");

    doc_id
}

// ---------------------------------------------------------------------------
// Pagination test 1: first page caps at page_size
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn pagination_first_page_caps_at_page_size() {
    let f = boot_with_seed().await.expect("boot");
    let doc_id = seed_large_doc(&f).await;

    let bundle = load_doc(
        &f.client,
        &doc_id,
        PageRequest {
            page: 1,
            page_size: 25,
        },
    )
    .await
    .expect("load_doc page 1");

    assert_eq!(
        bundle.blocks.len(),
        25,
        "page 1 must return exactly page_size blocks; got {}",
        bundle.blocks.len()
    );
    assert_eq!(
        bundle.page.page_size, 25,
        "page_size in PageInfo must match request"
    );
    assert!(
        bundle.page.total_pages > 1,
        "60+ block doc with page_size=25 must span more than one page; got total_pages={}",
        bundle.page.total_pages
    );
}

// ---------------------------------------------------------------------------
// Pagination test 2: second page block ids do not overlap with first page
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn pagination_second_page_continues_sequence() {
    let f = boot_with_seed().await.expect("boot");
    let doc_id = seed_large_doc(&f).await;

    let page1 = load_doc(
        &f.client,
        &doc_id,
        PageRequest {
            page: 1,
            page_size: 25,
        },
    )
    .await
    .expect("load_doc page 1");

    let page2 = load_doc(
        &f.client,
        &doc_id,
        PageRequest {
            page: 2,
            page_size: 25,
        },
    )
    .await
    .expect("load_doc page 2");

    let ids1: std::collections::HashSet<_> = page1.blocks.iter().map(|b| &b.id).collect();
    let overlap: Vec<_> = page2
        .blocks
        .iter()
        .filter(|b| ids1.contains(&b.id))
        .collect();

    assert!(
        overlap.is_empty(),
        "page 2 must not contain any block ids from page 1; overlap: {overlap:?}"
    );
    assert!(
        !page2.blocks.is_empty(),
        "page 2 must have at least one block"
    );
}

// ---------------------------------------------------------------------------
// Pagination test 3: last page is partial when page_size does not divide total
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn pagination_last_page_partial() {
    let f = boot_with_seed().await.expect("boot");
    let doc_id = seed_large_doc(&f).await;

    // Use page_size=40: with 63+ blocks (60 paragraphs + doc + 2 headings),
    // the remainder is non-zero.
    let page_size = 40;
    let first = load_doc(&f.client, &doc_id, PageRequest { page: 1, page_size })
        .await
        .expect("load_doc page 1 to learn total_pages");

    let total_pages = first.page.total_pages;
    assert!(
        total_pages > 1,
        "expected multiple pages with page_size={page_size}"
    );

    let last = load_doc(
        &f.client,
        &doc_id,
        PageRequest {
            page: total_pages,
            page_size,
        },
    )
    .await
    .expect("load_doc last page");

    assert!(
        !last.blocks.is_empty(),
        "last page must have at least one block"
    );
    assert!(
        last.blocks.len() < page_size,
        "last page must be partial (< page_size={}); got {}",
        page_size,
        last.blocks.len()
    );
}

// ---------------------------------------------------------------------------
// Pagination test 4: requesting an out-of-bounds page clamps to the last page
//
// The paginator (siyuan-model::pagination::paginate) clamps the requested
// page to total_pages when the caller asks for a page past the end, so the
// returned bundle is the *last* real page — not an empty one. This contract
// is what BUG-1 relies on; the test below pins it down with concrete asserts
// instead of vacuous bounds.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn pagination_oob_page_clamps_to_last() {
    let f = boot_with_seed().await.expect("boot");

    // f.doc_id is the small seeded doc (6+ blocks).
    let bundle = load_doc(
        &f.client,
        &f.doc_id,
        PageRequest {
            page: 999,
            page_size: 25,
        },
    )
    .await
    .expect("load_doc page 999");

    // Hard guarantees of clamping behavior on a non-empty doc.
    assert_eq!(
        bundle.page.page, bundle.page.total_pages,
        "out-of-bounds page must be clamped to total_pages"
    );
    assert!(
        bundle.blocks.len() <= bundle.page.page_size,
        "page must not exceed page_size"
    );
    assert!(
        !bundle.blocks.is_empty(),
        "seeded doc is non-empty so the clamped last page must contain blocks"
    );

    // Soft contract assertions kept for documentation.
    assert!(
        bundle.page.total_pages > 0,
        "total_pages must be positive for a real doc"
    );
    assert!(
        bundle.blocks.len() <= bundle.page.total_blocks,
        "returned blocks must not exceed total_blocks"
    );
}

// ---------------------------------------------------------------------------
// Error test 1: wrong auth token returns Auth error
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn auth_error_when_token_wrong() {
    let f = boot_with_seed().await.expect("boot");

    // Construct a second client pointing at the same container but with the wrong token.
    let bad_client =
        SiyuanClient::new(f.container.base_url(), "definitely-wrong-token").expect("client build");

    let err = bad_client
        .ls_notebooks()
        .await
        .expect_err("wrong-token call must fail");

    assert_eq!(
        err.kind(),
        ErrorKind::Auth,
        "wrong token must yield Auth error; got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Error test 2: unknown block id — uniform NotFound contract
//
// Investigation finding: SiYuan kernel v3.6.5 returns code=0 with an empty
// kramdown string for unknown block ids — but it returns the same shape for
// blocks whose content is genuinely empty. The client disambiguates by doing
// a SQL existence probe on the empty-kramdown branch; only confirmed absence
// surfaces as `SiyuanError::NotFound`. This test pins down the contract for
// the absence case.
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn invalid_block_id_yields_not_found() {
    let f = boot_with_seed().await.expect("boot");

    // A syntactically valid id that no block in the kernel will match.
    let fake_id = BlockId::parse("20000101000000-fake000").expect("fake id parse");

    let err = f
        .client
        .get_block_kramdown(&fake_id)
        .await
        .expect_err("unknown block id must yield Err");

    assert_eq!(
        err.kind(),
        ErrorKind::NotFound,
        "unknown block id must surface as NotFound; got: {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Error test 3: load_doc on a missing doc id surfaces typed NotFound
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn missing_doc_load_bails() {
    let f = boot_with_seed().await.expect("boot");

    let fake_id = BlockId::parse("20000101000000-fake000").expect("fake id parse");

    let err = load_doc(&f.client, &fake_id, PageRequest::default())
        .await
        .expect_err("missing doc must return Err");

    // load_doc returns a typed SiyuanError::NotFound wrapped in anyhow;
    // downcast back to the typed variant so we verify semantics, not strings.
    let typed = err
        .downcast_ref::<SiyuanError>()
        .expect("error must be a SiyuanError");
    assert_eq!(
        typed.kind(),
        ErrorKind::NotFound,
        "missing doc must surface as NotFound; got: {typed:?}"
    );
}

// ---------------------------------------------------------------------------
// Error test 4: SQL against a non-existent table returns Api error
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn sql_error_for_invalid_statement() {
    let f = boot_with_seed().await.expect("boot");

    // Wait briefly to confirm the kernel is indexed; the SQL endpoint itself
    // is always available after boot, but referencing a missing table is an
    // SQLite error that the kernel surfaces as a non-zero code.
    wait_for(
        || async {
            // Confirm the SQL endpoint is online with a benign query.
            let rows = f
                .client
                .sql("SELECT 1 AS x")
                .await
                .map(|_| Some(()))
                .unwrap_or(None);
            Ok::<_, anyhow::Error>(rows)
        },
        Duration::from_secs(5),
    )
    .await
    .expect("SQL endpoint did not become available");

    let err = f
        .client
        .sql("SELECT * FROM no_such_table_xyz")
        .await
        .expect_err("SQL on missing table must fail");

    assert_eq!(
        err.kind(),
        ErrorKind::Api,
        "SQL error on missing table must yield Api error; got: {err:?}"
    );
}
