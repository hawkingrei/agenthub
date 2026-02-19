# Agents Terminal Jump-To-Bottom Restore

## Background

`InputDock` jump-to-bottom action on `/` (Agents page) was wired only to ACP conversation
state. In non-ACP terminal sessions, users could scroll up but no jump button was rendered.

## Scope

- `web/src/app.tsx`
- `web/src/app.input_dock_jump_mode.test.ts`

## Key Decisions

1. Track terminal stickiness in App state (`terminalShowJump`) in addition to
   `terminalStickToBottomRef`.
2. Restore jump button behavior for terminal mode:
   - show button when terminal is not near bottom;
   - hide button when auto-follow is active or after jumping.
3. Keep ACP behavior unchanged by introducing a small selector helper:
   `resolveInputDockJumpMode`.

## Validation Evidence (2026-02-19)

- Command:
  - `cd web && npm run test -- src/app.input_dock_jump_mode.test.ts`
- Result:
  - `resolveInputDockJumpMode` tests passed.

- Command:
  - `cd web && npm run build`
- Result:
  - Vite production build passed.
