---
title: Agent Runtime Robustness Fixes
date: 2026-02-10
status: implemented
---

## Summary

Harden agent lifecycle and ACP handling to avoid stale sessions, brittle deletes,
and partial tool-call output regressions.

## Background

A few edge cases surfaced in production:

- Deleting agents with existing events could fail due to foreign key ordering,
  leaving rows in the list after refresh.
- Deleting agents with pending ACP permissions could fail due to foreign key
  constraints against agent sessions.
- `start_agent` could return a stale session_id if the process exited before the
  exit watcher cleaned up.
- Gemini/Kimi were treated as ACP providers based solely on the command name,
  even when ACP flags/subcommands were missing.
- Clearing ACP sessions relied on a client-provided provider value.
- Tool-call content merging could drop longer fragments when updates were not
  strict prefixes.

## Decision

- Delete ACP permission rows and agent events before sessions and commit
  deletions in a transaction.
- Detect stale child processes during `start_agent` and finalize their exit
  before starting a new session.
- Require Gemini/Kimi ACP args to confirm ACP provider detection.
- Clear ACP sessions without requiring a client-provided provider; fall back to
  `codex` if the agent record is missing.
- Prefer the longer tool-call fragment when updates are not prefix-related.

## Scope

- `src/agent/manager.rs`
- `src/api/agents.rs`
- `src/acp.rs`
- `web/src/app.tsx`
- `web/src/api.ts`
- `web/src/acp.ts`
- `web/src/create_agent_modal.test.tsx`

## Validation

- [ ] Delete an agent with existing output; verify it stays deleted after refresh.
- [ ] Start an agent after an exited run; verify a new session starts.
- [ ] Create Gemini/Kimi without ACP args and confirm it is treated as non-ACP.
- [ ] Clear ACP session without a provider and ensure it succeeds.
- [ ] Confirm tool-call output keeps the longest fragment during updates.
