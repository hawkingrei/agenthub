# Mobile Input White-Screen Guard

## Summary

Prevent viewport collapse when mobile keyboards trigger transient invalid
`visualViewport` dimensions during text input.

## Background

The app shell height/width are synchronized from `visualViewport` into CSS
variables (`--agenthub-vh` / `--agenthub-vw`). On some mobile keyboard
transitions, browsers can briefly report unstable viewport values (for example
`0`, `1`, or non-finite numbers). Writing those values directly can collapse
the shell to near-zero size and appear as a white screen while typing.

## Scope

- `web/src/app.tsx`
- `web/src/app.permission_scope.test.ts`
- `web/src/app.runtime_effects.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Guard runtime viewport dimensions before writing CSS variables:
   - Treat non-finite and `<= 1` viewport values as invalid.
   - Fall back to `window.innerHeight` / `window.innerWidth`.
2. Keep the existing viewport sync flow unchanged otherwise, so behavior on
   valid viewport updates remains the same.
3. Add unit coverage for invalid viewport inputs to avoid regression.

## Validation

```bash
cd web
npm run test -- src/app.permission_scope.test.ts
npm run test -- src/app.runtime_effects.test.tsx
npm run build
```

Expected outcomes:

- Invalid transient viewport values no longer collapse the app shell.
- Existing viewport sync behavior still works for normal values.
- Runtime app-shell effects recover cleanly after invalid (`0/1` or non-finite)
  viewport transitions and resume valid viewport sizing.
- Web tests/build pass.
