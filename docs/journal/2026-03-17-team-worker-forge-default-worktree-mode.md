# Team worker forge defaults follow create-worktree policy

## Summary

The Team `Add Agent` modal could open in an invalid worker configuration:

- `role = worker`
- `worktree_mode = use_existing`
- `workdir = ""`

Submitting from that state failed locally with `Agent workdir is required`, so the UI appeared to do nothing and never reached the backend create-agent request.

This also conflicted with the backend Team worker runtime policy, which requires `worktree_mode=create_worktree`.

## Changes

- Updated `resolveTeamForgeDefaults()` so worker agents default to:
  - `worktreeMode = "create_worktree"`
  - `agentWorkdir = default_worktree_root`
  - `worktreeRepo = preferred repo inferred from the current Team spec`
- Repo inference now prefers:
  - an existing worker runtime `worktree_repo`
  - otherwise the leader runtime `workdir`
- Updated the Team page call sites to pass `selectedTeam.spec` into the forge-default resolver.
- Added a focused regression test covering a Team whose leader runtime points at `/Users/weizhenwang/devel/opensource/agent/tidb`.

## Why this fixes the issue

Worker creation now opens in a state that matches the Team runtime policy:

- the modal uses `create_worktree` immediately
- the workdir falls back to the runtime default worktree root
- the repo path can be prefilled from existing Team member runtime hints

That removes the invalid empty `use_existing + empty workdir` combination that blocked submission before any request was sent.

## Validation

- Focused helper regression: `src/pages/team/forge_helpers.test.ts`
- Frontend lint: `src/pages/team/forge_helpers.ts`, `src/pages/team/forge_helpers.test.ts`, `src/pages/team_page.tsx`
- Frontend production build

## Chrome DevTools MCP

Baseline reproduction was captured on `https://agenthub.hawkingrei.com/teams/...` before the code change:

- `Add Agent` for a worker opened with an empty `Workdir`
- clicking `Create Agent` did not issue a backend create request
- the page surfaced `Agent workdir is required`

The fix is local code only at this stage; deployed-domain regression still needs verification after rollout.
