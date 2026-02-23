---
title: ACP Input Dock Jump-to-Bottom Placement Above Editor
date: 2026-02-23
status: implemented
---

## Summary

Move ACP conversation `Jump to bottom` control from viewport-floating position to
an input-dock-local row above the input editor.

## Background

The previous `jump-bottom` style used a fixed viewport position (`top: 50%`),
which separated the control from input context and made interaction feel
inconsistent on mobile/tablet layouts.

## Key Decisions

- Keep jump control outside the editor grid flow to avoid textarea/send layout
  coupling.
- Add dedicated `input-jump-row` between actions row and editor row.
- Change `.jump-bottom` from `position: fixed` to dock-local layout with
  right alignment.
- Keep button size and icon semantics unchanged.

## Scope

- `web/src/components/input_dock.tsx`
- `web/src/styles.css`
- `web/src/input_dock_render.test.tsx`
- `docs/todo.md`

## Validation

- [x] `npm --prefix web test -- input_dock_render.test.tsx`
