# Input Dock Interrupt Relocation

## Summary

Move the ACP `Interrupt` action from the conversation header into the input
dock top row, colocated with `History`, so run control is near the command
entry area.

## Background

On mobile and narrow layouts, the ACP header already contains multiple controls
and tabs. Keeping `Interrupt` in the header makes the control area crowded and
separates it from input actions that users expect near the dock.

## Scope

- `web/src/components/acp_panel.tsx`
- `web/src/components/input_dock.tsx`
- `web/src/app.tsx`
- `web/src/styles.css`
- `web/src/acp_panel.test.tsx`
- `web/src/output_body.test.tsx`
- `web/src/input_dock_render.test.tsx`
- `docs/todo.md`

## Key Decisions

- Remove `Interrupt` from ACP header (`AcpPanel`).
- Add `Interrupt` button into `InputDock` top row with `History`.
- Keep existing run gating logic:
  - enabled only when ACP is controllable and run/tool status is active.
- Keep `Interrupt` hidden when ACP mode is absent.
- Keep compact chip styling in dock row to avoid extra height.
- Add focused unit coverage for dock interaction logic:
  - keyboard action derivation (escape/send/history navigation),
  - outside-click close listener binding/cleanup,
  - ACP tab interaction callbacks and badge rendering.

## Validation

```bash
cd web
npm run test -- src/acp_panel.test.tsx src/output_body.test.tsx src/input_dock_render.test.tsx src/input_dock_keyboard.test.ts
npx vitest run src/input_dock_keyboard.test.ts src/input_dock_render.test.tsx --coverage.enabled --coverage.provider=v8 --coverage.reporter=text --coverage.include=src/components/input_dock.tsx
npx vitest run src/acp_panel.test.tsx --coverage.enabled --coverage.provider=v8 --coverage.reporter=text --coverage.include=src/components/acp_panel.tsx
npm run lint -- src/components/acp_panel.tsx src/components/input_dock.tsx src/app.tsx src/acp_panel.test.tsx src/output_body.test.tsx src/input_dock_render.test.tsx
npm run build
```

## Follow-ups

- Verify tap ergonomics of `Interrupt` + `History` row on real mobile devices.
