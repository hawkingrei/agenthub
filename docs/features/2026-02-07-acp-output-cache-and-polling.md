# ACP Output Cache And Polling Adjustments

## Background

We observed intermittent Output blanking/jumps. Polling cadence for events and permissions also needed clearer control.

## Scope

- Separate ACP event cache from general output cache to prevent ACP history from being evicted by non-ACP output.
- Event polling enters a 1s boost window after the user sends input.
- Permission polling slows to 5s when pending exists; after responding it briefly speeds up to 1s.
- Agent status adds `idle` as active; thinking is shown only when status is `running`.

## Key Decisions

- Build the ACP view only from the ACP cache to keep the conversation stable.
- Polling favors user actions and backs off when idle to avoid unnecessary refresh.

## Validation

```bash
cd web && npm test
```

## Follow-ups

- Confirm ACP events are no longer dropped unexpectedly.
- If backend adds more `run_status` values, update idle/active rules accordingly.
