# Feature Docs Standard

This directory stores stable, domain-oriented technical specifications.
Chronological implementation records are stored in `docs/journal/`.

For repo-local writing rules, also use:

- `.agents/skills/agenthub-docs-spec/SKILL.md`
- `.agents/skills/agenthub-docs-journal/SKILL.md`

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
- Active specs in `docs/features/` may define normative contracts.
- Historical references in `docs/features/` must say so explicitly near the top and point to the
  active replacement specs.
- `docs/journal/`: date-prefixed implementation records (`YYYY-MM-DD-topic.md`).
- When a feature evolves, update the canonical feature doc and add/append a journal note.

## Compaction Policy

When multiple journal notes describe the same area:

1. extract stable conclusions into one feature spec in `docs/features/`;
2. keep detailed implementation timeline in `docs/journal/`;
3. update `docs/todo.md` to reference the canonical feature spec for ongoing validation;
4. avoid duplicating operational contracts across multiple feature specs.

When an older spec becomes historical:

1. keep one short historical reference page instead of a full parallel spec;
2. move any still-active contracts into the current canonical specs first;
3. add explicit links from the historical page to the active replacements;
4. do not let a historical page silently remain normative.

When older journals are superseded:

1. leave the chronology intact;
2. add a short note or follow-up pointer to the newer canonical spec/journal when helpful;
3. avoid editing new normative rules into old rollout journals.

## Current Canonical Specs

Categories are maintained in
[`agent-operating-workflows.md`](agent-operating-workflows.md#2-feature-spec-categories). Use them
when adding or compacting specs so this directory stays navigable.

- `docs/features/frontend-design.md`
- `docs/features/agent-nodes.md`
- `docs/features/acp-runtime.md`
- `docs/features/agents-teams.md`
- `docs/features/agent-runtime-profiles.md`
- `docs/features/team-channels-threads.md`
- `docs/features/team-create-flow.md`
- `docs/features/team-agent-adoption.md`
- `docs/features/team-execution-vocabulary.md`
- `docs/features/teams-collaboration-playbook.md`
- `docs/features/actor-foundation.md`
- `docs/features/team-mailbox-intake-and-ownership.md`
- `docs/features/team-conversation-event-bus.md`
- `docs/features/backend-runtime-logic.md`
- `docs/features/runtime-diagnostics.md`
- `docs/features/rara-direct-integration.md`
- `docs/features/distributed-node-architecture.md`
- `docs/features/distributed-node-registry-and-gossip.md`
- `docs/features/workspace-unified-ia.md`
- `docs/features/logical-message-metadata-contract.md`
- `docs/features/message-archive-lancedb.md`
- `docs/features/message-storage-tiering.md`
- `docs/features/agent-operating-workflows.md`
- `docs/features/team-system-prompt-contract.md`
- `docs/features/test-regression-guardrails.md`
- `docs/features/pyroscope-profiling.md`
- `docs/features/npm-binary-distribution.md`
- `docs/features/debian-systemd-distribution.md`
- `docs/features/app-linkers.md`
- `docs/features/slock-oauth-linkers.md`

## Historical References

- `docs/features/team-mcp-enforcement.md`
