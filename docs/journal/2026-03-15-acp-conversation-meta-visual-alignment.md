# ACP Conversation Meta Visual Alignment

## Summary

- Reduced the saturation gap between ACP conversation bubbles and the rest of the agent shell.
- Aligned conversation metadata, tool fold labels, and segmented controls with the quieter shell/meta palette used elsewhere in the workbench.

## Implementation

- Retuned the ACP bubble surfaces in `web/src/styles.css` so agent, user, thinking, plan, and tool-call bubbles all use lower-contrast backgrounds and borders.
- Unified tool fold summary text, segmented footer meta text, segmented buttons, and plan progress meta colors around the same muted gray-blue range as the rest of the shell.
- Updated `web/src/components/acp_conversation.tsx` and `web/src/ui/tailwind_classes.ts` so plan/thinking sections stop using the older violet-heavy accents and instead use the same restrained surfaces as the surrounding UI.

## Validation

- `cd web && npx vitest run src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_panel.test.tsx`
- `cd web && npm run lint -- src/components/acp_conversation.tsx src/ui/tailwind_classes.ts`
- `cd web && npm run build`
- Chrome DevTools MCP baseline was checked against `https://agenthub.hawkingrei.com/` at mobile width before the edit. The domain still reflects the pre-deploy baseline, so the MCP check for this pass is visual comparison against the current production shell rather than a direct post-edit rendering of the local branch.
