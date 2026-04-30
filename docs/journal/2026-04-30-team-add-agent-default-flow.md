## Team Add Agent Default Flow

- Removed prompt authoring from the default Team `Add Agent` flow so novice users are no longer asked to edit system instructions directly.
- Kept runtime controls in the default path:
  - workspace path
  - workspace mode / worktree settings
  - code mode
- Updated Team setup and forge copy to describe the flow as adding a role, description, and workspace instead of configuring skills and prompt text.
- Hid the launch-command summary from the default Team forge modal while preserving the generic modal capability for other surfaces.

## Validation

- `cd web && pnpm exec vitest run src/create_agent_modal.test.tsx src/create_agent_modal.interaction.test.tsx src/pages/team/team_management_modals.test.tsx`
- `cd web && npm exec tsc -- --noEmit`
- `cd web && npm run lint`
- Chrome DevTools MCP baseline:
  - verified the current deployed Team `Add Agent` modal still shows `LAUNCH COMMAND`, confirming the UI change is local-only until this patch is deployed
