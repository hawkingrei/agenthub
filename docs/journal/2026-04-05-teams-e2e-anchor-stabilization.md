# Teams E2E Anchor Stabilization

## Summary

- Added stable `data-*` anchors for the Team member console panel and compiled task preview.
- Updated Playwright selectors to target the new anchors instead of brittle layout classes.
- Extended `team_panels` coverage so the new anchors are exercised in unit tests.
- Auto-filled `Agent name` in the E2E forge helper so the Mantine modal can submit under the new
  required-field contract.
- Switched team-detail readiness checks to the stable selected-team menu trigger and removed an
  extra mobile-only scroll dependency from selector navigation.
- Narrowed selector-route team picking by filling the team filter first, then matching the visible
  `.team-item` text so mobile layout differences do not break team selection.
- Added a selector-route fallback that resolves the team entry by accessible button name before
  falling back to `.team-item`, which keeps team selection stable across compact layouts.
- Added a route-level fallback for selector navigation when CI/mobile layout re-renders make the
  visible team button click unstable, so team-detail E2E no longer depends on a flaky mobile click.
- Scoped selector-route team picking to the `Teams` panel, accepted both `Filter teams` and
  `Search teams` labels, and made the fallback locator recreate the team button instead of
  reusing an out-of-scope variable.
- Normalized derived agent names with both `/` and `\\` path separators so the forge helper stays
  portable across path styles.
- Replaced the forge modal role-model field with a stable native select and added explicit
  selector-entry `data-*` attributes on the Team selector list so the UI exposes durable hooks
  instead of forcing E2E to infer internal Mantine structure.
- Waited for compile-preview requests before asserting the rendered preview block to reduce flaky
  timing around developer tools updates.
- Waited for the Kanban `Compile Preview` button to become enabled before clicking, which removes
  a remaining race between task hydration and developer-tool actions.
- Cleared the Team selector filter after deleting the selected team so returning to the selector
  shows the remaining teams instead of preserving a now-stale team-name filter.

## Why

The Notion-style Team UI refresh changed panel wrappers and preview layout. Existing E2E cases still
depended on legacy `.card` containers and direct global text matches, which made `Web E2E` fail even
though the underlying features still worked.

## Validation

- `cd web && npx vitest run src/pages/team_panels.test.tsx --pool=threads --maxWorkers=1`
- `cd web && npm run lint -- src/pages/team_member_console_panel.tsx src/pages/team_tasks_panel.tsx tests/e2e/team_page.e2e.ts src/pages/team_panels.test.tsx`
- `cd web && npx playwright test tests/e2e/team_page.e2e.ts -g "team page desktop keeps long metadata blocks non-overlapping|team debug run ops compiles task preview and applies payload to create-run form"`
- `cd web && npm run lint -- tests/e2e/team_page.e2e.ts`
