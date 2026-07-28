# Team Agent Adoption Contract

## Summary

Added a canonical Team agent adoption spec that separates existing-agent reuse into two explicit
modes:

- `copy existing agent into Team`
- `move existing agent to Team`

The accepted rollout direction is `copy first, move later`.

## Background

Team onboarding had already been simplified around a coordinator-first create flow, but `Add Agent`
still implicitly assumed that the operator would always forge a brand-new Team-owned agent from
inside the Team flow.

That left a product gap:

- operators may already have useful agents in the global agent catalog
- reusing those agents inside a Team needs clearer semantics than a generic "import" action
- the product must distinguish a safe configuration clone from an ownership transfer

## Scope

- define the stable feature contract for Team adoption of existing agents
- separate `copy` and `move` semantics
- document the recommended rollout order
- add the new spec to the active feature index and TODO backlog references

## Key Decisions

- Team adoption is not a single mode; it must distinguish `copy` from `move`
- `copy` means creating a new Team-owned agent from a source agent template:
  - new agent id
  - source agent unchanged
  - no runtime/history continuity carried over
- `move` means transferring the original agent into Team ownership:
  - same agent id
  - ownership changes
  - runtime/history visibility rules need stricter guardrails
- first implementation slice should target `copy` only
- `move` should be specified now but deferred until stopped-only and ownership-transfer
  constraints are defined in code

## Validation

```bash
sed -n '1,260p' docs/features/team-agent-adoption.md
sed -n '1,220p' docs/journal/2026-05-03-team-agent-adoption-contract.md
sed -n '1,80p' docs/features/README.md
sed -n '1,20p' docs/todo.md
```

## Follow-Ups

- implement `Add Existing Agent` with `copy into Team` as the first delivery slice
- keep `move to Team` behind a later rollout that explicitly defines stopped-only and ownership
  transfer behavior
- add focused backend/web validation once the first `copy` implementation begins

## 2026-07-29 Extension Contract Follow-Up

The canonical spec now separates post-copy adoption extensions into explicit reviewable modes:

- `copy configuration and workspace content`
- `copy configuration and seed memory/context`
- `move stopped agent to Team`

Key boundary:

- default copy remains configuration-only
- workspace copy must create a new Team-owned workspace root or worktree, exclude runtime-local and
  credential-bearing state, and produce a copy manifest
- memory/context seeding must be provenance-marked, idempotent, Team-scoped, and detached from later
  source-agent memory changes
- move remains deferred and must be stopped-only with atomic ownership transfer

Validation:

```bash
sed -n '1,340p' docs/features/team-agent-adoption.md
sed -n '1,120p' docs/journal/2026-05-03-team-agent-adoption-contract.md
sed -n '20,34p' docs/todo.md
```

Remaining work:

- implement each extension mode behind typed API requests and focused backend/web tests
- collect browser evidence once the UI exposes these modes
