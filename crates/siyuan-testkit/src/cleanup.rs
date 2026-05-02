//! Sweep stale `siyuan-testkit-*` workspace dirs left behind by killed or
//! crashed test runs.
//!
//! # Why this exists
//!
//! `TempWorkspace` wraps `tempfile::TempDir` whose Drop calls
//! `std::fs::remove_dir_all`. That works for cleanly-finishing tests, but:
//!
//! 1. The SiYuan container writes files into the bind-mounted workspace as
//!    a sub-uid (rootless podman → `/etc/subuid` mapping). The host
//!    process cannot delete them with plain `remove_dir_all`, so even a
//!    clean Drop fails silently.
//! 2. If `cargo test` is killed (Ctrl-C, OOM, panic in another thread),
//!    Drop never runs at all and the dir survives the process.
//!
//! Either failure mode leaves ~17 MB of seeded SiYuan workspace per orphan
//! in `/tmp`. After hundreds of test runs `/tmp` runs out of space and
//! every shell command in the dev environment starts failing.
//!
//! # What this does
//!
//! On every `SiyuanContainerBuilder::start()` call we scan `std::env::
//! temp_dir()` for entries whose name starts with `siyuan-testkit-` and
//! whose mtime is older than `STALE_AFTER`. Anything that old is by
//! definition not bound to a live test (containers boot in seconds and
//! tests finish in tens of seconds), so we delete them via
//! `podman unshare rm -rf` to defeat the sub-uid ownership problem.
//!
//! Errors are logged at `debug` and swallowed — we never abort the
//! current test session for a stale dir we couldn't clean.

use std::fs;
use std::time::{Duration, SystemTime};

use crate::podman;

const TEMP_PREFIX: &str = "siyuan-testkit-";
/// Mtime older than this counts as stale. Long enough that we never race
/// with a concurrent test booting on a slow machine, short enough that a
/// failing test session doesn't leave junk for the next dev session.
const STALE_AFTER: Duration = Duration::from_secs(10 * 60);

/// Sweep stale workspace dirs out of `std::env::temp_dir()`.
///
/// Best-effort: every error path is logged at `debug` and ignored. The
/// function is safe to call from any test fixture (it's idempotent and
/// performs no allocation beyond reading one tmp dir entry).
pub fn sweep_stale_workspaces() {
    let tmp = std::env::temp_dir();
    let entries = match fs::read_dir(&tmp) {
        Ok(e) => e,
        Err(err) => {
            tracing::debug!(?err, dir=%tmp.display(), "could not read temp dir for sweep");
            return;
        }
    };

    let now = SystemTime::now();
    let mut removed = 0usize;
    let mut kept_recent = 0usize;
    let mut failed = 0usize;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = match name.to_str() {
            Some(s) => s,
            None => continue,
        };
        if !name_str.starts_with(TEMP_PREFIX) {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !metadata.is_dir() {
            continue;
        }
        let mtime = match metadata.modified() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
        if age < STALE_AFTER {
            kept_recent += 1;
            continue;
        }

        let path = entry.path();
        match podman::unshare_rm_rf(&path) {
            Ok(()) => removed += 1,
            Err(err) => {
                failed += 1;
                tracing::debug!(?err, path=%path.display(), "stale workspace cleanup failed");
            }
        }
    }

    if removed > 0 || failed > 0 {
        tracing::info!(
            removed,
            kept_recent,
            failed,
            "siyuan-testkit stale workspace sweep"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// We can't easily fake `mtime` portably, so the assertion side of these
    /// tests is light: the mtime branch is exercised in practice on every
    /// test session, and the unit check just pins the safe-by-default path.
    #[test]
    fn sweep_does_not_panic() {
        sweep_stale_workspaces();
    }

    #[test]
    fn sweep_skips_recent_dirs() {
        // Create a fresh dir matching the prefix, sweep, verify it is
        // still present. Uses our own tempdir so we don't pollute /tmp
        // beyond the test lifetime.
        let dir = tempfile::Builder::new()
            .prefix(TEMP_PREFIX)
            .tempdir()
            .expect("create marker dir");
        let marker = dir.path().to_path_buf();
        sweep_stale_workspaces();
        assert!(
            marker.exists(),
            "fresh testkit dir must survive the sweep; got missing path {}",
            marker.display()
        );
    }
}
