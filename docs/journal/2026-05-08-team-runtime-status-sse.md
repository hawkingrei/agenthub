# Team Runtime Status SSE

## Summary

Team runtime status now has a dedicated SSE invalidation channel. The web runtime watcher uses this stream to refresh the selected team's runtime cache on status changes and keeps interval polling only as a fallback while SSE is unavailable or reconnecting.

## Background

The Team workbench already had SSE paths for shared-thread messages, member ACP output, and run-context invalidation. Team runtime status still depended on a one-minute resume/poll loop, so member/runtime status could lag behind active Team execution.

## Scope

- Add a lightweight `/sse/teams/{team_id}/runtime` backend stream.
- Keep the SSE payload as an invalidation signal instead of duplicating the full runtime response schema.
- Update the Team runtime hook to disable interval polling while the SSE stream is connected.
- Preserve focus, visibility, online, and interval fallback behavior when SSE is unavailable.

## Key Decisions

- The backend compares a compact runtime fingerprint built from team status, member agent/session status, session id, and pending inbox count.
- The frontend refreshes the canonical runtime snapshot through the existing `getTeamRuntime` API after each SSE delta.
- The existing run-context SSE remains scoped to run snapshots/events/mailbox. Runtime status has its own channel because it can change without a run-context delta.

## Validation

Focused checks run before pushing:

```bash
cargo fmt --all --check
cd web && npm exec vitest -- run src/pages/team/use_team_runtime_effects.test.tsx src/api.test.ts
cd web && npm exec tsc -- --noEmit
```

`cargo test` was started after a local `cargo clean`, but it was still rebuilding dependencies when the rollout moved to CI-first validation.

## Follow-Ups

- Audit remaining non-runtime status surfaces, especially permissions and app-level agent list refresh, to decide whether they should remain fallback polling or move to dedicated SSE invalidation.

## Permission State Follow-Up

The app-level ACP permission state now uses existing agent SSE output as an invalidation source:

- ACP live output events with `permission_request`, `permission_response`, `permission_timeout`, or `permission_review_dispatch_error` trigger permission refresh for the affected agent ids.
- Pending permission counts, the active agent pending list, and debug permission history keep an initial load, then disable interval polling while the app-level agent SSE stream is connected.
- The old permission polling loops remain as fallback while SSE is unavailable or disconnected.

Focused checks run during this follow-up:

```bash
cd web && npm exec vitest -- run src/app_live_output.test.ts src/use_app_permissions.test.tsx
cd web && npm exec tsc -- --noEmit
```

Remaining audit work:

- Verify deployed runtime cards and app-level status surfaces on `agenthub.hawkingrei.com` after these PRs merge.

## Agent List Status Follow-Up

The app-level agent list/status surface now follows the same SSE-first shape:

- Initial agent list loading remains backed by `api.listAgents`.
- ACP `run_status` events consumed from app-level agent SSE continue to update the affected `agents[].status` entries in-place.
- The 10-second agent list polling loop is disabled while app-level agent SSE is connected.
- The polling loop remains as a fallback while SSE is unavailable or disconnected.

Focused checks run during this follow-up:

```bash
cd web && npm exec vitest -- run src/use_app_agents.test.tsx
cd web && npm exec tsc -- --noEmit
```

Remaining audit work:

- Verify deployed runtime cards, app-level agent status, and long-session status recovery on `agenthub.hawkingrei.com` after this PR merges.
