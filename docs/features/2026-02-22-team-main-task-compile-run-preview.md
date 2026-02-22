# Team Main Task Compile Run Preview

## Background

The chat-first Team workflow requires a deterministic bridge from negotiation
artifacts (`main_task` + conversation messages) to execution artifacts (`team
run` payload). Before this change, Team APIs could persist negotiation history,
but there was no explicit compile step that produced a stable run payload for
leader confirmation before run start.

## Scope

- Add a new Team API endpoint for compile preview:
  - `POST /api/teams/:id/main_tasks/:main_task_id/compile_run_preview`
- Compile deterministic run payload preview from:
  - team spec (`members`, `steps`, leader role)
  - main-task context
  - ordered conversation message updates
- Emit compile outputs that include:
  - fixed step template (`step_key`, `member_id`, `role`, `depends_on`)
  - role-bound assignment mapping (`member_id` -> `step_keys`)
  - task list, acceptance criteria, optional deadline
  - `run_payload` (`context_id` + `input`) compatible with run creation API
- Keep compile stage side-effect free (no run is created by preview endpoint).

## Key Decisions

1. Keep compile deterministic and replayable:
   - default `context_id` to `main_task_id` unless caller overrides it.
   - avoid non-deterministic fields (no compile timestamp/random IDs).
2. Keep compile extraction robust for mixed chat payloads:
   - parse plan updates from `payload.plan_update` first.
   - fallback to top-level payload object keys when present.
   - ignore unrelated payload keys instead of failing compile.
3. Keep assignment semantics role-bound and explicit:
   - leader role is derived from `leader_member_id` (or role fallback).
   - assignment output always includes every member (empty `step_keys` allowed).
4. Keep run start as a separate explicit action:
   - compile preview returns a payload for confirmation.
   - existing run creation endpoint remains unchanged.

## Validation

Executed locally:

```bash
cargo test -q team_main_task_compile_preview_builds_deterministic_role_bound_payload -- --nocapture
cargo test -q team_main_task_messages_api_supports_route_and_redaction -- --nocapture
cargo test -q teams_router_http_contract -- --nocapture
cargo test -q team_main_task -- --nocapture
```

All passed.

## Follow-up

- Wire Team UI chat flow to call `compile_run_preview` before submitting run
  start.
- Add Playwright E2E for `main task -> compile preview -> run start`.
- Add optional compile-to-run execute endpoint if product wants a one-click
  confirmed transition.
