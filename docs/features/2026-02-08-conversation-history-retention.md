# Conversation History Retention Under Long Output

## Background
Long-running output can push caches past `maxCachedEvents`. The UI selection
effect used to replace in-memory outputs with truncated cache slices, which
drops older messages and can distort conversation order after loading history.

## Scope
- Web UI: preserve previously loaded history for the active agent/session when
  cache slices refresh.
- No changes to backend storage or UUIDv7 generation.

## Decisions
- When the active agent/session key is unchanged, merge cached outputs with the
  currently displayed outputs instead of replacing them.
- Keep cache trimming behavior unchanged; history preservation is UI-only.
- Add `mergeOutputsPreserveHistory` helper with unit tests.

## Validation
- `pnpm -C web test`
- Manual: scroll up to load older events, then wait for new output; older
  messages should remain visible and ordered.
