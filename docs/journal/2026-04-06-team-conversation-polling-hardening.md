# Team Conversation Polling Hardening

## Summary

- reduced repeated Team task detail fetches when a selected thread still has no latest run
- aligned Team conversation SSE fallback so 4s polling only runs when SSE is unavailable or disconnected
- kept selected task thread stable while `taskList` is temporarily stale but `selectedConversationDetail` is still present
- fixed Team thread optimistic echoes to reuse the active thread `conversation_id`
- hardened shared primitives and input handling with:
  - `SelectableListItem` defaulting to `type="button"`
  - input dock height reporter resetting when callback identity changes
  - IME-aware Enter handling in Team channel composer
  - ANSI segment cache refreshing access order on hits

## Validation

- `cd web && npm run test -- src/pages/team_panels.test.tsx src/pages/team/use_team_conversation_actions.test.tsx src/pages/team/use_team_conversation_effects.test.tsx src/ui/primitives.test.tsx`
- `cd web && npm run lint -- --ignore-pattern dist-debug --ignore-pattern dist-debug-current`
- `cd web && npm run build`
- `make build-web`

## MCP Notes

- live root page still loads cleanly without eager agents workbench hydration when no agent is selected
- live Team page now returns to `ONLINE · SSE CONNECTED` after reload without the earlier catalog-refresh storm
- Team page still shows periodic `/api/agents` and `/permissions` traffic, so broader Team runtime polling reduction remains a follow-up
