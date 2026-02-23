---
title: ACP Jump-to-Bottom Placement Above Input Dock
date: 2026-02-23
status: implemented
---

## Summary

Move ACP conversation `Jump to bottom` control from viewport-floating position to
an app-level row above the entire input dock.

## Background

The previous `jump-bottom` style used a fixed viewport position (`top: 50%`),
which separated the control from input context and made interaction feel
inconsistent on mobile/tablet layouts.

## Key Decisions

- Keep jump control outside `InputDock` so it does not appear inside the input
  component layout.
- Add dedicated `output-jump-row` in App layout between conversation output and
  input dock.
- Change `.jump-bottom` from `position: fixed` to dock-local layout with
  right alignment.
- Keep button size and icon semantics unchanged.

## Scope

- `web/src/app.tsx`
- `web/src/components/input_dock.tsx`
- `web/src/styles.css`
- `web/src/input_dock_render.test.tsx`
- `docs/todo.md`

## Validation

- [x] `npm --prefix web test -- input_dock_render.test.tsx`
