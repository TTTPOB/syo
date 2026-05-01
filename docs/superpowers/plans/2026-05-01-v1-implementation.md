# SiYuan CLI v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 SiYuan Block Graph Harness 第一版：一个 Rust CLI（`siyuan`），围绕 SiYuan HTTP API 提供面向 Agent 的稳定块级操作；同时把核心模型抽成可复用的 library crates。

**Architecture:** Cargo workspace 多 crate 拆分：`siyuan-types`（无依赖的数据模型）、`siyuan-client`（typed HTTP client）、`siyuan-model`（语义层：bundle / section / container / pagination / graph）、`siyuan-render`（agent-md + JSON 渲染）、`siyuan-cli`（clap 命令层）。CLI 是唯一前端，MCP 留给后续。

**Tech Stack:** rust 2024、tokio、reqwest、serde、clap (derive)、anyhow / thiserror、tracing、insta（端到端 snapshot）、`siyuan-testkit`（来自 test-framework plan）。

**Prerequisites:** test-framework plan 已经执行完毕（`siyuan-testkit` 可用）。

**Out of scope（来自设计讨论的明确否决）:** plan/apply 两阶段、`dry_run` mode、snapshot token、并发保护（`expected_hash`）、超级块创建/layout 修改、AV 编辑、WebSocket、multi-workspace、本地 backup、MCP transport、history/trash（思源原生有）、daily note、template、打包发布。

---

## File Structure

新增/修改的所有 crate 与文件，按 phase 分组：

```
crates/
  siyuan-types/                       # Phase A
    Cargo.toml
    src/
      lib.rs                          # re-exports
      id.rs                           # BlockId, NotebookId, hpath types
      block.rs                        # BlockType, BlockSubtype, BlockRole, BlockNode
      position.rs                     # Position enum (insert/move targets)
      error.rs                        # SiyuanError, ErrorKind
  siyuan-client/                      # Phase B
    Cargo.toml
    src/
      lib.rs
      client.rs                       # SiyuanClient + post helper
      response.rs                     # SiyuanResponse<T> + code → ErrorKind mapping
      api/
        mod.rs
        system.rs                     # version, currentTime
        notebook.rs                   # ls/open/close/create/rename/remove notebook
        filetree.rs                   # createDocWithMd / rename / move / remove / list / getHPathByID / getIDsByHPath
        block.rs                      # get/insert/append/prepend/update/delete/move + getChildBlocks + getBlockKramdown
        attr.rs                       # get/setBlockAttrs
        query.rs                      # sql
        asset.rs                      # upload
        export.rs                     # exportMdContent
  siyuan-model/                       # Phase C
    Cargo.toml
    src/
      lib.rs
      bundle.rs                       # DocBundle, BlockBundle structs
      load.rs                         # load_doc pipeline
      section.rs                      # heading section boundary
      container.rs                    # container vs leaf classification
      pagination.rs                   # 50 blocks/page slicing
      relations.rs                    # relation hints from refs/spans
      graph.rs                        # neighborhood BFS
      tag.rs                          # tag listing/search
  siyuan-render/                      # Phase D
    Cargo.toml
    src/
      lib.rs
      agent_md.rs                     # annotated markdown
      json_bundle.rs                  # canonical JSON
  siyuan-cli/                         # Phase E
    Cargo.toml
    src/
      main.rs
      config.rs                       # env / flag resolution
      output.rs                       # OutputFormat enum (json / agent-md)
      commands/
        mod.rs
        get_doc.rs
        get_block.rs
        create_doc.rs
        update_block.rs
        insert_blocks.rs
        move_block.rs
        delete_block.rs
        set_attrs.rs
        notebook.rs
        doc.rs                        # rename / move / set-icon / set-sort
        tag.rs
        asset.rs
        graph.rs
        search.rs
    tests/
      cli_integration.rs              # uses siyuan-testkit
```

Repo 根 `Cargo.toml`（workspace 已在 test-framework plan 建立）会扩 `members` 列表，并把原本的根 `[package]` / `src/main.rs` 删除（迁移到 `crates/siyuan-cli/`）。

---

## Phases

This plan is split into one file per phase. Execute them in order; each is self-contained and assumes the previous phases are complete.

1. **[Phase A: Foundation](2026-05-01-v1-implementation/phase-a-foundation.md)** — workspace skeleton + `siyuan-types` (BlockId, BlockNode, Position, errors). Tasks A1–A4.
2. **[Phase B: HTTP Client](2026-05-01-v1-implementation/phase-b-client.md)** — `siyuan-client` typed wrapper over the kernel HTTP API. Tasks B1–B5.
3. **[Phase C: Model layer](2026-05-01-v1-implementation/phase-c-model.md)** — `siyuan-model`: bundles, `load_doc`, sections/containers, pagination, relations, tags, graph BFS. Tasks C1–C3.
4. **[Phase D: Render](2026-05-01-v1-implementation/phase-d-render.md)** — `siyuan-render`: agent-md and JSON renderers. Tasks D1–D2.
5. **[Phase E: CLI](2026-05-01-v1-implementation/phase-e-cli.md)** — `siyuan-cli` (binary `siyuan`): every subcommand wired up. Tasks E1–E6.
6. **[Phase F: Integration tests](2026-05-01-v1-implementation/phase-f-integration.md)** — end-to-end tests on top of `siyuan-testkit`. Tasks F1–F2.

---

## Done check

After all tasks:

- [ ] `cargo check --workspace` 通过
- [ ] `cargo test --workspace` 通过（不含 `--ignored`，应该全部是 unit 测试）
- [ ] `cargo test --workspace -- --ignored --nocapture` 通过（含 testkit smoke + cli integration）
- [ ] `./target/debug/siyuan --help` 列出全部 15 个子命令
- [ ] `git log --oneline | wc -l` 至少 ~17 个新 commit
- [ ] `Cargo.toml`(workspace 根) `members` = `["crates/*"]`，包含 6 个 crate（types, client, model, render, cli, testkit）

后续阶段（不在本 plan 内）：MCP transport、AV 编辑、超级块创建、history/trash 包装、daily note、template、性能/rate-limit。
