//! Smoke test: actually boot SiYuan inside Podman.
//!
//! Run with: `cargo test -p siyuan-testkit --test smoke -- --ignored --nocapture`
//!
//! Both tests hit /api/notebook/lsNotebooks because it actually enforces the
//! token. /api/system/version, the natural-looking choice, is unauthenticated
//! and would let any token pass.

use std::time::Duration;

use reqwest::Client;
use siyuan_testkit::{SiyuanContainer, init_tracing};

#[tokio::test]
#[ignore = "starts a real podman container; opt-in"]
async fn boots_siyuan_and_authenticates() {
    init_tracing();

    let sy = SiyuanContainer::builder()
        .ready_timeout(Duration::from_secs(120))
        .start()
        .await
        .expect("siyuan should start");

    let client = Client::new();
    let resp = client
        .post(format!("{}/api/notebook/lsNotebooks", sy.base_url()))
        .header("Authorization", format!("Token {}", sy.token()))
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("HTTP request");
    assert!(
        resp.status().is_success(),
        "lsNotebooks endpoint should be 200"
    );

    let body: serde_json::Value = resp.json().await.expect("json");
    assert_eq!(
        body["code"].as_i64(),
        Some(0),
        "api code should be 0; body={body}"
    );
    assert!(
        body["data"]["notebooks"].is_array(),
        "lsNotebooks response should carry data.notebooks; body={body}"
    );
}

#[tokio::test]
#[ignore = "starts a real podman container; opt-in"]
async fn rejects_wrong_token() {
    init_tracing();

    let sy = SiyuanContainer::start().await.expect("siyuan should start");
    let client = Client::new();
    let resp = client
        .post(format!("{}/api/notebook/lsNotebooks", sy.base_url()))
        .header("Authorization", "Token deliberately-wrong")
        .header("Content-Type", "application/json")
        .body("{}")
        .send()
        .await
        .expect("HTTP request");

    // SiYuan v3.6.5+ returns HTTP 401 for auth failures. Assert the status is
    // a 4xx client-error so a future version that switches to, say, 403 still
    // passes, while a 2xx (auth layer bypassed) or 5xx (crash) fails loudly.
    assert!(
        resp.status().is_client_error(),
        "wrong token should be rejected with a 4xx; got {}",
        resp.status()
    );
}

#[tokio::test]
#[ignore = "starts a real podman container; opt-in"]
async fn container_is_removed_after_drop() {
    init_tracing();

    let id = {
        let sy = SiyuanContainer::start().await.expect("siyuan should start");
        sy.container_id().to_string()
    };

    // Give podman a beat to finish the rm
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let out = std::process::Command::new("podman")
        .args([
            "ps",
            "-a",
            "--filter",
            &format!("id={id}"),
            "--format",
            "{{.ID}}",
        ])
        .output()
        .expect("podman ps");
    let listed = String::from_utf8_lossy(&out.stdout);
    assert!(
        listed.trim().is_empty(),
        "container {id} should be gone, but podman ps shows: {listed}"
    );
}

#[tokio::test]
#[ignore = "starts two real podman containers; opt-in"]
async fn two_containers_can_run_in_parallel() {
    init_tracing();

    let (a, b) = tokio::try_join!(SiyuanContainer::start(), SiyuanContainer::start())
        .expect("both containers should start");

    assert_ne!(a.base_url(), b.base_url(), "base urls must differ");
    assert_ne!(a.container_id(), b.container_id(), "ids must differ");
    assert_ne!(a.token(), b.token(), "tokens must differ");

    let client = Client::new();
    for sy in [&a, &b] {
        let resp = client
            .post(format!("{}/api/notebook/lsNotebooks", sy.base_url()))
            .header("Authorization", format!("Token {}", sy.token()))
            .header("Content-Type", "application/json")
            .body("{}")
            .send()
            .await
            .expect("HTTP");
        assert!(
            resp.status().is_success(),
            "lsNotebooks on {} should work",
            sy.base_url()
        );
    }
}
