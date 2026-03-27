# Actor inbox snapshot consistency

## Summary

- Changed Team `actor inbox` reads to load `pending_count` and inbox rows from the same SQLite read transaction instead of two independent queries.
- Added a focused mailbox test that documents the existing cursor semantics: `pending_count` is total unread pending mail, while `messages` is only the current page after `cursor` filtering.

## Why

- In production a short-lived race could return `messages = []` together with `pending_count = 1` for a default inbox poll, which made the queue look empty even though one unread message still existed.
- The old implementation counted pending rows first and listed inbox rows second, without a shared snapshot.
- The same response shape can still happen intentionally when the caller passes a cursor that filters all current-page rows, so that behavior now has an explicit regression test and should not be confused with the snapshot bug.

## Validation

- `cargo fmt --all -- src/team/manager/mailbox.rs src/team/manager.rs src/team/manager/tests.rs`
- `cargo test actor_mailbox_service_`
- `git diff --check`
