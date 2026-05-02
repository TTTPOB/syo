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
    /// Default request timeout used by [`SiyuanClient::new`].
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Build a client with the default 30-second request timeout.
    pub fn new(base_url: impl AsRef<str>, token: impl Into<String>) -> Result<Self, SiyuanError> {
        Self::new_with_timeout(base_url, token, Self::DEFAULT_TIMEOUT)
    }

    /// Build a client with an explicit per-request timeout.
    ///
    /// `Duration::ZERO` is treated as "no timeout" — useful for callers that
    /// front the kernel with their own deadline, and for diagnostic runs
    /// where slow responses should be observed rather than terminated.
    pub fn new_with_timeout(
        base_url: impl AsRef<str>,
        token: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, SiyuanError> {
        let parsed = Url::parse(base_url.as_ref())
            .map_err(|e| SiyuanError::Parse(format!("base_url: {e}")))?;
        let mut builder = reqwest::Client::builder();
        if !timeout.is_zero() {
            builder = builder.timeout(timeout);
        }
        let http = builder
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_timeout_accepts_short_duration() {
        // We only assert that the constructor returns Ok with a small timeout;
        // verifying the timeout actually fires would require a hung server.
        let client =
            SiyuanClient::new_with_timeout("http://localhost:1", "tok", Duration::from_millis(50));
        assert!(
            client.is_ok(),
            "constructor must succeed: {:?}",
            client.err()
        );
    }

    #[test]
    fn new_with_timeout_zero_means_unbounded() {
        // Duration::ZERO is documented to mean "no timeout"; we just verify
        // that the underlying reqwest builder accepts the request.
        let client = SiyuanClient::new_with_timeout("http://localhost:1", "tok", Duration::ZERO);
        assert!(
            client.is_ok(),
            "zero timeout must build: {:?}",
            client.err()
        );
    }

    #[test]
    fn new_uses_default_timeout() {
        let client = SiyuanClient::new("http://localhost:1", "tok");
        assert!(client.is_ok());
    }
}
