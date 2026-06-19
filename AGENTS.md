# 项目结构

本仓库是一个 Rust workspace，所有 crate 都在 `crates/` 下，根 `Cargo.toml`
只负责 workspace 成员、共享版本和共享依赖。

## Workspace Crates

- `crates/siyuan-types`：跨 crate 共享的基础类型，例如 block id、notebook id、位置类型和错误类型。
- `crates/siyuan-client`：SiYuan kernel HTTP API 的 typed client。这里负责请求/响应封装、API endpoint 和 SQL 字符串转义。
- `crates/siyuan-model`：面向业务语义的模型层，组合 client 调用并提供文档加载、分页、文档树、标签、关系图等能力。
- `crates/siyuan-render`：把模型层数据渲染成 agent-md 或 JSON bundle。
- `crates/syo-cli`：`syo` 二进制入口，包含 CLI 命令、输出格式和 MCP stdio 启动入口。
- `crates/syo-mcp`：MCP server、tool registry 和 MCP tool 实现。
- `crates/siyuan-testkit`：基于 Podman 的一次性 SiYuan kernel 测试容器和集成测试工具。

## CLI 入口

`crates/syo-cli/src/main.rs` 只负责：

- 定义顶层 clap 参数和顶层命令枚举。
- 解析配置并创建 `SiyuanClient`。
- 将命令分派到 `crates/syo-cli/src/commands/`。

`crates/syo-cli/src/commands/mod.rs` 只暴露顶层命令模块，并放置跨命令复用的小工具：

- `read_markdown_input`
- `parse_position`

## CLI 命令模块约定

CLI 模块组织必须和命令形式对齐。命令树是模块树的来源。

- 顶层命令使用 `commands/<command>.rs`，或者在有子命令时使用 `commands/<command>/mod.rs`。
- 二级子命令使用 `commands/<command>/<subcommand>.rs`。
- 父命令的 `mod.rs` 定义该命令组的 clap `Subcommand` enum，并只做分派。
- 子命令文件定义自己的 clap args 和 `run` 函数。
- 共享但不直接对应用户命令的实现细节放在同一父目录的私有模块中，例如 `commands/doc/lookup.rs`、`commands/search/hit.rs`。
- 不再把二级命令实现放在 `commands/` 根层，例如不要新增 `get_doc.rs`、`delete_block.rs`、`set_attrs.rs` 这类文件。

当前 CLI 命令模块形状：

```text
crates/syo-cli/src/commands/
├── asset/
│   ├── mod.rs
│   ├── reference.rs
│   └── upload.rs
├── attrs/
│   ├── mod.rs
│   └── set.rs
├── block/
│   ├── delete.rs
│   ├── get.rs
│   ├── insert.rs
│   ├── mod.rs
│   ├── move.rs
│   └── update.rs
├── doc/
│   ├── create.rs
│   ├── get.rs
│   ├── lookup.rs
│   ├── mod.rs
│   ├── move.rs
│   ├── remove.rs
│   ├── rename.rs
│   ├── resolve.rs
│   ├── set_icon.rs
│   ├── set_sort.rs
│   └── tree.rs
├── graph/
│   ├── backlinks.rs
│   ├── mod.rs
│   ├── neighborhood.rs
│   └── outgoing.rs
├── notebook/
│   ├── create.rs
│   ├── ls.rs
│   ├── mod.rs
│   ├── remove.rs
│   └── rename.rs
├── search/
│   ├── blocks.rs
│   ├── hit.rs
│   ├── mod.rs
│   └── text.rs
├── tag/
│   ├── ls.rs
│   ├── mod.rs
│   └── search.rs
├── mod.rs
├── serve_mcp.rs
├── sql.rs
└── status.rs
```

## 测试

- 默认本地测试：`cargo test -p syo-cli`
- 真实 SiYuan kernel 集成测试：`cargo test -p syo-cli -- --ignored --test-threads=1`
- 集成测试通过 `siyuan-testkit` 启动 Podman 容器；需要本机 `podman` 可用。

## 版本 Bump 流程

版本 bump 必须单独提交，不要和功能改动、依赖新增、测试修复混在同一个 commit。

1. 先确认工作区状态和最近提交，确保功能改动已经单独提交：

   ```bash
   git status --short --branch
   git log --oneline -5
   ```

2. 参考上一次 bump 的范围和提交信息：

   ```bash
   git log --oneline --grep='bump version'
   git show --stat --oneline <last-bump-commit>
   ```

3. 更新所有 workspace crate 的 `[package].version` 到目标版本：

   - `crates/siyuan-client/Cargo.toml`
   - `crates/siyuan-model/Cargo.toml`
   - `crates/siyuan-render/Cargo.toml`
   - `crates/siyuan-testkit/Cargo.toml`
   - `crates/siyuan-types/Cargo.toml`
   - `crates/syo-cli/Cargo.toml`
   - `crates/syo-core/Cargo.toml`
   - `crates/syo-mcp/Cargo.toml`

4. 用 Cargo 刷新 `Cargo.lock`，不要盲目全局替换 lockfile 里的版本号：

   ```bash
   cargo check --workspace --all-targets
   ```

5. 检查 diff 只包含预期版本号变化：

   ```bash
   git diff --stat
   git diff -- Cargo.lock crates/*/Cargo.toml
   rg -n '^version = "<old-version>"' crates/*/Cargo.toml Cargo.lock
   ```

6. 跑发布前验证：

   ```bash
   cargo fmt --check
   cargo test -p syo-core
   cargo test -p syo-mcp
   cargo test -p syo-cli
   cargo test -p syo-cli --no-run
   ```

7. 单独提交 bump：

   ```bash
   git add Cargo.lock crates/*/Cargo.toml
   git commit -m "chore: bump version to <new-version>"
   ```

8. push 前最后确认工作区干净、提交顺序正确：

   ```bash
   git status --short --branch
   git log --oneline -5
   git push origin master
   ```
