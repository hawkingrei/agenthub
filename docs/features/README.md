# Feature Docs Standard

This directory stores stable, domain-oriented technical specifications.
Chronological implementation records are stored in `docs/journal/`.

## Goal

Keep a small set of durable feature docs as the source of truth for:

- architecture boundaries
- runtime/data contracts
- UI/UX interaction contracts
- operational constraints and failure handling
- validation matrix and open risks

## Required Structure

Each active feature spec should include:

- `Problem`
- `Scope`
- `Non-Goals`
- `Architecture`
- `Contracts`
- `Validation Matrix`
- `Operational Notes`
- `Open Risks`
- `Source Journals` (links to journal records)

## File Policy

- `docs/features/`: theme/domain docs only (no date-prefixed changelog notes).
- `docs/journal/`: date-prefixed implementation records (`YYYY-MM-DD-topic.md`).
- When a feature evolves, update the canonical feature doc and add/append a journal note.

## Compaction Policy

When multiple journal notes describe the same area:

1. extract stable conclusions into one feature spec in `docs/features/`;
2. keep detailed implementation timeline in `docs/journal/`;
3. update `docs/todo.md` to reference the canonical feature spec for ongoing validation;
4. avoid duplicating operational contracts across multiple feature specs.

## Current Canonical Specs

- `docs/features/frontend-design.md`
- `docs/features/agents-teams.md`
- `docs/features/teams-collaboration-playbook.md`
- `docs/features/actor-foundation.md`
- `docs/features/team-mcp-enforcement.md`
- `docs/features/team-conversation-event-bus.md`
- `docs/features/backend-runtime-logic.md`
