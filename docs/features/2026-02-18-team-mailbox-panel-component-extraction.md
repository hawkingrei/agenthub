# Team Mailbox Panel Component Extraction

## Summary

Extract Team mailbox IM UI from `web/src/pages/team_page.tsx` into a dedicated `TeamMailboxPanel` component while keeping behavior and API contracts unchanged.

## Background

`team_page.tsx` grew to ~4k LOC and mixed team creation, run control, mailbox IM, and member console in one render tree. Mailbox IM is already a coherent feature slice, so extracting it is the lowest-risk first step toward the larger page refactor.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_mailbox_panel.tsx`
- `docs/todo.md`

## Key Decisions

1. Preserve existing state ownership in `TeamPage` for now.
   - `TeamMailboxPanel` is presentational + callback-driven.
   - No data-fetching or state migration in this phase.
2. Keep mailbox IM behavior exactly aligned with the previous inline implementation:
   - member selection and unread indicators,
   - conversation auto-follow and ack actions,
   - chat quick-send (`chat_message` payload),
   - advanced JSON controls (send/inbox/template).
3. Track the broader page refactor as still-open work in `docs/todo.md`.

## Validation

Executed (2026-02-18):

```bash
npm --prefix web run lint
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
/bin/zsh -lc 'set -euo pipefail; npm --prefix web run dev -- --host 127.0.0.1 --port 5173 --strictPort >/tmp/agenthub-vite.log 2>&1 & VITE_PID=$!; trap "kill $VITE_PID 2>/dev/null || true" EXIT; for i in {1..30}; do curl -sf http://127.0.0.1:5173 >/dev/null && break; sleep 1; done; PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/team_page.e2e.ts -g "team mailbox IM mode supports conversation focus"'
```
