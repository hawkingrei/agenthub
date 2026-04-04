## Summary

- Aligned the current Notion-style UI branch with CI expectations after the first round of frontend refactors.
- Kept runtime-shell coverage focused on still-supported global CSS invariants instead of removed legacy ACP selectors.
- Restored Team mailbox compatibility hooks used by Playwright while preserving the newer Tailwind layout.
- Hid output header `Details` behind developer mode so debug-only metadata does not leak into the normal agent view.

## Changes

- Updated `tests/web_assets.rs` to validate the remaining shared CSS/runtime shell contract instead of requiring legacy `.acp-conversation` rules in `styles.css`.
- Updated `web/src/acp.ts` so `config_option_update` preserves existing selector state when `config_options` is absent and clears state only when the backend explicitly sends an empty array.
- Added ACP selector regression coverage in `web/src/acp.test.ts`.
- Restored Team mailbox compatibility affordances in `web/src/pages/team_mailbox_panel.tsx`:
  - `.teams-chat-head`
  - `.teams-member-unread`
  - `auto_follow=on/off`
- Added matching assertions in `web/src/pages/team_panels.test.tsx`.
- Removed the duplicate Google Fonts import from `web/src/tailwind.css`.
- Restricted `web/src/components/output_header.tsx` developer metadata details to developer mode only and updated `web/src/output_header.test.tsx`.

## Verification

- `cd web && npm run lint`
- `cd web && npm run test -- src/output_header.test.tsx src/pages/team_panels.test.tsx src/acp.test.ts src/pages/team_page.smoke.test.tsx`
- `cargo test styles_keep_runtime_shell_constraints --test web_assets -- --nocapture`
- `cd web && npm run build`
- `make build-web`

## Notes

- Local Playwright reproduction for the remaining CI-style browser checks was blocked by the configured dev-server startup path because port `5173` was already occupied in the local environment.
- Chrome DevTools MCP regression checks were performed against the local shell (`http://127.0.0.1:4175/`) and the live Team page. The local shell loaded correctly with the expected backend-less JSON parse warning, and the live Team page still showed the pre-existing `ERR_HTTP2_PROTOCOL_ERROR` / `404` noise but no new frontend exception attributable to this change set.
