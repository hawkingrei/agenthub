## Why

The Agents ACP conversation view was rendering an extra bottom spacer equal to the input dock
clearance. Because the input dock already sits in normal document flow below the conversation
panel, the spacer created a visible blank gap between the latest conversation item and the input
composer.

## What Changed

- kept `scrollPaddingBottom` on the ACP conversation scroll container so jump-to-bottom and
  auto-scroll logic still account for the dock height;
- removed the rendered `dock-clearance` spacer element from the ACP conversation body so the
  latest conversation item now sits directly above the input dock;
- updated the ACP panel regression test to assert scroll padding remains while no dock spacer is
  emitted.

## Validation

- `cd web && npx vitest run src/acp_panel.test.tsx src/acp_conversation_render.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
- `git diff --check`
