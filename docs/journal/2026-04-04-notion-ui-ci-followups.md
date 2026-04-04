## Summary

- Aligned the current Notion-style UI branch with CI expectations after the first round of frontend refactors.
- Kept runtime-shell coverage focused on still-supported global CSS invariants instead of removed legacy ACP selectors.
- Restored Team mailbox compatibility hooks used by Playwright while preserving the newer Tailwind layout.
- Hid output header `Details` behind developer mode so debug-only metadata does not leak into the normal agent view.

## Changes

- Updated `tests/web_assets.rs` to validate the remaining shared CSS/runtime shell contract instead of requiring legacy `.acp-conversation` rules in `styles.css`.
- Updated `web/src/acp.ts` so `config_option_update` preserves existing selector state when `config_options` is absent and clears state only when the backend explicitly sends an empty array.
- Added ACP selector regression coverage in `web/src/acp.test.ts`.
- Updated `web/src/api.ts` so network jitter on idempotent browser requests uses bounded retry with backoff before surfacing `Failed to fetch`, while mutating `POST` flows keep their previous fail-fast behavior.
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

## 2026-04-04 follow-up

- Restored ACP markdown rendering for tool-call `content` blocks instead of forcing them through plain-text fallback.
- Broadened markdown detection for tool payload text to cover headings, lists, links, emphasis, and tables while still preserving ASCII-art-like payloads as plain text.
- Made the dock `History` affordance read as a dropdown selector by adding a count badge, chevron, and explicit menu semantics.
- Switched the shared `--notion-font` / `--notion-mono` stacks to Notion-like system fonts and removed the Google font import from legacy global CSS.
- Updated stale unit/E2E assertions and compatibility selectors used by Team page regressions:
  - `.teams-overview-meta`
  - `.teams-run-list-head`
  - `.actions`

## 2026-04-04 follow-up verification

- `cd web && npm run test -- src/acp_conversation.interaction.test.tsx src/acp_conversation_render.test.tsx src/acp_panel.test.tsx src/app.runtime_effects.test.tsx src/input_dock_render.test.tsx`
- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
- `make build-web`

## 2026-04-04 follow-up notes

- Chrome DevTools MCP transport was unavailable during this pass (`Transport closed`), so no browser-tree regression snapshot could be captured from MCP for these edits.
- Local Playwright still cannot be used as a final oracle on this machine because Chromium headless launch aborts with the existing macOS Mach-port permission failure (`bootstrap_check_in ... Permission denied (1100)`), so E2E-facing fixes were verified by code-path review plus the updated unit/build checks rather than a successful local browser run.

## 2026-04-04 review and CI cleanup

- Reverted `web/src/app.tsx` ACP config submission back to a zero-argument callback so `AcpDebug` no longer risks calling `.trim()` on `undefined` through the existing click-handler contract.
- Removed stale Team UI review leftovers:
  - dropped the unused `React` import from `web/src/pages/team_run_panel.tsx`
  - removed the unused `pageLimit` prop from `TeamRunPanel`
  - removed redundant local Tailwind aliases from `web/src/pages/team_tasks_panel.tsx`
  - deleted the unused `web/src/ui/floating_surfaces.ts` helper module
- Tightened the Team overview member meta row so long IDs no longer force `.teams-member-list` to overflow on desktop; the row now keeps only compact `model=` and `pending=` metadata with wrapping enabled.
- Updated `tests/web_assets.rs` to validate the current system-font global CSS contract instead of the removed Google Fonts import.
- Aligned `web/tests/e2e/input_dock_layout.e2e.ts` with the current dock behavior by treating the bottom gutter as a deliberate `20px` inset rather than a flush-to-viewport requirement.

## 2026-04-04 review and CI verification

- `cargo test styles_keep_runtime_shell_constraints --test web_assets -- --nocapture`
- `cd web && npm run test -- src/acp_conversation_render.test.tsx src/components/thread_rich_text.test.tsx src/input_dock_render.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`

## 2026-04-04 live verification note

- ACP markdown rendering was verified against the authenticated live AgentHub page through `agent-browser --auto-connect` because Chrome DevTools MCP remained unavailable. The rendered ACP message HTML now contains real markdown structures such as `<code>` and `<ul>` instead of the previous plain-text fallback.

## 2026-04-04 responsive dock follow-up

- Removed the remaining horizontal dock margin (`mx-4` / `sm:mx-6`) so the ACP input dock now stretches flush to the panel edges at every breakpoint instead of preserving a centered gutter.

## 2026-04-04 agents sidebar row follow-up

- Reflowed the Agents sidebar row so the first line keeps only the agent name, pending-permission dot, and action buttons.
- Moved model / node / status badges onto a dedicated second row to stop long names from being squeezed out by tags on both desktop and mobile widths.

## 2026-04-04 Tailwind theme token fix

- Added the missing Tailwind v4 `@theme` color tokens for the shared `notion-*` palette (`text`, `text-muted`, `border`, `hover`, `sidebar`, `accent`, `accent-bg`).
- This restores utility classes such as `bg-notion-accent`, `text-notion-text`, and `border-notion-border` so controls like the login button no longer degrade into transparent backgrounds with white text.

## 2026-04-04 floating surface primitives

- Reintroduced `web/src/ui/floating_surfaces.ts` as a real shared primitive instead of scattered one-off dropdown styling.
- Unified the following click-triggered surfaces onto the same Notion-style floating tokens:
  - Mantine `Menu` dropdowns in agents/team workbench chrome
  - ACP output `Details`
  - Input dock `History`
  - Team message read-state hover card
  - Mantine modals used by create-agent and permission review flows

## 2026-04-04 workbench chrome follow-up

- Removed the global Team detail header title so the selected team name no longer repeats above the workbench chrome.
- Slimmed the Team shared-channel composer shell and textarea density to keep the bottom tool area lighter.
- Raised `OutputHeader` status/details onto the same horizontal row as the agent title and promoted the `Details` panel to a higher floating z-layer.
- Anchored the ACP dock-local jump-to-bottom button to the dock shell (`bottom: calc(100% + 0.75rem)`) so it no longer sits under the dock when the input area is visible.
