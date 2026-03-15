# ACP Payload Density Tuning

## Summary

- Tightened ACP payload and subfold density so tool-call details consume less vertical space.
- Brought grouped tool entries and payload cards closer to the same compact visual rhythm used by the surrounding agent shell.

## Implementation

- Reduced padding and contrast on grouped tool entries, payload cards, and nested payload sections in `web/src/styles.css`.
- Tightened `acp-subfold` summary spacing, preview width, and mono text sizing so previews read like metadata instead of full content rows.
- Reduced payload grid gaps and key/value column widths on both desktop and mobile to increase information density without changing structure.

## Validation

- `cd web && npx vitest run src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_panel.test.tsx`
- `cd web && npm run lint -- src/components/acp_conversation.tsx src/ui/tailwind_classes.ts src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_panel.test.tsx`
- `cd web && npm run build`
- Chrome DevTools MCP baseline remained `https://agenthub.hawkingrei.com/` on mobile width. The domain still shows the pre-deploy baseline, so this pass is validated against code/tests/build plus domain-side baseline comparison rather than a deployed post-edit rendering.
