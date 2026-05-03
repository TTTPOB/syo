# Design Decisions

A log of non-obvious tradeoffs made while building the v1 agent harness. Each entry captures *why* the code looks the way it does and *what would justify changing it*. New decisions go at the bottom; do not delete entries — strike them through and add a follow-up entry instead.

Entry format:

- **Context** — the situation we were in.
- **Decision** — what we picked.
- **Rationale** — why.
- **Tradeoff** — what we gave up.
- **Reversal trigger** — what evidence would reopen the question.

---

## 1. Single-binary harness (`syo serve-mcp`)

**Context.** The harness originally shipped two binaries: `syo` (CLI) and `syo-mcp` (MCP server over stdio). Both statically linked the same dependency closure — tokio, reqwest+rustls, serde, tracing, clap, plus every internal crate (`siyuan-types`, `siyuan-client`, `siyuan-model`, `siyuan-render`). On disk, `target/debug/` showed `syo` ≈ 100 MB and `syo-mcp` ≈ 127 MB; the delta was almost entirely `rmcp`.

**Decision.** Merge `syo-mcp` into `siyuan` as a `serve-mcp` subcommand. Drop the standalone binary entirely. No backwards-compatibility shim, no Cargo feature gate.

**Rationale.** Two near-identical binaries doubled disk footprint, doubled compile time, and forced users (and MCP client configs) to track which one to invoke for what. A subcommand is the conventional way to bundle related modes (`git fetch` vs `git push`, `cargo build` vs `cargo run`). Stderr was already the right destination for `tracing` logs in a CLI; making the MCP variant work just required ensuring `init_tracing` doesn't write to stdout (the JSON-RPC channel).

**Tradeoff.** Pure CLI users now compile `rmcp` whether they need it or not. There is no way to build a "no-MCP" variant without editing the workspace. We rejected a Cargo feature gate because the user's stated principle was "don't design for hypothetical opt-out".

**Reversal trigger.** `rmcp` (or whichever MCP SDK we depend on) becoming significantly heavier in compile time or binary footprint, or a need for a transport other than stdio that would naturally live in a separate binary.

---

## 2. Drop `notebook_open` and `notebook_close` from the public surface

**Context.** The original surface mirrored the kernel HTTP API verbatim: `syo_siyuan_notebook_open` / `syo_siyuan_notebook_close` (MCP) and `syo notebook open` / `notebook close` (CLI). We considered keeping them and adding an internal `ensure_notebook_opened` helper that auto-opens any closed notebook before high-level operations touch it.

**Decision.** Remove both from the public surface AND do not add any auto-open helper. The harness simply does not handle user-closed notebooks. Kernel errors (e.g. `opened notebook [<id>] not found`, mapped to `invalid_params`) and empty SQL results propagate up unchanged.

**Rationale.** Walking the SiYuan frontend code (`app/src/menus/navigation.ts`, `app/src/layout/dock/Files.ts`) showed that even SiYuan's own UI buries close behind a notebook right-click → Close menu item, and open lives in a collapsible "Closed notebooks" drawer that is empty for users who never closed anything. An agent harness exposing these as first-class operations gives them more weight than the upstream UI does. The kernel-level effect of close (`Unindex` in `kernel/model/mount.go`) is genuine memory release, not a UI flag — auto-opening would silently override a user's deliberate decision to drop a notebook from the working set. Better to fail loudly.

**Tradeoff.** Agents calling `syo_siyuan_doc_create` against a closed notebook see a typed `invalid_params` error rather than a transparent recovery. They cannot programmatically close a notebook to free index memory; if a workflow needs that, it goes through the SiYuan UI.

**Reversal trigger.** A real workflow surfaces where agents legitimately need to manage notebook open/close state — e.g. long-running automation that must reduce kernel memory pressure, or batch operations across notebooks where agent-side close/reopen avoids index thrash.

---

## 3. Keep `notebook_create`

**Context.** Considered dropping `notebook_create` on the same "rare admin op" reasoning that justified removing open/close.

**Decision.** Keep it. CLI `syo notebook create` and MCP `syo_siyuan_notebook_create` stay.

**Rationale.** Unlike open/close — which manage state of *existing* notebooks — creation is a one-shot scaffolding operation. An agent bootstrapping a project structure ("create a new notebook called X, add docs Y and Z") would otherwise need to break the loop and ask the user to open the SiYuan UI. That's a worse experience than letting the agent run.

**Tradeoff.** In some kernel versions the new notebook lands with `closed: true` and is invisible to subsequent `syo_siyuan_doc_get` / `syo_siyuan_sql` reads until the user opens it in the UI. We cannot auto-open (decision #2). The MCP tool description was softened to acknowledge this honestly.

**Reversal trigger.** `notebook_create` produces "ghost" notebooks (created-then-closed) often enough that the create-and-can't-use-it footgun outweighs the bootstrap convenience.

---

## 4. Unified `syo_siyuan_doc_resolve` (replaced `_resolve` + `_hpath_by_id`)

**Context.** Original surface had two separate tools doing inverse lookups: `syo_siyuan_doc_resolve(notebook, hpath) -> { ids: [...] }` and `syo_siyuan_doc_hpath_by_id(id) -> { hpath: "..." }`. The CLI only had the hpath direction. Operations like `syo_siyuan_doc_rename` / `_move` / `_remove` need *storage* `.sy` paths, not hpaths, so callers typically had to follow `syo_siyuan_doc_resolve` with a manual SQL hop to fetch the storage path.

**Decision.** Collapse into one MCP tool / one CLI subcommand named `syo_siyuan_doc_resolve`. Accepts EITHER `id` XOR `(notebook + hpath)`. Returns an array of `ResolvedDoc { id, hpath, notebook_id, notebook_name, title, storage_path }`.

**Rationale.** Two tools doing inverse lookups of the same metadata is duplicate surface. Returning richer metadata up-front saves a SQL round-trip for the common follow-up (rename/move/remove all need storage_path). Mutual-exclusion is enforced in three layers — the `DocLookup` enum makes invalid states unrepresentable inside the library; clap's `ArgGroup` rejects bad CLI input at parse; the MCP handler validates and returns `invalid_params` with a field-naming message. Empty array (not error) on no-match means callers handle "didn't find it" the same way regardless of input direction.

**Tradeoff.** Agents must check `docs.length > 0` rather than rely on errors. The library does one SQL `IN (...)` query joining a single `lsNotebooks` call in-memory; if a notebook id has been removed between calls, `notebook_name` is empty (indistinguishable from a literally-empty notebook name except by cross-checking `syo_siyuan_notebook_ls`). Documented in the field doc-comment.

**Reversal trigger.** Agents commonly want strict "exactly one match" semantics — at which point we'd add a separate tool that errors on `len != 1` rather than burdening the general case.

---

## 5. CLI parity: `syo sql` raw SQL command

**Context.** MCP had `syo_siyuan_sql` as a read-only SQL escape hatch. CLI users had no equivalent and were limited to the typed wrappers (`syo search text`, `syo tag search`, etc.).

**Decision.** Add `syo sql --stmt "<SELECT ...>"`. Pretty-prints rows as JSON. Same read-only semantics as the MCP tool.

**Rationale.** Capability parity. An agent (or a human operator) driving both surfaces should not have to switch transports just to run an ad-hoc query.

**Tradeoff.** Inherits the MCP tool's caveats: not parameterised, callers must escape single quotes themselves; INSERT/UPDATE/DELETE/DDL get rejected by the kernel (the harness does not enforce read-only). Help text states this loudly.

**Reversal trigger.** None foreseen. Removing it would just push users back to `curl` against the kernel HTTP API.

---

## 6. Verbose, agent-friendly help and tool descriptions

**Context.** CLI `--help` and MCP tool descriptions were originally terse — typically a single sentence per command. Agents had to infer the response shape, the difference between sibling tools, and footguns like `path` (storage `.sy`) vs `hpath` (human path).

**Decision.** Every command/tool description must carry five elements:

1. What it does (imperative one-liner).
2. When to use it vs. neighbours (sibling tools/commands named verbatim, with the difference).
3. Input invariants (required/optional, format, mutual-exclusion, path-vs-hpath traps).
4. Minimal in/out example (concrete JSON, or tab-separated literals for tab-printing CLI commands).
5. Async-index lag note for any tool that mutates content or relies on SQL indexing.

Position-aware operations (`block_insert` family, `block_move`, CLI `block insert` / `block move`) get an exhaustive position-kind table that names what `--anchor` must be, what happens to existing siblings, and which kinds the operation does *not* support.

**Rationale.** The audience is an LLM with no codebase context. Humans pick up "the response is wrapped in a `data` envelope" from peripheral cues (sibling tools, examples elsewhere); agents do not. Footguns like path-vs-hpath are easier to dodge when called out *literally* in the description rather than implied by a parameter name. Verbose help is the cheap fix; behavioural changes (e.g. accepting both forms) would be more invasive and reduce explicitness.

**Tradeoff.** Descriptions are 3–5× longer than before. Source files have more inline string literals to maintain — when a behaviour changes, the description must change with it. This is the same maintenance cost as any other authoritative documentation; we accept it because the alternative (out-of-tree docs) drifts faster.

**Reversal trigger.** Agents getting confused *despite* the verbose docs — at that point the answer is probably structured tool annotations / MCP-level metadata, not more prose.

---

## 7. CLI tracing always to stderr

**Context.** `syo` originally used `tracing_subscriber::fmt()` with the default writer, which goes to stdout. After merging `syo-mcp` into `siyuan` (decision #1), the same binary now handles JSON-RPC framing over stdio for `serve-mcp`. Stdout pollution would break the MCP transport.

**Decision.** `init_tracing` writes to stderr unconditionally. All subcommands inherit this.

**Rationale.** Stderr is the canonical destination for log output in a UNIX CLI. Every subcommand prints user-facing output via `println!` (stdout), so the change is invisible to any consumer that wasn't already abusing stdout for both data and logs. Doing it unconditionally avoids a "stderr only when serve-mcp" branch that would introduce its own bug surface.

**Tradeoff.** Users who ran `siyuan ... > log.txt` expecting `tracing` output to land in `log.txt` now need `2> log.txt`. This conforms to convention.

**Reversal trigger.** None — this is the canonical shape.

---

## 8. No `ensure_notebook_*` helper (companion to #2)

**Context.** When dropping `notebook_open` / `notebook_close`, the open question was whether to add a private `ensure_notebook_opened(notebook_id)` helper that high-level operations (`create_doc_with_md`, `get_ids_by_hpath`, etc.) call before forwarding to the kernel.

**Decision.** Do not add such a helper. The library does not silently mutate notebook state.

**Rationale.** Auto-opening hides a user-visible side effect: re-indexing a previously-closed notebook costs memory and time, and it overrides the user's deliberate close. An agent harness should be transparent about which operations have effects beyond the immediate API call. If the harness silently opened notebooks, an agent (or its operator) reading logs would have no signal that this happened.

**Tradeoff.** Operations against closed notebooks fail with kernel errors that the agent must understand. Ergonomics regression for any workflow that wants the auto-open semantic.

**Reversal trigger.** Frequent "I called `syo_siyuan_doc_create` and it failed because the notebook was closed" reports — at which point the right fix is probably a typed error class (`SiyuanError::NotebookClosed`) plus an explicit `--ensure-opened` flag, not a silent helper.

---

## 9. `serve-mcp` token tolerance carve-out

**Context.** Every CLI subcommand requires `SIYUAN_TOKEN` (or `--token`); the strict `Config::resolve` errors out before dispatch when it's missing. The original standalone `syo-mcp` binary, in contrast, warned about a missing token and started anyway — an MCP host might be configured to inject the token at request time rather than at process spawn.

**Decision.** Preserve that behaviour. The `Cmd::ServeMcp` arm runs through a separate `Config::resolve_optional_token` path that accepts `None`. All other subcommands use the strict path.

**Rationale.** Faithful porting of pre-merge behaviour (decision #1 was about *consolidating* binaries, not changing semantics). Hosts that bind `SIYUAN_TOKEN` at MCP request time rather than process spawn would otherwise be broken.

**Tradeoff.** `siyuan --help` shows `--token` as global, but `serve-mcp` actually doesn't require it; small docs inconsistency. The dispatch in `main.rs` has a small early-return shortcut to bypass strict resolve for `ServeMcp`. Code reviewer flagged this as slightly awkward but not blocking.

**Reversal trigger.** Token-at-spawn becomes the only realistic configuration in practice (i.e. no production MCP host actually injects per-request) — at which point we'd unify on the strict path and update the description accordingly.

---

## 10. Ergonomic stage: dual-mode doc locator + format flag + doc tree

**Context.** Dogfooding surfaced three rough edges. (a) `doc rename` / `doc remove` / `doc move` leaked storage paths — every call required a prior `doc resolve` and pasting back unreadable `/<nb>/<doc>.sy` strings. (b) Output-format conventions drifted across read commands: TSV from `notebook ls` / `tag ls` / `tag search` / `search text` / `search blocks`, `json-pretty` from `doc resolve`, JSON-only from `sql`, no opt-in. (c) No `doc tree` primitive existed — agents fell back to raw `syo sql` for filetree navigation. Plus four smaller fixes: missing `tag search --limit`, `block move` position kinds that bailed at runtime, `doc get` returning a misleading "empty doc" message for a missing id, and `sql`'s unverified read-only claim.

**Decision.** Land all seven items (A–G in the spec) as atomic semantic commits. Reuse the `DocLookup` enum and `parse_doc_lookup` helper from `doc resolve` on rename/remove/move, plus a new `resolve_one_storage(client, lookup) -> Result<(NotebookId, String), SiyuanError>` that maps 0 hits to `NotFound` and >1 to `AmbiguousPath`. Add `siyuan-model::doc_tree`, which pulls the subtree via one bounded `SELECT`, builds the tree from `path` parent-prefix relationships, slices to depth, and computes `doc_count_recursive` from the FULL preload so a depth=1 view still reports correct descendant counts. Propagate `--format <agent-md|json|json-pretty>` to the five list commands and `doc resolve`, preserving each command's current default byte shape exactly (`agent-md` is rejected for `doc resolve` since its output is structured metadata).

**Rationale.** Agents should not switch between `--id`, `--notebook --hpath`, and `--path` for what is conceptually the same "address a doc" op; extending `DocLookup` to every doc-mutator collapses the mental model to one-and-done. `doc tree` is a basic navigation primitive whose absence pushed agents into raw SQL for trivial questions — the friction a harness exists to remove. `--format` gives consumers JSON for `jq` while keeping terse defaults for human/LLM reading.

**Tradeoff.** `--path` and `--from-paths` disappear from the CLI/MCP surface entirely; scripts that piped `doc resolve` output into rename/remove/move must migrate to id mode. `storage_path` stays as a *field* on `ResolvedDoc` for `curl`/`jq` users, but no command consumes it as input. `doc tree` does the full-subtree SQL pull even at default depth=1 — slightly more work for very large notebooks, but buys correct `doc_count_recursive` without a second round trip.

**Reversal trigger.** A workflow surfaces where storage-path inputs are essential — e.g. recovering from a corrupted index where `doc resolve` cannot find the doc but the storage path is still usable. At that point, add a `--storage-path` flag as an explicit third locator mode — escape hatch, not default.

---

## 11. SQL read-only enforcement is the harness's job, not the kernel's

**Context.** `syo sql` / `syo_siyuan_sql` originally rejected non-read-only input with a trim/lowercase leading-keyword check (`select` or `with`). The surrounding documentation said "the kernel rejects INSERT/UPDATE/DELETE/DDL", with the implication that our client-side check was just a UX nicety. An empirical probe and a source-code read of the SiYuan kernel together disproved that.

The kernel's `/api/query/sql` handler dispatches to `sql.Query(stmt, limit)`, which tries two SQL parsers in sequence (rqlite/sql for SQLite-leaning input, vitess sqlparser as MySQL-leaning fallback) and falls through to `db.Query(stmt)` for anything that doesn't classify as a `Select`/`Union`. The 88250 fork of `mattn/go-sqlite3` performs a lazy `sqlite3_step` — it only steps when `Rows::Next` is called — so a DML/DDL statement reaching `db.Query` is *prepared* but never *executed* if the kernel's wrapper sees zero result columns and exits early. That is the entire reason a probe of `DELETE FROM blocks WHERE 1=1` left the row count untouched: not because the kernel filters writes, but because three independent components (SQLite parser fall-through, kernel's `nil cols → return`, driver's lazy step) happen to short-circuit before the write effect lands. SiYuan security advisories GHSA-jqwg-75qf-vmf9 and GHSA-j7wh-x834-p3r7 confirm `/api/query/sql` historically *did* execute writes; the current kernel only gates the endpoint via admin-role + non-publish-mode middleware, neither of which is a SQL-level filter.

**Decision.** Treat the harness as the *only* trustworthy gate for read-only SQL. Replace the leading-keyword check with `siyuan-model::sql_guard::validate_read_only`, which parses the statement with `sqlparser` (SQLite dialect, single-statement) and accepts only `Statement::Query` whose `SetExpr` body recurses to `Select` / `Values` / `Table` / set-operations of those, plus `EXPLAIN` of the same. CTE-tail writes (`WITH cte AS (...) DELETE ...`), multi-statement input (`SELECT 1; DROP ...`), `EXPLAIN INSERT ...`, and `PRAGMA writable_schema = 1` are all rejected before any kernel round trip. Help text and MCP tool description both state explicitly that the kernel is NOT a trustworthy second layer at the SQL level.

**Rationale.** A defence-in-depth posture that depends on an undocumented arrangement of three independent components is not defence in depth — it's coincidence. AST-level validation gives a contract we control: today's mechanism is "single Query node, recursing through SetExpr". The cost is a `sqlparser` dep (~1 MB compiled), worth it because the alternative is a guard whose correctness shifts under us when any of {kernel parser dispatch, kernel result-iterator, sqlite3 driver step semantics} change.

**Tradeoff.** Some sqlparser-rs `Statement` variants we did not enumerate in `kind_label` fall through to a debug-rendered short label. Acceptable: the variant identifier is still informative and the catch-all is default-deny. Statements that rely on SQLite-extension syntax sqlparser-rs does not support (rare; mostly window-function corner cases) hit `Parse` rather than `NonReadOnly`, which is a true rejection but with a less helpful message. The kernel's MySQL-flavour fallback parser path (vitess) means callers can still write `CONCAT(...)` / backticks / `LIMIT m, n` and have them parse server-side but execute incorrectly — our guard does not catch that, since we validate only read-only-ness, not dialect compatibility. Help text instructs the user to write SQLite syntax.

**Reversal trigger.** SiYuan adds an in-kernel SQL-level read-only filter that we can verify (e.g. an explicit AST whitelist before `db.Query`). At that point the client-side guard becomes redundant defence and could be relaxed to a fast-fail UX nicety, or the validator could be moved to a feature flag.

---

## 12. Rootless-podman testkit needs explicit workspace cleanup

**Context.** `siyuan-testkit` allocates a `tempfile::TempDir` for each container, bind-mounts it as `/siyuan/workspace:Z`, and relies on `TempDir::Drop` to remove the dir at end of test. The kernel container processes write into that workspace as a sub-UID (rootless podman maps container UIDs through `/etc/subuid`), so files inside appear, from the host's perspective, to be owned by a UID the host user has no permission to touch. `TempDir::Drop` calls `std::fs::remove_dir_all` which silently fails on those files, leaks the dir, and exits cleanly. After a few hundred test runs `/tmp` accumulated 5 GB of orphan workspaces and froze the surrounding shell — `df`, `cargo`, even `echo` failed because `bash` could not allocate scratch. Test processes killed mid-run (Ctrl-C, OOM, panic on another thread) leave the same garbage even more readily, since Drop never fires at all.

**Decision.** Two-part fix in the testkit. (a) `SiyuanContainer::Drop` now takes the `TempWorkspace` out of `TempDir`'s auto-cleanup path via `into_persistent()` and removes the dir via `podman unshare rm -rf`, which re-enters the user namespace where the sub-UID'd files appear as the host user's own. Failure logs the exact recovery command (`podman unshare rm -rf <path>`). (b) New `siyuan_testkit::sweep_stale_workspaces()`, called from `SiyuanContainerBuilder::start()`, scans `std::env::temp_dir()` for `siyuan-testkit-*` entries with mtime older than 10 minutes and removes them via the same `podman unshare rm -rf`. The age floor is long enough never to race with a slow boot of a concurrent test, short enough that a killed session self-heals before the next dev session.

**Rationale.** `TempDir`'s "best-effort cleanup" abstraction is incompatible with rootless podman's UID mapping; pretending otherwise produces a slow-leak that only manifests on the dev's machine, days into a project, as wholly unrelated tooling failures. Explicit `podman unshare rm -rf` matches the way the files were created (through podman's namespace) and is the documented recovery procedure. The sweep on start is belt-and-suspenders for the killed-process path which Drop cannot cover.

**Tradeoff.** The testkit now spawns an extra `podman unshare` subprocess per test (~50 ms). Negligible against the ~2 s container boot. Errors during cleanup are logged at `debug` and swallowed — we never abort a test session because we couldn't delete a stale dir. Stale dirs younger than 10 minutes survive the sweep, so a worst-case crash leaves a fresh leak around for one cycle until the next session reaps it.

**Reversal trigger.** SiYuan testkit migrates to `podman --userns=keep-id` (or equivalent) which maps the container's writes back to the host user's UID directly. At that point `TempDir::Drop` would Just Work and the explicit unshare-rm becomes dead code that can be removed.
