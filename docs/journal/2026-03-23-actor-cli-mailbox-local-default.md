# Actor CLI Mailbox Local Default

## Summary

- Kept `agenthub actor inbox|ack|send` on the local `TeamManager` mailbox service by default.
- Limited internal gRPC mailbox clients to ACP hint delivery only.
- Added a regression test that proves runtime internal gRPC env vars do not hijack CLI mailbox commands away from the local mailbox database.

## Why

- Agent runtime shells inherit internal gRPC env vars used for ACP input nudges.
- CLI mailbox commands were incorrectly treating those env vars as a signal to switch mailbox transport to remote internal gRPC.
- In local runtime scenarios this caused `actor inbox/ack/send` to fail with internal mailbox errors even though the local mailbox sqlite state was valid.

## Validation

- `cargo fmt --all`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `git -c core.fsmonitor=false diff --check`
