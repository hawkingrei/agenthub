# Mobile Input Safe-Bottom Inset Suppression

## Summary

Prevent large white gaps under the input dock on mobile keyboards by suppressing
app-shell bottom safe-area padding during keyboard-overlap states.

## Background

The app shell applies mobile bottom padding using `safe-area-inset-bottom`.
During some keyboard transitions, viewport/layout deltas can make this bottom
inset visually over-apply, leaving a large blank area below the dock and making
the dock look detached from the bottom edge.

## Scope

- `web/src/app.tsx`
- `web/src/styles.css`
- `web/src/app.permission_scope.test.ts`
- `web/src/app.runtime_effects.test.tsx`
- `docs/todo.md`

## Key Decisions

1. Add runtime bottom-inset suppression logic:
   - detect keyboard-overlap states when `layoutHeight - viewportHeight` crosses
     a threshold;
   - set `--agenthub-safe-bottom` to `0px` in keyboard-overlap states;
   - restore `--agenthub-safe-bottom` to `env(safe-area-inset-bottom, 0px)` for
     normal states.
2. Keep viewport size sync logic unchanged for `--agenthub-vh` / `--agenthub-vw`
   to avoid collateral layout regressions.
3. Remove default `section` bottom gap from `.workspace` to keep the output panel
   and input dock naturally flush with the shell bottom.
4. Use `--agenthub-safe-bottom` in mobile `.app` padding so safe-area behavior is
   centrally controlled by runtime state.

## Validation

```bash
cd web
npm run test -- src/app.permission_scope.test.ts src/app.runtime_effects.test.tsx
npm run lint -- src/app.tsx src/app.permission_scope.test.ts src/app.runtime_effects.test.tsx

cd ..
cargo test --test web_assets
```

Expected outcomes:

- Keyboard-open states no longer show large blank space under the input dock.
- Non-keyboard mobile states still honor bottom safe-area inset.
- Input dock remains visually bottom-anchored with stable viewport transitions.
- Runtime viewport/style guard tests remain green.
