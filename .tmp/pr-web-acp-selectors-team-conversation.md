## Summary

- upgrade ACP debug controls to consume provider-driven `config_options` for mode/model selection instead of relying on raw text entry;
- wire Team member ACP detail to the existing agent ACP `set mode` / `set model` / `set config` APIs;
- align Team shared-channel conversation browsing with ACP conversation by sharing the same tail window and near-bottom stick-to-bottom behavior.

## Why

- Team member ACP detail already exposed mode/model UI state, but the actions were still `NOOP`;
- model switching should come from ACP provider options instead of manual user input so non-Codex providers such as Gemini can surface their own choices;
- Team shared-channel conversation was using a much smaller tail window and a more aggressive bottom-stick threshold than ACP conversation, so browsing behavior drifted between the two views.

## Changes

- parse ACP `config_options` into `AcpView` and derive `currentMode` from provider config when explicit mode events are absent;
- render `select` controls for ACP mode/model when provider options are available, while keeping text fallback for runtimes that do not expose options;
- hook Team member ACP detail to the existing agent ACP APIs so session controls are no longer display-only;
- export a shared conversation tail-window constant and reuse it in both ACP conversation and Team channel conversation;
- remove the Team-only `24px` bottom-stick threshold override so Team channel uses the same near-bottom behavior as ACP conversation;
- add/update focused tests and journals for the ACP selector path and the Team conversation alignment.

## Validation

- `cd web && npm run test -- src/acp.test.ts src/acp_debug.test.tsx src/acp_debug.interaction.test.tsx src/pages/team_member_acp_panel.test.tsx`
- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`

## MCP Verification

- baseline: checked live Team workbench and `# all` channel on `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` before the conversation-alignment changes;
- regression: reloaded local `http://127.0.0.1:4175/` after the edits and verified the shell loaded without new frontend errors;
- console summary:
  - local dev shell only showed expected Vite logs and the pre-existing backend-less `404`;
  - live Team page did not show new console errors from this change;
- note: the live Team member I checked did not have an active ACP session at the time, so provider-driven mode/model selectors could not be visually confirmed there; a TODO item was added for real Gemini/Kimi runtime verification.
