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
