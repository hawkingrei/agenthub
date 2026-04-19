# Workspace Agent Pane Chrome Tightening

## Summary

- softened the agent workspace header controls so the two right-side entry points read as `Agent` and `More` instead of action-heavy menu labels
- flattened ACP secondary tabs from a segmented-control treatment into lighter inline page tabs
- renamed ACP surface labels from `Conversation / Plan / Debug` to `Activity / Plan / Inspect`
- reduced the selected team menu to action-only chrome and shortened shared-channel copy plus empty-state language
- softened the global shell menu label and reduced selector / unavailable fallback copy so empty states read more like content pages than system prompts
- reduced the sidebar fallback header and empty-state copy so the left rail reads like a directory, not a system status panel
- changed the shared workspace header tabs from `# all / Kanban` to `Channels / Kanban` so the page title carries the active lane while the tabs express the view category
- removed explicit `Refresh channel` / `Refresh thread` controls from the conversation surface because the Team workspace already auto-refreshes and the extra icon still read as toolbar chrome
- shortened message delivery/read accessibility copy from `Pending delivery` and `Seen by x of y recipients` to lighter `Pending` / `Seen x/y` metadata language
- softened the remaining `Details / Thread` inline actions so message-header controls render closer to metadata than button chrome
- shortened and dimmed the composer helper line to `@name to reply · Enter to send` so the input footer reads as a hint instead of a product explainer
- removed the duplicated `Channels / Kanban` header tabs from Team workspace pages and kept `# all / Kanban` entry points in the left rail only

## Why

- the previous chrome still looked too much like a product prototype, especially in populated Team agent workspace states
- Slock keeps these controls much lighter: directory-first shell, restrained menus, and page tabs that do not read like toolbar buttons
- AgentHub still needs to keep the same runtime affordances, but the UI should present them with quieter, more Notion-like chrome
- the populated channel header duplicated `# all` in both the title and the view tabs, which made the page feel heavier than Slock's lighter `channel name + view tabs` pattern
- Slock does not expose a dedicated refresh affordance in the channel header, and leaving ours visible made the conversation pane feel more like a debug surface than a channel timeline

## Validation

- `cd web && npm run test -- vite.config.test.ts src/pages/team/team_workspace_header.test.tsx src/pages/team_member_acp_panel.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `make build-web`
- Chrome DevTools MCP baseline:
  - `https://agenthub.hawkingrei.com/workspace/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37?lens=search`
  - confirmed the old chrome still showed `Open agent workspace menu`, `More workspace actions`, `WORKSPACE SEARCH`, and `Conversation / Plan / Debug`
- Chrome DevTools MCP local smoke:
  - `http://127.0.0.1:4174/workspace/teams/demo?lens=search`
  - frontend-only shell loaded and preserved `Workspace` header plus `Channels / Tasks / Members / Search`
  - backend data was intentionally unavailable in the local smoke environment, so populated agent workspace content was validated through focused tests rather than live data
