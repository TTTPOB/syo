# Phase B: HTTP Client

> **Part of:** [v1 Implementation Plan](../2026-05-01-v1-implementation.md) · **Prev:** [Phase A: Foundation](phase-a-foundation.md) · **Next:** [Phase C: Model layer](phase-c-model.md)
>
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this phase task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Build the typed HTTP wrapper `siyuan-client` over the SiYuan kernel API: a `SiyuanClient` core with envelope decoding, plus per-domain modules (`system`, `notebook`, `filetree`, `block`, `attr`, `asset`, `query`, `export`).

---

## Task B1: SiyuanClient core + response decode

**Files:**
- Modify: `crates/siyuan-client/src/client.rs`
- Modify: `crates/siyuan-client/src/response.rs`

**Background:** SiYuan kernel HTTP API 基本规则：
- 全是 `POST` + `application/json`
- Body 是 JSON object（即使没参数也传 `{}`）
- 鉴权 header：`Authorization: Token <token>`
- 响应统一 `{"code": <i32>, "msg": <string>, "data": <T?>}`，code=0 成功

把这个统一封装在 `client.rs::post_json`，业务模块直接拿到反序列化好的 `T`。

- [ ] **Step 1: 写 `response.rs`**

Replace `crates/siyuan-client/src/response.rs`:

```rust
use serde::Deserialize;

use siyuan_types::SiyuanError;

#[derive(Debug, Deserialize)]
pub struct SiyuanResponse<T> {
    pub code: i32,
    #[serde(default)]
    pub msg: String,
    #[serde(default = "Option::default")]
    pub data: Option<T>,
}

impl<T> SiyuanResponse<T> {
    pub fn into_result(self) -> Result<T, SiyuanError> {
        if self.code == 0 {
            self.data.ok_or_else(|| SiyuanError::Parse(
                "kernel returned code=0 but no data field".into(),
            ))
        } else {
            Err(SiyuanError::Api { code: self.code, msg: self.msg })
        }
    }

    /// Some endpoints (e.g. removeNotebook) legitimately return `data: null`
    /// on success.
    pub fn into_result_or_unit(self) -> Result<Option<T>, SiyuanError> {
        if self.code == 0 {
            Ok(self.data)
        } else {
            Err(SiyuanError::Api { code: self.code, msg: self.msg })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_returns_data() {
        let raw = r#"{"code":0,"msg":"","data":{"v":42}}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        let out = r.into_result().unwrap();
        assert_eq!(out["v"], 42);
    }

    #[test]
    fn nonzero_code_becomes_api_error() {
        let raw = r#"{"code":21,"msg":"bad","data":null}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        match r.into_result() {
            Err(SiyuanError::Api { code, msg }) => {
                assert_eq!(code, 21);
                assert_eq!(msg, "bad");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn null_data_with_code_zero_is_unit_ok() {
        let raw = r#"{"code":0,"msg":"","data":null}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        assert!(r.into_result_or_unit().unwrap().is_none());
    }
}
```

- [ ] **Step 2: 写 `client.rs`**

Replace `crates/siyuan-client/src/client.rs`:

```rust
use std::time::Duration;

use reqwest::Url;
use serde::{Serialize, de::DeserializeOwned};
use tracing::{debug, trace};

use siyuan_types::SiyuanError;

use crate::response::SiyuanResponse;

/// Thin HTTP wrapper over the SiYuan kernel API.
#[derive(Debug, Clone)]
pub struct SiyuanClient {
    base_url: Url,
    token: String,
    http: reqwest::Client,
}

impl SiyuanClient {
    pub fn new(base_url: impl AsRef<str>, token: impl Into<String>) -> Result<Self, SiyuanError> {
        let parsed = Url::parse(base_url.as_ref())
            .map_err(|e| SiyuanError::Parse(format!("base_url: {e}")))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| SiyuanError::Http(e.to_string()))?;
        Ok(Self { base_url: parsed, token: token.into(), http })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// POST `<base>/<path>` with `body` as JSON, decode `data` into `R`.
    pub async fn post<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, SiyuanError> {
        let resp: SiyuanResponse<R> = self.post_envelope(path, body).await?;
        resp.into_result()
    }

    /// POST returning the raw envelope, for endpoints whose `data` may be null.
    pub async fn post_envelope<B: Serialize + ?Sized, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<SiyuanResponse<R>, SiyuanError> {
        let url = self
            .base_url
            .join(path.trim_start_matches('/'))
            .map_err(|e| SiyuanError::Parse(format!("join {path}: {e}")))?;
        debug!(method = "POST", %url, "siyuan call");

        let resp = self
            .http
            .post(url.clone())
            .header("Authorization", format!("Token {}", self.token))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| SiyuanError::Http(e.to_string()))?;

        let status = resp.status();
        let body_text = resp.text().await.map_err(|e| SiyuanError::Http(e.to_string()))?;
        trace!(%status, body = %body_text, "siyuan response");

        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SiyuanError::Auth);
        }
        if !status.is_success() {
            return Err(SiyuanError::Http(format!("HTTP {status}: {body_text}")));
        }

        serde_json::from_str(&body_text)
            .map_err(|e| SiyuanError::Parse(format!("decode {url}: {e}; body={body_text}")))
    }
}
```

- [ ] **Step 3: cargo check + 单测**

Run: `cargo test -p siyuan-client response::`

Expected: 3 passed.

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-client/src/client.rs crates/siyuan-client/src/response.rs
git commit -m "feat(client): SiyuanClient + envelope decoder"
```

---

## Task B2: api/system + api/notebook

**Files:**
- Modify: `crates/siyuan-client/src/api/system.rs`
- Modify: `crates/siyuan-client/src/api/notebook.rs`

- [ ] **Step 1: 写 `api/system.rs`**

Replace `crates/siyuan-client/src/api/system.rs`:

```rust
use serde::Deserialize;
use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub version: Option<String>,
}

impl SiyuanClient {
    /// `/api/system/version` — returns the kernel version string.
    pub async fn system_version(&self) -> Result<String, SiyuanError> {
        // Endpoint returns `data` as a plain string.
        self.post::<_, String>("/api/system/version", &serde_json::json!({})).await
    }
}
```

- [ ] **Step 2: 写 `api/notebook.rs`**

Replace `crates/siyuan-client/src/api/notebook.rs`:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{NotebookId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notebook {
    pub id: NotebookId,
    pub name: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub sort: i64,
    #[serde(default)]
    pub closed: bool,
}

#[derive(Debug, Deserialize)]
struct LsNotebooksData {
    notebooks: Vec<Notebook>,
}

#[derive(Debug, Serialize)]
struct OneNotebook<'a> {
    notebook: &'a NotebookId,
}

#[derive(Debug, Serialize)]
struct CreateNotebook<'a> {
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct RenameNotebook<'a> {
    notebook: &'a NotebookId,
    name: &'a str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedNotebook {
    pub notebook: Notebook,
}

impl SiyuanClient {
    pub async fn ls_notebooks(&self) -> Result<Vec<Notebook>, SiyuanError> {
        let data: LsNotebooksData = self.post("/api/notebook/lsNotebooks", &serde_json::json!({})).await?;
        Ok(data.notebooks)
    }

    pub async fn open_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/openNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn close_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/closeNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn create_notebook(&self, name: &str) -> Result<Notebook, SiyuanError> {
        let data: CreatedNotebook = self.post("/api/notebook/createNotebook", &CreateNotebook { name }).await?;
        Ok(data.notebook)
    }

    pub async fn rename_notebook(&self, id: &NotebookId, new_name: &str) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/renameNotebook", &RenameNotebook { notebook: id, name: new_name })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn remove_notebook(&self, id: &NotebookId) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/notebook/removeNotebook", &OneNotebook { notebook: id })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }
}
```

- [ ] **Step 3: cargo check**

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 4: 提交**

```bash
git add crates/siyuan-client/src/api/system.rs crates/siyuan-client/src/api/notebook.rs
git commit -m "feat(client): system.version + notebook CRUD"
```

---

## Task B3: api/filetree

**Files:**
- Modify: `crates/siyuan-client/src/api/filetree.rs`

**Background:** `createDocWithMd` 返回新建文档的 block id。`getIDsByHPath` 把 hpath 解析为 block id 列表（多匹配时多于 1 个）。

- [ ] **Step 1: 写实现**

Replace `crates/siyuan-client/src/api/filetree.rs`:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, NotebookId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct CreateDocReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
    markdown: &'a str,
}

#[derive(Debug, Serialize)]
struct DocId<'a> {
    id: &'a BlockId,
}

#[derive(Debug, Serialize)]
struct RenameDocReq<'a> {
    #[serde(rename = "notebook")]
    notebook: &'a NotebookId,
    path: &'a str,
    title: &'a str,
}

#[derive(Debug, Serialize)]
struct MoveDocsReq<'a> {
    #[serde(rename = "fromPaths")]
    from_paths: &'a [String],
    #[serde(rename = "toNotebook")]
    to_notebook: &'a NotebookId,
    #[serde(rename = "toPath")]
    to_path: &'a str,
}

#[derive(Debug, Serialize)]
struct RemoveDocReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
}

#[derive(Debug, Serialize)]
struct GetIdsReq<'a> {
    notebook: &'a NotebookId,
    path: &'a str,
}

#[derive(Debug, Serialize)]
struct GetHPathReq<'a> {
    id: &'a BlockId,
}

impl SiyuanClient {
    /// `/api/filetree/createDocWithMd` — returns the new doc's block id.
    pub async fn create_doc_with_md(
        &self,
        notebook: &NotebookId,
        hpath: &str,
        markdown: &str,
    ) -> Result<BlockId, SiyuanError> {
        let raw: String = self
            .post(
                "/api/filetree/createDocWithMd",
                &CreateDocReq { notebook, path: hpath, markdown },
            )
            .await?;
        BlockId::parse(raw).map_err(|e| SiyuanError::Parse(e.to_string()))
    }

    pub async fn rename_doc(
        &self,
        notebook: &NotebookId,
        path: &str,
        new_title: &str,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/filetree/renameDoc",
                &RenameDocReq { notebook, path, title: new_title },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn move_docs(
        &self,
        from_paths: &[String],
        to_notebook: &NotebookId,
        to_path: &str,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope(
                "/api/filetree/moveDocs",
                &MoveDocsReq { from_paths, to_notebook, to_path },
            )
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    pub async fn remove_doc(&self, notebook: &NotebookId, path: &str) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/filetree/removeDoc", &RemoveDocReq { notebook, path })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }

    /// `/api/filetree/getIDsByHPath` — resolve a human path to one or more block ids.
    pub async fn get_ids_by_hpath(
        &self,
        notebook: &NotebookId,
        hpath: &str,
    ) -> Result<Vec<BlockId>, SiyuanError> {
        let raw: Vec<String> = self
            .post("/api/filetree/getIDsByHPath", &GetIdsReq { notebook, path: hpath })
            .await?;
        raw.into_iter()
            .map(|s| BlockId::parse(s).map_err(|e| SiyuanError::Parse(e.to_string())))
            .collect()
    }

    /// `/api/filetree/getHPathByID` — opposite of above.
    pub async fn get_hpath_by_id(&self, id: &BlockId) -> Result<String, SiyuanError> {
        self.post("/api/filetree/getHPathByID", &GetHPathReq { id }).await
    }
}
```

- [ ] **Step 2: cargo check**

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-client/src/api/filetree.rs
git commit -m "feat(client): filetree CRUD + hpath resolution"
```

---

## Task B4: api/block (the big one)

**Files:**
- Modify: `crates/siyuan-client/src/api/block.rs`

**Background:** This is where SiYuan's block ops live. SiYuan returns "transactions" containing `doOperations`/`undoOperations` arrays; we extract the `id` of the first new block from `doOperations[0].id`.

- [ ] **Step 1: 写实现**

Replace `crates/siyuan-client/src/api/block.rs`:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, SiyuanError};

use crate::SiyuanClient;

// -------- request types --------

#[derive(Debug, Serialize)]
struct ById<'a> {
    id: &'a BlockId,
}

#[derive(Debug, Serialize)]
struct InsertReq<'a> {
    #[serde(rename = "dataType")]
    data_type: &'a str,
    data: &'a str,
    #[serde(rename = "previousID", skip_serializing_if = "Option::is_none")]
    previous_id: Option<&'a BlockId>,
    #[serde(rename = "nextID", skip_serializing_if = "Option::is_none")]
    next_id: Option<&'a BlockId>,
    #[serde(rename = "parentID", skip_serializing_if = "Option::is_none")]
    parent_id: Option<&'a BlockId>,
}

#[derive(Debug, Serialize)]
struct AppendOrPrependReq<'a> {
    #[serde(rename = "dataType")]
    data_type: &'a str,
    data: &'a str,
    #[serde(rename = "parentID")]
    parent_id: &'a BlockId,
}

#[derive(Debug, Serialize)]
struct UpdateReq<'a> {
    id: &'a BlockId,
    #[serde(rename = "dataType")]
    data_type: &'a str,
    data: &'a str,
}

#[derive(Debug, Serialize)]
struct MoveReq<'a> {
    id: &'a BlockId,
    #[serde(rename = "previousID", skip_serializing_if = "Option::is_none")]
    previous_id: Option<&'a BlockId>,
    #[serde(rename = "parentID", skip_serializing_if = "Option::is_none")]
    parent_id: Option<&'a BlockId>,
}

// -------- response types --------

#[derive(Debug, Deserialize)]
pub struct DoOperation {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(rename = "parentID", default)]
    pub parent_id: Option<String>,
    #[serde(rename = "previousID", default)]
    pub previous_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    #[serde(rename = "doOperations", default)]
    pub do_operations: Vec<DoOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChildBlock {
    pub id: BlockId,
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub subtype: String,
}

#[derive(Debug, Deserialize)]
pub struct BlockKramdown {
    pub id: BlockId,
    pub kramdown: String,
}

// -------- helpers --------

fn first_new_id(txs: &[Transaction]) -> Result<BlockId, SiyuanError> {
    for tx in txs {
        for op in &tx.do_operations {
            if let Some(id) = op.id.as_deref() {
                return BlockId::parse(id).map_err(|e| SiyuanError::Parse(e.to_string()));
            }
        }
    }
    Err(SiyuanError::Parse("no id found in transaction operations".into()))
}

// -------- methods --------

impl SiyuanClient {
    pub async fn get_block_kramdown(&self, id: &BlockId) -> Result<BlockKramdown, SiyuanError> {
        self.post("/api/block/getBlockKramdown", &ById { id }).await
    }

    pub async fn get_child_blocks(&self, id: &BlockId) -> Result<Vec<ChildBlock>, SiyuanError> {
        self.post("/api/block/getChildBlocks", &ById { id }).await
    }

    /// Insert before/after an anchor block. Pass exactly one of
    /// `previous_id` / `next_id` (typically `previous_id` for "after").
    pub async fn insert_block_markdown(
        &self,
        markdown: &str,
        previous_id: Option<&BlockId>,
        next_id: Option<&BlockId>,
        parent_id: Option<&BlockId>,
    ) -> Result<BlockId, SiyuanError> {
        let txs: Vec<Transaction> = self
            .post(
                "/api/block/insertBlock",
                &InsertReq {
                    data_type: "markdown",
                    data: markdown,
                    previous_id,
                    next_id,
                    parent_id,
                },
            )
            .await?;
        first_new_id(&txs)
    }

    pub async fn append_block_markdown(
        &self,
        markdown: &str,
        parent_id: &BlockId,
    ) -> Result<BlockId, SiyuanError> {
        let txs: Vec<Transaction> = self
            .post(
                "/api/block/appendBlock",
                &AppendOrPrependReq { data_type: "markdown", data: markdown, parent_id },
            )
            .await?;
        first_new_id(&txs)
    }

    pub async fn prepend_block_markdown(
        &self,
        markdown: &str,
        parent_id: &BlockId,
    ) -> Result<BlockId, SiyuanError> {
        let txs: Vec<Transaction> = self
            .post(
                "/api/block/prependBlock",
                &AppendOrPrependReq { data_type: "markdown", data: markdown, parent_id },
            )
            .await?;
        first_new_id(&txs)
    }

    pub async fn update_block_markdown(
        &self,
        id: &BlockId,
        markdown: &str,
    ) -> Result<(), SiyuanError> {
        let _: Vec<Transaction> = self
            .post(
                "/api/block/updateBlock",
                &UpdateReq { id, data_type: "markdown", data: markdown },
            )
            .await?;
        Ok(())
    }

    pub async fn delete_block(&self, id: &BlockId) -> Result<(), SiyuanError> {
        let _: Vec<Transaction> = self.post("/api/block/deleteBlock", &ById { id }).await?;
        Ok(())
    }

    pub async fn move_block(
        &self,
        id: &BlockId,
        previous_id: Option<&BlockId>,
        parent_id: Option<&BlockId>,
    ) -> Result<(), SiyuanError> {
        let _: Vec<Transaction> = self
            .post("/api/block/moveBlock", &MoveReq { id, previous_id, parent_id })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_new_id_extracts_from_transaction() {
        let json = r#"[{"doOperations":[{"action":"insert","id":"20260501093000-abc1234"}]}]"#;
        let txs: Vec<Transaction> = serde_json::from_str(json).unwrap();
        let id = first_new_id(&txs).unwrap();
        assert_eq!(id.as_str(), "20260501093000-abc1234");
    }

    #[test]
    fn first_new_id_errors_on_empty() {
        let txs: Vec<Transaction> = vec![];
        assert!(first_new_id(&txs).is_err());
    }
}
```

- [ ] **Step 2: cargo test**

Run: `cargo test -p siyuan-client api::block::`

Expected: 2 passed.

- [ ] **Step 3: 提交**

```bash
git add crates/siyuan-client/src/api/block.rs
git commit -m "feat(client): block get/insert/append/prepend/update/delete/move"
```

---

## Task B5: attr / asset / query / export

**Files:**
- Modify: `crates/siyuan-client/src/api/attr.rs`
- Modify: `crates/siyuan-client/src/api/asset.rs`
- Modify: `crates/siyuan-client/src/api/query.rs`
- Modify: `crates/siyuan-client/src/api/export.rs`

- [ ] **Step 1: 写 `api/attr.rs`**

Replace:

```rust
use std::collections::BTreeMap;

use serde::Serialize;

use siyuan_types::{BlockId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct ById<'a> {
    id: &'a BlockId,
}

#[derive(Debug, Serialize)]
struct SetAttrsReq<'a> {
    id: &'a BlockId,
    attrs: &'a BTreeMap<String, String>,
}

impl SiyuanClient {
    pub async fn get_block_attrs(
        &self,
        id: &BlockId,
    ) -> Result<BTreeMap<String, String>, SiyuanError> {
        self.post("/api/attr/getBlockAttrs", &ById { id }).await
    }

    pub async fn set_block_attrs(
        &self,
        id: &BlockId,
        attrs: &BTreeMap<String, String>,
    ) -> Result<(), SiyuanError> {
        let _: serde_json::Value = self
            .post_envelope("/api/attr/setBlockAttrs", &SetAttrsReq { id, attrs })
            .await?
            .into_result_or_unit()?
            .unwrap_or(serde_json::Value::Null);
        Ok(())
    }
}
```

- [ ] **Step 2a: 给 `client.rs` 加一个 crate 内 token 访问器**

Asset 上传走 multipart，不能复用 `post_json`。在 `crates/siyuan-client/src/client.rs` 的 `impl SiyuanClient` 块内追加：

```rust
    pub(crate) fn token(&self) -> &str {
        &self.token
    }
```

- [ ] **Step 2b: 写 `api/asset.rs`**

Replace `crates/siyuan-client/src/api/asset.rs`:

```rust
use std::path::Path;

use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Deserialize)]
pub struct UploadResult {
    /// Map from original filename → kernel-stored relative path (e.g.
    /// `assets/foo-20260501093000-abcdefg.png`).
    #[serde(default, rename = "succMap")]
    pub succ_map: std::collections::BTreeMap<String, String>,
    #[serde(default, rename = "errFiles")]
    pub err_files: Vec<String>,
}

impl SiyuanClient {
    /// Upload a single file as an asset. Returns the kernel-relative path,
    /// e.g. `assets/myimg-20260501093000-abcdefg.png`, suitable for embedding
    /// in markdown as `![alt](assets/...)`.
    pub async fn upload_asset(&self, file_path: &Path) -> Result<String, SiyuanError> {
        let bytes = std::fs::read(file_path)
            .map_err(|e| SiyuanError::Http(format!("read {}: {e}", file_path.display())))?;
        let filename = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| SiyuanError::Parse(format!("bad file name: {}", file_path.display())))?
            .to_string();

        let part = Part::bytes(bytes).file_name(filename.clone());
        let form = Form::new().part("file[]", part);

        let url = self
            .base_url()
            .join("api/asset/upload")
            .map_err(|e| SiyuanError::Parse(e.to_string()))?;

        let resp = reqwest::Client::new()
            .post(url)
            .header("Authorization", format!("Token {}", self.token()))
            .multipart(form)
            .send()
            .await
            .map_err(|e| SiyuanError::Http(e.to_string()))?;

        let body = resp.text().await.map_err(|e| SiyuanError::Http(e.to_string()))?;
        let parsed: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| SiyuanError::Parse(e.to_string()))?;
        let code = parsed.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
        if code != 0 {
            return Err(SiyuanError::Api {
                code: code as i32,
                msg: parsed
                    .get("msg")
                    .and_then(|m| m.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        let upload: UploadResult = serde_json::from_value(
            parsed.get("data").cloned().unwrap_or(serde_json::Value::Null),
        )
        .map_err(|e| SiyuanError::Parse(e.to_string()))?;
        if !upload.err_files.is_empty() {
            return Err(SiyuanError::Api {
                code: -1,
                msg: format!("upload failed for: {:?}", upload.err_files),
            });
        }
        upload
            .succ_map
            .get(&filename)
            .cloned()
            .ok_or_else(|| SiyuanError::Parse(format!("succMap missing entry for {filename}")))
    }
}
```

- [ ] **Step 3: 写 `api/query.rs`**

Replace:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::SiyuanError;

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct SqlReq<'a> {
    stmt: &'a str,
}

impl SiyuanClient {
    /// `/api/query/sql` — read-only SQL. Returns rows as JSON objects.
    /// Note: in publish mode the kernel disables this endpoint and returns a
    /// non-zero code; callers should handle `SiyuanError::Api` and surface
    /// `SqlUnavailable` if recognised.
    pub async fn sql(&self, stmt: &str) -> Result<Vec<serde_json::Value>, SiyuanError> {
        match self
            .post::<_, Vec<serde_json::Value>>("/api/query/sql", &SqlReq { stmt })
            .await
        {
            Ok(rows) => Ok(rows),
            Err(SiyuanError::Api { code, msg }) if msg.to_lowercase().contains("publish") => {
                let _ = code;
                Err(SiyuanError::SqlUnavailable)
            }
            Err(e) => Err(e),
        }
    }

    /// Typed convenience: deserialise rows into `T`.
    pub async fn sql_typed<T: for<'de> Deserialize<'de>>(
        &self,
        stmt: &str,
    ) -> Result<Vec<T>, SiyuanError> {
        let rows = self.sql(stmt).await?;
        rows.into_iter()
            .map(|v| serde_json::from_value::<T>(v).map_err(|e| SiyuanError::Parse(e.to_string())))
            .collect()
    }
}
```

- [ ] **Step 4: 写 `api/export.rs`**

Replace:

```rust
use serde::{Deserialize, Serialize};

use siyuan_types::{BlockId, SiyuanError};

use crate::SiyuanClient;

#[derive(Debug, Serialize)]
struct ExportReq<'a> {
    id: &'a BlockId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedDoc {
    #[serde(default)]
    pub h_path: String,
    pub content: String,
}

impl SiyuanClient {
    pub async fn export_md_content(&self, doc_id: &BlockId) -> Result<ExportedDoc, SiyuanError> {
        self.post("/api/export/exportMdContent", &ExportReq { id: doc_id }).await
    }
}
```

- [ ] **Step 5: cargo check + 单测**

Run: `cargo test -p siyuan-client`

Expected: 之前的 5 个测试通过；新模块只 cargo check（没集成测试就不写单测）。

Run: `cargo check -p siyuan-client`

Expected: 通过。

- [ ] **Step 6: 提交**

```bash
git add crates/siyuan-client/src/api crates/siyuan-client/src/client.rs
git commit -m "feat(client): attr/asset/query/export APIs"
```

