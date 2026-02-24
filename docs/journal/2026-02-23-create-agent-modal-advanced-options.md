# Create Agent Modal Advanced Options (Collapsed by Default)

## Background

Agent creation (including Team Forge leader/worker creation) showed worktree tuning controls inline by default.
For common flows, this made the modal feel heavy and required users to scan low-frequency options before creating an agent.

## Scope

- Frontend-only UX refinement in `CreateAgentModal`.
- No backend API changes.
- No database schema changes.

## Key Decisions

- Keep essential fields visible in the primary form:
  - `Agent name`
  - `Agent preset`
  - `Workdir`
  - `Code mode`
- Move worktree tuning controls behind an explicit advanced toggle:
  - `Worktree mode`
  - `Worktree repo path`
  - `Worktree ref`
- Default to collapsed advanced options for `use_existing` mode.
- Auto-expand advanced options when:
  - `worktreeMode !== use_existing`, or
  - `worktreeError` is present.
- Review follow-up:
  - only auto-expand when the condition transitions from `false` to `true`
  - do not force re-open after the user manually collapses while the condition remains `true`
- Preserve existing create-worktree helper behavior (`Auto-create under` + `Customize path`) without changing request payload rules.

## Files Changed

- `web/src/components/create_agent_modal.tsx`
- `web/src/create_agent_modal.test.tsx`
- `web/src/create_agent_modal.interaction.test.tsx`

## Validation

Executed local checks:

- `npm --prefix web run test -- src/create_agent_modal.test.tsx`
- `npm --prefix web run test -- src/create_agent_modal.interaction.test.tsx`
- `npm --prefix web run test -- src/create_agent_modal.test.tsx src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

All passed.

## Chrome DevTools MCP Verification

- Attempted baseline and post-change verification via Chrome DevTools MCP.
- Blocked in this session due MCP transport failure (`tools/call failed: Transport closed`) after stale browser-profile contention.
- Follow-up TODO was added to complete before/after MCP snapshots once the MCP transport is healthy.
