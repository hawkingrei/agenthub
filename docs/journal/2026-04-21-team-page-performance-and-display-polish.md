# Team Page Performance And Display Polish

## Summary

- moved Team workspace secondary panels behind `React.lazy` boundaries so the default Team route does not eagerly ship `runs`, `overview`, `events`, `steps`, `mailbox`, `member console`, and modal chrome
- kept debug-only `steps` and `mailbox` surfaces lazy in `team_page.tsx` as well so they do not pin those modules back into the primary Team chunk
- changed the shared workspace header to give the lens bar a dedicated mobile row instead of forcing `Workspace` and the Team tabs into one cramped line
- allowed Team conversation message metadata to wrap so `Pending`, `Details`, and `Thread` stay readable on narrow widths
- aligned focused tests with the updated Team selector/header language and the new wrapping behavior

## Why

- Chrome DevTools MCP on `https://agenthub.hawkingrei.com/workspace/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37` showed the Team page was visually cramped on narrow viewports and that the route LCP was dominated by render delay rather than network transfer
- the previous Team route eagerly loaded too much panel code for the default shared-channel view, even when operators only needed the conversation surface
- message-level metadata controls were effectively laid out as one line, which made the shared-channel timeline hard to scan on mobile

## Validation

```bash
cd web && npm run build
cd web && pnpm exec vitest run src/components/workspace_shell_header.test.tsx src/pages/team_panels.test.tsx
```

- production build evidence:
  - before this change, `route-teams` was `286.23 kB` (`77.49 kB` gzip)
  - after this change, `route-teams` is `250.64 kB` (`70.50 kB` gzip)
- Chrome DevTools MCP baseline on production:
  - navigated to `https://agenthub.hawkingrei.com/workspace/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
  - performance trace reported `LCP 2228 ms`
  - LCP breakdown showed `TTFB 449 ms` and `render delay 1779 ms`
  - mobile snapshot showed the Team header tabs crowding the title lane and the message header metadata reading as one cramped line
- Chrome DevTools MCP local regression check:
  - ran against a local Vite server with a small `fetch`/`EventSource` mock injected through DevTools
  - verified the Team header now keeps `Workspace` on its own lane while `Channels / Tasks / Members / Search` render on a dedicated row
  - verified shared-channel message metadata stays readable with wrapped `Pending` and `Thread` controls on a narrow viewport

## Notes

- the production site still returns `favicon.ico` with `404`; this pass did not change static asset wiring
- the biggest remaining frontend payload is still `route-acp-shared`, but it is no longer on the critical Team default-path chunk for this workflow
