# Summary

Started the first CLI ergonomics slice for issue `#244` (`ux: reduce leader orchestration friction across mailbox and team-task workflows`).

This round keeps the storage/runtime model intact:

- direct mailbox traffic remains run-scoped
- human-visible shared updates move through Team task/shared-thread note paths instead of overloading mailbox sends

## Why

Leader workflows were paying too much mechanical cost at the CLI boundary:

- `ack` required repeated flag-heavy message identifiers
- `send` required remembering the older `--to-actor-id` / `--channel-id all` spelling
- `team-task-update` context patch inputs were verbose
- human-visible shared progress updates still looked like mailbox operations even though they map better to Team conversation/task history

At the same time, removing `run_id` from all mailbox commands would be incorrect because mailbox persistence, replay, and dedupe are still run-scoped.

## What Changed

### Mailbox command ergonomics

- `agenthub actor ack` now accepts positional message ids
  - example: `agenthub actor ack 41 42`
- `agenthub actor send` now accepts:
  - `--to` and `--direct` as aliases for direct actor targets
  - `--shared` as shorthand for `--channel-id all`
- missing `run_id` errors now point operators at the current runtime-env fallback:
  - `AGENTHUB_ACTOR_CURRENT_RUN_ID`

### Task workflow ergonomics

- `agenthub actor team-task-update` now also accepts:
  - `--context-file`
  - `--context-merge-file`
- `agenthub actor team-task-note` now accepts:
  - `--shared-thread`
- `--shared-thread` resolves the canonical Team `all` thread through existing Team task metadata instead of forcing operators to look up a shared task id manually

### Help text

- actor CLI help now documents:
  - positional `ack`
  - `send --to/--direct`
  - `send --shared`
  - `team-task-update --context-file`
  - `team-task-note --shared-thread`
- help text explicitly steers human-visible shared updates toward `team-task-note --shared-thread`

## Validation

- `cargo test -p agenthub parse_ack_accepts_positional_message_ids -- --nocapture`
- `cargo test -p agenthub parse_send_accepts_direct_alias_and_shared_flag -- --nocapture`
- `cargo test -p agenthub parse_team_task_update_accepts_context_merge_file_alias -- --nocapture`
- `cargo test -p agenthub parse_team_task_note_accepts_shared_thread_without_task_id -- --nocapture`
- `cargo test -p agenthub resolve_shared_thread_task_id_prefers_canonical_shared_thread_task -- --nocapture`
- `cargo fmt --all`
- `cargo clippy --locked -p agenthub --all-targets -- -D warnings`

## Follow-up

This does **not** yet implement unique-active-run inference for direct mailbox commands.

Current contract after this change:

- human-visible shared updates: use `team-task-note --shared-thread`, `run_id` optional
- direct mailbox traffic: still run-scoped, but now with better hints and shorter flags

The remaining issue `#244` follow-up is to resolve `run_id` automatically when actor/team runtime context yields exactly one active candidate, while failing loudly when multiple active runs make the scope ambiguous.
