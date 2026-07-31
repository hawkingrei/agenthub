# Workspace Shell Compactness Pass

- Date: 2026-04-18

## Summary

Tighten the unified workspace shell so the top-level UI feels quieter and more Notion-like instead
of reading like a control dashboard.

## Supersession

Stable shell-density rules from this note now live in
`docs/features/workspace-unified-ia.md#81-shell-density-and-chrome-contract`. This journal remains
the rollout evidence for the 2026-04-18 compactness pass.

## Changes

- Remove verbose shell subtitles from the standalone workspace header and keep the Team selector
  subtitle as the only explicit helper line.
- Compress workspace lens buttons into a quieter segmented strip instead of a second explanatory
  row.
- Reduce visual noise in the Agent rail:
  - lighter toolbar chrome
  - tighter metric cards
  - less verbose per-agent meta (`code on/off` instead of a separate label line)
  - desktop row actions stay visually recessed until hover/focus so the row reads as content first
- Compact the Team selector:
  - add a small `Workspace` eyebrow
  - reduce button weight
  - fold runtime label into the same metadata row as the team summary
  - shorten the empty-state copy

## Validation

- `cd web && npm run test -- vite.config.test.ts src/agents_panel.test.tsx src/pages/team/team_selector_panel.test.tsx src/pages/team/team_page_header.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run build`
- Chrome DevTools MCP:
  - `http://127.0.0.1:4173/workspace` shows a quieter shell header with no extra descriptive copy
  - `http://127.0.0.1:4173/workspace/teams` shows the slimmer Team selector copy and denser layout
  - frontend-only smoke still shows the expected bootstrap JSON error because backend APIs were not
    running during the MCP check
