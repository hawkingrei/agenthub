## Summary

Split `src/actor_cli.rs` into smaller responsibility-oriented modules without changing command behavior.

## Why

The file had grown into a mixed parser/runtime/execution/test surface. That made navigation expensive for both human review and agent context loading.

## What Changed

- Kept `src/actor_cli.rs` as the parent module and test host.
- Moved command help text into `src/actor_cli/help.rs`.
- Moved output selection and encoding into `src/actor_cli/output.rs`.
- Moved runtime/internal-gRPC/bootstrap helpers into `src/actor_cli/runtime.rs`.
- Moved CLI argument parsing into `src/actor_cli/parse.rs`.
- Moved command execution into `src/actor_cli/execute.rs`.
- Tightened module-local imports so the split does not leave large parent-level unused import sets behind.

## Validation

- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all`
- `git -c core.fsmonitor=false diff --check`
