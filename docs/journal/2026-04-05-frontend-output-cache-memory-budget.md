## Summary

- tightened the frontend output-cache default budget so the browser no longer keeps the previous `800 events x 40 sessions` cache footprint alive by default;
- added an explicit storage-budget regression test to keep the persisted output cache below a multi-megabyte payload even under representative long-run histories.

## Details

- `web/src/storage/output_cache_budget.ts`
  - introduced shared frontend cache-budget constants for output history retention.
- `web/src/app.tsx`
  - switched the live output-cache configuration to the shared budget constants instead of the previous wide inline defaults.
- `web/src/output_cache_storage.test.ts`
  - added a memory-budget regression that seeds oversized output/acp caches, persists them through `saveOutputCaches(...)`, and asserts the stored payload is trimmed to the shared session/event limits and stays under a 2 MB serialized budget.

## Rationale

- live inspection showed Team pages were not holding large DOM trees, but the browser still retained a multi-megabyte `agenthub_output_cache_v2` payload plus matching in-memory cache state;
- the previous default budget (`800 events`, `40 sessions`) was much larger than the currently visible UI windows and doubled ACP-heavy histories across both `outputCache` and `acpOutputCache`;
- shrinking the default cache budget reduces steady-state heap pressure without changing protocol behavior or removing server-backed history loading.

## Validation

- `cd web && npm run test -- src/output_cache_storage.test.ts`
- `cd web && npm run lint`
- `cd web && npm run build`
