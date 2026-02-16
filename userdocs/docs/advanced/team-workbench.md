---
sidebar_position: 1
---

# Team Workbench

Team Workbench (`/teams`) is the multi-agent workflow area for orchestrated
team runs.

## When to Use Team Workbench

Use Team Workbench when a task is better modeled as coordinated steps across
multiple actors, not a single linear agent session.

Examples:

- Planner + implementer + reviewer pipelines
- Message-driven worker collaboration
- Step dependencies and explicit run-state transitions

## Main UI Areas

- **Teams**: create/select team definitions
- **Create / Load Run**: start new runs or load existing run IDs
- **Active Run**: run metadata and cancel action
- **Tabs**:
  - `Events`: event timeline with optional auto refresh
  - `Steps`: submit steps and apply step actions
  - `Messages`: actor mailbox send/inbox/ack flow

## Basic Workflow

1. Open `/teams`.
2. Create or select a team.
3. Create a run with optional `context_id`.
4. Watch `Events` for lifecycle progress.
5. Inspect or operate `Steps` as needed.
6. Use `Messages` for actor-level coordination.

## Step Actions in UI

Available actions:

- `start`
- `complete`
- `fail`
- `input_required`
- `resume`

Use these only when you understand the current run state and transition
expectations.

## Operational Tips

- Keep team specs small for first rollout.
- Prefer explicit step dependencies over implicit ordering assumptions.
- Keep event logs for audit and run replay.
- Use idempotency keys for repeated message send operations when needed.

## Related Pages

- [OpenAPI and Automation](./openapi-and-automation.md)
- [Session Lifecycle](../core/session-lifecycle.md)
- [Troubleshooting](../operations/troubleshooting.md)
