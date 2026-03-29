# Summary

Finish the remaining `#244` mailbox run-scope ergonomics work for direct
mailbox commands without changing the underlying mailbox storage model.

This round keeps mailbox persistence run-scoped, but removes the need to pass
`--run-id` on every `actor send` / `actor ack` when Team runtime context already
identifies one unambiguous active run.

## Why

The previous slice improved operator UX for inbox reads and shared-thread task
notes, but direct mailbox commands still stopped too early at the CLI boundary:

- `actor send` and `actor ack` required an explicit `run_id` even when the
  actor runtime already exposed the current run;
- Team members working in a Team-scoped shell could not reuse the unique active
  Team run without manually looking it up first;
- the fallback story between shared-thread conversation updates and run-scoped
  mailbox sends remained uneven.

That friction was especially visible in leader/worker orchestration loops where
the mailbox command itself was correct, but the operator still had to retype
scope information that AgentHub already knew.

## What Changed

### Direct mailbox commands now resolve run scope at execute time

- `agenthub actor ack` and `agenthub actor send` now accept missing `run_id`
  during parsing.
- Execution resolves the effective run scope in this order:
  - use explicit `--run-id` when present;
  - otherwise use `AGENTHUB_ACTOR_CURRENT_RUN_ID` from actor runtime env;
  - otherwise ask internal gRPC to resolve actor/team scope.

### Added internal run-scope resolution RPC

- Introduced `ResolveActorRunScope` on the internal Team control service.
- Resolution policy:
  - prefer the current actor runtime context when the agent is already running;
  - otherwise, if Team scope is available, fall back to the unique active Team
    run for that Team;
  - reject ambiguous multi-run cases with concrete candidate hints instead of
    guessing.

### Shared-thread behavior stays separate

- `actor inbox` keeps using the canonical Team shared-thread mailbox run fallback
  from the earlier slice when only Team scope is available.
- Human-visible shared progress updates still belong on
  `agenthub actor team-task-note --shared-thread ...`, not on direct mailbox
  send.

### Parser / idempotency follow-up

- `actor send` default idempotency keys are now finalized at execute time once
  the effective `run_id` is known.
- Explicit idempotency keys and `--allow-duplicate` semantics stay unchanged.

## Validation

- `cargo test -p agenthub parse_ack_allows_team_scope_without_current_run_id -- --nocapture`
- `cargo test -p agenthub parse_send_defers_default_idempotency_key_without_current_run_id -- --nocapture`
- `cargo test -p agenthub actor_cli::tests -- --nocapture`
- `cargo test -p agenthub list_active_runs_for_team_excludes_shared_thread_mailbox_runs -- --nocapture`
- `cargo test -p agenthub internal_grpc_resolve_actor_run_scope_ -- --nocapture`
- `cargo test -p agenthub grpc_client_resolves_unique_actor_run_scope_from_team_context -- --nocapture`
- `make proto-check`
- `cargo fmt --all`

## Follow-up

- Record push / `pull_request` CI run IDs here after merge verification.
- Keep shared-thread mailbox fallback and direct-mailbox run inference documented
  together with `docs/journal/2026-03-29-actor-inbox-shared-thread-run-fallback.md`.
