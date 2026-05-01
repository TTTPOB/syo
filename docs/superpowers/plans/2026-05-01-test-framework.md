# SiYuan CLI 测试框架 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立一个 `siyuan-testkit` crate，让后续每个集成测试都能用一行 `let sy = SiyuanContainer::start().await?;` 拿到一个干净的、带已知 API token 的 SiYuan 实例（podman 容器），结束时自动销毁。

**Architecture:** 单 cargo workspace；`siyuan-testkit` 是独立 crate，不依赖业务 client。容器生命周期通过直接调用本机 `podman` CLI（`std::process::Command`）管理，不用 testcontainers crate（规避 Docker socket vs Podman socket 的环境差异）。鉴权通过启动前在挂载 workspace 中预写 `conf/conf.json` 注入固定 token。

**Tech Stack:** rust 2024 edition、tokio、reqwest、serde / serde_json、tempfile、tracing、tracing-subscriber、insta、anyhow、podman CLI。

**Known unknown:** SiYuan kernel 启动时是否真会读取 `conf/conf.json` 里 `api.token` 字段、首次启动是否会覆盖该字段，需要在 Task 6 的 spike 测试中验证。如果不行，回退方案是 Task 7 启动后用 `--accessAuthCode` + 一个临时小脚本通过 `/api/setting/setAPIToken`（如该 endpoint 存在）写入 token。

**Out of scope:** fixture 数据 seeding（依赖业务 client，放到 v1 plan）、CI workflow（本地 podman 跑通即可）、容器复用优化（每个测试一个新容器，简单优先）。

---

## File Structure

```
/home/tpob/playground/siyuan-cli/
├── Cargo.toml                              # 改造为 workspace 根
├── .gitignore                              # 加 target/, *.snap.new 等
├── crates/
│   └── siyuan-testkit/
│       ├── Cargo.toml
│       ├── README.md                       # 短说明
│       └── src/
│           ├── lib.rs                      # 重导出 + tracing 初始化 helper
│           ├── port.rs                     # 端口分配
│           ├── workspace.rs                # 临时 workspace + conf.json 预写
│           ├── podman.rs                   # podman CLI 薄封装
│           ├── container.rs                # SiyuanContainer 主类型 + Drop
│           └── health.rs                   # /api/system/version 健康检查
└── tests/
    └── (无；testkit 自己的测试在 crates/siyuan-testkit/tests/)
```

`crates/siyuan-cli/` 暂不创建（保留原 `src/main.rs` 由 v1 plan 的 Task 1 移动）。

---

## Task 1: 把仓库改造为 cargo workspace

**Files:**
- Modify: `Cargo.toml` (整体重写)
- Create: `.gitignore` (覆盖原有 1 行的 `.gitignore`，扩充)
- Move: `src/main.rs` → 暂时保留原位，下个 plan 再移走

**Background:** 当前 `Cargo.toml` 是单 crate（package = siyuan-cli）。我们要让它变成 workspace 根，`siyuan-cli` 改名暂存为 root binary（v1 plan 会迁到 `crates/siyuan-cli/`）。

- [ ] **Step 1: 备份并重写 Cargo.toml**

写入：

```toml
[workspace]
members = ["crates/*"]
resolver = "3"

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/tpob/siyuan-cli"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tempfile = "3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
thiserror = "2"
insta = { version = "1", features = ["yaml", "json"] }

# 临时保留旧的 binary 直到 v1 plan 把它迁走
[package]
name = "siyuan-cli"
version = "0.1.0"
edition = "2024"

[dependencies]
```

- [ ] **Step 2: 扩充 .gitignore**

写入：

```
/target
**/*.rs.bk
*.snap.new
.DS_Store
```

- [ ] **Step 3: 验证 workspace 解析**

Run: `cargo metadata --no-deps --format-version 1 | head -c 200`

Expected: 输出 JSON 含 `"workspace_root"` 字段，无错误。

- [ ] **Step 4: 提交**

```bash
git add Cargo.toml .gitignore
git commit -m "chore: convert repo into a cargo workspace"
```

---

## Task 2: 创建 `siyuan-testkit` crate 骨架

**Files:**
- Create: `crates/siyuan-testkit/Cargo.toml`
- Create: `crates/siyuan-testkit/src/lib.rs`

- [ ] **Step 1: 用 cargo 生成 crate**

Run: `cargo new --lib crates/siyuan-testkit --name siyuan-testkit --vcs none`

Expected: 输出 `Creating library 'siyuan-testkit' package`，新增 `crates/siyuan-testkit/{Cargo.toml, src/lib.rs}`。

- [ ] **Step 2: 改写 `crates/siyuan-testkit/Cargo.toml`**

写入：

```toml
[package]
name = "siyuan-testkit"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Spin up disposable SiYuan instances in Podman for integration tests."

[dependencies]
tokio = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tempfile = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
insta = { workspace = true }
```

- [ ] **Step 3: 改写 `crates/siyuan-testkit/src/lib.rs`**

写入：

```rust
//! Spin up disposable SiYuan instances in Podman for integration tests.
//!
//! Typical usage:
//! ```no_run
//! # use siyuan_testkit::SiyuanContainer;
//! # async fn demo() -> anyhow::Result<()> {
//! let sy = SiyuanContainer::start().await?;
//! let url = sy.base_url();
//! let token = sy.token();
//! // ... call SiYuan HTTP API ...
//! // the container is destroyed when sy goes out of scope
//! # Ok(())
//! # }
//! ```

mod container;
mod health;
mod podman;
mod port;
mod workspace;

pub use container::{SiyuanContainer, SiyuanContainerBuilder};

/// Initialise tracing for tests. Idempotent — safe to call from every test.
pub fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,siyuan_testkit=debug")),
            )
            .with_test_writer()
            .try_init();
    });
}
```

- [ ] **Step 4: 验证编译**

Run: `cargo check -p siyuan-testkit`

Expected: 因为 `mod container/health/podman/port/workspace` 引用的文件还不存在会失败。这是预期的 —— 接下来的任务会创建这些模块。我们用 stub 让它先编译过，便于后续增量。

- [ ] **Step 5: 给每个 mod 写空 stub**

Create: `crates/siyuan-testkit/src/port.rs`
```rust
// stub, populated in Task 3
```

Create: `crates/siyuan-testkit/src/workspace.rs`
```rust
// stub, populated in Task 4
```

Create: `crates/siyuan-testkit/src/podman.rs`
```rust
// stub, populated in Task 5
```

Create: `crates/siyuan-testkit/src/health.rs`
```rust
// stub, populated in Task 6
```

Create: `crates/siyuan-testkit/src/container.rs`
```rust
// stub, populated in Task 7

#[derive(Debug)]
pub struct SiyuanContainer;

#[derive(Debug, Default)]
pub struct SiyuanContainerBuilder;
```

- [ ] **Step 6: 再次验证编译**

Run: `cargo check -p siyuan-testkit`

Expected: 警告允许，但 `Finished` 成功，无 error。

- [ ] **Step 7: 提交**

```bash
git add crates/siyuan-testkit
git commit -m "feat(testkit): scaffold siyuan-testkit crate"
```

---

## Task 3: 端口分配模块

**Files:**
- Modify: `crates/siyuan-testkit/src/port.rs`
- Test: 同文件 `#[cfg(test)] mod tests`

**Approach:** 用 `TcpListener::bind("127.0.0.1:0")` 让 OS 分配空闲端口，立即关闭，把端口号交给 podman。存在小竞态窗口但对集成测试可接受。

- [ ] **Step 1: 写失败测试**

写入 `crates/siyuan-testkit/src/port.rs`：

```rust
use std::net::TcpListener;

/// Ask the OS for an unused TCP port on 127.0.0.1, then immediately release it.
///
/// There is a tiny race window between releasing and the caller binding, but it is
/// acceptable for test orchestration.
pub fn allocate_loopback_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_unique_ports() {
        let a = allocate_loopback_port().unwrap();
        let b = allocate_loopback_port().unwrap();
        assert_ne!(a, b, "two consecutive allocations should not collide");
        assert!(a >= 1024, "should be in the unprivileged range, got {a}");
    }

    #[test]
    fn allocated_port_is_actually_bindable() {
        let port = allocate_loopback_port().unwrap();
        let _bound = TcpListener::bind(("127.0.0.1", port))
            .expect("port returned by allocate_loopback_port should be bindable");
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p siyuan-testkit port::`

Expected: 2 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/src/port.rs
git commit -m "feat(testkit): allocate ephemeral loopback ports"
```

---

## Task 4: 临时 workspace 模块（含 conf.json 预写）

**Files:**
- Modify: `crates/siyuan-testkit/src/workspace.rs`

**Background:** 思源 kernel 启动时读取 `<workspace>/conf/conf.json`。我们在挂载前把这个文件写入一个固定 API token，让 SiYuan 启动后 HTTP API 直接可以用 `Authorization: Token <token>` 调用。conf.json 的结构猜测如下（在 Task 12 的 spike 测试中验证）：

```json
{
  "api": { "token": "<our token>" },
  "system": { "uploadErrLog": false, "downloadInstallPkg": false }
}
```

如果 SiYuan 启动时覆盖此文件，记录到 README known-issues 段，后续再 patch。

- [ ] **Step 1: 写实现 + 测试**

写入 `crates/siyuan-testkit/src/workspace.rs`：

```rust
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tempfile::TempDir;

/// A scratch SiYuan workspace: a `tempfile::TempDir` with `conf/conf.json`
/// pre-populated so the kernel boots with a known API token.
#[derive(Debug)]
pub struct TempWorkspace {
    dir: TempDir,
    token: String,
}

impl TempWorkspace {
    /// Create a workspace with a freshly generated UUID-style token.
    pub fn new() -> Result<Self> {
        let token = generate_token();
        Self::with_token(token)
    }

    /// Create a workspace using a caller-supplied token. Useful for tests that
    /// want a deterministic token in snapshots.
    pub fn with_token(token: impl Into<String>) -> Result<Self> {
        let dir = tempfile::Builder::new()
            .prefix("siyuan-testkit-")
            .tempdir()
            .context("creating tempdir for SiYuan workspace")?;

        let token = token.into();
        write_conf_json(dir.path(), &token)?;
        Ok(Self { dir, token })
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Forget the TempDir without deleting it. Useful for debugging a failed
    /// integration test by inspecting the workspace contents.
    pub fn into_persistent(self) -> PathBuf {
        self.dir.keep()
    }
}

fn write_conf_json(workspace: &Path, token: &str) -> Result<()> {
    let conf_dir = workspace.join("conf");
    std::fs::create_dir_all(&conf_dir).context("creating conf/ inside workspace")?;
    let conf = serde_json::json!({
        "api": { "token": token },
    });
    let conf_path = conf_dir.join("conf.json");
    std::fs::write(&conf_path, serde_json::to_vec_pretty(&conf)?)
        .with_context(|| format!("writing {}", conf_path.display()))?;
    Ok(())
}

fn generate_token() -> String {
    // Deterministic-style format: timestamp-random8. No external uuid dep.
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("tk-{now:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_conf_json_with_token() {
        let ws = TempWorkspace::with_token("token-abc").unwrap();
        let conf_path = ws.path().join("conf").join("conf.json");
        assert!(conf_path.exists(), "conf.json should exist");

        let raw = std::fs::read_to_string(&conf_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["api"]["token"].as_str(), Some("token-abc"));
    }

    #[test]
    fn generated_tokens_differ() {
        let a = TempWorkspace::new().unwrap();
        let b = TempWorkspace::new().unwrap();
        assert_ne!(a.token(), b.token());
    }

    #[test]
    fn workspace_is_cleaned_up_on_drop() {
        let path = {
            let ws = TempWorkspace::new().unwrap();
            ws.path().to_path_buf()
        };
        assert!(!path.exists(), "tempdir should be removed when TempWorkspace drops");
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p siyuan-testkit workspace::`

Expected: 3 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/src/workspace.rs
git commit -m "feat(testkit): pre-write conf.json into a temp workspace"
```

---

## Task 5: Podman CLI 薄封装

**Files:**
- Modify: `crates/siyuan-testkit/src/podman.rs`

**Approach:** 不用 podman 的 REST API。直接 `std::process::Command::new("podman")`，每个动作一个函数。先写阻塞版本（`Command::output()`），调用方在 tokio 中通过 `tokio::task::spawn_blocking` 包裹（Task 6 处理）。

- [ ] **Step 1: 写实现 + 测试**

写入 `crates/siyuan-testkit/src/podman.rs`：

```rust
use std::ffi::OsStr;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use tracing::debug;

/// Confirm the local `podman` binary is available. Call once before any container
/// operations to fail loudly on misconfigured CI / dev machines.
pub fn require_podman() -> Result<()> {
    let out = Command::new("podman")
        .arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("failed to spawn `podman`. Is it installed and on PATH?")?;
    if !out.status.success() {
        bail!(
            "`podman --version` exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    debug!(version = %String::from_utf8_lossy(&out.stdout).trim(), "podman ok");
    Ok(())
}

/// `podman run -d ...`. Returns the container ID (full hex string, trimmed).
pub fn run_detached<I, S>(args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Command::new("podman")
        .arg("run")
        .arg("-d")
        .args(args)
        .output()
        .context("spawning `podman run -d`")?;
    if !out.status.success() {
        bail!(
            "`podman run -d` exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `podman stop --time=<timeout> <id>`. Best-effort; does not error if container
/// is already gone.
pub fn stop(container_id: &str, timeout_secs: u32) -> Result<()> {
    let out = Command::new("podman")
        .args(["stop", "--time", &timeout_secs.to_string(), container_id])
        .output()
        .context("spawning `podman stop`")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        if err.contains("no such container") {
            return Ok(());
        }
        bail!("`podman stop` exited {}: {err}", out.status);
    }
    Ok(())
}

/// `podman rm -f <id>`. Best-effort.
pub fn force_remove(container_id: &str) -> Result<()> {
    let out = Command::new("podman")
        .args(["rm", "-f", container_id])
        .output()
        .context("spawning `podman rm -f`")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        if err.contains("no such container") {
            return Ok(());
        }
        bail!("`podman rm -f` exited {}: {err}", out.status);
    }
    Ok(())
}

/// `podman logs <id>`. Returns combined stdout + stderr.
pub fn logs(container_id: &str) -> Result<String> {
    let out = Command::new("podman")
        .args(["logs", container_id])
        .output()
        .context("spawning `podman logs`")?;
    let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn require_podman_succeeds_when_installed() {
        require_podman().expect("podman must be installed locally to run testkit tests");
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test -p siyuan-testkit podman::`

Expected: 1 passed (前提：本机 `podman` 在 PATH 中)。如果失败，安装 podman 后重试。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/src/podman.rs
git commit -m "feat(testkit): wrap podman CLI for run/stop/rm/logs"
```

---

## Task 6: 健康检查模块

**Files:**
- Modify: `crates/siyuan-testkit/src/health.rs`

**Background:** SiYuan kernel 提供 `/api/system/version`（POST，可空 body），返回 `{"code":0,"msg":"","data":"<version>"}`。鉴权头加上以验证 token 也已生效。

- [ ] **Step 1: 写实现**

写入 `crates/siyuan-testkit/src/health.rs`：

```rust
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use reqwest::Client;
use tracing::{debug, warn};

/// Poll `<base>/api/system/version` until it returns code=0 with the supplied
/// `Authorization: Token <token>` header, or `timeout` elapses.
pub async fn wait_for_ready(base_url: &str, token: &str, timeout: Duration) -> Result<String> {
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5))
        .build()
        .context("building reqwest client")?;

    let url = format!("{}/api/system/version", base_url.trim_end_matches('/'));
    let started = Instant::now();
    let mut attempt = 0u32;

    loop {
        attempt += 1;
        match probe(&client, &url, token).await {
            Ok(version) => {
                debug!(attempts = attempt, version = %version, "siyuan is ready");
                return Ok(version);
            }
            Err(err) => {
                if started.elapsed() >= timeout {
                    bail!(
                        "siyuan never became ready within {:?} ({} attempts). last error: {err:#}",
                        timeout, attempt
                    );
                }
                if attempt % 10 == 0 {
                    warn!(attempts = attempt, ?err, "still waiting for siyuan");
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

async fn probe(client: &Client, url: &str, token: &str) -> Result<String> {
    let resp = client
        .post(url)
        .header("Authorization", format!("Token {token}"))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .context("HTTP send")?;
    let status = resp.status();
    let body = resp.text().await.context("HTTP read body")?;
    if !status.is_success() {
        bail!("HTTP {status}: {body}");
    }
    let parsed: serde_json::Value = serde_json::from_str(&body).context("parsing response")?;
    let code = parsed.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        bail!("api code {code}: {body}");
    }
    let version = parsed
        .get("data")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    Ok(version)
}
```

- [ ] **Step 2: cargo check**

Run: `cargo check -p siyuan-testkit`

Expected: 编译通过；不写单元测试，因为它需要真容器，由 Task 8 的集成测试覆盖。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/src/health.rs
git commit -m "feat(testkit): poll /api/system/version for readiness"
```

---

## Task 7: `SiyuanContainer` 主类型 + Drop 清理

**Files:**
- Modify: `crates/siyuan-testkit/src/container.rs`

**Background:** 把前面的部件粘起来：分配端口 → 建 workspace → 拼 podman run 参数 → 启动 → 健康检查 → 暴露 base_url/token。Drop 时强制 `podman rm -f`，即使测试 panic 也保证清理。

镜像版本通过 env `SIYUAN_TEST_IMAGE` 控制，默认 `b3log/siyuan:latest`（README 里建议用户 pin）。

- [ ] **Step 1: 写实现**

替换 `crates/siyuan-testkit/src/container.rs` 整个文件：

```rust
use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{debug, error, info};

use crate::health::wait_for_ready;
use crate::podman;
use crate::port::allocate_loopback_port;
use crate::workspace::TempWorkspace;

const DEFAULT_IMAGE: &str = "b3log/siyuan:latest";
const DEFAULT_READY_TIMEOUT: Duration = Duration::from_secs(60);
const STOP_TIMEOUT_SECS: u32 = 5;

#[derive(Debug, Clone)]
pub struct SiyuanContainerBuilder {
    image: String,
    ready_timeout: Duration,
    access_auth_code: String,
}

impl Default for SiyuanContainerBuilder {
    fn default() -> Self {
        Self {
            image: std::env::var("SIYUAN_TEST_IMAGE").unwrap_or_else(|_| DEFAULT_IMAGE.into()),
            ready_timeout: DEFAULT_READY_TIMEOUT,
            access_auth_code: "testkit".into(),
        }
    }
}

impl SiyuanContainerBuilder {
    pub fn image(mut self, image: impl Into<String>) -> Self {
        self.image = image.into();
        self
    }
    pub fn ready_timeout(mut self, timeout: Duration) -> Self {
        self.ready_timeout = timeout;
        self
    }
    pub fn access_auth_code(mut self, code: impl Into<String>) -> Self {
        self.access_auth_code = code.into();
        self
    }

    pub async fn start(self) -> Result<SiyuanContainer> {
        podman::require_podman()?;

        let workspace = TempWorkspace::new()?;
        let port = allocate_loopback_port()?;
        let base_url = format!("http://127.0.0.1:{port}");

        let workspace_path = workspace.path().to_string_lossy().into_owned();
        let port_mapping = format!("127.0.0.1:{port}:6806");
        let access_flag = format!("--accessAuthCode={}", self.access_auth_code);

        info!(image = %self.image, port, workspace = %workspace_path, "starting siyuan container");

        let id = tokio::task::spawn_blocking(move || {
            podman::run_detached([
                "--rm",
                "-v",
                &format!("{workspace_path}:/siyuan/workspace:Z"),
                "-p",
                &port_mapping,
                &self.image,
                "--workspace=/siyuan/workspace",
                &access_flag,
            ])
        })
        .await
        .context("spawn_blocking podman run panicked")??;

        debug!(container_id = %id, "podman run returned");

        let token = workspace.token().to_string();
        let container = SiyuanContainer {
            container_id: id,
            base_url,
            token,
            workspace: Some(workspace),
            disarmed: false,
        };

        match wait_for_ready(&container.base_url, &container.token, self.ready_timeout).await {
            Ok(version) => {
                info!(version = %version, "siyuan ready");
                Ok(container)
            }
            Err(err) => {
                error!(?err, "siyuan failed to become ready; dumping logs");
                let logs = tokio::task::spawn_blocking({
                    let id = container.container_id.clone();
                    move || podman::logs(&id)
                })
                .await
                .ok()
                .and_then(|r| r.ok())
                .unwrap_or_default();
                error!(%logs, "container logs at failure");
                drop(container); // triggers cleanup
                Err(err)
            }
        }
    }
}

#[derive(Debug)]
pub struct SiyuanContainer {
    container_id: String,
    base_url: String,
    token: String,
    workspace: Option<TempWorkspace>,
    disarmed: bool,
}

impl SiyuanContainer {
    /// Convenience entry point with all defaults.
    pub async fn start() -> Result<Self> {
        SiyuanContainerBuilder::default().start().await
    }

    pub fn builder() -> SiyuanContainerBuilder {
        SiyuanContainerBuilder::default()
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
    pub fn workspace_path(&self) -> Option<&std::path::Path> {
        self.workspace.as_ref().map(|w| w.path())
    }

    /// Persist the workspace dir for post-mortem inspection. The container is
    /// still removed on drop, but the on-disk files survive.
    pub fn persist_workspace_on_drop(&mut self) {
        if let Some(ws) = self.workspace.take() {
            let path = ws.into_persistent();
            tracing::warn!(workspace = %path.display(), "TempWorkspace persisted; clean it up manually");
        }
    }

    /// Skip Drop-time cleanup. Useful when testing the cleanup logic itself.
    /// Caller is responsible for `podman rm -f` afterwards.
    pub fn disarm_for_testing(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for SiyuanContainer {
    fn drop(&mut self) {
        if self.disarmed {
            return;
        }
        let id = self.container_id.clone();
        debug!(container_id = %id, "stopping siyuan container");
        if let Err(err) = podman::stop(&id, STOP_TIMEOUT_SECS) {
            tracing::warn!(?err, "podman stop failed; will force rm");
        }
        if let Err(err) = podman::force_remove(&id) {
            tracing::warn!(?err, "podman rm -f failed");
        }
    }
}
```

- [ ] **Step 2: cargo check**

Run: `cargo check -p siyuan-testkit`

Expected: 编译通过。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/src/container.rs
git commit -m "feat(testkit): SiyuanContainer lifecycle with Drop cleanup"
```

---

## Task 8: 集成测试 — smoke

**Files:**
- Create: `crates/siyuan-testkit/tests/smoke.rs`

**Background:** 这是第一个真正启动思源容器的测试。它验证：image 拉得到、容器启动、conf.json 注入的 token 生效、`/api/system/version` 返回 code=0。如果失败，最大可能性是 `conf/conf.json` 字段不被识别（known unknown），需要 spike 调整 Task 4 的 conf 结构。

由于启动一次 SiYuan 大约需要 10–30s，这个测试默认不在 `cargo test` 跑，靠 `--ignored` 或自定义 feature 触发，避免 `cargo check` 工作流变慢。

- [ ] **Step 1: 写测试**

写入 `crates/siyuan-testkit/tests/smoke.rs`：

```rust
//! Smoke test: actually boot SiYuan inside Podman.
//!
//! Run with: `cargo test -p siyuan-testkit --test smoke -- --ignored --nocapture`

use std::time::Duration;

use reqwest::Client;
use siyuan_testkit::{SiyuanContainer, init_tracing};

#[tokio::test]
#[ignore = "starts a real podman container; opt-in"]
async fn boots_siyuan_and_authenticates() {
    init_tracing();

    let sy = SiyuanContainer::builder()
        .ready_timeout(Duration::from_secs(120))
        .start()
        .await
        .expect("siyuan should start");

    let client = Client::new();
    let resp = client
        .post(format!("{}/api/system/version", sy.base_url()))
        .header("Authorization", format!("Token {}", sy.token()))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("HTTP request");
    assert!(resp.status().is_success(), "version endpoint should be 200");

    let body: serde_json::Value = resp.json().await.expect("json");
    assert_eq!(body["code"].as_i64(), Some(0), "api code should be 0; body={body}");
    assert!(
        body["data"].as_str().is_some(),
        "version response should carry data; body={body}"
    );
}

#[tokio::test]
#[ignore = "starts a real podman container; opt-in"]
async fn rejects_wrong_token() {
    init_tracing();

    let sy = SiyuanContainer::start().await.expect("siyuan should start");
    let client = Client::new();
    let resp = client
        .post(format!("{}/api/system/version", sy.base_url()))
        .header("Authorization", "Token deliberately-wrong")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("HTTP request");

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert!(
        status == reqwest::StatusCode::UNAUTHORIZED
            || body.contains("\"code\":-1")
            || body.contains("\"code\":21"),
        "wrong token should be rejected; got status={status}, body={body}"
    );
}
```

- [ ] **Step 2: 拉镜像（一次性预热）**

Run: `podman pull b3log/siyuan:latest`

Expected: 拉取成功。如果失败（如墙），改用国内镜像源或在 README 里记录。

- [ ] **Step 3: 跑 smoke 测试**

Run: `cargo test -p siyuan-testkit --test smoke -- --ignored --nocapture`

Expected: 2 passed in ~30–60s. 如果 `boots_siyuan_and_authenticates` 失败：
- 看 `tracing` 输出里 `siyuan failed to become ready; dumping logs` 段，判断是否是 token 没被识别
- 如果 token 没识别，调整 Task 4 的 `write_conf_json`，conf 结构可能要嵌套到 `system` / `editor` 等字段下，或者实际字段名不同（如 `apiToken`）
- 把 spike 结论补回 Task 4 的实现

如果 `rejects_wrong_token` 失败但 `boots_siyuan_and_authenticates` 通过：说明思源对 wrong token 的响应码不是上面三种之一，调整断言。

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-testkit/tests/smoke.rs
git commit -m "test(testkit): smoke test for container boot + token auth"
```

---

## Task 9: 集成测试 — Drop 清理

**Files:**
- Modify: `crates/siyuan-testkit/tests/smoke.rs`

**Background:** 验证 `SiyuanContainer` 离开作用域后容器真的被 podman 清掉。用 `podman ps -a --filter id=<id>` 检查。

- [ ] **Step 1: 追加测试**

在 `crates/siyuan-testkit/tests/smoke.rs` 末尾追加：

```rust
#[tokio::test]
#[ignore = "starts a real podman container; opt-in"]
async fn container_is_removed_after_drop() {
    init_tracing();

    let id = {
        let sy = SiyuanContainer::start().await.expect("siyuan should start");
        sy.container_id().to_string()
    };

    // Give podman a beat to finish the rm
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let out = std::process::Command::new("podman")
        .args(["ps", "-a", "--filter", &format!("id={id}"), "--format", "{{.ID}}"])
        .output()
        .expect("podman ps");
    let listed = String::from_utf8_lossy(&out.stdout);
    assert!(
        listed.trim().is_empty(),
        "container {id} should be gone, but podman ps shows: {listed}"
    );
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p siyuan-testkit --test smoke container_is_removed -- --ignored --nocapture`

Expected: 1 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/tests/smoke.rs
git commit -m "test(testkit): container is removed when SiyuanContainer drops"
```

---

## Task 10: 集成测试 — 并发隔离

**Files:**
- Modify: `crates/siyuan-testkit/tests/smoke.rs`

**Background:** 若两个测试用例同时启动两个容器，端口和 workspace 必须互不冲突。

- [ ] **Step 1: 追加测试**

在 `crates/siyuan-testkit/tests/smoke.rs` 末尾追加：

```rust
#[tokio::test]
#[ignore = "starts two real podman containers; opt-in"]
async fn two_containers_can_run_in_parallel() {
    init_tracing();

    let (a, b) = tokio::try_join!(SiyuanContainer::start(), SiyuanContainer::start())
        .expect("both containers should start");

    assert_ne!(a.base_url(), b.base_url(), "base urls must differ");
    assert_ne!(a.container_id(), b.container_id(), "ids must differ");

    let client = Client::new();
    for sy in [&a, &b] {
        let resp = client
            .post(format!("{}/api/system/version", sy.base_url()))
            .header("Authorization", format!("Token {}", sy.token()))
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .expect("HTTP");
        assert!(resp.status().is_success(), "version on {} should work", sy.base_url());
    }
}
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p siyuan-testkit --test smoke two_containers -- --ignored --nocapture`

Expected: 1 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/tests/smoke.rs
git commit -m "test(testkit): two containers can run in parallel"
```

---

## Task 11: insta snapshot 基础设施

**Files:**
- Create: `crates/siyuan-testkit/tests/snapshot_setup.rs`
- Create: `crates/siyuan-testkit/.config/insta.yaml`

**Background:** v1 plan 会大量用 insta 做端到端响应快照。这里只验证 insta 能跑、redaction 能用，给后续提供模板。

- [ ] **Step 1: 配置 insta**

Create `crates/siyuan-testkit/.config/insta.yaml`：

```yaml
review:
  warn_undiscovered: true
  warn_obsolete: true
output:
  format: yaml
```

- [ ] **Step 2: 写一个 snapshot 示例**

Create `crates/siyuan-testkit/tests/snapshot_setup.rs`：

```rust
//! Verifies that insta is wired up correctly. Not gated on `--ignored`
//! because no container is needed.

use serde_json::json;

#[test]
fn redacted_snapshot_is_stable() {
    let value = json!({
        "code": 0,
        "msg": "",
        "data": {
            "version": "v3.1.7",
            "container_id": "deadbeefcafebabe",
        }
    });

    insta::assert_yaml_snapshot!(value, {
        ".data.container_id" => "[redacted]",
    });
}
```

- [ ] **Step 3: 接受 baseline**

Run: `INSTA_UPDATE=always cargo test -p siyuan-testkit --test snapshot_setup`

Expected: 1 passed; 生成 `crates/siyuan-testkit/tests/snapshots/snapshot_setup__redacted_snapshot_is_stable.snap`。

- [ ] **Step 4: 复跑确认稳定**

Run: `cargo test -p siyuan-testkit --test snapshot_setup`

Expected: 1 passed (no diff)。

- [ ] **Step 5: 提交**

```bash
git add crates/siyuan-testkit/.config crates/siyuan-testkit/tests/snapshot_setup.rs crates/siyuan-testkit/tests/snapshots/
git commit -m "test(testkit): bootstrap insta snapshot infrastructure"
```

---

## Task 12: README 与公共 API doc

**Files:**
- Create: `crates/siyuan-testkit/README.md`

- [ ] **Step 1: 写 README**

写入 `crates/siyuan-testkit/README.md`：

````markdown
# siyuan-testkit

Spin up disposable SiYuan instances in Podman for integration tests.

## Prerequisites

- `podman` on PATH (`podman --version` must succeed)
- A SiYuan image pulled locally; default is `b3log/siyuan:latest`. Pin via:

  ```bash
  export SIYUAN_TEST_IMAGE=b3log/siyuan:3.1.7
  podman pull "$SIYUAN_TEST_IMAGE"
  ```

## Usage

```rust
use siyuan_testkit::{SiyuanContainer, init_tracing};

#[tokio::test]
#[ignore = "needs podman + siyuan image"]
async fn my_integration_test() {
    init_tracing();
    let sy = SiyuanContainer::start().await.unwrap();
    // sy.base_url(), sy.token() — call SiYuan API as you wish.
    // Container is removed when `sy` drops, including on panic.
}
```

## Running smoke tests

```bash
cargo test -p siyuan-testkit -- --ignored --nocapture
```

The smoke suite boots a real container, takes ~30–60s per test, and is gated
behind `--ignored` so plain `cargo test` stays fast.

## Known issues / spikes

- The kernel reads `<workspace>/conf/conf.json` and we pre-seed `api.token` there.
  If a SiYuan upgrade changes this layout, the smoke test
  `boots_siyuan_and_authenticates` will fail and you should adjust
  `workspace::write_conf_json`.
- Image pulls require network access. CI must pre-pull the pinned image.

## Debugging a failed test

Set `RUST_LOG=siyuan_testkit=debug,info` to see container IDs and per-attempt
health check output. To inspect the workspace contents after a failure:

```rust
let mut sy = SiyuanContainer::start().await?;
sy.persist_workspace_on_drop();
// ... run test, inspect path printed in the warn! log
```
````

- [ ] **Step 2: cargo doc 验证**

Run: `cargo doc -p siyuan-testkit --no-deps`

Expected: 编译通过，无 broken intra-doc link 警告。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-testkit/README.md
git commit -m "docs(testkit): usage and troubleshooting README"
```

---

## Done check

After all tasks:

- [ ] `cargo check --workspace` 通过
- [ ] `cargo test -p siyuan-testkit` (不带 `--ignored`) 通过 — 单测 + snapshot 测试
- [ ] `cargo test -p siyuan-testkit -- --ignored --nocapture` 通过 — 4 个 smoke 测试
- [ ] `git log --oneline` 至少 12 个 commit，每个对应一个 task
- [ ] `crates/siyuan-testkit/README.md` 存在
- [ ] 没有 untracked 文件

执行完成后，v1 plan 即可消费 `siyuan_testkit::SiyuanContainer` 来跑端到端集成测试。
