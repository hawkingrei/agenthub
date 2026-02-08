# Output Cache Consistency Update

## Background

`loadAgentEvents` previously relied on `setState` updater side effects when merging caches, which could regress output ordering under concurrent rendering or SSE append races.

## Scope

- Introduce ref-synced snapshots for output and ACP caches.
- Route all cache writes through shared update helpers.
- Keep `loadAgentEvents` focused on cache/meta updates; outputs are derived from cache effects.

## Key Decisions

- Use refs to hold the latest cache snapshots for every write path.
- Preserve real-time append behavior to avoid user-visible latency.

## Validation

- Manual: trigger SSE output while polling is active; ordering stays stable without rollback.
- Automated:

```bash
cd web
npm test
```
