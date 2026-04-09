# Web Live Output Retention

## Summary

- reduced the in-memory live output retention window from `1200` to `600`
- reduced the in-memory live ACP output retention window from `1200` to `600`

## Why

The browser-facing live output window is only a convenience tail for the active page state. It does not need to retain the same depth as persisted history or explicit older-event pagination.

Cutting the live window in half lowers steady-state browser memory and render pressure for long-lived Agent and ACP sessions while preserving:

- explicit older-event loading
- persisted output cache slices
- session-scoped history switching

## Validation

- `cd web && npm run lint`
