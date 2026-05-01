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
