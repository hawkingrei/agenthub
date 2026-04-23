## Summary

Aligned user-facing documentation with the merged Rust `1.95.0` and Codex
`0.121.x` repository baseline.

## Updated Pages

- `userdocs/docs/intro.md`
- `userdocs/docs/getting-started/installation.md`
- `userdocs/docs/getting-started/configuration-basics.md`
- `userdocs/docs/operations/troubleshooting.md`
- `userdocs/docs/overview/feature-overview.md`
- `userdocs/docs/overview/architecture-overview.md`
- `userdocs/docs/advanced/team-workbench.md`

## What Changed

- Made the validated local build baseline explicit in installation docs:
  - Rust `1.95.0`
  - default ACP adapter `agenthub-codex-acp`
  - official Codex `0.121.x`
- Clarified that custom `codex_acp.binary` overrides should stay protocol-
  compatible with the same ACP surface.
- Added a focused troubleshooting section for stale or incompatible ACP adapter
  binaries that start and exit immediately.
- Refreshed Team/user-facing feature descriptions to match recent product
  changes:
  - channel-first Team shell language
  - explicit `Execution Runs` terminology
  - token-based Agent Node onboarding

## Validation

Documentation-only change. No code or config behavior was modified.
