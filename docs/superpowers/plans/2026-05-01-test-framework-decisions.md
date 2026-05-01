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
