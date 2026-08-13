# User Documentation Release Readiness

## Summary

The README and published user documentation now describe the current product and release behavior
instead of mixing intended features, developer workflows, and stale endpoint examples. The refresh
covers installation, first-run onboarding, agents, worktrees, replay and recovery, OpenAPI,
notifications, Agent Nodes, security, deployment, operations, and storage boundaries.

## Background

The existing site had accumulated implementation drift across several user-critical paths. Examples
included obsolete authentication and push routes, environment overrides that the server actually
ignores, monitoring and orchestration contracts that are not implemented, unsafe worktree cleanup,
an incomplete backup set, and installation channels whose published artifacts no longer matched the
guide.

A live distribution audit also found that GitHub release archives and Debian packages publish the
current `agenthub` plus `agenthub-acp` pair, npm publishes only `agenthub`, and the Homebrew formula
still points to `v0.0.7` with the legacy `agenthub-codex-acp` helper. The Linux glibc compatibility
floor remains open and must not be inferred from the workflow runner label.

## Scope

- Make Debian packages and paired GitHub archives the complete-runtime installation paths.
- Document npm's main-binary-only boundary and mark Homebrew as lagging until it reaches release and
  adapter parity.
- Align first-run root setup, agent creation, status, worktree, replay, and recovery guidance with
  the current UI and backend routes.
- Separate REST history from SSE live delivery and describe the bounded reconnect behavior.
- Describe the incremental OpenAPI surface without implying that every HTTP route is published.
- Correct role, passkey, notification, Agent Node, safe-path, and internal gRPC security boundaries.
- Document the complete SQLite, event database, archive, message-body, object, and VAPID backup set.
- Keep future monitoring, webhook, Kubernetes, and high-availability work out of current user
  procedures.

## Key Decisions

- User documentation follows shipped behavior and exact artifact shape, not intended channel shape.
- `agenthub` and `agenthub-acp` should come from the same release whenever installed separately.
- The public OpenAPI document is an incremental automation contract, not a mirror of every internal
  UI route.
- `safe_paths` is an input-validation guardrail, not an operating-system sandbox.
- Agent event replay uses persisted per-agent SQLite databases; SSE is the bounded live transport.
- Official release artifacts include the OpenDAL S3 backend, while default source builds remain
  feature-gated and runtime storage remains `fs` until explicitly configured.
- Shipping S3 support is distinct from certifying every S3-compatible provider.
- Normal server settings come from `config.toml`; detected `AGENTHUB_*` config overrides are logged
  as ignored.
- Backup and restore procedures operate on a consistent complete data set rather than individual
  database or storage files.

## Validation

```bash
npm --prefix userdocs ci
npm --prefix userdocs run build
git diff --check
rg -n "/api/(login|register|admin/push|agents/stream|events/stream)" userdocs/docs README.md
gh release view v0.0.11 --repo hawkingrei/agenthub --json assets,tagName,url
npm view @linkerdog/agenthub version dist-tags bin optionalDependencies engines --json
gh api repos/linkerdog/homebrew-tap/contents/Formula/agenthub.rb
```

## Follow-Ups

- Restore Homebrew release and adapter parity before advertising it as a current complete install.
- Freeze and enforce the minimum Linux glibc baseline, then replace the compatibility caution with a
  tested support matrix.
- Keep deployed-browser behavior and public API examples in the release verification loop so the
  user guide does not drift back toward implementation intent.
