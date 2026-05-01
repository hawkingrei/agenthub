---
sidebar_position: 1
---

# Team Workbench

Team Workbench (`/teams`) is AgentHub's multi-agent collaboration surface.

## Mental Model

Think about Team Workbench as three connected layers:

- **Channels** (`# all` by default): the human-facing shared lane
- **Kanban**: the canonical task lane
- **Execution Runs and debug surfaces**: execution telemetry and deep inspection

On smaller screens, Team Workbench switches between two panes instead of stacking
everything into one page:

- the Team rail
- the active workspace

Use the header toggle to move between them.

The important distinction is that Team `Execution Runs` and `steps` are
execution and debug artifacts. They are not the primary planning surface.

## Recent Updates

Recent Team workbench updates changed the operator-facing shape in a few useful
ways:

- the shell is now more clearly `Channels`-first
- run history is consistently labeled `Execution Runs`
- the left rail carries most Team switching and workspace navigation
- the shared `# all` lane stays the default place for human goals and team-wide
  coordination

## Main UI Areas

- **Channels**:
  - shared channel lane, usually `# all`
  - `# all` is a stable Team-level channel target resolved by the backend, not whichever task
    happens to be visible first in the Kanban list
  - optimized for live coordination: the composer stays pinned to the bottom and the workbench
    shows a bounded recent-10 message window by default
  - human goals, constraints, approvals, and `@member_id` coordination requests
- **Kanban**:
  - task ownership and lifecycle
  - the durable place to see work status
  - not the place for human operators to manually author canonical Team tasks
- **Agents**:
  - member list and per-member entry into `Agent ACP`
- **Execution Runs**:
  - explicit execution history, start, and selection
- **Advanced / Debug**:
  - lower-level execution and inspection tools

## What Good Team Usage Looks Like

Healthy Team usage usually follows a stable split:

- humans talk in `Channels`, usually `# all`
- the coordinator turns agreed work into canonical tasks
- workers report progress and facts without taking over planning
- `Kanban` stays the source of truth for task state
- `Execution Runs` and `Agent ACP` stay available for debugging, not for day-to-day
  planning

If `Execution Runs` becomes the main place people track work, the Team model is
usually drifting too close to raw execution details.

## Typical Workflow

1. Create or select a Team.
2. Start the Team runtime.
3. Use `Channels`, usually `# all`, to state the goal and constraints.
4. Let the coordinator turn agreed work into canonical Kanban tasks.
5. Track ownership and progress in `Kanban`.
6. Drop into `Agent ACP`, `Execution Runs`, or `Advanced` only when you need
   execution details or debugging.

## Mentions And Shared Channels

- No `@mention`: message is team-wide and the coordinator should respond first.
- `@member_id`: message is still visible in the shared channel, but it
  prioritizes the mentioned member or members.
- Workers can contribute direct implementation progress or scoped answers, but
  planning and final synthesis still converge through the coordinator.

In practice:

- use `@member_id` when one specific worker should answer or inspect something
- avoid turning every status update into a direct mention
- keep final human-facing synthesis concise, even if the debug surfaces contain
  much richer detail

## Task-First Collaboration

Team Workbench is now task-first:

- `Kanban` is the durable coordination surface
- `Channels` (`# all`) provide a separate stable shared lane, not a workspace task you need to
  recover manually from Kanban ordering
- human requests and clarifications live in `Channels`; coordinator/runtime turns agreed work into
  canonical Kanban tasks
- the normal Team UI and public HTTP surface do not expose direct canonical task creation; requests
  go through `Channels` and coordinator/runtime materialize tasks onto `Kanban`
- tasks can exist without an assigned member yet
- assignment can happen later as the plan becomes clearer
- step/run data remains execution telemetry, not the primary ownership model

The workbench also refreshes more aggressively on resume:

- runtime state rechecks on focus / reconnect
- `Kanban` refreshes on resume instead of only waiting for the next poll
- shared `Channels` refresh immediately on restore/reconnect in addition to SSE

This keeps context attached to the task instead of fragmenting it across many
small execution steps.

## Shared Channel Connection Model

The Team header keeps the shared channel connection state compact instead of
turning it into a heavyweight status bar.

The shared channel is intentionally tuned for live coordination, not infinite
log browsing. The default Team surface keeps only a small recent window in
memory so the workbench stays responsive. If you need deeper execution history,
or raw event detail, drop into `Agent ACP`, `Execution Runs`, or other debug surfaces
instead of treating `# all` as the primary archive.

## What To Watch Operationally

- whether the Team runtime is started or stopped
- whether the Team runtime has correctly fallen back to `stopped` / `degraded` after members exit
- whether a member that already exited no longer shows a stale `running` badge after refresh
- whether Kanban ownership matches the real executing member
- whether `Channels` and `Kanban` stay in sync with the current plan
- whether permission review requests route to the expected reviewer

## Agent ACP In Team Mode

Use `Teams -> Agents -> Agent ACP` when you need to inspect one member deeply:

- ACP conversation and tool-call history
- detailed debugging
- member-specific prompt or runtime behavior
- session controls such as `Set Mode`, `Set Model`, `Set Config`, `Cancel Run`,
  and `Force New Session` when the active ACP runtime exposes them

Use this sparingly during normal Team work. The primary human workflow should
still live in `Channels` and `Kanban`.

When a provider returns structured selector options, AgentHub uses those
provider-driven choices instead of forcing you to type raw model or mode IDs by
hand.

## Permission Review In Teams

Permission review follows the Team routing model rather than a generic tool
prompt:

- the system automatically assigns a non-requester reviewer
- reviewers see the approval action in their Team workflow when they are the active reviewer
- the requester never self-reviews
- if agent review does not complete in time, the shared Team surface falls back
  to a human-visible review card
- resolved or timed-out review cards collapse to compact status cards so the
  shared channel keeps more space for active content

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
