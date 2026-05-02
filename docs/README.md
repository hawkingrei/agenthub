# Documentation Guide

This folder contains the engineering-facing documentation for AgentHub.

Use it as the contributor entrypoint after the repository `README.md`. The
root `README.md` is product-facing; this directory is architecture- and
implementation-facing.

## Start Here

- Contributor setup and local workflow:
  - [developer-setup.md](developer-setup.md)
- Architecture index:
  - [architecture-map.md](architecture-map.md)
- Active backlog:
  - [todo.md](todo.md)
- Feature spec standard:
  - [features/README.md](features/README.md)

## Repo-Local Documentation Skills

- Journal writing and review:
  - `.agents/skills/agenthub-docs-journal/SKILL.md`
- Canonical feature spec writing and review:
  - `.agents/skills/agenthub-docs-spec/SKILL.md`

## Documentation Surfaces

- `docs/features/`
  - stable domain-oriented engineering specifications
- `docs/journal/`
  - dated implementation checkpoints and compaction notes
- `docs/todo.md`
  - active follow-up backlog only
- `docs/api_naming.md`
  - payload naming conventions for AgentHub-owned APIs
- `docs/developer-setup.md`
  - contributor setup, repository layout, and common commands
- `userdocs/`
  - published end-user documentation site (Docusaurus)

## Architecture Topic Map

Use these docs instead of expanding [AGENTS.md](../AGENTS.md) with detailed
workflow or runtime rules.

- Product and runtime model:
  - [features/agents-teams.md](features/agents-teams.md)
  - [features/backend-runtime-logic.md](features/backend-runtime-logic.md)
  - [features/actor-foundation.md](features/actor-foundation.md)
- ACP and runtime rendering:
  - [features/acp-runtime.md](features/acp-runtime.md)
- Team channels, conversation, and threads:
  - [features/team-channels-threads.md](features/team-channels-threads.md)
  - [features/team-conversation-event-bus.md](features/team-conversation-event-bus.md)
- Team workspace memory and continuity:
  - [features/team-workspace-memory-contract.md](features/team-workspace-memory-contract.md)
- Frontend and workspace shell:
  - [features/frontend-design.md](features/frontend-design.md)
  - [features/workspace-unified-ia.md](features/workspace-unified-ia.md)
- Nodes and distributed execution:
  - [features/agent-nodes.md](features/agent-nodes.md)
  - [features/distributed-node-architecture.md](features/distributed-node-architecture.md)
- CI and active verification:
  - [todo.md](todo.md)
  - related entries in [journal/](journal/)

## When You Change Code

Use this checklist for every non-trivial change:

1. Update the canonical feature spec in `docs/features/` when contracts or
   behavior changed.
   - For Agents / ACP / Team UI behavior, this usually means
     `docs/features/frontend-design.md`.
2. Add or append a dated journal in `docs/journal/` for the implementation
   checkpoint.
3. Add a follow-up item in `docs/todo.md` only when open work remains.
4. If API payloads changed, ensure naming still conforms to
   `docs/api_naming.md`.
5. If behavior is user-visible, update `userdocs/docs/` accordingly.
   - Team workbench changes should usually update
     `userdocs/docs/advanced/team-workbench.md`.

## Journal Convention

- Filename: `YYYY-MM-DD-topic.md`
- Keep notes concise and operational:
  - Summary
  - Background
  - Scope
  - Key decisions
  - Validation
  - Follow-ups
- Prefer the repo-local journal skill for structure and compaction rules:
  - `.agents/skills/agenthub-docs-journal/SKILL.md`

## Compaction Rules

- `docs/features/` holds stable domain docs only; do not turn it into a
  chronological changelog.
- `docs/journal/` holds dated implementation records (`YYYY-MM-DD-topic.md`).
- When a feature evolves, update the canonical feature doc and add or append a
  journal note.
- When documentation-only journals stop carrying distinct decisions, merge them
  into a background journal and remove the stale micro-journals.
- Prefer the repo-local feature spec skill when extracting stable conclusions:
  - `.agents/skills/agenthub-docs-spec/SKILL.md`

## TODO Hygiene

- Keep `docs/todo.md` as an active backlog, not a historical ledger.
- Remove completed items after evidence is captured in a journal, PR, or
  canonical feature spec.
- Prefer one umbrella rollout item over many duplicated verification bullets
  for the same surface.

## Working With User Docs

`userdocs/` is the published user documentation site.

```bash
npm --prefix userdocs ci
npm --prefix userdocs run start
npm --prefix userdocs run build
```

Build artifacts are generated at `userdocs/build/`.
