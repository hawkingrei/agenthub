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

## Single-Node Bootstrap

Before starting Team runs on a single node, bootstrap Team role skills into
your local `skills.json`:

```bash
scripts/setup_team_skills.sh
```

This command copies Team skill files into:

- `~/.agenthub/worktrees/team-skills`

and appends those paths into:

- `~/.agenthub/skills.json`

Because `~/.agenthub/worktrees` is part of default `safe_paths`, these skill
paths are accepted by ACP skill loading without extra config.

If you explicitly want repository paths instead of copied files, use:

```bash
scripts/setup_team_skills.sh --use-repo-skill-paths
```

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

## Codex Rule For Actor CLI

If you run Team actors through a local Codex environment, allow the canonical
actor CLI prefix so mailbox coordination does not pause on repeated approval
prompts.

Append this rule to your local Codex rules file:

```text
prefix_rule(pattern=["agenthub", "actor"], decision="allow")
```

Recommended location:

- `~/.codex/rules/default.rules`

After that, the canonical Team mailbox flow stays short and predictable:

```bash
agenthub actor inbox
agenthub actor ack --message-id <id>
agenthub actor send --to-actor-id <actor_id> --text "<markdown>"
```

## Related Pages

- [OpenAPI and Automation](./openapi-and-automation.md)
- [Session Lifecycle](../core/session-lifecycle.md)
- [Troubleshooting](../operations/troubleshooting.md)
