# Dependabot Cadence And Grouping Relaxation

## Background

The existing Dependabot configuration on `main` was valid, but it was intentionally conservative:

- weekly cadence only
- one catch-all grouped PR per ecosystem
- low visibility unless a large update batch accumulated

This made Dependabot look inactive in day-to-day usage even though it had already produced at least one PR:

- `#123` `build(deps): bump the npm_and_yarn group across 2 directories with 5 updates`

## Changes

- Switched Dependabot schedules from weekly to daily for all configured ecosystems.
- Removed catch-all `groups` blocks so updates are no longer collapsed into a single broad PR per ecosystem.
- Removed explicit `open-pull-requests-limit` caps from the config and let Dependabot use its normal behavior.
- Expanded the YAML config to plain mappings without anchors/aliases because Dependabot rejects YAML aliases during config parsing.

## Why

The previous configuration optimized for low noise, but it also hid activity:

- grouped PRs reduced visibility into which dependency changed
- weekly cadence meant long stretches with no visible PR activity
- broad grouped PRs made review scope larger than necessary

Daily ungroupped updates make the automation easier to observe and easier to review incrementally.

## Validation

Observed from GitHub:

- `.github/dependabot.yml` is present on `main`
- historical Dependabot PR evidence exists (`#123`)
- Dependabot alerts are active for the repository

Follow-up verification after merge:

- confirm daily PR generation for `github-actions`, `cargo`, `web`, and `userdocs`
- record the next successful PR links/run evidence in `docs/todo.md` / PR notes
