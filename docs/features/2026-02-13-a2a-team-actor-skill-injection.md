# A2A Team Actor Runtime Skill Injection

## Summary

Inject actor mailbox usage guidance into ACP sessions at startup through a built-in
runtime skill, and provide a local actor CLI so agents can consume/send actor
messages through execute tools without direct DB or HTTP wiring in prompts.

## Background

Actor mailbox APIs already existed in Team backend, but agents had no stable,
boot-time protocol prompt and no dedicated local tool command to operate inbox,
ack, and send workflows.

## Scope

- `crates/agenthub-acp/src/lib.rs`
- `src/agent/manager.rs`
- `src/actor_cli.rs`
- `src/main.rs`
- `docs/todo.md`

## Key Decisions

- Add `AcpActorSkillContext` and inject a built-in runtime skill when actor
  context is present.
- Actor context is enabled by `AGENTHUB_ACTOR_RUN_ID` and defaults actor id from
  `AGENTHUB_ACTOR_ID` or agent name.
- Pass actor runtime env vars into spawned agent process:
  - `AGENTHUB_ACTOR_RUN_ID`
  - `AGENTHUB_ACTOR_ID`
  - `AGENTHUB_ACTOR_CHANNEL`
  - `AGENTHUB_ACTOR_CLI`
- Add local CLI entrypoint:
  - `agenthub actor inbox`
  - `agenthub actor ack`
  - `agenthub actor send`
- CLI supports env fallback for run/actor/channel to keep tool calls concise in
  agent prompts.

## Validation

```bash
cargo test actor_cli::tests -- --nocapture
cargo test actor_runtime_context_ -- --nocapture
cargo test -p agenthub-acp
```

## Follow-ups

- Replaced by explicit start-time actor context payload in
  `docs/features/2026-02-14-a2a-team-actor-runtime-context-start-api.md`.
- Actor send idempotency defaults and duplicate-delivery controls are implemented
  in `docs/features/2026-02-14-a2a-team-actor-idempotent-send.md`.
