use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{debug, error, info};

use crate::health::wait_for_ready;
use crate::podman;
use crate::port::allocate_loopback_port;
use crate::workspace::TempWorkspace;

const DEFAULT_IMAGE: &str = "docker.io/b3log/siyuan:latest";
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
    #[doc(hidden)]
    pub fn disarm_for_testing(&mut self) {
        self.disarmed = true;
    }
}

// NOTE: stop/force_remove are blocking calls. When this Drop fires from inside
// a #[tokio::test], it stalls a tokio worker for up to STOP_TIMEOUT_SECS. We
// accept this because Drop happens at end-of-test, the timeout is short, and
// stable Rust has no async Drop. spawn_blocking would need a runtime handle
// that is not reliably available here.
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
            tracing::error!(?err, container_id = %id, "podman rm -f failed; container may be leaked");
        }
    }
}
