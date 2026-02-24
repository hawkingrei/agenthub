# Team Mailbox Structured Templates And Clarification Loop

## Summary

Add structured mailbox payload templates to the Team Workbench so leaders and workers
can use a deterministic message contract for assignment, clarification, status reports,
and profile patch proposals.

## Background

Team runs already expose mailbox send/inbox/ack APIs, but authoring payload JSON by hand
causes drift and slows collaboration between leader and worker roles. We need a default
contract that is easy to apply in UI and is aligned with backend prompt defaults.

## Scope

- `web/src/pages/team_page.tsx`
- `web/src/pages/team_page.runs.test.ts`
- `src/team/manager.rs`
- `src/team/manager/mailbox.rs`
- `crates/agenthub-team-actor/src/mailbox.rs`
- `src/api/teams.rs`
- `src/api/teams/tests_core.rs`
- `docs/todo.md`

## Key Decisions

1. Add typed mailbox template keys and a reusable payload builder in Team page helpers.
2. Add a template selector + `Apply Template` action in the Mailbox send panel.
3. Keep payload editable after template apply so operators can adjust fields per run.
4. Align backend default leader/worker prompts with the same structured payload contracts.
5. Auto-apply `profile_patch_proposal` after message send when the mailbox message is newly created
   (idempotent retries with same key do not re-apply patch).
6. Support two targets:
   - `team`: patch persisted into `team_definitions.spec`
   - `run`: patch persisted into `team_runs.input.profile_overrides.members`
7. Emit `profile_patch_applied` run events with before/after payload snapshots for audit replay.
8. Overlay run-level profile overrides into snapshot member prompt/skills rendering.
9. Add API tests for `target=team` and `target=run`, plus idempotency behavior.

## Validation

```bash
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
cargo test --test web_assets styles_keep_acp_conversation_scoped -- --nocapture
cargo test team_run_messages_profile_patch_proposal_ -- --nocapture
cargo test team_run_messages_api_supports_idempotency_key -- --nocapture
cargo test team_run_snapshot_api_returns_member_status_and_mailbox_summary -- --nocapture
```
