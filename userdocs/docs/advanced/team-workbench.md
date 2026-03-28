---
sidebar_position: 1
---

# Team Workbench

Team Workbench (`/teams`) is AgentHub's multi-agent collaboration surface.

## Mental Model

Think about Team Workbench as three connected layers:

- **Conversation** (`# all`): the human-facing shared thread
- **Kanban**: the canonical task lane
- **Runs and debug surfaces**: execution telemetry and deep inspection

The important distinction is that Team `runs` and `steps` are execution and
debug artifacts. They are not the primary planning surface.

## Main UI Areas

- **Channel**:
  - shared conversation thread, usually `# all`
  - human goals, constraints, approvals, and `@member_id` coordination requests
- **Kanban**:
  - task ownership and lifecycle
  - the durable place to see work status
- **Agents**:
  - member list and per-member entry into `Agent ACP`
- **Runs**:
  - explicit execution history, start, and selection
- **Advanced / Debug**:
  - lower-level execution and inspection tools

## What Good Team Usage Looks Like

Healthy Team usage usually follows a stable split:

- humans talk in `Conversation`
- the leader turns agreed work into canonical tasks
- workers report progress and facts without taking over planning
- `Kanban` stays the source of truth for task state
- `Runs` and `Agent ACP` stay available for debugging, not for day-to-day
  planning

If `Runs` becomes the main place people track work, the Team model is usually
drifting too close to raw execution details.

## Typical Workflow

1. Create or select a Team.
2. Start the Team runtime.
3. Use `Conversation` to state the goal and constraints.
4. Let the leader turn agreed work into canonical Kanban tasks.
5. Track ownership and progress in `Kanban`.
6. Drop into `Agent ACP`, `Runs`, or `Advanced` only when you need execution
   details or debugging.

## Mentions And Shared Conversation

- No `@mention`: message is team-wide and the leader should respond first.
- `@member_id`: message is still visible in the shared thread, but it
  prioritizes the mentioned member or members.
- Workers can contribute direct implementation progress or scoped answers, but
  planning and final synthesis still converge through the leader.

In practice:

- use `@member_id` when one specific worker should answer or inspect something
- avoid turning every status update into a direct mention
- keep final human-facing synthesis concise, even if the debug surfaces contain
  much richer detail

## Task-First Collaboration

Team Workbench is now task-first:

- `Kanban` is the durable coordination surface
- tasks can exist without an assigned member yet
- assignment can happen later as the plan becomes clearer
- step/run data remains execution telemetry, not the primary ownership model

This keeps context attached to the task instead of fragmenting it across many
small execution steps.

## What To Watch Operationally

- whether the Team runtime is started or stopped
- whether a member that already exited has fallen back to `stopped` / `degraded` after refresh
  instead of staying stuck on a stale `running` badge
- whether Kanban ownership matches the real executing member
- whether `Conversation` and `Kanban` stay in sync with the current plan
- whether permission review requests route to the expected reviewer

## Agent ACP In Team Mode

Use `Teams -> Agents -> Agent ACP` when you need to inspect one member deeply:

- ACP conversation and tool-call history
- detailed debugging
- member-specific prompt or runtime behavior

Use this sparingly during normal Team work. The primary human workflow should
still live in `Conversation` and `Kanban`.

## Permission Review In Teams

Permission review follows the Team routing model rather than a generic tool
prompt:

- worker-originated requests route to the leader
- leader-originated requests route to a subordinate worker
- nobody should review their own request
- if agent review does not complete in time, the shared Team surface falls back
  to a human-visible review card

## Managed Runtime Vs Local Development

Managed Team sessions already receive the role indexes and actor runtime
environment they need.

If you run Team actors manually in a local Codex environment outside the normal
managed runtime, bootstrap the Team skills first:

```bash
scripts/setup_team_skills.sh
```

Managed Team runtime actor commands are now auto-approved by the ACP
permission layer when `agenthub actor ...` resolves to the managed runtime
binary.

If you also run `agenthub actor ...` manually in a local Codex shell and want
those manual commands to stop prompting repeatedly, allow the human-typed
prefix:

```text
prefix_rule(pattern=["agenthub", "actor"], decision="allow")
```

Recommended location:

- `~/.codex/rules/default.rules`

## Related Pages

- [Feature Overview](../overview/feature-overview.md)
- [Agent Nodes and Remote Execution](../core/agent-nodes.md)
- [OpenAPI and Automation](./openapi-and-automation.md)
- [Troubleshooting](../operations/troubleshooting.md)
