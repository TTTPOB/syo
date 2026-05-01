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

/// `podman stop --ignore --time=<timeout> <id>`. Best-effort; `--ignore` makes
/// podman exit 0 if the container is already gone, so we don't have to parse
/// stderr.
pub fn stop(container_id: &str, timeout_secs: u32) -> Result<()> {
    let out = Command::new("podman")
        .args(["stop", "--ignore", "--time", &timeout_secs.to_string(), container_id])
        .output()
        .context("spawning `podman stop`")?;
    if !out.status.success() {
        bail!(
            "`podman stop` exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

/// `podman rm --ignore -f <id>`. Best-effort; `--ignore` makes podman exit 0
/// if the container is already gone.
pub fn force_remove(container_id: &str) -> Result<()> {
    let out = Command::new("podman")
        .args(["rm", "--ignore", "-f", container_id])
        .output()
        .context("spawning `podman rm -f`")?;
    if !out.status.success() {
        bail!(
            "`podman rm -f` exited {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr)
        );
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
