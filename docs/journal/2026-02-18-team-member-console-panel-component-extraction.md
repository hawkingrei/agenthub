# Team Member Console Panel Component Extraction

## Summary

Extract Team member console UI from `web/src/pages/team_page.tsx` into `TeamMemberConsolePanel` so mailbox/member-console tab logic no longer lives in one large render block.

## Background

After extracting mailbox IM into `TeamMailboxPanel`, `team_page.tsx` still held a large `member_console` tab with mixed render/interaction logic. This tab is also a coherent feature slice (member selection, preview timeline, per-member event stream), so it is a safe second extraction step.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_member_console_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep run/event state ownership in `TeamPage` and pass callbacks/derived data into the new panel.
2. Keep refresh policy unchanged:
   - selected member: refresh member event stream,
   - no selected member: refresh run events preview.
3. Keep preview-mode semantics unchanged:
   - no member selected => latest limited run records,
   - member selected => full member event timeline with `Load Older`.

## Validation

Executed (2026-02-18):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts -g "team mailbox IM mode supports conversation focus"'
```
