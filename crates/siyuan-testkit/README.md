# siyuan-testkit

Spin up disposable SiYuan instances in Podman for integration tests.

## Prerequisites

- `podman` on PATH (`podman --version` must succeed). Other container
  runtimes are not supported — the testkit shells out to `podman` directly.
- A SiYuan image pulled locally; default is `docker.io/b3log/siyuan:latest`
  (the fully-qualified form, required by Podman setups without a default
  unqualified-search registry such as Podman on WSL2). Pin a specific version via:

  ```bash
  export SIYUAN_TEST_IMAGE=docker.io/b3log/siyuan:3.6.5
  podman pull "$SIYUAN_TEST_IMAGE"
  ```

## Usage

```rust
use siyuan_testkit::{SiyuanContainer, init_tracing};

#[tokio::test]
#[ignore = "needs podman + siyuan image"]
async fn my_integration_test() {
    init_tracing();
    let sy = SiyuanContainer::start().await.unwrap();
    // sy.base_url(), sy.token() — call SiYuan API as you wish.
    // Container is removed when `sy` drops, including on panic.
}
```

## Running smoke tests

```bash
cargo test -p siyuan-testkit -- --ignored --nocapture
```

The smoke suite boots a real container, takes a few seconds per test,
and is gated behind `#[ignore]` so plain `cargo test` stays fast.

## Verifying authentication

The testkit pre-seeds `<workspace>/conf/conf.json` with
`{"api": {"token": "..."}}` so SiYuan accepts `Authorization: Token <token>`.
This was verified empirically against SiYuan v3.6.5. If a future SiYuan
release changes the conf.json layout, the smoke test
`boots_siyuan_and_authenticates` will fail and you should adjust
`workspace::write_conf_json`.

Note that `/api/system/version` is unauthenticated — any token (or none)
returns success. Use `/api/notebook/lsNotebooks` or any other authenticated
endpoint to actually exercise the token.

## Known issues

- Image pulls require network access. CI must pre-pull the pinned image.
- `Drop` runs synchronous `podman stop` and `podman rm -f`, which can
  briefly stall a `tokio` worker thread (up to ~5 s if the kernel hangs
  waiting for SIGKILL). Acceptable at end-of-test.

## Debugging a failed test

Set `RUST_LOG=siyuan_testkit=debug,info` to see container IDs and
per-attempt health-check output. To inspect the workspace contents
after a failure:

```rust
let mut sy = SiyuanContainer::start().await?;
sy.persist_workspace_on_drop();
// ... run test, inspect path printed in the warn! log
```
