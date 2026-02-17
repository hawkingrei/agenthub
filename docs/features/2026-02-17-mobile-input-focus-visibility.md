# Mobile Input Focus Visibility

## Summary

Keep the input dock visible while typing on mobile/tablet when the virtual keyboard changes the visible viewport.

## Background

In narrow viewports, users could focus the message textarea and lose sight of the active input area after virtual keyboard transitions.
The dock stayed at layout bottom, but viewport changes (keyboard/pan) could place the focused textarea outside the currently visible region.

## Scope

- `web/src/components/input_dock.tsx`
- `web/src/input_dock_keyboard.test.ts`
- `docs/todo.md`

## Key Decisions

1. Add a mobile-only focus visibility guard in `InputDock`.
2. While textarea is focused, listen to `visualViewport` resize/scroll and window resize.
3. Detect when textarea vertical bounds leave visible viewport and scroll the dock container into view with a single `scrollIntoView` call to avoid double-jump effects.
4. Keep desktop behavior unchanged by gating this logic behind mobile input breakpoint detection.

## Validation

```bash
npm --prefix web run test -- input_dock_keyboard.test.ts
npm --prefix web run build
```

Expected outcomes:

- New viewport-visibility helper tests pass.
- Viewport helper coverage includes shifted viewport and viewport-shrink edge cases.
- Web build succeeds without type/style regressions.

## Follow-ups

- Add Playwright mobile E2E case that simulates textarea focus and verifies dock remains visible during viewport height changes.
