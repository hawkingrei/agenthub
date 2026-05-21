# Summary

This slice closed the most immediate `task-first Team model` drift and landed the first concrete
`Team mailbox phase 3` implementation step.

- `task-first`: canonical create paths now have explicit regression coverage for required
  `priority` and `assigned_member_id`, and router-level task reads no longer rely on a legacy
  ownerless seed path when validating the main Team task contract.
- `mailbox phase 3`: actor mailbox payloads now project canonical inbound-envelope metadata for the
  main human/system intake paths, so later reply-obligation/runtime work can build on one stable
  shape instead of re-deriving source intent from ad-hoc payload variants.
- `mailbox phase 3`: run snapshots and Team mailbox/overview surfaces now expose open
  reply-obligation counts and obligation details derived from those canonical envelope semantics.
- `mailbox phase 3`: Team mailbox now exposes a minimal operator resolution path that can triage
  an unresolved reply obligation to `ignored` or `completed` and immediately clear unread/open
  summary state.

# Background

The current Team backlog still tracks two related gaps:

1. verify the task-first Team model end to end
2. finish Team mailbox phase 3

Recent Team work already tightened canonical task creation and mailbox triage/ownership, but the
remaining drift was visible in two places:

- some tests still validated Team task reads through a legacy bootstrap helper that left
  `assigned_member_id = NULL`, even though canonical Kanban task creation now requires an explicit
  owner
- mailbox payloads still lacked one canonical envelope projection for `source_kind`,
  `source_surface`, `reply_target`, and `requires_user_visible_reply`

# Scope

This slice intentionally stayed narrow:

- add focused regression coverage for task-first canonical create requirements
- align router contract coverage with canonical task seeding
- add a reusable mailbox envelope projection helper
- normalize human conversation/thread mailbox forwards and permission-review system notices onto
  the same envelope metadata shape

It did not yet attempt to:

- add final database columns for every envelope field
- enforce the full `requires_user_visible_reply` runtime invariant with explicit transfer/reply
  state transitions
- add explicit transfer/escalation/manual-takeover flows beyond the minimal `ignored` /
  `completed` resolution path

# Key Decisions

## 1. Keep `TeamManager::create_task(...)` as a compatibility helper for now

`create_task(...)` is still used by tests and narrow bootstrap/setup paths. This slice did not try
to delete or hard-retire it. Instead, it now carries an explicit boundary note that canonical
Kanban task authoring should use `create_task_with_metadata(...)`, where explicit owner and
priority remain visible at the call site.

## 2. Lock task-first drift with tests before changing more runtime code

The most useful task-first work here was not another behavior change but making the post-change
truth executable:

- CLI `team-task-create` requires `priority`
- CLI `team-task-create` requires `assigned_member_id`
- internal gRPC `CreateTeamTask` rejects missing `assigned_member_id`
- router-level Team task reads use canonical seeded tasks instead of an ownerless helper path

## 3. Start mailbox phase 3 with projection and normalization, not a full schema migration

Adding new top-level actor message fields immediately would have forced a large cross-crate update
through many tests, relay fixtures, and proto/client constructors. For the first phase-3 slice,
the better tradeoff was:

- add a canonical envelope projection helper in `agenthub-team-actor`
- normalize known send paths so the canonical metadata appears in payloads consistently
- leave full persistence-layer reshaping for a later slice

This keeps the new semantics available to runtime and UI code without forcing a broad migration in
the same change.

## 4. Treat human conversation/thread mailbox forwards as reply-obligation candidates now

Human-originated Team conversation and thread messages forwarded into mailbox now explicitly carry:

- `source_kind = human`
- `source_surface = conversation | thread`
- `reply_target = ...`
- `requires_user_visible_reply = true`

Permission-review system notices now explicitly carry:

- `source_kind = system`
- `source_surface = system`
- `requires_user_visible_reply = false`

## 5. Start operator-visible reply-obligation surfacing with conservative pair-level credits

This slice still does not have first-class persisted reply-resolution links. Instead, snapshot/UI
surfacing now uses a conservative heuristic and projects read-only obligation details:

- human-originated mailbox messages marked `requires_user_visible_reply = true` stay open unless
  they are explicitly `ignored` / `completed`
- later visible `agent -> human` chat replies consume one earlier open obligation for the same
  actor pair

This is intentionally conservative and reviewable. It gives operators immediate visibility without
locking phase 3 into a broad schema migration before the end-state contract is fully settled.

## 6. Add a minimal operator resolution path before full transfer/takeover flows

This slice now also exposes a narrow but useful action path:

- Team mailbox can triage the underlying mailbox item to:
  - `watching`
  - `claimed`
  - `released`
  - `ignored`
  - `completed`
- the HTTP surface reuses actor-mailbox triage rather than inventing a second resolution API
- `ignored` immediately removes the item from unread-actionable inbox views and from run-snapshot
  reply-obligation summaries
- `completed` is now guarded by visible-reply evidence; unresolved reply-obligation rows do not
  offer `completed` directly because that would violate the runtime invariant
- `watching` / `claimed` / `released` remain operator-visible mailbox states so takeover intent
  and current topic ownership are visible in snapshot/UI without pretending the human reply
  obligation is satisfied

This is intentionally smaller than the final phase-3 target. It does not yet model:

- explicit user-visible reply evidence links
- transfer with preserved reply responsibility
- escalation to coordinator/human as a first-class obligation outcome
- cross-actor takeover beyond the current recipient actor's `watching` / `claimed` / `released`
  state

# Validation

The following focused checks were run for this slice:

```bash
cargo fmt --all
cargo test -p agenthub parse_team_task_create_requires_assigned_member_id -- --nocapture
cargo test -p agenthub internal_grpc_team_task_create_requires_assigned_member_id -- --nocapture
cargo test -p agenthub teams_router_http_contract -- --nocapture
cargo test -p agenthub-team-actor -- --nocapture
cargo test -p agenthub team_thread_reply_api_notifies_existing_thread_participants -- --nocapture
cargo test -p agenthub team_task_messages_api_forwards_shared_thread_human_chat_without_active_run -- --nocapture
cargo test -p agenthub team_task_messages_api_infers_direct_route_for_single_mention_and_normalizes_detail_ref -- --nocapture
cargo check -p agenthub --tests
cd web && npm exec tsc -- --noEmit
cd web && npm exec vitest -- run src/pages/team/use_team_mailbox_actions.test.tsx src/pages/team_panels.test.tsx
cargo test -p agenthub team_run_messages_api_triage_rejects_completed_without_visible_reply -- --nocapture
cargo test -p agenthub team_run_messages_api_triage_resolves_open_reply_obligation -- --nocapture
cargo test -p agenthub team_run_messages_api_triage_surfaces_takeover_state -- --nocapture
```

# Follow-Ups

- Finish the remaining `task-first Team model` verification items that still depend on broader
  prompt/docs/runtime evidence, then remove the `P0` backlog item from `docs/todo.md`.
- Build the next mailbox phase-3 slice on top of the new envelope projection:
  - add explicit escalation/transfer outcomes that satisfy reply-required work without relying on
    `ignored`
  - add cross-actor-takeover outcomes on top of the current recipient-scoped triage-state surface
- Decide whether the envelope projection should remain payload-backed or be promoted into explicit
  stored actor-message columns once the runtime/UI contract stabilizes.
