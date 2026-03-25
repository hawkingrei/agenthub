# Team ACP jump control and channel tail loading

## Summary

- Deduplicated ACP jump controls so the team agent ACP view no longer renders both the ACP-local jump button and the input-dock jump button at the same time.
- Changed shared-thread conversation loading to fetch a small recent tail first, then hydrate a larger history window in the background.

## Why

- Chrome MCP on `agenthub.hawkingrei.com` showed two visible `Jump to bottom` buttons stacked next to the ACP input box after scrolling the ACP transcript upward.
- The shared `# all` thread still loaded `messages?limit=200` and `snapshot?message_limit=200` on first open, which delayed first paint and anchored the initial window further away from the newest records than necessary.

## Validation

- `npm test -- src/app.input_dock_jump_mode.test.ts`
- `npm test -- src/pages/team/use_team_conversation_actions.test.tsx`
- `git diff --check`
- Chrome MCP baseline on `https://agenthub.hawkingrei.com/teams/276a2682-9ce7-4af5-aa6c-f12575d13c37`
  - ACP view reproduced two `Jump to bottom` buttons before local fix.
  - `# all` thread still requested `limit=200` and `message_limit=200` on the live bundle before local fix.
