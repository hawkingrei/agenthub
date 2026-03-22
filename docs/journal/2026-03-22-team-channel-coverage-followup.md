## Summary

Added focused test coverage for the Team channel mailbox helpers and actor channel idempotency helpers on `main`.

## What changed

- Added mailbox helper unit tests in `src/team/manager/mailbox.rs` covering:
  - channel payload normalization for string/non-object inputs
  - deterministic correlation-id fallback behavior
  - channel mailbox forward payload metadata + mention propagation
  - human-visible chat reply persistence guardrails
  - canonical chat reply extraction from stringified JSON payloads
- Added idempotency helper unit tests in `crates/agenthub-team-actor/src/idempotency.rs` covering:
  - per-recipient fanout idempotency determinism
  - channel target changes producing distinct default idempotency keys

## Validation

- `cargo test -p agenthub-team-actor --lib idempotency::tests -- --nocapture`
- `cargo test team::manager::mailbox::tests -- --nocapture`
- `cargo fmt -- crates/agenthub-team-actor/src/idempotency.rs src/team/manager/mailbox.rs`
- `git -c core.fsmonitor=false diff --check`
