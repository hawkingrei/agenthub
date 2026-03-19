# Team Agent Loop Idle Follow-up

## Summary

This change adds phase-1 ACP `agent_loop` support without changing the default
Team workflow.

The feature is intentionally narrow:

- it is disabled by default
- it is enabled externally by a human/operator
- Team usage is per-agent/member, not a global team switch
- it watches ACP silence only and injects a configured follow-up prompt later
- it must not block normal start/send/profile-edit flows when disabled or when
  loop updates fail

## Backend

- Extended `agents` persistence with:
  - `agent_loop_enabled`
  - `agent_loop_idle_seconds`
  - `agent_loop_prompt`
- Added `POST /api/agents/:id/agent_loop` for human/operator control.
- Implemented `AgentManager::set_agent_loop_config(...)`:
  - validates idle timeout and prompt when enabling
  - persists config even if the agent is not currently running
  - reconfigures or spawns the ACP idle watchdog for running ACP sessions
  - disables the watchdog without blocking the rest of the runtime flow
- Added an ACP idle watchdog that:
  - subscribes to agent output
  - ignores its own synthetic loop prompt events
  - injects one configured prompt after a silence timeout
  - rearms only after real non-loop output resumes

## Team UI / Team Member Editing

- Team-owned member profile editing now exposes:
  - loop enabled/disabled
  - idle timeout seconds
  - loop prompt
- Saving Team member profile still updates the Team spec first.
- Loop config is applied best-effort afterwards through `/api/agents/:id/agent_loop`.
- If loop config update fails, profile save remains successful and the UI shows a
  warning instead of blocking the whole edit flow.

## Prompt / Skill Contract

- Updated Team leader/worker prompts and shared Team skills so agents know:
  - `agent_loop` is operator-controlled
  - it is disabled by default
  - injected loop prompts are follow-up nudges, not new human intent
  - agents should not self-enable or retune `agent_loop` unless explicitly asked

## Validation

- `cargo test set_agent_loop_config_updates_agent_row_without_blocking_runtime`
- `cargo test set_agent_loop_route_updates_agent_config_without_blocking`
- `cargo test -p agenthub-team-prompts`
- `cd web && npx vitest run src/pages/team/create_helpers.test.ts src/pages/team/member_helpers.test.ts`

## Follow-up

- Verify deployed Team member profile UI can enable/disable loop settings for a
  selected member without breaking normal profile edits.
- Verify a running ACP Team member receives the configured loop prompt only
  after the silence timeout and that normal output rearms the watchdog.
