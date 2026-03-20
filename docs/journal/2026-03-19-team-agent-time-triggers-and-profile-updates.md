# Team Agent Time Triggers And Profile Updates

## Summary

This change closes two Team runtime gaps:

1. agents can now self-maintain their own member/profile metadata through
   `profile_patch_proposal`, including durable `description` updates; and
2. agents can create one-shot time triggers that later inject ACP prompts back
   into the same agent session.

The goal is to reduce operator-only maintenance for Team member cards and to
support deferred follow-up work without requiring a human to stay in the loop.

## Backend

- Extended `profile_patch_proposal` handling to accept `description` in both
  Team-target and run-target updates.
- Applied `description` patches to:
  - `spec.members[].description` for durable Team identity-card updates
  - `run.input.profile_overrides.members.*.description` for run-scoped
    temporary overrides
- Included before/after `description` fields in
  `profile_patch_applied` event payloads.
- Added `agent_time_triggers` persistence, claim/requeue/fired lifecycle, and a
  background dispatcher that sends a future ACP prompt back into the agent via
  existing `send_input`.

## Agent Tools

- Added actor MCP tools:
  - `agent_time_trigger_set`
  - `agent_time_trigger_list`
  - `agent_time_trigger_cancel`
- Kept the trigger model intentionally narrow for phase 1:
  - one-shot only
  - delay-based scheduling
  - trigger replays as ACP prompt text

## Prompt / Skill Contract

- Updated Team leader/worker default prompts to state that:
  - human channel input remains free-form
  - agents may self-update their own profile/description via
    `profile_patch_proposal`
  - agents may create time-based self-reminders through
    `agent_time_trigger_set/list/cancel`
- Updated Team AGENTS/skills docs so these capabilities appear in the shared
  contract instead of being hidden in backend-only implementation details.

## Validation

- `cargo test team_run_messages_profile_patch_proposal_updates_team_spec_and_is_idempotent`
- `cargo test team_run_messages_profile_patch_proposal_updates_run_overrides_and_snapshot_view`
- `cargo test agent_time_trigger_tools_roundtrip`
- `cargo test actor_tools_exposes_expected_tool_names`
- `cargo test agent_time_trigger_routes_create_list_and_cancel`
- `cargo test time_trigger_worker_dispatches_due_trigger_once`
- `cargo test time_trigger_worker_requeues_failed_delivery`
- `cd web && npx vitest run src/pages/team/mailbox_helpers.test.ts`
- `cd web && npm run lint -- src/pages/team/member_helpers.ts src/pages/team/mailbox_helpers.ts src/pages/team/mailbox_helpers.test.ts`

## Follow-up

- Decide whether standalone `/agents` UI should expose a first-class trigger
  panel or stay MCP/API-only for phase 1.
- Decide whether repeating/cron-like triggers belong in the same table/model or
  should wait for a separate scheduler abstraction.
