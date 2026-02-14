# Input Dock Tablet Overlay Fix

## Summary

Eliminate tablet overlap between `Interrupt`/`History` chips and the input
textarea by moving `InputDock` from absolute-position chip overlay to explicit
flow layout rows.

## Background

`InputDock` used an absolutely positioned action row (`.input-row`) above the
textarea area. This worked on very narrow mobile breakpoints where layout rules
switched to static flow, but tablet widths could still hit the overlay path and
render `Interrupt` on top of the editor.

## Scope

- `web/src/components/input_dock.tsx`
- `web/src/styles.css`
- `web/src/input_dock_render.test.tsx`
- `tests/web_assets.rs`
- `docs/todo.md`

## Key Decisions

1. Keep control placement in `InputDock`, but define fixed layout slots:
   - `input-row` for action chips
   - `input-editor-row` for textarea + send button
2. Remove absolute positioning from `input-row` and switch to wrapped flow
   alignment (`justify-content: flex-end`).
3. Keep narrow-screen sizing tweaks, but reuse the same flow model across
   desktop/tablet/mobile breakpoints.
4. Add style guard coverage in `tests/web_assets.rs` so overlay-prone absolute
   positioning does not regress silently.
5. Keep editor controls on one row by using a fixed `input-editor-row` track
   (`textarea` + `Send`) and enlarge `Send` tap target for touch ergonomics.
6. Left-align `Interrupt` and `History` chips in the actions row to keep the
   control anchor stable across desktop/tablet/mobile widths.

## Validation

```bash
cd web
npm run test -- src/input_dock_render.test.tsx src/input_dock_keyboard.test.ts
npm run lint -- src/components/input_dock.tsx src/input_dock_render.test.tsx
npm run build
cd ..
cargo test --test web_assets
```

## Follow-ups

- Verify iPad portrait/landscape ergonomics on real devices:
  - no overlap between actions row and textarea
  - stable tap targets for `Interrupt` and `History`
- Verify the one-row editor layout keeps `Send` visible and easy to tap on
  tablet portrait and narrow mobile widths.
