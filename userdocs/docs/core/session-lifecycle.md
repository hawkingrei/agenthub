---
sidebar_position: 8
---

# Session Lifecycle

This page explains how session state changes over time and what each action
means for users.

## Typical States

- `created`: agent exists but not running yet
- `running`: task process is active
- terminal states such as `completed`, `failed`, `cancelled`, `exited`

## User Actions and Effects

Start:

- Boots runtime process for the selected agent
- Reuses single active runtime guard to prevent duplicate starts

Stop/Interrupt:

- Requests the in-progress run to stop
- Keeps history for later audit and replay

Delete:

- Removes the agent entry from management list
- Should be used only after output/history are no longer needed

## Reconnect Behavior

If browser closes during a run:

- Backend process keeps the session alive
- Reopening the UI lets you continue from existing state

## Operational Tips

- Prefer one goal per session to keep history understandable
- Use new sessions for risky refactors or experiments
- Keep failed sessions for debugging; do not delete immediately
