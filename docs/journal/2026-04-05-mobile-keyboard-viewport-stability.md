# fix(web): keep shell height stable during mobile keyboard transitions

## Summary

Avoid full-page height jumps when the mobile keyboard opens over an input.

## Changes

- keep `--agenthub-vh` and `--agenthub-vw` stable when `visualViewport` shrinks only because the keyboard is present and the viewport width is unchanged
- continue updating `--agenthub-keyboard-inset` so input surfaces can still react to keyboard overlap without forcing the entire shell to reflow
- update viewport sync tests to cover keyboard-only shrink separately from true viewport width changes

## Validation

- `cd web && npx vitest run src/app.runtime_effects.test.tsx src/app.permission_scope.test.ts --pool=threads --maxWorkers=1`
- `cd web && npm run lint -- src/app.tsx src/app.runtime_effects.test.tsx src/app.permission_scope.test.ts`
- `make build-web`
