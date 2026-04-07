# Team Page Workspace And Debug Extraction

## Summary

- extracted the Team workspace header chrome from `web/src/pages/team_page.tsx` into
  `web/src/pages/team/team_workspace_header.tsx`
- extracted the Team page top header chrome from `web/src/pages/team_page.tsx` into
  `web/src/pages/team/team_page_header.tsx`
- extracted Team debug/run-ops shells from `web/src/pages/team_page.tsx` into
  `web/src/pages/team/team_debug_panels.tsx`
- extracted the Team selector route UI from `web/src/pages/team_page.tsx` into
  `web/src/pages/team/team_selector_panel.tsx`
- added focused SSR coverage for both extracted surfaces

## Why

`team_page.tsx` had accumulated multiple unrelated UI shells:

- workspace title and action chrome
- Team debug tab switcher
- run creation / run lookup forms
- repeated "Go to Runs" empty states

These blocks had stable rendering boundaries, but they were still embedded in the
route component. Extracting them reduces the size of the route file and makes later
work on Team routing, lazy loading, and workbench state easier to review.

## Validation

- `cd web && npm run test -- src/pages/team/team_page_header.test.tsx src/pages/team/team_workspace_header.test.tsx src/pages/team/team_debug_panels.test.tsx src/pages/team/team_selector_panel.test.tsx src/pages/team/team_management_modals.test.tsx src/pages/team_page.smoke.test.tsx src/pages/team_panels.test.tsx`
- `cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current`
- `cd web && npm run build`
- `make build-web`
