# Message Archive Step Lifecycle Run Events

## Summary

Team step lifecycle run events now dual-write to the message archive after the enclosing SQLite
transaction commits. This closes the previous gap where run submission, public run events, mailbox
events, and memory flush events were searchable live, but transaction-heavy step lifecycle events
still required migration to appear in archive-backed run-event search.

## Background

The message archive rollout keeps SQLite as the transactional source of truth while making
message-shaped history searchable through the archive backend. Step lifecycle transitions emit
human-visible operational run events such as `step_submitted`, `step_working`,
`step_input_required`, `step_resumed`, `step_continued`, `step_completed`, `step_failed`,
`step_canceled`, reconcile-round events, continuity-state events, and matching run-status events.
Those events are created inside SQLite transactions, so archive writes must happen only after commit.

## Scope

- Centralized transaction-local run-event insertion through `append_run_event_tx`.
- Collected created run-event records during step lifecycle transactions.
- Spawned archive work only after commit so rolled-back lifecycle attempts are not searchable.
- Batched multi-event archive fan-out into one background task and one archive append per committed
  event batch.
- Reused run-scope and task-conversation lookup caches while building batch archive documents.
- Added focused manager tests for run creation, step lifecycle transitions, cancellation, reconcile
  continuation, failure, and run-context read models.

## Key Decisions

- Archive failures remain best-effort warnings and do not roll back committed Team state.
- Batch archive writes preserve deterministic per-event document identities while avoiding one
  background task and one archive append per materialized step.
- Shared-thread mailbox bootstrap runs continue to be skipped by the archive document builder.
- The stable behavior contract remains in `docs/features/message-archive-lancedb.md`; this journal
  records implementation evidence for the step-lifecycle slice.

## Validation

```bash
cargo fmt --all
git diff --check
cargo test -p agenthub step_lifecycle_transitions_persist_and_emit_events -- --nocapture
cargo test -p agenthub create_run_materializes_input_step_template_into_run_steps -- --nocapture
cargo test -p agenthub migrate_team_messages_to_archive_replays_agent_events_with_acp_aggregation -- --nocapture
cargo test -p agenthub cancel_run_only_cancels_active_steps -- --nocapture
cargo test -p agenthub reconcile_loop_step_tracks_round_state_and_events -- --nocapture
cargo test -p agenthub fail_step_updates_status_and_emits_event -- --nocapture
cargo test -p agenthub run_context_read_models_reflect_actor_and_session_state -- --nocapture
```

CI follow-up for PR `#501`:

```bash
gh pr checks 501 | cat
```

## Follow-Ups

- Keep the archive recovery boundary documented as historical migration plus deterministic document
  ids until the archive backend exposes inserted/updated write results.
- Continue rolling archive-backed read/search paths forward in small PRs after this live dual-write
  slice lands.
