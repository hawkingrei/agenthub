# Web Tailwind Baseline Integration

## Background

The web app currently uses Mantine components and handcrafted stylesheet blocks
(`web/src/styles.css`). We need to introduce Tailwind CSS as a utility framework
without destabilizing existing UI flows.

## Scope

- `web/package.json`
- `web/postcss.config.cjs`
- `web/tailwind.config.cjs`
- `web/src/tailwind.css`
- `web/src/main.tsx`
- `docs/todo.md`

## Key Decisions

1. Introduce Tailwind as a baseline dependency set only:
   - `tailwindcss`
   - `postcss`
   - `autoprefixer`
2. Keep current visual behavior stable during migration by disabling Tailwind
   preflight reset (`corePlugins.preflight = false`).
3. Load Tailwind directives once from `web/src/tailwind.css` and import that file
   in `web/src/main.tsx` alongside existing Mantine/custom styles.
4. Keep migration incremental:
   - no immediate wholesale rewrite of existing style blocks;
   - new/modified UI sections can adopt Tailwind utilities gradually.

## Validation Plan

- Build check:
  - `npm --prefix web run build`
- Follow-up manual checks (desktop + mobile):
  - login/join pages
  - agents page
  - teams workbench
  - ensure no baseline typography/layout reset regressions.

## Notes

- This change intentionally does not alter existing component styling contracts.
- A phased migration should prioritize high-churn UI paths first and keep shared
  style tokens centralized to avoid style drift.
