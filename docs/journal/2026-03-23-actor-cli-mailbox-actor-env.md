# Actor CLI Mailbox Actor-ID Env Tightening

## Summary

- tightened mailbox-oriented Actor CLI commands to require `AGENTHUB_ACTOR_ID` for env fallback
- stopped `inbox`, `ack`, and `send` from implicitly using `AGENTHUB_ACTOR_AGENT_ID`
- kept explicit `--agent-id`, `--from-agent-id`, and `--to-agent-id` flag aliases unchanged for now

## Why

- mailbox routing keys are actor ids, not runtime agent ids
- allowing `AGENTHUB_ACTOR_AGENT_ID` as an implicit fallback made mailbox debugging ambiguous
- when the runtime only exported an agent id, CLI mailbox commands could target the wrong recipient and surface confusing internal mailbox failures

## Validation

- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `git -c core.fsmonitor=false diff --check`
