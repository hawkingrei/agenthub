## Summary

- record merge verification evidence for the recent actor/internal gRPC/Codex ACP runtime changes
- remove the corresponding completed verification items from `docs/todo.md`
- keep the still-open Codex ACP multi-agent config knob verification item in place because the latest default-branch push is not fully settled yet

## Updated Journals

- `docs/journal/2026-03-30-clap-root-cli-refactor.md`
- `docs/journal/2026-03-30-codex-acp-apply-patch-deadlock.md`
- `docs/journal/2026-03-30-actor-ack-status-changed.md`
- `docs/journal/2026-03-30-internal-grpc-fail-fast-startup.md`
- `docs/journal/2026-03-30-actor-internal-grpc-deadline.md`
- `docs/journal/2026-03-30-internal-grpc-service-narrow-deps.md`
- `docs/journal/2026-03-29-actor-cli-run-scope-ux.md`
- `docs/journal/2026-03-29-actor-inbox-shared-thread-run-fallback.md`

## Closed TODO Items

- staged clap root CLI refactor
- codex-acp apply-patch deadlock fix
- actor ack status-change diagnostics
- internal gRPC fail-fast startup
- actor internal gRPC deadline guard
- internal gRPC narrow-deps follow-up
- issue `#244` mailbox run-scope UX

## Validation

- `git diff --check`
