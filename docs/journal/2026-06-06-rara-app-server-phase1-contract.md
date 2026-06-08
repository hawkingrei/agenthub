# Rara App-Server Phase 1 Contract

## Summary

- Tightened the Rara direct integration spec after PR review feedback on the merged phase-0 spec.
- Made `rara app-server --protocol-version 1 --transport stdio-jsonl` the concrete phase-1 command
  and transport shape.
- Added request ack, event replay/idempotency, and remote-node capability preflight requirements.

## Background

The initial Rara direct integration spec intentionally established the high-level boundary: AgentHub
should use Rara app-server/runtime-control directly and must not use `rara acp`, TUI, print, or wire
as the owned integration path.

Post-merge review identified four contracts that need to be normative before implementation starts:

- app-server command and wire framing;
- request acceptance and ack correlation;
- event replay/idempotency;
- remote-node capability checks.

## Scope

- `docs/features/rara-direct-integration.md`

## Key Decisions

- Phase 1 uses one child-process transport: UTF-8 JSON Lines over stdio.
- The first stdout frame must be the app-server handshake. AgentHub must not send runtime-control
  requests until the handshake is accepted.
- Every submitted `RuntimeControlEnvelope.request_id` must correlate with an accepted, queued, or
  rejected Rara response/event before AgentHub treats browser/API state as committed.
- AgentHub should persist Rara `event_id`, monotonic `sequence`, Rara thread/session id, and
  AgentHub `agent_sessions.id` for dedupe, replay, and diagnostics.
- Remote Rara placement must preflight compatible Rara app-server capability before creating or
  starting the remote runtime session.

## Validation

```bash
git diff --check
cargo fmt --check
```

## Follow-Ups

- Implement the phase-1 command/handshake in Rara before AgentHub depends on it.
- Add AgentHub provider-adapter tests for request ack, replay/idempotency, and remote-node capability
  preflight when implementation begins.
