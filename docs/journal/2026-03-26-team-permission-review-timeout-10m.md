# Team Permission Review Timeout To Ten Minutes

## Summary

- Extended the default ACP permission review timeout from 5 minutes to 10 minutes.
- Extended the default Team human fallback delay to match the new 10-minute review window.

## Why

- Five minutes is still short for real Team review flows, especially when the assigned reviewer is
  active but not continuously watching the ACP stream.
- Keeping the ACP-side timeout and the Team human-fallback delay aligned avoids mixed behavior
  where one side expires before the other.

## What Changed

- `agenthub-acp` now waits `Duration::from_secs(600)` for permission review responses before
  marking the request as timed out.
- `TeamPermissionReviewDispatcherSettings::default().human_fallback_delay` now defaults to
  `Duration::from_secs(600)`.
- The Team permission-review default-delay unit test now locks the default to 10 minutes.
- Active verification backlog text now refers to the 10-minute review window.

## Validation

- `cargo test permission_review_dispatcher_default_human_fallback_is_ten_minutes -- --nocapture`
