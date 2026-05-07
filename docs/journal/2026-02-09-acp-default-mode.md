# ACP Default Mode

## Background
ACP permission UI does not appear when sessions run under a full-access approval preset. We need a consistent way to apply a non-full-access preset on ACP session start so permission requests surface in the UI.

## Scope
- Add `codex_acp.default_mode` to the config schema.
- Apply the configured mode immediately after ACP session creation.
- Log the configured mode at startup and warn if applying it fails.

## Key Decisions
- Use a simple string mode ID from configuration; no database migration required.
- Apply the mode for each ACP session start; on failure, keep the session running and log a warning.
- Users should set `default_mode` to a mode ID exposed by Codex ACP (e.g., via Debug -> Raw Events `current_mode` / config options).

## Configuration
```toml
[codex_acp]
default_mode = "auto"
```

`default_mode = "full-access"` enables the Codex full-access preset. For operator convenience,
AgentHub also normalizes the legacy or CLI-style aliases `yolo`, `yalo`,
`danger_full_access`, and `danger-full-access` to `full-access` before sending the ACP mode
change.

## 2026-05-08 Follow-Up

### Summary

Codex full-access mode can now be selected with either the canonical ACP mode ID
`full-access` or the common YOLO-style aliases. This applies to `codex_acp.default_mode`,
standalone ACP Debug mode changes, and Team member ACP Debug mode changes through the
backend ACP mode API boundary.

### Validation

```bash
cargo fmt --check
cargo test -p agenthub-config codex_acp_default_mode -- --nocapture
npm --prefix web run test -- src/acp_debug.interaction.test.tsx src/pages/team_member_acp_panel.test.tsx
cargo check -p agenthub
git diff --check
```

## Validation
- Restart agenthub after updating `~/.agenthub/config.toml`.
- Start a new ACP agent session.
- Confirm Debug -> Raw Events shows `current_mode` reflecting the configured mode.
- Trigger a tool permission (or `/permission-demo`) and verify the permission entry appears in Debug and the permission modal.
