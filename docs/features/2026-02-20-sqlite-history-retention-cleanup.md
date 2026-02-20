# SQLite History Retention Cleanup

## Summary

Add startup retention cleanup for `agent_events` with configurable history window,
defaulting to 5 days. Support optional post-cleanup database compaction via
`VACUUM` for operators who prefer reclaiming file size aggressively.

## Background

Long-running AgentHub deployments accumulate large `agent_events` volume,
especially ACP streams. Without retention, SQLite file growth becomes significant
and WAL checkpoint alone cannot reclaim historical rows that should no longer be
kept.

## Scope

- Add `history` config section in `AppConfig`:
  - `history.event_retention_days` (default `5`; `0` disables cleanup)
  - `history.vacuum_on_cleanup` (default `false`)
- Add DB helper to delete old `agent_events` rows by `ts` cutoff.
- Add startup hook in `AppState::init` to run cleanup once per process start.
- Add index `idx_agent_events_ts` to keep retention delete scans bounded.
- Add unit coverage for config defaults/overrides and cleanup cutoff behavior.

## Key Decisions

1. Keep retention focused on `agent_events` only.
   - This table dominates growth and is safe to prune independently.
2. Use startup-triggered cleanup instead of periodic background worker.
   - Minimal runtime complexity and no extra scheduling state.
3. Make `VACUUM` opt-in.
   - `VACUUM` can be expensive on large files; default should favor startup
     predictability.
4. Treat cleanup failures as non-fatal.
   - Startup continues, but warning logs preserve diagnostics.

## Validation

Suggested commands:

```bash
cargo test history_
cargo test cleanup_agent_event_history_deletes_rows_older_than_retention
cargo build
```

Manual config check example:

```toml
[history]
event_retention_days = 5
vacuum_on_cleanup = false
```

## Follow-ups

- Consider periodic incremental cleanup with bounded delete batch size for very
  large deployments that run continuously without restarts.
- Consider additional retention knobs for high-volume audit-like tables if they
  become primary growth contributors.
