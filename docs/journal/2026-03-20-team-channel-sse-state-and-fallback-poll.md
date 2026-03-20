# Team channel SSE state and fallback polling

## Summary

- fixed the Team workbench connection badge so `Teams -> Channel` no longer hardcodes
  `Online · SSE idle`
- exposed the shared-thread conversation SSE lifecycle (`idle/connecting/connected/reconnecting`)
  from the Team conversation effects hook back to `TeamPage`
- kept a 4 second fallback refresh running while the Team channel is open, even when the
  EventSource reports `open`, so channel updates still surface if SSE becomes quiet or stops
  delivering conversation events

## Root cause

- `TeamPage` derived its connection badge with `deriveConnectionBadge(networkOnline, false, "idle")`
  so the workbench header always showed idle regardless of the real shared-thread stream state
- `useTeamConversationEffects` maintained a private `sseConnectedRef`, but did not expose that
  state to the page shell
- the fallback refresh loop stopped polling as soon as the EventSource reached `open`
  which made the Team channel overly dependent on SSE actually delivering every update

## Validation

- `cd web && npx vitest run src/pages/team/use_team_conversation_effects.test.tsx`
- `cd web && npm run lint -- src/pages/team/use_team_conversation_effects.ts src/pages/team/use_team_conversation_effects.test.tsx src/pages/team_page.tsx`
- `cd web && npm run build`

## Follow-up

- verify on deployed `agenthub.hawkingrei.com` that `Teams -> # all`:
  - reports `connecting/connected/reconnecting` instead of a permanently idle badge
  - continues to receive fresh channel updates without manual refresh even if SSE goes quiet
