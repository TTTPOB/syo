# Test Framework Plan — Execution Decisions Log

This file captures decisions made during execution of `2026-05-01-test-framework.md` to
NOT fix code review findings, with rationale. Issues that WERE fixed land in the git
history instead. Append-only; ordered by review point.

## Task 3 — port allocation

- **Code review M/Important: `allocates_unique_ports` asserts `a >= 1024`.**
  Reviewer noted this tests the OS ephemeral port range, not module behavior.
  Decision: kept verbatim per plan spec. The assertion is a benign sanity check;
  removing it would diverge from spec for cosmetic reasons. If it ever fires it
  signals an OS / container misconfiguration worth surfacing.

- **Code review M/Important: `allocated_port_is_actually_bindable` has inherent
  race window.** Reviewer recommended an in-test comment acknowledging the race.
  Decision: kept verbatim per plan spec. The function-level doc comment already
  documents the race; an extra inline comment in the test would be redundant.

## Task 5 — podman wrapper

- **Code review Important: explicit `Stdio::piped()` on run/stop/rm/logs.**
  Reviewer suggested adding `.stdout(Stdio::piped()).stderr(Stdio::piped())` to
  match `require_podman` for stylistic consistency.
  Decision: skipped. `Command::output()` already pipes implicitly per the docs;
  the change is purely cosmetic and would add four duplicated lines per fn.

- **Code review Minor: no `tracing` in run/stop/force_remove/logs.**
  Reviewer suggested adding `debug!` lines around each command for diagnostic.
  Decision: skipped. The plan does not include them; container.rs (Task 7)
  already wraps these calls in its own info!/debug! lines that include
  container_id, which is the level a debugger actually wants.

- **Code review Minor: only happy-path test for `require_podman`.**
  Reviewer suggested an extra test that exercises the `bail!` path via
  `run_detached("invalid:image")`. Plan opted not to.
  Decision: kept per plan. Such a test pulls the network and is slow; the
  smoke suite already exercises error paths end-to-end.

- **Code review Minor: no `PODMAN_BIN` env override.**
  Reviewer suggested making the binary path overridable.
  Decision: skipped. Out of scope for this plan; can be added later if a CI
  with non-PATH podman appears.

- **Code review Minor: `logs()` does not separate stdout from stderr.**
  Reviewer suggested a `\n--- stderr ---\n` separator.
  Decision: skipped. SiYuan's kernel writes through a single Go log writer
  so streams are already interleaved; a separator would be misleading.

## Task 6 — health check

- **Code review Important: `code == 21` (auth rejected) keeps retrying until
  timeout.** Reviewer recommended fail-fast on code 21.
  Decision: deferred. SiYuan's kernel startup sequence is not fully mapped —
  the kernel could plausibly return non-zero codes briefly during warmup
  before auth is fully initialised. Failing fast on code 21 risks aborting
  on a transient state. The 60s outer timeout in container.rs (Task 7)
  bounds the worst case, and the dump-logs path on timeout already gives a
  clear diagnostic. If Task 8 shows that auth state stabilises fast enough
  to make fail-fast safe, revisit.

- **Code review Important: `unwrap_or("unknown")` masks malformed `data`.**
  Reviewer suggested bailing on missing-or-non-string `data` field.
  Decision: deferred. Plan explicitly uses unwrap_or. SiYuan's kernel
  always emits `data` in this endpoint; if it ever doesn't we want the
  successful boot to still proceed and surface the issue downstream,
  not block startup on a tracing artifact.

- **Code review Minor: `unwrap_or(-1)` sentinel undocumented.**
  Reviewer suggested an inline comment naming -1 as the sentinel.
  Decision: deferred. Cosmetic; the bail message includes the body which
  makes context obvious in failure logs.

- **Code review Minor: timeouts and intervals are hardcoded, not tunable.**
  Reviewer noted future reuse outside container startup might want overrides.
  Decision: deferred. Out of scope for this plan; the only consumer is
  container.rs.

- **Code review Minor: `wait_for_ready` could be `pub(crate)` instead of `pub`.**
  Decision: deferred. Plan uses `pub`; semantically equivalent inside the
  crate; rename can happen in a later cleanup pass if a public API surface
  is ever defined.

## Task 7 — SiyuanContainer

- **Code review Minor: `to_string_lossy` on workspace path.**
  Reviewer suggested `.to_str().context(...)?.to_string()` to fail loudly
  on non-UTF-8 tempdir paths.
  Decision: deferred. tempfile prefix is ASCII; only `$TMPDIR` could
  introduce non-UTF-8, and that's vanishingly rare on Linux. If it ever
  bites, the resulting podman error will be obvious.

- **Code review Minor: spawn_blocking closure block-as-argument style.**
  Reviewer suggested hoisting `let id = container.container_id.clone();`
  out of the spawn_blocking call.
  Decision: deferred. Cosmetic; the block scopes the temporary clone to
  the closure which is arguably more readable.

- **Code review Minor: add `port()` accessor.**
  Reviewer suggested storing port and exposing it.
  Decision: deferred. Out of plan scope. base_url() carries the port; if
  needed, callers can parse it. Add later if we get a real consumer.

- **Code review Minor: document `access_auth_code` default uniqueness.**
  Reviewer noted the bare "testkit" default could clash across containers.
  Decision: deferred. Only matters for UI access; tests use the API token,
  which IS unique per workspace. Document if a UI test scenario surfaces.

## Task 8 — smoke test (boot + auth)

- **Spike result (kept here for record):** plan changed mid-Task. Both tests
  switched from `/api/system/version` (unauthenticated, can't validate token)
  to `/api/notebook/lsNotebooks` (auth-enforced). conf.json schema
  `{"api":{"token":...}}` confirmed correct against SiYuan v3.6.5.
  Wrong-token responses are HTTP 401 with no JSON body (NOT 200 with code=-1
  as initially guessed); the `rejects_wrong_token` assertion uses
  `is_client_error()` to be robust against future 4xx code changes.

- **Code review Minor: hardcoded 120s ready_timeout vs default 60s.**
  Reviewer suggested a comment explaining the asymmetry.
  Decision: deferred. The asymmetry is incidental — the auth test was given
  more headroom for the first cold pull on a CI runner; the second test,
  written second, didn't bother. Both work; cosmetic.

- **Code review Minor: each test builds a fresh reqwest::Client.**
  Reviewer suggested testkit-provided client or shared helper.
  Decision: deferred. Two tests; abstraction premature. Revisit at 5+.

- **Code review Minor: request boilerplate duplicated.**
  Decision: deferred. Same rationale as above.
