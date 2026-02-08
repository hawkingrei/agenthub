# Output Ordering With Mixed Seq Formats

## Background
Some environments still contain historical events with numeric `seq` values
while new events use UUIDv7 strings. Pure lexicographic ordering breaks when
these formats mix, which can cause output history to appear out of order.

## Scope
- Unify output ordering across output caches and ACP conversation rendering.
- Prefer seq ordering when both entries share the same format, otherwise fall
  back to timestamp ordering.

## Decisions
- Introduce `compareEventOrder` to compare UUIDv7, numeric seq, and timestamps.
- Use this comparator in output caches, event fetch ordering, and conversation
  message ordering.

## Validation
- `pnpm -C web test`
- Manual: scroll through long output with mixed legacy and new events and
  confirm ordering is stable.
