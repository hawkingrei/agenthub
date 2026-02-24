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

Executed (2026-02-18):

```bash
cargo test api::teams::tests::team_run_snapshot_api_returns_member_status_and_mailbox_summary -- --nocapture
cargo test api::teams::tests:: -- --nocapture
npm --prefix web run test -- src/pages/team_page.runs.test.ts
npm --prefix web run build
```
