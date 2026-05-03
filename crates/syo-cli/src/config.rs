use anyhow::{Context, Result};

use siyuan_client::SiyuanClient;

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub token: String,
}

impl Config {
    /// Read from CLI flags first, then env, then a default of localhost.
    pub fn resolve(flag_base_url: Option<String>, flag_token: Option<String>) -> Result<Self> {
        let base_url = flag_base_url
            .or_else(|| std::env::var("SIYUAN_BASE_URL").ok())
            .unwrap_or_else(|| "http://127.0.0.1:6806".into());
        let token = flag_token
            .or_else(|| std::env::var("SIYUAN_TOKEN").ok())
            .context("--token (or SIYUAN_TOKEN env var) is required")?;
        Ok(Self { base_url, token })
    }

    /// Like [`Config::resolve`] but tolerates a missing token, defaulting to
    /// the empty string. The MCP server uses this so it can boot without
    /// auth and surface a warning instead of failing fast — auth-requiring
    /// kernel calls still error at request time.
    pub fn resolve_optional_token(
        flag_base_url: Option<String>,
        flag_token: Option<String>,
    ) -> Self {
        let base_url = flag_base_url
            .or_else(|| std::env::var("SIYUAN_BASE_URL").ok())
            .unwrap_or_else(|| "http://127.0.0.1:6806".into());
        let token = flag_token
            .or_else(|| std::env::var("SIYUAN_TOKEN").ok())
            .unwrap_or_default();
        Self { base_url, token }
    }

    pub fn into_client(self) -> Result<SiyuanClient> {
        SiyuanClient::new(&self.base_url, &self.token).map_err(anyhow::Error::from)
    }
}
