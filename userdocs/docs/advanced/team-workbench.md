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

## What To Watch Operationally

- whether the Team runtime is started or stopped
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

## Managed Runtime Vs Local Development

Managed Team sessions already receive the role indexes and actor runtime
environment they need.

If you run Team actors manually in a local Codex environment outside the normal
managed runtime, bootstrap the Team skills first:

```bash
scripts/setup_team_skills.sh
```

If you want actor CLI actions to stop prompting repeatedly in local Codex
development, allow the canonical prefix:

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
