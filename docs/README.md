# Documentation Guide

This folder contains engineering-facing documentation for AgentHub
contributors.

## Structure

- `docs/features/`: stable domain-oriented engineering specs
- `docs/journal/`: dated implementation checkpoints and compaction notes
- `docs/todo.md`: active follow-up backlog only
- `docs/api_naming.md`: payload naming conventions for AgentHub-owned APIs
- `docs/developer-setup.md`: contributor setup, repository layout, and common commands
- `userdocs/`: published end-user documentation site (Docusaurus)

## Topic Map

Use these docs instead of expanding `AGENTS.md` with detailed workflow rules:

- Team product/runtime model:
  - `docs/features/agents-teams.md`
  - `docs/features/actor-foundation.md`
  - `docs/features/acp-runtime.md`
- Team channels, conversation, and thread behavior:
  - `docs/features/team-channels-threads.md`
  - `docs/features/team-conversation-event-bus.md`
- Team workspace memory and context boundaries:
  - `docs/features/team-workspace-memory-contract.md`
- Frontend and workspace-shell design:
  - `docs/features/frontend-design.md`
  - `docs/features/workspace-unified-ia.md`
- CI / validation follow-up rules:
  - `docs/todo.md`
  - related entries in `docs/journal/`

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

## Compaction Rules

- `docs/features/` holds stable domain docs only; do not turn it into a
  chronological changelog.
- `docs/journal/` holds dated implementation records (`YYYY-MM-DD-topic.md`).
- When a feature evolves, update the canonical feature doc and add or append a
  journal note.
- When documentation-only journals stop carrying distinct decisions, merge them
  into a background journal and remove the stale micro-journals.

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
