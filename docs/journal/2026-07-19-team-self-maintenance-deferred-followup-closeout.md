# Team Self-Maintenance Deferred Follow-Up Closeout

## Summary

The Team self-maintenance and deferred follow-up TODO is closed. Current local coverage proves the
three intended flows operate independently: `profile_patch_proposal` updates member profile scope,
`agent_time_trigger_*` creates/lists/cancels one-shot reminders, and operator-controlled
`agent_loop` can be configured without blocking normal Team profile progress.

## Background

The stable contracts in `docs/features/agents-teams.md` and
`docs/features/teams-collaboration-playbook.md` already define the intended boundaries:
self-authored profile changes are limited to identity/prompt fields, timed triggers are reminders
rather than task tracking, and `agent_loop` remains externally configured and disabled by default.
This checkpoint records the current tests that make the TODO verifiable.

## Scope

- Close the TODO for local behavior consistency across self-maintenance and deferred follow-up
  flows.
- Keep remote/multi-node mailbox routing verification separate.
- Do not change runtime behavior in this checkpoint.

## Key Decisions

- `profile_patch_proposal` supports durable Team spec updates and run-scoped overrides, stays
  idempotent, and rejects member-authored skill changes.
- `agent_time_trigger_*` remains a one-shot reminder control plane with create/list/cancel wire
  compatibility.
- `agent_loop` remains operator-controlled. UI profile saves are allowed to complete even when the
  later loop configuration call fails, so loop-control errors do not block normal Team profile
  progress.

## Validation

```bash
cargo test team_run_messages_profile_patch_proposal -- --nocapture
cargo test internal_grpc_time_trigger_controls_are_wire_compatible -- --nocapture
cd web && npm exec vitest -- run src/pages/team_page.agent_loop.test.tsx
```

## Follow-Ups

- Real multi-node direct mailbox routing remains tracked separately in `docs/todo.md`.
