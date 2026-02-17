# Mobile Input Dock Bottom Alignment

## Summary

Ensure the input dock stays pinned to the bottom of the output panel on mobile
and tablet layouts.

## Background

The input dock uses `margin-top: auto` to stay at the bottom, but this depends
on its parent column having a definite stretch height. In the workspace grid,
the right panel could shrink to content height under narrow layouts, causing the
input dock to float above the bottom.

## Scope

- `web/src/styles.css`
- `tests/web_assets.rs`
- `web/tests/e2e/input_dock_layout.e2e.ts`
- `docs/todo.md`

## Key Decisions

1. Make workspace grid rows explicit with `grid-template-rows: minmax(0, 1fr)`.
2. Stretch and clip the right workspace panel (`align-self: stretch`,
   `overflow: hidden`) so the output area + input dock layout stays constrained
   and bottom-aligned.
3. Add style-guard assertions to prevent regression in future CSS refactors.
4. Add Playwright layout assertions for three runtime states:
   - mobile workspace with agents collapsed,
   - mobile workspace while toggling agents collapsed/expanded,
   - virtual-keyboard style viewport shrink/restore transitions.

## Validation

```bash
PLAYWRIGHT_NO_WEBSERVER=1 npm --prefix web run e2e -- tests/e2e/input_dock_layout.e2e.ts --project=chromium
cargo test --test web_assets
```

Expected outcomes:

- Input dock remains visually anchored to panel bottom on mobile/tablet.
- Input dock stays pinned during agent panel collapse/expand transitions.
- Input dock stays pinned during keyboard-like viewport height changes.
- Workspace right panel does not expand unexpectedly from internal scroll areas.
- Style guard test passes.
