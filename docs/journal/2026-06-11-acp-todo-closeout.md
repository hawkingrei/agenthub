# ACP TODO Closeout

## Summary

Closed the remaining ACP-specific active TODO matrix after the generic Codex adapter rollout landed
on `main`. The active backlog now focuses on release packaging, Team workspace follow-ups, Team
runtime verification, message storage, and docs compaction.

## Background

The ACP backlog had accumulated verification tails for long-session browser behavior,
provider-driven selectors, and Codex native input polish. After PR #750 completed the generic
`agenthub-acp codex` rollout and the operator confirmed the ACP TODO line is complete, keeping the
ACP matrix in `docs/todo.md` would make the active backlog stale.

## Scope

- Remove the ACP Long-Session Matrix section from `docs/todo.md`.
- Keep the stable ACP contracts in `docs/features/` unchanged.
- Do not change ACP runtime behavior, provider detection, or web UI code in this closeout.

## Key Decisions

- Treat this as backlog cleanup rather than a new ACP implementation slice.
- Preserve ACP stable contracts in feature specs so future provider work can still reference them.
- Leave any new ACP provider or UX work to future explicit TODO entries instead of carrying broad
  historical verification text forward.

## Validation

Evidence checked before the closeout:

```bash
gh pr view 750 --json url,state,mergedAt,mergeCommit,title
gh pr checks 750 | cat
```

Observed PR #750 merged on 2026-06-11 with merge commit
`998525c412b2599623eafb20170bc08c19ae4a7c`. Bazel, Rust, Web, Web E2E, Web E2E Mobile, User Docs,
Distributed P2P Pipeline, and project coverage checks reported passing after the merge.

## Follow-Ups

None for the removed ACP TODO matrix. New ACP work should be added as a narrower active item only
when there is a fresh implementation or verification target.
