# Output LocalStorage Persistence

## Background

Chat output was held only in memory, so a page refresh or tab crash cleared the current session history. This made recovery painful during long agent runs.

## Scope

- Persist recent output and ACP output caches to `localStorage`.
- Restore caches on page load to repopulate Output views.
- Cap stored events per session and number of sessions to avoid storage bloat (currently 800 events per session, 40 sessions).

## Key Decisions

- Store a single payload with versioning under `agenthub_output_cache_v2`.
- Bump the cache key when the schema changes (e.g. `event_id` required) to avoid
  mixing older cached payloads.
- Persist on a short debounce to avoid excessive synchronous writes.

## Validation

```bash
cd web && npm test
```

## Follow-ups

- Confirm output history restores correctly after reload for both ACP and terminal output.
- Decide whether to persist active session selection.
