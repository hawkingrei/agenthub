---
title: Root Apache-2.0 License Adoption
date: 2026-03-22
status: implemented
---

## Summary

Adopt Apache License 2.0 at the repository root so the main AgentHub workspace
aligns with the requested Calcite-style licensing baseline.

## Scope

- add a root `LICENSE` file with the standard Apache License 2.0 text
- update `README.md` to reflect `Apache-2.0`
- add `license = "Apache-2.0"` to the root crate and workspace-owned crates
- record a compliance follow-up for `agenthub-codex-acp`

## Compliance Notes

- The root repository now declares Apache-2.0 as its primary license.
- This change intentionally does not rewrite the existing
  `agenthub-codex-acp/` local license/provenance files.
- If `agenthub-codex-acp` contains MIT-derived code, the required upstream MIT
  notice must remain preserved for that subcomponent even after the root
  repository adopts Apache-2.0. That audit remains an explicit follow-up item
  in `docs/todo.md`.

## Validation

- reviewed the requested Calcite `LICENSE` file and aligned on Apache-2.0 as
  the repository-level license while excluding Calcite-specific third-party
  appendix content that does not apply to AgentHub
- verified the repository root previously lacked a root `LICENSE`
- updated Rust package metadata so tooling can resolve the same license value
