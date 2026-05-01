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
