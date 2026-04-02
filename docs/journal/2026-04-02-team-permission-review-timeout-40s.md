# Team Permission Review Timeout To Forty Seconds

## Summary

- Reduced the ACP permission review timeout from 10 minutes to 40 seconds.
- Reduced the default Team human fallback delay to the same 40-second window so agent-first and
  human-fallback behavior stay aligned.

## Why

- The 10-minute window is too long for interactive permission review flows and makes a stalled
  review feel like the command is hanging.
- Keeping the ACP-side timeout and the Team human fallback delay aligned avoids a split state where
  the requester times out long before the shared human fallback appears.

## What Changed

- `agenthub-acp` now waits `Duration::from_secs(40)` for permission review responses before
  applying the timeout fallback.
- `TeamPermissionReviewDispatcherSettings::default().human_fallback_delay` now defaults to
  `Duration::from_secs(40)`.
- Focused default-value tests now lock both timeouts to 40 seconds.
- The active verification backlog text now refers to the 40-second review window.

## Validation

- `cargo test -p agenthub-acp permission_review_timeout_defaults_to_forty_seconds -- --nocapture`
- `cargo test -p agenthub permission_review_dispatcher_default_human_fallback_is_forty_seconds -- --nocapture`
