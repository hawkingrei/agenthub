# 2026-03-16 Agents Codex ACP Auto Default Mode

## Summary

- Standalone `Agents` already had ACP tool-call rendering and permission polling wired.
- The practical gap for urgent MCP usage was session startup behavior: without an explicit
  `codex_acp.default_mode`, Codex ACP sessions could start in a preset that never surfaced
  permission prompts.
- The config fallback now defaults Codex ACP mode to `auto` when the setting is omitted or blank.

## Why This Change

The Agents workbench already renders ACP tool calls through the normal ACP event pipeline and
shows pending permissions through the scoped permission modal.

What blocked real usage was the startup default:

- `AgentManager` already applies `codex_acp.default_mode` after creating a Codex ACP session.
- `AppConfig::codex_acp_default_mode()` previously returned `None` when the config file did not
  set a value.
- In that case, new sessions relied on upstream defaults, which could bypass the permission gate
  entirely and make MCP permission UX appear broken from the user perspective.

## What Changed

- `crates/agenthub-config/src/lib.rs`
  - added built-in fallback `DEFAULT_CODEX_ACP_MODE = "auto"`;
  - `codex_acp_default_mode()` now returns configured non-blank values when present;
  - otherwise it returns `Some("auto")`.
- tests
  - added config tests covering:
    - unset config falls back to `auto`;
    - explicit configured value is preserved;
    - blank override still falls back to `auto`.

## Validation

- `cargo test -p agenthub-config`

## Notes

- This change is intentionally limited to Codex ACP because the runtime already applies
  `codex_acp.default_mode` only for the Codex provider.
- Explicit config still overrides the built-in fallback.
