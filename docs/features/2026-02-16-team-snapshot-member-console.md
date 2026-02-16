# Team Snapshot And Member Console

## Background

The standalone `/teams` workbench exists, but team-level observability is fragmented:

- Team create is JSON-first and does not provide a guided leader/worker authoring flow.
- The UI fetches run/events/steps/mailbox separately, which makes member status and mailbox
  overview harder to reason about.
- There is no direct member-centric output console to inspect worker/leader session output.

## Scope

- Extend Team spec validation to support richer member metadata:
  - `leader_member_id`
  - `spec.members[].role` (`leader` / `worker`)
  - `spec.members[].model`
  - `spec.members[].prompt`
  - `spec.members[].skills`
- Add Team run snapshot API:
  - `GET /api/teams/runs/{run_id}/snapshot`
  - Returns run/team/steps/latest events/mailbox summary/member snapshots in one payload.
- Expose snapshot API in OpenAPI and Team API client wrappers.
- Update `/teams` page:
  - Guided create-team form for leader + workers (with model/prompt/skills)
  - Output tabs: `Overview`, `Events`, `Steps`, `Mailbox`, `Member Console`
  - Member Console fetches agent event stream for selected member session.

## Key Decisions

- Keep compatibility with existing orchestrator bootstrap:
  - `entrypoint` + `members` remain valid as before.
  - Rich member fields are additive and validated for shape/consistency.
- Use a dedicated snapshot endpoint to reduce front-end fan-out and align the UI around
  one server-side aggregated run view.
- Keep Team page as an independent workspace (`/teams`) and avoid cross-coupling with
  Agent workspace state.

## Validation

Suggested checks:

```bash
cargo test team_run_snapshot_api_returns_member_status_and_mailbox_summary -- --nocapture
cargo test teams_router_http_contract -- --nocapture
cargo test openapi_json_contains_team_runs_list_path -- --nocapture
npm --prefix web run lint
npm --prefix web run build
```

Manual checks:

1. Create a team using leader/worker form fields and confirm generated spec is accepted.
2. Create a run and verify `Overview` shows member status/mailbox counters.
3. Send mailbox messages and verify `Mailbox` tab summary and list update.
4. Select a member with an active session and verify `Member Console` shows agent events.
