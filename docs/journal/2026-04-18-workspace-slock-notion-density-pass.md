# Workspace Slock / Notion Density Pass

## Summary

- reduced shell header chrome so the workspace title and global lenses read closer to a compact directory shell
- tightened Team selector rows and Team sidebar rows into a `title + one compact meta line` pattern
- reduced Agents rail metadata so rows behave more like a DM directory instead of a status console

## Validation

```bash
cd web && npm run test -- vite.config.test.ts src/agents_panel.test.tsx src/pages/team/team_selector_panel.test.tsx src/pages/team/team_page_header.test.tsx src/pages/team_panels.test.tsx
make build-web
```

## Chrome DevTools MCP

- baseline: `https://agenthub.hawkingrei.com/`
  - confirmed `Workspace` header with `Chat / Threads / Tasks / Members / Search`
- reference: `https://app.slock.ai/s/hawkingrei/dm/25f1e9b2-c89e-49cf-9117-c821e04ec7e6`
  - used as structure reference for compact top nav plus directory-first left rail
- post-edit local check:
  - `http://127.0.0.1:4173/`
  - `http://127.0.0.1:4173/workspace/teams`
  - backend was intentionally absent, so the local pages showed the expected bootstrap JSON error; shell/header and selector structure still rendered and were verified
