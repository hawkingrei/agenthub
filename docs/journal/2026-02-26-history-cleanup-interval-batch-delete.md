# Periodic History Cleanup With Batched Delete

## Summary

Improve `agent_events` retention cleanup to avoid long-running full-table delete
transactions and reduce retention lag on long-lived processes:

- add periodic cleanup worker in runtime state initialization;
- add bounded batched delete in DB cleanup helper;
- add new `history` config knobs for cleanup cadence and batch size.

## Scope

- `src/config.rs`
  - add `history.cleanup_interval_seconds` (default `300`, clamped to `[10, 86400]`)
  - add `history.delete_batch_size` (default `10000`, clamped to `[100, 200000]`)
- `src/db.rs`
  - change `cleanup_agent_event_history` to delete by batches:
    `DELETE ... WHERE id IN (SELECT id ... LIMIT ?)`
  - expose `delete_batches` in cleanup result for observability/tests
- `src/state.rs`
  - keep startup cleanup pass
  - add background periodic cleanup worker using configured interval and batch size
  - extend cleanup logs with trigger, batch size, and batch count
- `docs/todo.md`
  - update retention verification item wording from startup-only cleanup to periodic + batch cleanup

## Validation

Executed:

```bash
cargo test --package agenthub history_
cargo test --package agenthub cleanup_agent_event_history
```

Both commands passed, including:

- `history_defaults_use_five_days_and_no_vacuum`
- `history_config_applies_custom_values`
- `history_retention_can_be_disabled_with_zero`
- `cleanup_agent_event_history_deletes_rows_older_than_retention`
- `cleanup_agent_event_history_deletes_in_multiple_batches`

## Notes

- Startup cleanup remains in place to keep behavior deterministic after restart.
- Periodic cleanup now handles long-uptime processes where startup-only cleanup
  was insufficient.
