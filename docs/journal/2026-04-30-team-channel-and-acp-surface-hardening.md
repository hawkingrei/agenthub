## Summary

- kept `channels` center timelines pinned to the current lane's canonical conversation instead of letting stale task selection hijack `# all`
- added a compact ACP activity summary strip for Team member ACP headers so operators can see loaded updates, tool calls, and older-history availability before scanning the full thread
- kept Team member ACP session identity sticky across transient runtime/snapshot refresh gaps so new activity does not blank the current ACP thread or clear the top-of-history state

## Validation

- `cd web && pnpm exec vitest run src/pages/team_page.helpers.test.ts src/pages/team/use_team_task_workspace_data.test.tsx src/pages/team_member_acp_panel.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`

## Notes

- Chrome DevTools MCP baseline showed the production ACP header still on the old layout before deployment; post-deploy regression should confirm the new ACP activity strip and verify that active member ACP threads no longer blank when new messages arrive.
