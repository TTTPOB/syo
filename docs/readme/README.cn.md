# siyuan-cli

为 [SiYuan（思源笔记）](https://github.com/siyuan-note/siyuan) 内核 HTTP API 打造的 Agent 友好工具集。**单一二进制**（`siyuan`），基于一份 typed Rust client，同时承担 CLI（人类/脚本使用）与 MCP（Model Context Protocol）服务（供 LLM agent 调用）两种角色 —— MCP 通过 `siyuan serve-mcp` 子命令启动。

底层 crate（`siyuan-types`、`siyuan-client`、`siyuan-model`、`siyuan-render`）也可独立作为库使用。

> 🇬🇧 English version: [`../../README.md`](../../README.md).
> 🧭 设计取舍与回退条件见 [`../decisions.md`](../decisions.md)。

## 状态

v1，单工作区、单用户场景，对应 2026-05 时点的 SiYuan 内核 HTTP API。
内核本身就是事实来源 —— 项目内部不维护本地缓存、snapshot token、两阶段提交。SQL 索引相关的读取（search、tag、`siyuan_sql` 等）是最终一致的，写入后大约 100–500 ms 内可能读到旧数据。

## 前置条件

- 一个在运行中、可通过 HTTP 访问的 SiYuan 内核（默认 `http://127.0.0.1:6806`）。
- 一个 API token，从思源 *设置 → 关于 → API token* 中获取。
- Rust toolchain ≥ 1.85（workspace 已固定 `edition = "2024"`）。
- （可选）Podman，仅当你想跑 `siyuan-testkit` 的集成测试时需要。

## 编译

```sh
cargo build --release
# 二进制输出到 target/release/siyuan
./target/release/siyuan --help
```

本地开发期间，`cargo run -p siyuan-cli -- <args>` 同样可用。

## 配置

| 变量                | 默认值                   | 说明                                                                     |
| ------------------- | ------------------------ | ------------------------------------------------------------------------ |
| `SIYUAN_BASE_URL`   | `http://127.0.0.1:6806`  | 内核 HTTP 根地址。可通过 `--base-url` 覆盖。                             |
| `SIYUAN_TOKEN`      | *（必填）*               | 以 `Authorization: Token <value>` 头发送。除 `serve-mcp` 外所有子命令必填；`serve-mcp` 允许由 MCP host 在请求时再注入。 |
| `SIYUAN_TIMEOUT_MS` | `30000`（`serve-mcp`）   | MCP 服务的单请求超时；`0` 表示不限超时。其他子命令使用 client 默认值。   |
| `RUST_LOG`          | `info`                   | 标准 `tracing-subscriber` 过滤；日志一律写 stderr（保证 `serve-mcp` 的 stdio JSON-RPC 不被污染）。 |

CLI 也支持全局参数 `--base-url` / `--token`，优先级高于环境变量。

## CLI 使用

先做连通性自测：

```sh
export SIYUAN_TOKEN=...你的 token...
siyuan status
# 输出内核版本，例如 3.1.x
```

CLI 是「扁平命令 + 少量子命令组」结构，完整列表请看 `siyuan --help`、`siyuan <cmd> --help`。常用片段：

```sh
# 笔记本
siyuan notebook ls
siyuan notebook create --name "Inbox"
# （open/close 不暴露；如需手动卸载/挂载请走思源 UI）

# 解析文档 —— 支持 --id 或 (--notebook + --hpath) 二选一
siyuan doc resolve --id 20260501090000-doc0001
siyuan doc resolve --notebook 20260501000000-nb00001 --hpath "/Projects/Plan"
# 输出 JSON 数组，每条含 { id, hpath, notebook_id, notebook_name, title, storage_path }

# 列出某个 notebook / 文件夹下的子树
# （--depth 默认 1，可填整数或 `all`；--format 默认 agent-md。）
siyuan doc tree --notebook 20260501000000-nb00001                                # 顶层文档
siyuan doc tree --notebook 20260501000000-nb00001 --hpath /Projects --depth all
siyuan doc tree --id 20260501090000-doc0001 --depth 2 --format json-pretty

# 文档文件树变更 —— 支持 --id 或 (--notebook + --hpath) 二选一。
# 不接受 `.sy` 存储路径，CLI 会在内部完成解析。
siyuan doc rename --id 20260501090000-doc0001 --title "Q3 Plan"
siyuan doc rename --notebook 20260501000000-nb00001 --hpath "/Projects/Plan" --title "Q3 Plan"
siyuan doc remove --id 20260501090000-doc0001
siyuan doc remove --notebook 20260501000000-nb00001 --hpath "/Projects/Plan"
# `doc move`：源地址 --from-ids 与 (--notebook --from-hpaths) 二选一，目的地用 hpath。
siyuan doc move --from-ids 20260501090000-doc0001 \
  --to-notebook 20260501000000-nb00002 --to-path /Archive
siyuan doc move --notebook 20260501000000-nb00001 --from-hpaths /Plan /Notes \
  --to-notebook 20260501000000-nb00002 --to-path /Archive

# 读文档：默认输出 agent-readable markdown，也可输出 JSON
siyuan doc get --id 20260501090000-doc0001
siyuan doc get --id 20260501090000-doc0001 --format json-pretty
siyuan doc get --id 20260501090000-doc0001 --page 2 --page-size 50

# 单个块的原始 kramdown
siyuan block get --id 20260501090000-blk0001

# 从 markdown 文件创建文档（用 `-` 表示读 stdin）
siyuan doc create \
  --notebook 20260501000000-nb00001 \
  --hpath "/Projects/New Page" \
  --markdown-file ./page.md

# 块写入
siyuan block update   --id <block-id> --markdown-file ./new.md
siyuan block insert  --position after_block --anchor <block-id> --markdown-file ./snippet.md
siyuan block move     --id <block-id> --position append_child --anchor <container-id>
siyuan block delete   --id <block-id>
# 注意：block delete 拒绝文档根块（type='d'），请使用 `siyuan doc remove` 删除文档。

# 块属性（自定义键必须 `custom-...`；空值表示删除某个键）
siyuan attrs set --id <block-id> --attr custom-status=done --attr custom-owner=alice

# Tag / 搜索
siyuan tag ls
siyuan tag search --tag project
siyuan search text   --query "load_doc" --limit 20
siyuan search blocks --type h --contains "Roadmap"

# 原生 SQL 逃生通道（只读；调用方自行转义单引号）
siyuan sql --stmt "SELECT id, hpath FROM blocks WHERE type = 'd' LIMIT 5"

# 引用关系图（BFS，最多 N 跳，500 节点 / 1000 边封顶）
siyuan graph backlinks    --id <block-id>
siyuan graph outgoing     --id <block-id>
siyuan graph neighborhood --id <block-id> --depth 2 --direction both

# Asset
siyuan asset upload    --file ./diagram.png
siyuan asset reference --path assets/diagram-20260501-abc.png --alt "Diagram"
```

### 输出格式

`doc get` / `block get` 接受 `--format`：

- `agent-md`（默认）—— markdown，外加 `<!-- sy:doc … -->` / `<!-- sy:block … -->` HTML 注释标记，承载 id、type、分页元数据。设计目标是让 LLM 读完之后能直接生成精确指向某个块的写入指令。
- `json` / `json-pretty` —— 标准结构化 bundle（`DocBundle`），含完整块元数据。
  当文档跨多页时，`agent-md` 输出在最后一个渲染块之后包含 `<!-- sy:page X/Y blocks remaining: Z -->` 页脚。

`notebook ls`、`tag ls`、`tag search`、`search text`、`search blocks` 也接受 `--format`。默认仍为 `agent-md`（即原有 TSV / 每行一项的形式，向后兼容）；`json` / `json-pretty` 输出同字段的结构化数组（分别为 `{status,id,name}`、tag 字符串、`{block_id,markdown_preview}`、`{id,type,markdown_preview}`）。

`doc resolve` 接受 `--format json` / `--format json-pretty`（默认 `json-pretty`），不支持 `agent-md`，因为输出本身就是结构化元数据。`sql` 始终输出 pretty JSON。变更类命令仍只打印一个 id 或 `ok`。

### Position 类型

`block insert` 与 `block move` 共用以下 `--position`：

| kind             | 含义                                | anchor               |
| ---------------- | ----------------------------------- | -------------------- |
| `after_block`    | 插入到 anchor 之后（同级）          | 块 id                |
| `before_block`   | 插入到 anchor 之前（同级）          | 块 id                |
| `append_child`   | 作为容器的最后一个子块              | 容器块 id            |
| `prepend_child`  | 作为容器的第一个子块                | 容器块 id            |
| `append_section` | 作为标题对应章节的最后一块          | 标题块 id            |
| `prepend_section`| 紧跟在标题块之后                    | 标题块 id            |
| `append_doc`     | 作为整篇文档的最后一块              | 文档根 id            |
| `prepend_doc`    | 作为整篇文档的第一块                | 文档根 id            |

`block move` 支持全部 8 种 position。注意：`prepend_child` 与 `prepend_doc` 因 kernel API 限制会将块放在容器末尾；如需严格的首位放置，请后续使用 `after_block` 调整。

## MCP server 使用

`siyuan serve-mcp` 通过 **stdio** 走 JSON-RPC，可接入任何兼容 MCP 的客户端（Claude Desktop、Claude Code、自研 host 等），只需把 SiYuan 的环境变量注入进程即可。

### Claude Desktop / Claude Code

```json
{
  "mcpServers": {
    "siyuan": {
      "command": "/abs/path/to/siyuan",
      "args": ["serve-mcp"],
      "env": {
        "SIYUAN_BASE_URL": "http://127.0.0.1:6806",
        "SIYUAN_TOKEN": "your-token-here"
      }
    }
  }
}
```

如果想为某个 server 单独设置请求超时，把 args 改成 `["serve-mcp", "--timeout-ms", "60000"]`。

暴露的 tool（一行简介，完整 agent-friendly 描述见 `crates/siyuan-mcp/src/registry.rs`）：

| Tool                       | 用途                                                                |
| -------------------------- | ------------------------------------------------------------------- |
| `siyuan_status`            | 内核可达性 + 版本检查。                                             |
| `siyuan_doc_get`           | 读文档，默认 agent-md，支持 JSON 与分页。                           |
| `siyuan_block_get`         | 读单块原始 kramdown。                                               |
| `siyuan_doc_create`        | 用 GFM markdown 创建文档。                                          |
| `siyuan_block_update`      | 整体替换块内容。                                                    |
| `siyuan_block_insert`      | 新增块。                                                            |
| `siyuan_block_move`        | 移动块（保留原 id 与子块）。                                        |
| `siyuan_block_delete`      | 永久删除块及其子树。                                                |
| `siyuan_attrs_get` / `siyuan_attrs_set` | 读 / 增量更新块属性。                                  |
| `siyuan_notebook_ls` / `_create` / `_rename` / `_remove` | 笔记本管理（不暴露 open/close）。      |
| `siyuan_doc_resolve`       | 统一查询：可按 id 或 (notebook + hpath) 反查；返回数组，含 `storage_path`。 |
| `siyuan_doc_tree`          | 以树形列出 notebook / 文件夹子树（id 与 notebook[+hpath] 二选一，`depth` 取 1..N 或 `all`）。 |
| `siyuan_doc_rename` / `_move` / `_remove` | 文件树操作。接受 id 或 (notebook + hpath) 二选一，harness 内部把存储 `.sy` 路径解析出来。 |
| `siyuan_tag_ls` / `siyuan_tag_search` | 列 tag / 按 tag 找块。                                  |
| `siyuan_search_text`       | 在 `blocks` 表上做 LIKE 子串搜索。                                  |
| `siyuan_sql`               | 只读 raw SQL，进阶工具，需自行转义参数。                            |
| `siyuan_asset_upload`      | 上传本地文件为 SiYuan asset。                                       |
| `siyuan_graph_neighborhood`| 引用关系图 BFS（depth ≤ 8，500 节点 / 1000 边封顶）。               |

写入类工具的响应统一被包装成 `{"data": <payload>, "_hint": "..."}`；`_hint` 提示后续行为（SQL 索引滞后、应当跟进调用哪个工具等），仅供参考，不影响正确性。

当内核返回已识别的错误码时，错误会被映射为 typed MCP error（`InvalidParams`、`NotFound`、`Unauthorized` 等）；其余情况以 `InternalError` 透传内核消息。

## 作为库使用

只需一个 typed Rust client 时，直接依赖 `siyuan-client`：

```toml
[dependencies]
siyuan-client = { git = "https://github.com/tpob/siyuan-cli" }
siyuan-types  = { git = "https://github.com/tpob/siyuan-cli" }
tokio = { version = "1", features = ["full"] }
```

```rust
let client = siyuan_client::SiyuanClient::new("http://127.0.0.1:6806", "TOKEN")?;
let v = client.system_version().await?;
println!("kernel = {v}");
```

`siyuan-model` 在此之上提供更高层的流程（`load_doc`、章节切分、分页、引用图 BFS、tag、doc-meta 解析等）；`siyuan-render` 把 `DocBundle` 渲染成 agent-md 或标准 JSON。

## 测试

```sh
cargo test --workspace                          # 单元测试，无需内核
cargo test --workspace -- --ignored --nocapture # 集成测试
```

`--ignored` 的用例会用 `siyuan-testkit` 通过 Podman 起一次性的内核实例，所以需要 `PATH` 上有可用的 `podman`。默认 `cargo test` 因此保持纯本地、可重复。

## 暂未覆盖的部分

v1 显式不做以下事，请按需自行扩展：

- **两阶段 plan / apply、`--dry-run`**：所有写入直接打到内核。
- **并发保护**（`expected_hash`、snapshot token）：最后写入者赢。
- **Notebook open/close**：已从公开表面移除；harness 不处理用户主动关闭的 notebook，详见 `docs/decisions.md §2`。
- **属性视图（AV）写入**：可通过 `siyuan_sql` 读，但不提供写。
- **超级块的创建与 layout 修改**：已存在的超级块以只读 `:::sy-superblock` 围栏形式呈现。
- **WebSocket / 推送通知**：所有调用都是同步 HTTP。
- **历史、回收站、daily note、模板**：思源 UI 已经原生覆盖，这里不重复实现。
- **多工作区切换**：单进程一次只面向一个工作区。
- **本地备份 / 同步编排**。
- **限速与重试策略**：单调用单请求、无 backoff。
- **打包发布**：尚无预编译产物 / Homebrew tap / Docker image，请从源码构建。
- **遥测 / 指标**：仅有 `tracing` 写到 stderr 的日志。

## 仓库结构

```
crates/
  siyuan-types/    # BlockId、BlockType、Position、错误类型 —— 无依赖
  siyuan-client/   # 基于 reqwest 的 typed 内核 HTTP 包装
  siyuan-model/    # DocBundle、load_doc、章节、分页、图 BFS、tag、doc-meta 解析
  siyuan-render/   # agent-md + 标准 JSON 渲染器
  siyuan-cli/      # `siyuan` 二进制（clap），既提供 CLI 也提供 `serve-mcp`
  siyuan-mcp/      # 库 crate；被 siyuan-cli 的 `serve-mcp` 子命令消费
  siyuan-testkit/  # 用 Podman 起一次性 SiYuan 实例
docs/
  decisions.md          # 设计取舍记录
  superpowers/plans/    # 设计与实现计划
  readme/               # 翻译版 README
```

## License

可任选 MIT 或 Apache-2.0。
