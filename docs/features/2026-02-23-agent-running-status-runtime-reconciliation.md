# Agent Running Status Runtime Reconciliation

## Background

After abrupt `agenthub` termination (for example direct kill), users could occasionally observe:

- UI shows agent status as `running`
- sending input returns `agent not running`

This mismatch indicates DB status/session rows may remain `running` while the in-memory runtime handle is gone.

## Scope

- Reconcile stale `running` rows before serving agent list/detail APIs.
- Preserve existing startup and send-input fallback behavior.
- Add regression tests for list/get auto-heal behavior.

Out of scope:

- distributed multi-instance runtime ownership
- long-term external process reattach support

## Key Decisions

1. Add a manager-level self-heal pass:
   - load DB agents with `status='running'`
   - compare against runtime in-memory active handles + startup window set
   - if an agent is `running` in DB but missing in runtime and not starting, mark it stale
2. For each stale running agent:
   - update `agent_sessions` (`running` -> `exited`, set `ended_at`)
   - update `agents` (`running` -> `exited`)
3. Trigger reconciliation before:
   - `list_agents`
   - `get_agent`
4. Keep `send_input` fallback unchanged as secondary protection.

## Files Changed

- `src/agent/manager.rs`
- `src/agent/manager/runtime.rs`
- `docs/todo.md`

## Validation

Executed during development:

- `cargo test list_agents_reconciles_stale_running_status_without_runtime_handle -- --nocapture`
- `cargo test get_agent_reconciles_stale_running_status_without_runtime_handle -- --nocapture`

## Risks And Follow-up

- Reconciliation is single-process oriented and assumes runtime truth is local in-memory handles.
- Multi-instance shared-DB ownership would require explicit leader-election/lock semantics before status healing.
