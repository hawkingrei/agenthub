# Mailbox ACK Writer Contention

## Summary

Acquire SQLite write ownership before reading a message for acknowledgement. Concurrent relay
or background writes must not turn an otherwise valid ACK into a read-to-write upgrade failure.

## Background

The first P2P CI attempt for PR #1168 at `9165a136` failed while acknowledging a received message:
`database is locked` escaped through the actor mailbox RPC. The ACK transaction began deferred,
read the current message, then tried to acquire write ownership for its delivery update.

## Scope

- Use `BEGIN IMMEDIATE` for the ACK read/update transaction.
- Preserve actor/run/peer checks, duplicate ACK behavior and the original delivery timestamp.
- Keep the RPC contract, schema and existing busy timeout unchanged.

## Key Decisions

Wait for the writer before opening the ACK read snapshot, using the same transaction mode as
other read-modify-write operations. Do not retry the external RPC or weaken the P2P assertions.

The regression uses production migrations and two pre-opened WAL connections. It holds a
competing write transaction while polling ACK, then releases it. Cases cover an unrelated writer
and a competing delivery of the same message, followed by an idempotent duplicate ACK.

## Validation

The new focused regression reproduces SQLite error 5 against the original deferred transaction.
After the fix, all 21 mailbox cases and the real two-daemon P2P fixture pass. On the local host,
the P2P health probe needs a command-local proxy bypass for loopback addresses.
Root library/test Clippy with warnings denied and workspace formatting also pass.

Validation commands:

```bash
cargo test --locked --offline -p agenthub --lib team::manager::tests::mailbox_basic_cases::
cargo build --locked --offline -p agenthub-daemon --bin agenthubd
NO_PROXY=localhost,127.0.0.1,::1 no_proxy=localhost,127.0.0.1,::1 cargo test --locked --offline --test distributed_p2p_pipeline -- --nocapture
cargo clippy --locked --offline -p agenthub --lib --tests -- -D warnings
cargo fmt --all --check
```

## Follow-Ups

Verify current-head CI after publishing the ACK fix. Deferred native capability acceptance
remains outside this concurrency correction.
