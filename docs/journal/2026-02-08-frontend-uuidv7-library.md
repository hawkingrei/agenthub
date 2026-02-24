# Frontend UUIDv7 Library

## Background
The frontend used a local UUIDv7 fallback implementation for client-side
sequence generation. Review feedback raised concerns about edge cases and
bit-shift correctness. A small, dedicated library is a safer default.

## Scope
- Replace the local UUIDv7 helper with the `uuidv7` npm package.
- Keep API usage unchanged (`uuidV7()` wrapper remains).

## Decisions
- Use `uuidv7@1.0.2` and expose it via the existing `uuidV7()` helper.
- Keep the helper as a thin wrapper to avoid refactors across the codebase.

## Validation
- `npm --prefix web install`
- `pnpm -C web lint`
