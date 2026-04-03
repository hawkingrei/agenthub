# Team Permission Review Timeout Coordination

## Summary

- kept the default Team human fallback delay at 40 seconds
- increased the ACP permission review timeout to 120 seconds
- added an explicit coordination test so the default human fallback must remain shorter than the
  ACP-side timeout window

## Why

- using the same 40-second value for both timers created a race where ACP could mark the request as
  `timeout` exactly when the Team human fallback tried to post the shared review card
- the fallback path only posts when the permission request is still `pending`, so equal timers made
  the human card best-effort instead of reliable
- a larger ACP timeout preserves interactive review while still giving the human fallback time to
  appear first

## What Changed

- `crates/agenthub-acp/src/lib.rs`
  - changed `ACP_PERMISSION_REVIEW_TIMEOUT` from 40 seconds to 120 seconds
  - exposed `acp_permission_review_timeout()` for cross-crate coordination assertions
- `src/team/permission_review.rs`
  - added a regression test asserting the default Team human fallback delay stays below the ACP
    permission timeout
- `web/src/pages/team_mailbox_panel.tsx`
  - changed long mailbox-header metadata wrapping from `break-all` to `break-words` so actor IDs
    remain more readable while still avoiding overflow

## Validation

- `cargo test -p agenthub-acp permission_review_timeout_defaults_to_two_minutes -- --nocapture`
- `cargo test permission_review_dispatcher_default_human_fallback_is_forty_seconds --lib`
- `cargo test permission_review_human_fallback_stays_below_acp_timeout --lib`
- `cd web && npm run test -- src/pages/team_panels.test.tsx`
- `cd web && npm run build`
