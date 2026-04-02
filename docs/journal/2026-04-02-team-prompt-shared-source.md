# Team Prompt Shared Source

## Summary

- removed the duplicated frontend-owned Team leader/worker prompt bodies from
  `web/src/pages/team/member_helpers.ts`
- moved the canonical Team prompt text into crate-owned shared template files so both the Rust
  runtime and the web Team draft/preview surfaces now read the same source
- aligned the canonical prompt wording with the explicit mailbox receive contract:
  inbox inspection stays read-only, and accepted work is a separate action

## Scope

- `crates/agenthub-team-prompts/prompts/default_team_leader_prompt.txt`
- `crates/agenthub-team-prompts/prompts/default_team_worker_prompt.txt`
- `crates/agenthub-team-prompts/src/lib.rs`
- `web/src/pages/team/member_helpers.ts`
- `web/src/pages/team/member_helpers.test.ts`
- `web/src/vite-env.d.ts`
- `docs/todo.md`

## Key Changes

1. Introduced shared Team prompt templates under the owning Rust crate
   - `agenthub-team-prompts` now stores the leader and worker prompt bodies as tracked text
     templates instead of inline `concat!` fragments
   - Rust runtime prompt constants load those templates via `include_str!`
2. Switched the web Team prompt preview/draft defaults to the same shared templates
   - Vite raw-text imports now feed `DEFAULT_TEAM_LEADER_PROMPT` and
     `DEFAULT_TEAM_WORKER_PROMPT`
   - frontend prompt preview can no longer silently drift from the canonical runtime prompt text
3. Tightened prompt contract coverage
   - Rust tests assert the receive/accept mailbox wording and reject the old
     `pull inbox` / `acknowledge after reading` phrasing
   - web tests now check the canonical actor CLI verbs (`agenthub actor team-members`,
     `agenthub actor team-tasks`, `agenthub actor time-trigger-*`) instead of the previous
     frontend-only mirror wording

## Validation

- `cargo test -p agenthub-team-prompts`
- `cd web && npm test -- --run src/pages/team/member_helpers.test.ts src/pages/team/create_helpers.test.ts`
- `cd web && npm test -- --run src/pages/team/use_team_mailbox_actions.test.tsx src/pages/team_panels.test.tsx src/pages/team_page.smoke.test.tsx`

## Follow-up

- if Team prompts need further policy changes, update the shared crate-owned template files first
  and let both runtime and frontend consume the same text
