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

mod cleanup;
mod container;
mod health;
mod podman;
mod port;
mod workspace;

pub use cleanup::sweep_stale_workspaces;
pub use container::{SiyuanContainer, SiyuanContainerBuilder};

/// Initialise tracing for tests. Idempotent — safe to call from every test.
pub fn init_tracing() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new("info,siyuan_testkit=debug")
                }),
            )
            .with_test_writer()
            .try_init();
    });
}
