# CI Pipeline Split

## Background

The combined CI workflow grew large and masked failures across unrelated areas.

## Scope

- Split CI into separate Rust, Web, and Web E2E workflows.

## Key Decisions

- Keep triggers aligned (`push` to main and `pull_request`).
- Preserve existing steps per area.

## Validation

- Confirm GitHub Actions shows three independent workflows.
