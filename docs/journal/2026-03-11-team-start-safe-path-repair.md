## Summary

Fixed `Start Team` failures caused by legacy worker runtime records that were missing `worktree_repo` and then fell through to a `safe_paths` rejection after partial repair.

## Root Cause

Legacy worker agents could persist invalid runtime configuration:

- `worktree_mode = use_existing`
- `worktree_repo = NULL`

`start_team` already tried to repair this into the enforced worker policy (`create_worktree` + inferred repo), but the repaired configuration still reused the old `workdir`. In affected deployments, the derived worker runtime root landed outside the configured safe-path allowlist, so runtime startup failed with `workdir not allowed`. The API path wrapped that into a generic `internal server error`.

## Changes

### Runtime repair

- Extend worker runtime repair to resolve `worktree_repo` from:
  - explicit member runtime hints
  - persisted agent config
  - repo inference from safe-path Git repositories plus member text hints
- Normalize repaired worker `workdir` so the derived worker runtime root stays under an allowed safe path when possible
- Fail with an explicit bad-request path when the worker runtime remains unrecoverable

### API behavior

- Route team runtime-start configuration failures through Team API bad-request mapping instead of leaking a generic 500

### Team create flow

- Preserve member runtime hints from Team create UI so new teams can carry worker repo/worktree context directly

## Validation

Local validation used:

- `cargo test teams_api_start_team_repairs_legacy_worker_runtime_from_prompt_hint -- --nocapture`
- `cargo test teams_api_start_team_returns_bad_request_for_unrecoverable_worker_runtime -- --nocapture`
- `cargo test default_actor_cli_path_resolves_existing_binary -- --nocapture`
- `cargo fmt --all`
- `cd web && npm run lint -- src/pages/team/create_helpers.ts src/pages/team/create_helpers.test.ts src/pages/team_page.tsx`

## Follow-up

- Verify the deployed domain path repairs legacy worker records during `Start Team` and now returns an explicit client error for unrecoverable worker runtime configs instead of a generic internal error.
