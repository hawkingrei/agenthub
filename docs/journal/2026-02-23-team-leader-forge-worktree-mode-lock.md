# Team Leader Forge Worktree Mode Lock

## Background

Team runtime policy already requires leader agents to start with:

- `worktree_mode=use_existing`
- an empty `workdir`

But Team Forge still reused the generic Create Agent modal controls, which exposed worktree mode/repo/ref options in the leader stage and could produce misleading configuration intent.

## Scope

- Frontend-only behavior hardening for Team Forge leader creation flow.
- No backend API contract changes.
- No database changes.

## Key Decisions

1. Keep Create Agent modal generic, but allow callers to disable worktree advanced controls.
2. In Team Forge leader stage, hide worktree advanced controls entirely.
3. In Team Forge submit path, enforce leader payload with:
   - `worktree_mode=use_existing`
   - `worktree_repo=null`
   - `worktree_ref=null`
4. Preserve worker-stage behavior unchanged (`create_worktree`/`reuse_worktree` options still available).
5. Add regression test coverage for the modal-level “advanced controls hidden” branch.

## Files Changed

- `web/src/components/create_agent_modal.tsx`
- `web/src/pages/team_page.tsx`
- `web/src/create_agent_modal.test.tsx`

## Validation

Executed local checks:

- `npm --prefix web run test -- src/create_agent_modal.test.tsx src/pages/team_panels.test.tsx`
- `npm --prefix web run lint`
- `npm --prefix web run build`

All passed.

## Chrome DevTools MCP Verification

- Attempted to run Chrome DevTools MCP checks.
- Blocked by MCP transport failure in current session (`tools/call failed: Transport closed`).
- Added TODO follow-up to capture before/after snapshots once MCP transport is healthy.
