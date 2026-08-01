# Team Adoption Extension Contract

## Summary

The Team adoption contract now defines post-copy-first extension modes without enabling them in the
runtime or UI. Stopped-only move, workspace-content copy, and memory/context seeding are separate
reviewable modes with distinct ownership, runtime, history, and provenance guardrails.

## Background

The copy-first rollout intentionally made `Copy into Team` the only executable adoption path. Later
work was left open for `Move to Team`, workspace-content copy, and memory/context seeding. Those
follow-ups needed a stable design boundary before implementation so that future runtime work does
not silently expand default copy semantics.

## Scope

This checkpoint covers design only:

- stopped-only `Move existing agent to Team`
- opt-in workspace-content copy
- opt-in memory/context seeding
- validation expectations for those later implementation slices

It does not enable new adoption actions, mutate agent ownership, copy filesystem content, or seed
memory/context at runtime.

## Key Decisions

- `Move existing agent to Team` must transfer the original identity and start with a stopped-only
  rollout.
- Move must reject active runtime sessions, active Team membership, pending permission reviews, and
  running task ownership dependencies before ownership transfer.
- Workspace-content copy is an opt-in extension to configuration copy. It creates a new Team-owned
  identity and must define destination, collision, ignored-artifact, and provenance behavior.
- Memory/context seeding is separate from workspace-content copy. It should snapshot explicitly
  eligible context sources, filter or reject secrets and machine-local cache state, and never share
  mutable source-agent memory with the copied member.
- Default `Copy into Team` remains configuration-only.

## Validation

Focused checks for this documentation slice:

```bash
git diff --check
```

Future implementation validation should add focused backend and web tests for each enabled mode.

## Follow-Ups

- Implement stopped-only move only after adding runtime dependency checks and ownership transfer
  tests.
- Implement workspace-content copy only after destination, collision, ignored-artifact, and
  provenance behavior are accepted.
- Implement memory/context seeding only after eligible sources and secret/cache filtering rules are
  accepted.
