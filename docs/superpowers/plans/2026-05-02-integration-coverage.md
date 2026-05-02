# Integration Test Coverage Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use `- [ ]` checkboxes.

**Goal:** Fix the 4 failing integration tests in `crates/siyuan-cli/tests/cli_integration.rs` and add integration coverage for the SiyuanClient / siyuan-model surface that today is exercised only by unit tests. All new tests bypass the CLI binary and call typed APIs directly.

**Architecture:**
- Tests are gated by `#[ignore]` and run with `cargo test -p siyuan-cli -- --ignored --test-threads=1`.
- Each test boots its own `SiyuanContainer` via `siyuan-testkit` (clean state per test).
- A shared `tests/common/mod.rs` exposes a `Fixture` and a generic `wait_for(probe, timeout)` helper to bridge the SQL-index lag between mutations and reads.
- New test binaries are added under `crates/siyuan-cli/tests/`. Keep file count moderate by grouping related coverage.

**Root cause of current failures:** SiYuan's SQL index is updated asynchronously after writes; `load_doc` (which reads `blocks` via `/api/query/sql`) sees stale data immediately after a `block`-API mutation. Tests panic on assertions that compare reload output to expected post-write state. The same lag affects `boot_with_seed`: section_children is empty until the SQL has indexed the freshly seeded paragraphs.

**Out of scope:** test against publish-mode SQL disablement (would require a different container flag), set-icon / set-sort doc metadata (no client method), CLI-binary subprocess tests.

---

## File Structure

```
crates/siyuan-cli/tests/
  common/
    mod.rs                           # extended: Fixture + wait_for helpers
  cli_integration.rs                 # fixed: 4 tests use wait_for
  notebook_filetree.rs               # NEW: notebook + filetree coverage
  block_advanced.rs                  # NEW: insert positions / move / delete / attrs
  tag_search.rs                      # NEW: tags + sql search
  asset_graph.rs                     # NEW: asset upload + reference, graph queries
  pagination_errors.rs               # NEW: multi-page docs + error paths
```

Each new file declares `mod common;` and uses `boot_with_seed` (or a custom seeder when needed).

---

## Task 1: Wait-for-convergence helper + fix `boot_with_seed`

**Files:**
- Modify: `crates/siyuan-cli/tests/common/mod.rs`

**Background:** Add a `wait_for<F, Fut, T>(probe, timeout) -> Result<T>` polling helper and a `wait_for_doc_indexed(client, doc_id, expected_block_count)` convenience. Update `boot_with_seed` to wait until the seeded doc's blocks are fully visible to SQL before returning. Suppress the `dead_code` warning on `Fixture.container` (it's load-bearing — its Drop impl tears down the container).

- [ ] **Step 1: Write `wait_for` + `wait_for_doc_indexed` (TDD)**

  Add unit-test-style coverage that exercises both helpers against a shared in-memory probe (no container needed). Then update `boot_with_seed` to call `wait_for_doc_indexed` after `create_doc_with_md` so by return time the doc has 6+ blocks visible.

  Signature suggestion:

  ```rust
  pub async fn wait_for<F, Fut, T>(mut probe: F, timeout: Duration) -> Result<T>
  where
      F: FnMut() -> Fut,
      Fut: std::future::Future<Output = Result<Option<T>>>;

  pub async fn wait_for_doc_indexed(
      client: &SiyuanClient,
      doc_id: &BlockId,
      min_blocks: usize,
  ) -> Result<()>;
  ```

  Default timeout: `Duration::from_secs(5)`. Poll interval: 100 ms.

- [ ] **Step 2: Verify Fixture container is no longer warned about**

  Either annotate the field `#[allow(dead_code)]` with a `// SAFETY:` style comment explaining lifetime, or hold it behind a typed wrapper. Keep the change minimal.

- [ ] **Step 3: Run `cargo check -p siyuan-cli --tests`**

  Expected: clean, no warnings.

- [ ] **Step 4: Commit** with `test(cli): add wait_for helper and ensure boot_with_seed waits for SQL convergence`

---

## Task 2: Fix the 4 failing happy-path tests

**Files:**
- Modify: `crates/siyuan-cli/tests/cli_integration.rs`

**Background:** Replace immediate `load_doc` calls after mutations with `wait_for(...)` polling against the expected post-mutation state. Tests must continue to assert the same end-state behavior — only the timing changes.

Failing tests:
- `update_block_then_reload_reflects_change`
- `insert_blocks_after_anchor_preserves_order`
- `delete_block_removes_it`
- `append_section_inserts_at_section_end` (this one fails at `populate_section_children` — Task 1's `boot_with_seed` fix should resolve it; verify)

- [ ] **Step 1: Update tests to poll for expected state**
- [ ] **Step 2: Run `cargo test -p siyuan-cli --test cli_integration -- --ignored --test-threads=1`**

  Expected: 6/6 passed.

- [ ] **Step 3: Commit** with `test(cli): poll for SQL convergence in mutation happy-path tests`

---

## Task 3: Notebook + filetree coverage

**Files:**
- Create: `crates/siyuan-cli/tests/notebook_filetree.rs`

**Coverage targets:**
- `ls_notebooks` — list contains the seeded notebook
- `open_notebook` / `close_notebook` — round-trip; closed notebooks are excluded from `ls_notebooks` filtering by `closed`
- `rename_notebook` — name changes propagate
- `remove_notebook` — gone from `ls_notebooks`
- `get_ids_by_hpath` — resolves `/IntegrationTestDoc` to the seeded doc id
- `get_hpath_by_id` — round-trip with the seeded doc id
- `rename_doc` — title changes propagate
- `move_docs` — across-notebook move, hpath updates
- `remove_doc` — `get_ids_by_hpath` returns empty afterwards

- [ ] **Step 1: Write tests using TDD (one at a time)** — each `#[tokio::test] #[ignore]` calling `boot_with_seed` and asserting outcome. Use `wait_for` after mutations that go through SQL-backed reads.

- [ ] **Step 2: Run** `cargo test -p siyuan-cli --test notebook_filetree -- --ignored --test-threads=1`

  Expected: all pass.

- [ ] **Step 3: Commit** with `test(cli): cover notebook + filetree APIs`

---

## Task 4: Block insert-positions, move, delete, attrs

**Files:**
- Create: `crates/siyuan-cli/tests/block_advanced.rs`

**Coverage targets:**
- `insert_block_markdown` with `previous_id` (after) — already covered, skip.
- `insert_block_markdown` with `next_id` (before) — new test
- `insert_block_markdown` with `parent_id` (under) — new test
- `append_block_markdown` to the doc root (= "append doc")
- `prepend_block_markdown` to the doc root (= "prepend doc")
- `append_block_markdown` to a heading id (= "append child" / append to section)
- `prepend_block_markdown` to a heading id (= "prepend child")
- `move_block` to a different parent — order verified via `load_doc`
- `move_block` to a different position under same parent (via `previous_id`)
- `delete_block` on a heading — children cascade behavior
- `get_block_attrs` / `set_block_attrs` — round-trip, including overwrites and unicode values

- [ ] **Step 1: TDD each test** — use `wait_for` for SQL-visible asserts.
- [ ] **Step 2: Run**
- [ ] **Step 3: Commit** with `test(cli): cover insert positions, move/delete, attrs`

---

## Task 5: Tags + SQL search

**Files:**
- Create: `crates/siyuan-cli/tests/tag_search.rs`

**Coverage targets:**
- Seed a doc whose markdown contains `#alpha#` and `#beta#` tags (siyuan tag syntax).
- `siyuan_model::tag::list_tags` — returns sorted list including `alpha` and `beta`.
- `siyuan_model::tag::search_by_tag("alpha")` — returns the tagged blocks with correct previews.
- `client.sql("SELECT id FROM blocks WHERE markdown LIKE '%alpha%'")` — typed SQL round-trip works.
- `siyuan_model::relations::relations_for(...)` — populates outgoing/incoming/tags for a small set.

- [ ] **Step 1: TDD** — note that tag indexing also lags; use `wait_for(|| list_tags())`.
- [ ] **Step 2: Run** & **Step 3: Commit** with `test(cli): cover tag listing + SQL search`

---

## Task 6: Asset upload + graph queries

**Files:**
- Create: `crates/siyuan-cli/tests/asset_graph.rs`

**Coverage targets:**
- `upload_asset` — write a tiny PNG (8 bytes is fine — `b"\x89PNG..."`-style) to a tempfile, upload, assert the returned path matches `assets/<original-stem>-<id>.<ext>`.
- After upload, embed the asset by `insert_block_markdown(format!("![tinypng]({asset_path})"), ...)` — verify the new block's markdown contains the path. (= asset reference coverage)
- Seed a doc with two paragraphs that ref each other via siyuan block-ref syntax `((blockId 'anchor'))` — this requires creating two paragraphs, capturing the second's id, then updating the first's markdown to reference it. Wait for SQL to index refs.
- `siyuan_model::graph::neighborhood(client, &target, 1, Direction::Incoming)` — backlinks to the second block include the first block.
- `Direction::Outgoing` from the first block — finds the second block.
- `Direction::Both` — symmetric.

- [ ] **Step 1: TDD** — keep PNG bytes inline; use `wait_for` around graph reads.
- [ ] **Step 2: Run** & **Step 3: Commit** with `test(cli): cover asset upload + graph neighborhood queries`

---

## Task 7: Pagination + error paths

**Files:**
- Create: `crates/siyuan-cli/tests/pagination_errors.rs`

**Coverage targets:**
- Pagination: seed a doc with 60+ paragraph blocks. `load_doc(client, doc_id, PageRequest { page: 1, page_size: 25 })` returns `total_pages > 1`. Page 2 returns the next 25, page 3 the remainder. Block ordering is stable across pages (no overlap, no gaps).
- Auth error: build a `SiyuanClient` with a wrong token, call any endpoint, assert `SiyuanError::Auth`.
- Invalid block id: call `get_block_kramdown(&BlockId::parse("99999999999999-fake000").unwrap())`, assert `SiyuanError::Api { code, .. }` (siyuan returns code != 0).
- Missing doc id: `load_doc(client, &fake_doc_id, ...)` returns the "no blocks" error.
- API error: `client.sql("SELECT * FROM nonsense_table")` — assert error variant.

- [ ] **Step 1: TDD** each case.
- [ ] **Step 2: Run** & **Step 3: Commit** with `test(cli): cover pagination + auth/api error paths`

---

## Done check

After all tasks:

- [ ] `cargo test --workspace` passes (no regressions in unit tests).
- [ ] `cargo test -p siyuan-cli -- --ignored --test-threads=1` passes (all integration tests).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean.
- [ ] All commits use semantic format (test:/fix:/refactor:).
- [ ] No untracked files.
