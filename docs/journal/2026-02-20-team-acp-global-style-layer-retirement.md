# Team/ACP Global Style Layer Retirement

## Background

`/teams` and ACP sections still referenced legacy global hierarchy selectors from `web/src/styles.css`, including:

- `.card`
- `.tab` / `.tab-bar` / `.team-tab-bar`
- `.toolbar` / `.toolbar-actions`
- `.actions`
- `.team-item.active`
- nested ACP selectors like `.acp-tabs .tab` and `.acp-debug-tabs .tab`

These selectors created cross-panel coupling and made Team layout overlap regressions easier to reintroduce after async refresh/re-render.

## Scope

- Team list/run/member item presentation in:
  - `web/src/pages/team_sidebar.tsx`
  - `web/src/pages/team_run_panel.tsx`
  - `web/src/pages/team_overview_panel.tsx`
  - `web/src/pages/team_mailbox_panel.tsx`
- ACP tab button presentation in:
  - `web/src/components/acp_panel.tsx`
  - `web/src/components/acp_debug.tsx`
- Legacy style-layer cleanup in:
  - `web/src/styles.css`
  - `web/src/ui/tailwind_classes.ts`

## Key Decisions

1. Migrate Team list item visuals to shared Tailwind constants in `tailwind_classes.ts` (`TEAM_LIST_ITEM_*`) and stop relying on `.team-item.active` hierarchy styles.
2. Replace ACP generic tab class tokens (`tab`, `active`, `tab-badge`) with component-scoped names (`acp-tab-button`, `acp-tab-badge`, `acp-debug-tab`) and Tailwind state classes.
3. Delete legacy global selector blocks from `styles.css` instead of keeping compatibility aliases, to avoid accidental future reuse.
4. Keep lightweight semantic marker classes (for tests/query hooks) without attaching old global hierarchy style logic.

## Validation

- `npm --prefix web run lint`
- `npm --prefix web run test -- src/acp_panel.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`
- Selector retirement grep check:
  - `rg -n "(^|[^-])\\.card\\b|\\.tab-bar\\b|\\.team-tab-bar\\b|\\.tab(\\.|\\s|\\{|$)|\\.toolbar\\b|\\.toolbar-actions\\b|\\.actions\\b|\\.team-item\\.active\\b" web/src/styles.css`

Result: lint and tests pass; grep returns no matches for the retired legacy selectors.
