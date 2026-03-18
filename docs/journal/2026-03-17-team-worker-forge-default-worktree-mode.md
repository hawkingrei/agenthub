# Team worker forge defaults support direct workdir validation

## Summary

The Team `Add Agent` modal could open in an invalid worker configuration:

- `role = worker`
- `worktree_mode = use_existing`
- `workdir = ""`

Submitting from that state failed locally with `Agent workdir is required`, so the UI appeared to do nothing and never reached the backend create-agent request.

For immediate validation against an existing repo such as `/Users/weizhenwang/devel/opensource/agent/tidb`, the Team worker path now allows direct `use_existing` workdir startup instead of forcing `create_worktree`.

## Changes

- Updated `resolveTeamForgeDefaults()` so worker agents default to:
  - `worktreeMode = "use_existing"`
  - `agentWorkdir = preferred repo/workdir inferred from the current Team spec`
  - `worktreeRepo = ""`
- Worker workdir inference now prefers:
  - an existing worker runtime `worktree_repo`
  - otherwise the leader runtime `workdir`
  - otherwise any existing runtime `workdir`
- Updated the Team page call sites to pass `selectedTeam.spec` into the forge-default resolver.
- Relaxed Team worker runtime start policy so `use_existing` workdir is accepted for validation, while `create_worktree` remains supported.
- Added focused frontend/backend regression tests covering a Team whose leader runtime points at `/Users/weizhenwang/devel/opensource/agent/tidb`.

## Why this fixes the issue

Worker creation now opens in a state that is directly usable for repo validation:

- the modal keeps `use_existing`
- the workdir can be prefilled from existing Team member runtime hints
- Team runtime startup no longer rejects worker `use_existing` workdirs during validation

That removes the invalid empty `use_existing + empty workdir` combination that blocked submission before any request was sent, and lets a worker point directly at an existing repo checkout for early validation.

## Validation

- Focused helper regression: `src/pages/team/forge_helpers.test.ts`
- Focused Rust regression: `src/agent/manager/tests.rs`
- Frontend lint: `src/pages/team/forge_helpers.ts`, `src/pages/team/forge_helpers.test.ts`, `src/pages/team_page.tsx`
- Frontend production build

## Chrome DevTools MCP

Baseline reproduction was captured on `https://agenthub.hawkingrei.com/teams/...` before the code change:

- `Add Agent` for a worker opened with an empty `Workdir`
- clicking `Create Agent` did not issue a backend create request
- the page surfaced `Agent workdir is required`

The fix is local code only at this stage; deployed-domain regression still needs verification after rollout.
