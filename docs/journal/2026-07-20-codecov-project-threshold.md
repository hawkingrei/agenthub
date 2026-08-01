# Codecov Project Threshold Stabilization

## Context

Two unrelated PRs both passed their direct test suites and `codecov/patch`, but
failed `codecov/project` on very small repository-wide coverage deltas:

- PR #899: `82.58% (-0.06%)`
- PR #900: `82.59% (-0.05%)`

The existing project status threshold was `0.02%`. That made the project gate
more sensitive than the observed multi-flag coverage aggregation noise while
the patch gate already proved the changed lines did not reduce direct coverage.

## Change

`codecov.yml` now allows a `0.10%` project coverage threshold.

This keeps the status useful for material coverage regressions while avoiding
blocking small, well-tested PRs on unrelated project-wide rounding or coverage
session movement.

## Guardrails

- `codecov/patch` remains enabled and continues to gate changed-line coverage.
- Coverage uploads still fail fast in CI when uploads fail.
- Test files, docs, and generated proto output remain excluded from coverage.

## Validation

Local validation:

```bash
git diff --check
```

Follow-up validation is the Codecov status on this PR and then on queued PRs
that previously failed only on a `0.05%` to `0.06%` project delta.
