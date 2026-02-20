# Web Tailwind UI Phase-6: OutputBody + OutputErrorBoundary + InputDock

## Background

After phase-5, the remaining high-frequency shell elements in the main agents
workspace are output state surfaces and input dock controls.

This phase migrates visual shells for:

- output loading/empty state;
- output render-error fallback;
- input dock container/history/interrupt styling.

## Scope

- `web/src/components/output_body.tsx`
- `web/src/components/output_error_boundary.tsx`
- `web/src/components/input_dock.tsx`
- `docs/todo.md`

## Key Decisions

1. Keep behavior unchanged:
   - output mode switching (`loading`/`acp`/`terminal`/`empty`)
   - error-boundary retry behavior
   - input dock keyboard shortcuts and history interactions
2. Preserve test-coupled class names where tests assert exact strings:
   - `input-row`
   - `input-editor-row`
   - `input-send-button`
   - `jump-bottom`
3. Layer Tailwind utilities on shell elements and non-test-coupled controls
   (root containers, textarea, history controls, interrupt button).

## Validation Evidence (local)

- Focused tests:
  - `npm --prefix web run test -- src/output_body.test.tsx src/input_dock_render.test.tsx src/input_dock_keyboard.test.ts`
- Workspace regression tests:
  - `npm --prefix web run test -- src/agents_panel.test.tsx src/output_header.test.tsx src/pages/team_panels.test.tsx`
- Lint:
  - `npm --prefix web run lint`
- Build:
  - `npm --prefix web run build`

## Follow-up Validation

- Manual desktop/mobile checks in `/`:
  - loading/empty output states and fallback retry card spacing
  - input dock history menu usability
  - interrupt/send button readability on small screens
  - keyboard behavior remains unchanged (Enter send, Shift+Enter newline, history up/down)

## Notes

- This phase intentionally avoids changing input action semantics or ACP/terminal
  rendering logic.
