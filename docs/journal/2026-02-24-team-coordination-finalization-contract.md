# 2026-02-24 Team Coordination And Finalization Contract

## Context

We reviewed additional planner/team reminder patterns and aligned AgentHub Team role guidance with clearer coordination and finalization behavior.

## Goal

- enforce stable teammate routing identity
- keep task status tracking current and auditable
- make run finalization mode-aware instead of one-size-fits-all shutdown

## Changes

1. Leader and worker skills now include routing key contract:
   - use `spec.members[].member_id` for teammate routing
   - avoid opaque runtime UUID/process identifiers in coordination artifacts

2. Added task status discipline to role skills:
   - status transitions must follow evidence
   - stale TODO entries should be compacted

3. Added run finalization policy to role skills:
   - persistent mode keeps team alive after response
   - one-shot/non-interactive mode requires graceful shutdown before final response

4. Synced canonical runtime spec with the same contracts.

5. Synced default Team leader prompt (Rust/Web) and added regression assertions.

6. Added TODO verification entry for coordination/finalization contract.

## Validation Plan

- `cargo test -p agenthub-team-prompts`
- `npm --prefix web run test -- src/pages/team/create_helpers.test.ts`

## Notes

This is guidance/prompt hardening. Runtime API behavior is unchanged.
