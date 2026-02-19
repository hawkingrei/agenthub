# Agent Unexpected Exit Alert

## Background

Users can observe long periods without useful feedback after an agent process exits unexpectedly.
Even when backend status is updated to `failed`/`exited`, web UI did not provide a direct alert tied to that transition.

## Scope

- Add explicit UI alert for unexpected active-agent exits.
- Keep normal stop flow (`stopped`) quiet to avoid false alarms.

## Key Decisions

1. Define unexpected exit as status `failed` or `exited`.
2. Show alert only on active status transition:
   - previous status is active (`running`/`idle`)
   - next status is unexpected (`failed`/`exited`)
3. Do not alert for `running/idle -> stopped` because that is commonly user-initiated.

## Implementation

- `web/src/agent_ws.ts`
  - Add:
    - `isAgentUnexpectedExitStatus(status)`
    - `shouldShowUnexpectedExitNotice(previousStatus, nextStatus)`
- `web/src/agent_ws.test.ts`
  - Add status helper coverage for unexpected-exit detection and transition gating.
- `web/src/app.tsx`
  - Track per-agent previous status in `activeAgentPrevStatusRef`.
  - When active agent transitions from active to unexpected-exit status, set explicit error banner:
    - `Agent process exited unexpectedly (status: <status>). Please restart the agent.`

## Validation

Executed (2026-02-19):

- `npm --prefix web run test -- src/agent_ws.test.ts src/app.permission_scope.test.ts`

## Follow-up

- Validate in a real browser by forcing agent process termination and confirming:
  - alert appears once per transition,
  - restart clears the stale state path,
  - normal manual stop does not trigger alert.
