## Summary

- added focused unit tests for `actor_runtime_env` helper branches
- added focused unit tests for `internal::client` parsing, path validation, and gRPC status
  mapping helpers

## Why

- `#177` was functionally green, but patch coverage was still red
- the lowest-cost coverage wins were in pure helper code:
  - runtime loopback / TLS path selection
  - internal mailbox client parsing and error mapping
- these tests improve regression protection without widening the behavior surface of the PR

## Validation

- `cargo test -p agenthub actor_runtime_env::tests -- --nocapture`
- `cargo test -p agenthub internal::client::tests -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
