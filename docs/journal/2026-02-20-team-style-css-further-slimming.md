# Team Style CSS Further Slimming

## Background

After the previous Team/ACP style-layer retirement pass, `web/src/styles.css` still contained several Team layout blocks that duplicated existing Tailwind utility classes in the Team page/components.
These duplicate blocks increased maintenance surface and made style ownership less clear.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_sidebar.tsx`
- `web/src/pages/team_run_panel.tsx`
- `web/src/styles.css`
- `docs/todo.md`

## Key Decisions

1. Move Team shell/layout concerns to component-level Tailwind classes:
   - `teams-layout` host now carries `min-h-0` in JSX.
   - `teams-main` host now carries `min-h-0` in JSX.
   - sidebar/form/list/member-strip/run-list footer layouts are now expressed directly in JSX utility classes.
2. Keep semantic class hooks where useful for tests/querying:
   - class names like `teams-list`, `teams-member-dot`, `teams-run-list-foot` remain in markup.
   - redundant global CSS definitions for these hooks are removed.
3. Remove redundant Team layout/style blocks from `web/src/styles.css`, including:
   - `teams-layout/sidebar/main/form/run-list/list-head/list-foot`
   - `teams-list`
   - `team-member-summary`
   - `teams-member-status-panel/strip/dot*`
   - corresponding `@media (max-width: 960px)` fallback for `teams-layout` and `teams-sidebar/main`.

## Validation

- `npm --prefix web run lint`
- `npm --prefix web run test -- src/pages/team_panels.test.tsx src/pages/team_page.runs.test.ts`
- `npm --prefix web run build`

All checks passed locally after the CSS slimming pass.
