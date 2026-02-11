---
title: Agent Start Already Running Handling
date: 2026-02-10
status: implemented
---

## Summary

Handle `agent already running` start requests gracefully by refreshing UI state
and returning the existing session ID from the backend.

## Background

Clicking Start on an agent that is already running can return an error while the
UI still shows it as stopped, leaving the user with a stale status and no clear
resolution.

## Decision

- Frontend: when the start request fails with `agent already running`, refresh
  the agents list instead of showing an error.
- Backend: treat start as idempotent by returning the running session ID when
  the agent is already active.

## Scope

- `web/src/app.tsx`
- `src/agent/manager.rs`

## Validation

- [ ] Start a running agent and confirm the UI refreshes to the running state
  without an error.
- [ ] Verify the backend returns the existing session ID for already-running
  agents.
