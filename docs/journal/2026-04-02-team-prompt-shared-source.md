# Team Prompt Shared Source

## Summary

- removed the duplicated frontend-owned Team leader/worker prompt bodies from
  `web/src/pages/team/member_helpers.ts`
- kept the canonical Team prompt text in the Rust-owned `agenthub-team-prompts` crate
- switched the web Team draft defaults from frontend file imports to an authenticated API fetch
  (`GET /api/teams/prompt_defaults`) so the backend remains the only prompt source of truth
- aligned the canonical prompt wording with the explicit mailbox receive contract:
  inbox inspection stays read-only, and accepted work is a separate action

## Scope

- `crates/agenthub-team-prompts/prompts/default_team_leader_prompt.txt`
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
- `crates/agenthub-team-prompts/src/lib.rs`
- `src/api/teams.rs`
- `src/api/openapi/spec.rs`
- `web/src/pages/team/member_helpers.ts`
- `web/src/pages/team/create_helpers.ts`
- `web/src/pages/team/forge_helpers.ts`
- `web/src/pages/team_page.tsx`
- `web/src/api.ts`
- `web/src/pages/team/member_helpers.test.ts`
- `web/src/pages/team/create_helpers.test.ts`
- `web/src/pages/team/forge_helpers.test.ts`
- `web/src/pages/team_page.smoke.test.tsx`

## Key Changes

1. Introduced shared Team prompt templates under the owning Rust crate
   - `agenthub-team-prompts` now stores the leader and worker prompt bodies as tracked text
     templates instead of inline `concat!` fragments
   - Rust runtime prompt constants load those templates via `include_str!`
2. Switched the web Team prompt preview/draft defaults to backend-fetched prompt defaults
   - the frontend no longer imports prompt template files from the repo at build/dev time
   - `GET /api/teams/prompt_defaults` now returns the leader/worker prompt bodies derived from
     the canonical crate-owned templates
   - Team create/forge/edit flows hydrate blank prompt fields from the API payload at runtime
3. Tightened prompt contract coverage
   - Rust tests assert the receive/accept mailbox wording and reject the old
     `pull inbox` / `acknowledge after reading` phrasing
   - router/OpenAPI tests now pin the authenticated prompt-defaults endpoint contract
   - web tests now verify role-based prompt fallback via API-provided defaults instead of
     frontend-owned inline strings

## Validation

- `cargo test -p agenthub-team-prompts`
- `cargo test teams_router_http_contract --lib`
- `cargo test openapi_json_contains_team_runs_list_path --lib`
- `cd web && npm test -- --run src/pages/team/member_helpers.test.ts src/pages/team/create_helpers.test.ts src/pages/team/forge_helpers.test.ts src/pages/team/state.test.ts src/pages/team/use_team_mailbox_actions.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`
- `cd web && npm run build`

## Follow-up

- if Team prompts need further policy changes, update the shared crate-owned template files first
  and let the backend prompt-defaults endpoint expose the new text to the frontend
