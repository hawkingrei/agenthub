---
title: Refresh Agent Status When Input Fails
date: 2026-02-09
status: implemented
---

## Summary

When sending input fails with `agent not running`, refresh agent status to
avoid stale UI state.

## Background

The UI can show `running` while the agent process has already exited. Sending
input in this state fails, but the UI continues to display a running status.

## Decision

- On `agent not running` errors, trigger `refreshAgents()` to sync UI.
- On the backend, mark running sessions as exited if the in-memory handle is
  missing when input arrives.

## Scope

- `src/agent/manager.rs`
- `web/src/app.tsx`

## Validation

- Kill an agent process, then try sending input; UI should refresh to non-running.
