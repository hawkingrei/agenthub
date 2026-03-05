# Per-Agent SQLite Split For Agent Events (Including Teams Paths)

## Summary

Split runtime event persistence from the shared `agenthub.db` into per-agent SQLite
files under `~/.agenthub/agent-events/<agent_id>.db`.

This change targets lock contention on `agent_events` writes and keeps teams memory
flush/delete paths compatible with the new storage layout.

## Scope

- `src/db.rs`
  - add `AgentEventDbRouter` to route/open/remove per-agent SQLite files
  - add per-agent `agent_events` schema bootstrap (`session_id/seq/ts/stream/message`)
  - reuse shared SQLite connection defaults through `connect_sqlite_with_defaults`
  - ensure per-agent DB parent dir is created before first connection
- `src/agent/manager.rs`
  - add `event_dbs` dependency to `AgentManager`
  - route `list_events`, `list_events_for_session`, failure/session/user input event writes to per-agent DB
  - pass `event_dbs` into ACP event sink and process-finalization path
- `src/agent/manager/runtime.rs`
  - route stdout/stderr/ACP run-status writes to per-agent DB
  - route process-exit finalization event write to per-agent DB
  - delete flow now removes per-agent event DB file via router
- `src/acp/event_sink.rs`
  - replace shared DB pool usage with `AgentEventDbRouter`
- `src/team/manager.rs`
  - add `event_dbs` dependency with `new_with_event_dbs`
  - `flush_run_context` now reads from per-agent DB instead of shared `agenthub.db`
  - `delete_team` now removes each member's per-agent event DB after SQL transaction commit
- State/test wiring
  - `src/state.rs`, `src/sse.rs`, `src/api/agents.rs`, `src/api/teams/tests.rs`
  - ensure `AgentManager` and `TeamManager` share the same `AgentEventDbRouter` in each state builder
- Test adaptations
  - `src/agent/manager/runtime.rs`
  - `src/team/manager/tests.rs`
  - `src/api/teams/tests_core.rs`
  - `src/api/teams/tests_router.rs`

## Validation

Executed:

```bash
cargo check -p agenthub
cargo test -p agenthub delete_agent_removes_related_rows -- --nocapture
cargo test -p agenthub flush_run_context_persists_artifact_and_then_noops_with_checkpoint -- --nocapture
cargo test -p agenthub spawn_output_reader_promotes_latest_codex_event_types_for_acp_agents -- --nocapture
cargo test -p agenthub teams_api_delete_team_cascades_related_run_data -- --nocapture
cargo test -p agenthub teams_router_delete_team_cleans_member_session_dependents_without_500 -- --nocapture
cargo test -p agenthub team_runs_api_supports_manual_context_flush -- --nocapture
```

All commands passed.

## Notes

- This phase splits `agent_events` storage first and updates teams read/delete flows.
- Team metadata tables remain in the main `agenthub.db`; a full `teams` DB split is a separate migration.
