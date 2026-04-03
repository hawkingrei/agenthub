## Summary

- upgraded ACP debug session controls to consume provider-driven `config_options` instead of forcing raw text entry for model changes;
- wired Team member ACP detail to the existing agent ACP APIs for `set mode`, `set model`, and generic `set config`;
- kept the implementation provider-neutral so Codex, Gemini, and other ACP providers can surface their own mode/model choices.

## Details

- `web/src/acp.ts`
  - added `configOptions` parsing to `buildAcpView()`;
  - derived `currentMode` from ACP config options when an explicit `current_mode` event is absent;
  - normalized select option payloads so UI can consume ACP provider output without hardcoded model lists;
  - explicit `config_options: []` updates now clear stale selector state instead of preserving the previous provider options.
- `web/src/components/acp_debug.tsx`
  - session controls now render `select` inputs for `mode` and `model` when ACP exposes select options;
  - button handlers submit the currently selected option value directly;
  - fallback mode/model text inputs now display the same resolved value that the submit action uses.
- `web/src/app.tsx`
  - single-agent ACP debug now passes parsed `configOptions` into `AcpDebug`;
  - `set mode` / `set model` submit the selected option value instead of relying on uncontrolled text state.
- `web/src/pages/team_member_acp_panel.tsx`
  - Team member ACP debug now accepts `canControlAcp`, `onAcpSetMode`, `onAcpSetModel`, and `onAcpSetConfig`;
  - the Team panel now passes ACP config options through to `AcpDebug`;
  - ACP debug actions are now disabled whenever the corresponding Team member handler is unavailable, so the panel no longer renders enabled no-op buttons;
  - `Cancel Run` now also respects the Team panel's interruptibility state instead of enabling a no-op cancel action when no ACP run is actually interruptible.
- `web/src/pages/team_page.tsx`
  - wired Team member ACP actions to `/api/agents/:id/acp/mode`, `/api/agents/:id/acp/model`, and `/api/agents/:id/acp/config`.

## Validation

- `cd web && npm run test -- src/acp.test.ts src/acp_debug.test.tsx src/acp_debug.interaction.test.tsx src/pages/team_panels.test.tsx src/pages/team_member_acp_panel.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
