# ACP Conversation Tool Content Split

## Summary

- extracted ACP tool payload/text/diff/terminal rendering from `web/src/components/acp_conversation.tsx` into `web/src/components/acp_tool_content.tsx`
- extracted `request_user_input` pending/result cards into `web/src/components/acp_request_user_input_cards.tsx`
- extracted tool-call bubble/group/explore rendering plus fold/live-state helpers into `web/src/components/acp_tool_bubbles.tsx`
- split payload/text/diff rendering a second time into `web/src/components/acp_tool_payload_content.tsx` so `acp_tool_content.tsx` can focus on terminal rendering, ANSI caches, and payload normalization helpers
- split the structured payload tree helpers/renderers again into `web/src/components/acp_tool_payload_tree.tsx` so `acp_tool_payload_content.tsx` can focus on markdown/plain-text/diff views instead of carrying both render paths
- split markdown/plain-text/diff rendering again into `web/src/components/acp_tool_text_views.tsx` so `acp_tool_payload_content.tsx` now stays a thin payload normalization/entry layer
- moved terminal rendering plus ANSI cache accounting into `web/src/components/acp_terminal_output.tsx` so `acp_tool_content.tsx` is now a small aggregation layer
- kept `acp_conversation.tsx` focused on conversation item dispatch, cache wiring, key/focus helpers, and scroll behavior
- preserved the existing public cache helpers by re-exporting `parseAnsiSegmentsCached` from `acp_conversation.tsx`
- preserved the existing public live/fold helper exports by re-exporting them from `acp_conversation.tsx`

## Validation

- `cd web && npm run test -- src/acp_conversation.test.ts src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx`
- `cd web && npm run test -- src/components/acp_tool_payload_tree.test.tsx src/acp_conversation_render.test.tsx src/acp_conversation.interaction.test.tsx`
- `cd web && npm run lint`
- `cd web && npm run build`
- `make build-web`

## Follow-up

- split shared ACP conversation list/window state from presentation so future performance work can land without reopening large render modules
- keep shrinking the ACP payload path by splitting the remaining markdown/plain-text/diff views if another focused edit needs those seams
- consider moving remaining key/focus/cache helpers into a tiny `acp_conversation_state` module only if the next edit needs that boundary
