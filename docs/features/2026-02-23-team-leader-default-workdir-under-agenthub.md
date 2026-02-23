# Team Leader Default Workdir Under `~/.agenthub`

## Background

Team Leader creation in Team Forge still required a manually provided `workdir` in some paths, which was unnecessary for the single-node workflow.
We already enforce Team Leader runtime policy as:

- `worktree_mode=use_existing`
- leader workspace should be empty and dedicated to coordination/context artifacts

To reduce friction, Leader Forge should provide a deterministic default path under `~/.agenthub` and allow users to proceed without manually typing the path.

## Scope

- Frontend Team Forge behavior for leader agent creation
- Backend runtime startup behavior for leader workspace directory existence
- Unit test coverage for the new path builder helper

No changes to worker worktree strategy or Team role policy semantics are included in this change.

## Key Decisions

1. Add a dedicated frontend helper to build default leader workdir paths:
   - `buildLeaderForgeDefaultWorkdir(defaultRoot, agentName, seed?)`
   - default root fallback remains `~/.agenthub/worktrees`
   - final format: `<root>/<normalized-agent-name>-<base36-seed>`
2. In Team Forge modal open path:
   - when role is `leader`, auto-fill `forgeAgentWorkdir` with helper output
3. In Team Forge create submit path:
   - when role is `leader` and input is empty, auto-derive `workdir` with the same helper
4. In backend agent startup:
   - for Team Leader runtime context only, if the target workdir does not exist, create it via `std::fs::create_dir_all`
   - preserve existing error recording/status update behavior on failure

## Files Changed

- `web/src/pages/team/create_helpers.ts`
- `web/src/pages/team/create_helpers.test.ts`
- `web/src/pages/team_page.tsx`
- `src/agent/manager.rs`

## Validation

Executed during development:

- `npm --prefix web run test -- src/pages/team/create_helpers.test.ts`
- `npm --prefix web run lint`
- `cargo test runtime_start_policy -- --nocapture`

Expected behavior after merge:

- Creating a Team Leader from Team Forge no longer requires manual `workdir` input
- Default path resolves under `~/.agenthub/worktrees/...`
- Leader startup no longer fails when the default directory is not pre-created

## Risks And Follow-up

- Default path uniqueness currently uses a timestamp-derived token; collisions are unlikely but not impossible under forced identical seed input.
- Deployment-level verification is still required to capture both push/PR CI run IDs and final UI behavior evidence (tracked in `docs/todo.md`).
