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
