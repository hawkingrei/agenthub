# Output Cache Slice Helper

## Background

Output cache updates were refactored to avoid stale closures during `loadAgentEvents` merges. To keep that logic consistent and testable, the slice/trim behavior should live in a shared helper.

## Scope

- Add `buildOutputCacheSlice` to unify merge + trim logic.
- Cover merge/trim/dedup scenarios with unit tests.

## Key Decisions

- Preserve existing merge semantics (dedupe by `seq` when present).
- Treat `maxCachedEvents <= 0` as "no trim".

## Validation

```bash
cd web
npm test
```
