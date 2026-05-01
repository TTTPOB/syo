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
    pub(crate) http: reqwest::Client,
}

impl SiyuanClient {
    pub fn new(base_url: impl AsRef<str>, token: impl Into<String>) -> Result<Self, SiyuanError> {
        let parsed = Url::parse(base_url.as_ref())
            .map_err(|e| SiyuanError::Parse(format!("base_url: {e}")))?;
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| SiyuanError::Http(e.to_string()))?;
        Ok(Self {
            base_url: parsed,
            token: token.into(),
            http,
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub(crate) fn token(&self) -> &str {
        &self.token
    }

    /// Parse a raw response body as a `SiyuanResponse` envelope and
    /// extract the `data` payload (or an `Api` error).  Shared by callers
    /// that cannot use `post()` because they send non-JSON bodies.
    pub(crate) fn decode_envelope<R: DeserializeOwned>(
        &self,
        body: &str,
    ) -> Result<R, SiyuanError> {
        let env: SiyuanResponse<R> =
            serde_json::from_str(body).map_err(|e| SiyuanError::Parse(e.to_string()))?;
        env.into_result()
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
        let body_text = resp
            .text()
            .await
            .map_err(|e| SiyuanError::Http(e.to_string()))?;
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
