# Agent-Idle Triggered History GC (Remove Startup/Interval Triggers)

## Summary

Switch history GC trigger strategy from process-level startup/interval execution to
agent-level idle-triggered checks.

New policy:

- if an agent has no input and no output for 5 minutes, run one GC check;
- while the agent remains idle, do not repeat checks;
- after the next activity, arm the next idle window again.

## Scope

- `src/state.rs`
  - remove startup immediate cleanup trigger
  - remove periodic background cleanup worker trigger
  - configure and inject idle-GC controller into `AgentManager`
- `src/db.rs`
  - add `AgentEventIdleGc` controller
  - maintain per-agent generation state to guarantee one check per idle window
  - run cleanup against per-agent event DB only when idle condition is met
- `src/agent/manager.rs`
  - add `idle_gc` dependency and `record_agent_activity` helper
  - mark activity on successful input paths
- `src/agent/manager/runtime.rs`
  - mark activity on stdout/stderr/run-status output paths
  - clear idle-GC state on process finalize/delete
- `src/acp/event_sink.rs`
  - mark activity on ACP event output path
- `src/config.rs`
  - remove unused interval GC config surface (`cleanup_interval_seconds`)
  - keep retention/vacuum/batch settings for idle-triggered checks

## Validation

Executed:

```bash
cargo check -p agenthub
cargo test -p agenthub idle_gc_checks_only_once_per_idle_window -- --nocapture
cargo test -p agenthub delete_agent_removes_related_rows -- --nocapture
cargo test -p agenthub team_runs_api_supports_manual_context_flush -- --nocapture
cargo test -p agenthub spawn_output_reader_promotes_latest_codex_event_types_for_acp_agents -- --nocapture
```

All commands passed.

## Notes

- Idle timeout is currently fixed at 5 minutes (`300s`) per requirement.
- The idle trigger is activity-driven (input/output); it does not add periodic global scans.
