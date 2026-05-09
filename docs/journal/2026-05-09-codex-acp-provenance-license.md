# Codex ACP Provenance License Audit

## Summary

Closed the pre-release `agenthub-codex-acp` provenance and license metadata
audit by making the adapter's Apache-2.0 metadata, local license file, upstream
notice attribution, and MIT-derived third-party notice text explicit.

## Background

The repository-level Apache-2.0 rollout left an explicit follow-up for
`agenthub-codex-acp`: the adapter originally came from a Zed Codex ACP baseline,
now integrates with official OpenAI Codex Rust crates, and contains at least one
file that is explicitly adapted from OpenAI Codex. The adapter also inherits the
OpenAI Codex notice that Codex includes code derived from Ratatui under MIT.

## Scope

- Kept `agenthub-codex-acp/Cargo.toml` on `license = "Apache-2.0"` because the
  adapter code and current OpenAI Codex dependency baseline are Apache-2.0.
- Normalized `agenthub-codex-acp/LICENSE` to the standard Apache License 2.0
  text instead of a stale Zed-only copyright preamble.
- Added `agenthub-codex-acp/NOTICE` for AgentHub, OpenAI Codex, and the earlier
  Zed Codex ACP implementation provenance.
- Added `agenthub-codex-acp/THIRD_PARTY_NOTICES.md` with the preserved Ratatui
  MIT copyright and permission notice text.
- Updated the adapter README to point readers at the local notice files.

## Key Decisions

- The package license remains `Apache-2.0`; the adapter does not need a mixed
  SPDX expression because the MIT-derived material is recorded as third-party
  notice provenance rather than relicensing the adapter.
- Zed attribution is kept in `NOTICE` because early adapter history and the old
  subdirectory license point to Zed's Codex ACP implementation.
- OpenAI Codex attribution is kept in `NOTICE` because the current adapter
  integrates with official OpenAI Codex Rust crates and local adapter code
  explicitly records OpenAI Codex adaptation provenance.
- Ratatui's MIT permission text is preserved in a dedicated third-party notice
  file so the pre-release audit is not relying only on a short upstream summary.

## Validation

```bash
rg -n "Zed|OpenAI|Ratatui|MIT|Apache-2.0|THIRD_PARTY_NOTICES" agenthub-codex-acp docs/todo.md docs/journal/2026-05-09-codex-acp-provenance-license.md
cargo package -p agenthub-codex-acp --list --allow-dirty
git diff --check
```

## Follow-Ups

- Re-check PR CI after the documentation-only change lands.
- If future adapter code copies additional upstream source files directly, add
  file-level provenance comments or extend `THIRD_PARTY_NOTICES.md` in the same
  PR.
