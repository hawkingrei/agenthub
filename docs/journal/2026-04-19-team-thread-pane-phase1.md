# Team Thread Pane Phase 1

## Summary

- implemented the first Team `channel/thread` shell contract in the web layer without pretending
  reply persistence already exists
- added route/query helpers for Team `channel` and `thread` selection
- added a right-side `ThreadPane` skeleton that opens from existing shared-channel messages
- kept `# all` as the default Team channel

## Scope

- `thread` is still derived from an existing channel message (`root_message_id`)
- this phase does not introduce backend thread persistence or thread-specific reply storage
- the pane currently acts as a shell projection around one selected root message plus navigation
  actions

## Validation

```bash
cd web && npm run test -- vite.config.test.ts src/pages/team_page.helpers.test.ts src/pages/team/team_thread_pane.test.tsx src/pages/team/team_page_header.test.tsx src/pages/team_panels.test.tsx
make build-web
```

## Notes

- message-level `Open thread` is only exposed on the shared Team channel right now
- the next phase should connect the pane to real Team actor thread-open / thread-reply behavior
- local Chrome DevTools MCP can confirm shell stability, but without a backend it cannot exercise a
  real thread-open path end to end yet
