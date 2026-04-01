# Team Leader Continuity Workspace And Session Semantics

## Summary

- keep Team leader runtime workdirs stable across ordinary restarts by removing the
  per-launch session token from the derived leader workspace path
- clarify in code comments that AgentHub's runtime `session_id` is still a per-launch identifier
- clarify that ACP provider continuity is persisted and resumed separately from the runtime launch id
- document that `Force New Session` is the intentional path that clears ACP continuity before restart

## Why

Team leader coordination artifacts such as `AGENTS.md` and `TODO.md` were effectively session-local.
The leader runtime derived its sandbox path with the current AgentHub launch session id, so an
ordinary restart moved the leader into a fresh directory and made previous coordination files appear
to disappear.

At the same time, ACP provider continuity is already modeled separately through persisted provider
session ids. That meant the runtime launch id was being used for workspace identity even though the
actual memory continuity boundary lived elsewhere.

Recent Team runtime work made it easy to conflate three related but distinct concepts:

- the AgentHub runtime launch id used for a specific local process start
- the persisted ACP/Codex session id used to resume provider-side memory and thread continuity
- the explicit Team `Force New Session` control that should drop provider continuity on purpose

This change fixes the leader workspace identity bug and keeps the code comments explicit at the
critical boundaries.

## Historical Context

- `docs/journal/2026-03-23-acp-resume-dirty-session-fallback.md`
  established that persisted ACP session ids are provider continuity state and may need targeted
  reset when a dirty resume exits immediately during startup
- `docs/journal/2026-03-24-team-runtime-force-new-session-and-dirty-codex-resume.md`
  established that Team `Force New Session` intentionally clears the selected member's persisted ACP
  session before restart
- `docs/journal/2026-03-19-team-agenthubmemory-contract.md`
  kept long-lived worker project memory under `.agenthubmemory/`, separate from runtime continuity
- `docs/journal/2026-02-23-team-leader-default-workdir-under-agenthub.md`
  documented the leader empty-workspace startup rule

## Files Updated

- `src/agent/manager.rs`
- `src/agent/manager/session.rs`
- `src/agent/manager/tests.rs`
- `src/team/runtime.rs`
- `docs/todo.md`

## Validation

- `git -c core.fsmonitor=false diff --check`
