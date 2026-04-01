# Runtime Launch vs ACP Continuity Comments

## Summary

- clarified in code comments that AgentHub's runtime `session_id` is a per-launch identifier
- clarified that ACP provider continuity is persisted and resumed separately from the runtime launch id
- documented that `Force New Session` is the intentional path that clears ACP continuity before restart

## Why

Recent Team runtime work made it easy to conflate three related but distinct concepts:

- the AgentHub runtime launch id used for a specific local process start
- the persisted ACP/Codex session id used to resume provider-side memory and thread continuity
- the explicit Team `Force New Session` control that should drop provider continuity on purpose

The implementation already modeled these separately, but the code lacked inline comments near the
critical call sites. This note ties the clarification back to the earlier Team runtime recovery and
ACP dirty-resume work.

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
- `src/team/runtime.rs`

## Validation

- `git -c core.fsmonitor=false diff --check`
