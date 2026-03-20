## Summary

- Fixed a Team detail white-page regression caused by referencing conversation SSE badge inputs before their `useState` / `useMemo` declarations in `web/src/pages/team_page.tsx`.
- Added a frontend lint guard so future TypeScript/React TDZ regressions are rejected before bundle build.

## Details

- The Team detail route blanked with `ReferenceError: Cannot access 'selectedTeamId' before initialization` after the Team channel SSE badge selectors were inserted above `selectedTeamId` / `selectedConversation`.
- `web/src/pages/team_page.tsx` now computes the conversation stream badge only after `selectedTeamId`, `selectedConversation`, and the Team UI state selectors are initialized.
- `web/eslint.config.js` now enables `@typescript-eslint/no-use-before-define` for TS/TSX with variable/class/enums checking enabled and function hoists allowed.
- Existing `web/src/pages/team_page.smoke.test.tsx` remains the route-level regression guard for both selector and detail renders.

## Validation

- `cd web && npx vitest run src/pages/team_page.smoke.test.tsx`
- `cd web && npm run lint -- src/pages/team_page.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run build`
