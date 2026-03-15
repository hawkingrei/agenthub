# Mobile Output Header ACP Merge

## Summary

- Reduced the visual separation between the mobile agent header and ACP tabs.
- Removed the extra ACP wrapper shell in ACP mode so the header and tabs can read as one continuous block.

## Implementation

- Added an ACP-specific output body wrapper in `web/src/components/output_body.tsx` and `web/src/ui/tailwind_classes.ts`.
- Restored the ACP panel's own top border/radius on mobile once ACP mode started hiding the standalone output header.
- Tightened the mobile output header spacing, kept the agent title and status on the same mobile row, and removed the bottom border/radius on the header in ACP mode.
- Hid the ACP subtitle on narrow screens, reduced mobile `workspace-right` / ACP head spacing, and shrank the mobile status badge in `web/src/styles.css`.
- Moved the mobile agent title into the ACP head so ACP mode can render `agent name + Conversation / Plan` on the same row while hiding the standalone mobile output header.
- Removed the legacy `.acp-actions` global class from the ACP header action container after confirming that its global `display` rule was overriding mobile `hidden` state and rendering a duplicate `Conversation / Plan` row on small screens.

## Validation

- `cd web && npx vitest run src/output_body.test.tsx src/acp_panel.test.tsx src/output_header.test.tsx src/workbench_header_menu.test.tsx`
- `cd web && npm run lint -- src/components/output_body.tsx src/components/acp_panel.tsx src/ui/tailwind_classes.ts src/output_body.test.tsx src/acp_panel.test.tsx src/output_header.test.tsx`
- `cd web && npm run build`
- Chrome DevTools MCP baseline at `390x844` on `https://agenthub.hawkingrei.com/` was used as the pre-edit mobile shell reference; the local branch UI compression change is not yet deployed to that domain.

## Notes

- This pass focuses on mobile vertical density. Desktop shell proportions are intentionally left mostly unchanged.
