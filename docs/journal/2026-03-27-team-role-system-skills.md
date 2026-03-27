# Team role-system skills

## Summary

This change removes Team member skill configuration from the public Team contract and treats Team
skills as system-managed role defaults. Member/card views now expose effective role-derived skills
instead of user-authored `skills` lists.

## Scope

- `src/team/role_skills.rs`
- `crates/agenthub-acp/src/team_role_skills.rs`
- `src/api/teams.rs`
- `src/api/agents.rs`
- `src/team/manager.rs`
- `web/src/pages/team/*.ts`
- `web/src/pages/team_page.tsx`
- `crates/agenthub-team-prompts/src/lib.rs`

## Decisions

1. Team role skills are mandatory system defaults derived from role:
   - leader:
     - `agenthub-actor-runtime`
     - `team-agents-index`
     - `team-leader-agents-index`
     - `team-leader-orchestrator`
     - `team-actor-mailbox`
   - worker:
     - `agenthub-actor-runtime`
     - `team-agents-index`
     - `team-worker-agents-index`
     - `team-worker-executor`
     - `team-actor-mailbox`
2. Team member create/edit flows no longer persist `spec.members[].skills`.
3. Team runtime actor context no longer consumes member-configured skill lists.
4. Discovery-card and snapshot views expose effective role-derived skills for operator visibility.
5. `profile_patch_proposal` now updates only prompt/description identity fields; `skills_add` is
   rejected because Team skills are system-managed from role.

## Validation

Executed:

```bash
cargo fmt --all
cargo check -p agenthub
cargo test -p agenthub-acp build_team_role_skills -- --nocapture
cargo test -p agenthub teams_api_strips_member_skill_configuration_from_team_spec -- --nocapture
cargo test -p agenthub discovery_card_route_exposes_agent_capabilities -- --nocapture
cargo test -p agenthub team_run_messages_profile_patch_proposal_updates_team_spec_and_is_idempotent -- --nocapture
cargo test -p agenthub team_run_messages_profile_patch_proposal_updates_run_overrides_and_snapshot_view -- --nocapture
cargo test -p agenthub team_run_messages_profile_patch_proposal_rejects_skills_add -- --nocapture
cd web && npx vitest run src/pages/team/create_helpers.test.ts src/pages/team/member_helpers.test.ts src/pages/team_page.runs.test.ts src/pages/team_panels.test.tsx src/pages/team/mailbox_helpers.test.ts
git diff --check
```

Observed:

- Team role skill injection stayed role-bound and deterministic.
- Team spec normalization strips member `skills` configuration.
- Discovery cards and runtime snapshots show effective system-managed role skills.
- Team profile patch proposals still support prompt/description updates but reject `skills_add`.
