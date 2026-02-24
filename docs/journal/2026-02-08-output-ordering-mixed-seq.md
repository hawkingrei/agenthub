# Output Ordering With Mixed Seq Formats

## Background
Some environments still contain historical events with numeric `seq` values
while new events use UUIDv7 strings. Pure lexicographic ordering breaks when
these formats mix, which can cause output history to appear out of order.

## Scope
- Unify output ordering across output caches and ACP conversation rendering.
- Prefer a stable ordering based on `event_id` and fall back to timestamps when
  needed, without interpreting mixed `seq` formats.

## Decisions
- Introduce `compareEventOrder` to compare `event_id` first and `ts` second.
- Use this comparator in output caches, event fetch ordering, and conversation
  message ordering; `seq` remains diagnostic-only.

## Validation
- `pnpm -C web test`
- Manual: scroll through long output with mixed legacy and new events and
  confirm ordering is stable.
