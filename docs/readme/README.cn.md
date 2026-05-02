# siyuan-cli

为 [SiYuan（思源笔记）](https://github.com/siyuan-note/siyuan) 内核 HTTP API 打造的 Agent 友好工具集。在统一的 Rust typed client 之上提供两个二进制：

- **`siyuan`** —— 命令行工具，针对运行中的内核做块级读写。
- **`siyuan-mcp`** —— Model Context Protocol 服务（stdio transport），把同样的能力以 MCP tool 的形式暴露给 LLM Agent。

底层 crate（`siyuan-types`、`siyuan-client`、`siyuan-model`、`siyuan-render`）也可独立作为库使用。

> 🇬🇧 English version: [`../../README.md`](../../README.md).

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
# 二进制输出到 target/release/
./target/release/siyuan --help
./target/release/siyuan-mcp --help
```

本地开发期间，`cargo run -p siyuan-cli -- <args>`、`cargo run -p siyuan-mcp` 同样可用。

## 配置

两个二进制读取相同的环境变量：

| 变量                | 默认值                   | 说明                                                        |
| ------------------- | ------------------------ | ----------------------------------------------------------- |
| `SIYUAN_BASE_URL`   | `http://127.0.0.1:6806`  | 内核 HTTP 根地址。可通过 `--base-url` 覆盖。                |
| `SIYUAN_TOKEN`      | *（必填）*               | 以 `Authorization: Token <value>` 头发送。                  |
| `SIYUAN_TIMEOUT_MS` | `30000`（`siyuan-mcp`）  | 单请求超时；`0` 表示不限超时。                              |
| `RUST_LOG`          | `info`                   | 标准 `tracing-subscriber` 过滤；日志走 stderr。             |

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
siyuan notebook open  --id 20260501000000-nb00001

# 通过人类可读路径解析文档 id
siyuan doc resolve --notebook 20260501000000-nb00001 --hpath "/Projects/Plan"

# 读文档：默认输出 agent-readable markdown，也可输出 JSON
siyuan get-doc --id 20260501090000-doc0001
siyuan get-doc --id 20260501090000-doc0001 --format json-pretty
siyuan get-doc --id 20260501090000-doc0001 --page 2 --page-size 50

# 单个块的原始 kramdown
siyuan get-block --id 20260501090000-blk0001

# 从 markdown 文件创建文档（用 `-` 表示读 stdin）
siyuan create-doc \
  --notebook 20260501000000-nb00001 \
  --hpath "/Projects/New Page" \
  --markdown-file ./page.md

# 块写入
siyuan update-block   --id <block-id> --markdown-file ./new.md
siyuan insert-blocks  --position after_block --anchor <block-id> --markdown-file ./snippet.md
siyuan move-block     --id <block-id> --position append_child --anchor <container-id>
siyuan delete-block   --id <block-id>

# 块属性（自定义键必须 `custom-...`；空值表示删除某个键）
siyuan set-attrs --id <block-id> --attr custom-status=done --attr custom-owner=alice

# Tag / 搜索
siyuan tag ls
siyuan tag search --tag project
siyuan search text   --query "load_doc" --limit 20
siyuan search blocks --type h --contains "Roadmap"

# 引用关系图（BFS，最多 N 跳，500 节点 / 1000 边封顶）
siyuan graph backlinks    --id <block-id>
siyuan graph outgoing     --id <block-id>
siyuan graph neighborhood --id <block-id> --depth 2 --direction both

# Asset
siyuan asset upload    --file ./diagram.png
siyuan asset reference --path assets/diagram-20260501-abc.png --alt "Diagram"
```

### 输出格式

`get-doc` / `get-block` 接受 `--format`：

- `agent-md`（默认）—— markdown，外加 `<!-- sy:doc … -->` / `<!-- sy:block … -->` HTML 注释标记，承载 id、type、分页元数据。设计目标是让 LLM 读完之后能直接生成精确指向某个块的写入指令。
- `json` / `json-pretty` —— 标准结构化 bundle（`DocBundle`），含完整块元数据。

其他命令统一打印一个 id 或制表符分隔的列表，方便管道。

### Position 类型

`insert-blocks` 与 `move-block` 共用以下 `--position`：

| kind             | 含义                                | anchor               |
| ---------------- | ----------------------------------- | -------------------- |
| `after_block`    | 插入到 anchor 之后（同级）          | 块 id                |
| `before_block`   | 插入到 anchor 之前（同级）          | 块 id（仅 insert）   |
| `append_child`   | 作为容器的最后一个子块              | 容器块 id            |
| `prepend_child`  | 作为容器的第一个子块                | 容器块 id            |
| `append_section` | 作为标题对应章节的最后一块          | 标题块 id（仅 insert）|
| `prepend_section`| 紧跟在标题块之后                    | 标题块 id（仅 insert）|
| `append_doc`     | 作为整篇文档的最后一块              | 文档根 id            |
| `prepend_doc`    | 作为整篇文档的第一块                | 文档根 id            |

`move-block` 在 v1 不支持 `before_block` 与 `*_section`，请改写成「上一兄弟块的 `after_block`」。

## MCP server 使用

`siyuan-mcp` 通过 **stdio** 走 JSON-RPC，可接入任何兼容 MCP 的客户端（Claude Desktop、Claude Code、自研 host 等），只需把 SiYuan 的环境变量注入进程即可。

### Claude Desktop / Claude Code

```json
{
  "mcpServers": {
    "siyuan": {
      "command": "/abs/path/to/siyuan-mcp",
      "env": {
        "SIYUAN_BASE_URL": "http://127.0.0.1:6806",
        "SIYUAN_TOKEN": "your-token-here"
      }
    }
  }
}
```

暴露的 tool（一行简介，完整描述见 `crates/siyuan-mcp/src/registry.rs`）：

| Tool                       | 用途                                                  |
| -------------------------- | ----------------------------------------------------- |
| `siyuan_status`            | 内核可达性 + 版本检查。                               |
| `siyuan_get_doc`           | 读文档，默认 agent-md，支持 JSON 与分页。             |
| `siyuan_get_block`         | 读单块原始 kramdown。                                 |
| `siyuan_create_doc`        | 用 GFM markdown 创建文档。                            |
| `siyuan_update_block`      | 整体替换块内容。                                      |
| `siyuan_insert_block` / `siyuan_append_block` / `siyuan_prepend_block` | 新增块。 |
| `siyuan_move_block`        | 移动块（保留原 id 与子块）。                          |
| `siyuan_delete_block`      | 永久删除块及其子树。                                  |
| `siyuan_get_attrs` / `siyuan_set_attrs` | 读 / 增量更新块属性。                    |
| `siyuan_notebook_ls` / `_open` / `_close` / `_create` / `_rename` / `_remove` | 笔记本管理。 |
| `siyuan_doc_resolve` / `_hpath_by_id` / `_rename` / `_move` / `_remove` | 文件树（注意：`_rename` / `_move` / `_remove` 接受存储 `.sy` 路径，不是 hpath）。 |
| `siyuan_tag_ls` / `siyuan_tag_search` | 列 tag / 按 tag 找块。                     |
| `siyuan_search_text`       | 在 `blocks` 表上做 LIKE 子串搜索。                    |
| `siyuan_sql`               | 只读 raw SQL，进阶工具，需自行转义参数。              |
| `siyuan_asset_upload`      | 上传本地文件为 SiYuan asset。                         |
| `siyuan_graph_neighborhood`| 引用关系图 BFS（depth ≤ 8，500 节点 / 1000 边封顶）。 |

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

`siyuan-model` 在此之上提供更高层的流程（`load_doc`、章节切分、分页、引用图 BFS、tag 等）；`siyuan-render` 把 `DocBundle` 渲染成 agent-md 或标准 JSON。

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
  siyuan-model/    # DocBundle、load_doc、章节、分页、图 BFS、tag
  siyuan-render/   # agent-md + 标准 JSON 渲染器
  siyuan-cli/      # `siyuan` 二进制（clap）
  siyuan-mcp/      # `siyuan-mcp` 二进制（rmcp，stdio transport）
  siyuan-testkit/  # 用 Podman 起一次性 SiYuan 实例
docs/
  superpowers/plans/  # 设计与实现计划
  readme/             # 翻译版 README
```

## License

可任选 MIT 或 Apache-2.0。
