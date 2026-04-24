# Release Prebuild Post-Merge Only

## Summary

Moved the `Release Prebuild` workflow out of the pull request path so release-target packaging no longer runs on every PR.

## What Changed

- Updated `.github/workflows/release-prebuild.yml`
- Removed the `pull_request` trigger
- Kept the `push` trigger on `main`

## Rationale

- PR validation should focus on correctness, unit/integration coverage, and user-facing regressions.
- Release packaging is slower and more expensive than normal PR feedback loops.
- Running prebuild only after merge keeps post-merge release confidence without making every PR pay the release matrix cost.

## Validation

- Reviewed workflow triggers locally after the change.
- No functional code path changed; this is a GitHub Actions trigger-only adjustment.

## Follow-Up

- Confirm after merge that `Release Prebuild` still starts from `push` to `main`.
- If needed later, consider keeping `workflow_dispatch` for ad-hoc release prebuild verification without restoring PR execution.
