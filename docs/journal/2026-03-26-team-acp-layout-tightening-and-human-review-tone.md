# Team ACP Layout Tightening And Human Review Tone

## Summary

- tighten the Team workspace and Team member ACP chrome so headers and toolbars consume less vertical space
- reserve explicit conversation bottom clearance above the ACP input dock so the latest message is not hidden behind the composer
- play a short local browser tone when a new human permission-review card first appears in the Team conversation

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_member_acp_panel.tsx`
- `web/src/components/acp_panel.tsx`
- `web/src/components/acp_conversation.tsx`
- `web/src/app.tsx`
- `web/src/pages/team_task_panel.tsx`
- `web/src/acp_panel.test.tsx`
- `web/src/pages/team_panels.test.tsx`

## Notes

- the conversation clearance is applied on the actual ACP scroll container via `scroll-padding-bottom` plus a dock-clearance spacer
- the human-review tone is best-effort only and should fire once per `permission_id` when the card becomes newly visible on the page
- this change keeps the existing Team data/API contract intact and does not depend on backend schema changes

## Validation

- browser baseline on `agenthub.hawkingrei.com` confirmed the Team conversation and permission-review card surfaces before the layout/tone changes
- local regression coverage should include `src/acp_panel.test.tsx` and `src/pages/team_panels.test.tsx`
- local Chrome DevTools MCP regression is limited when the Team route has no backing runtime data and renders `Team not found`, but the frontend bundle should still load without a page-level crash
