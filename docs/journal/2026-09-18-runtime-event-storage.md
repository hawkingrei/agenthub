# Direct Runtime Event Storage

## Summary

The per-agent event database now provides atomic native event deduplication, history
associations, contiguous replay cursors and durable control-request receipts. This is
the storage foundation for slice 17. Managed request mapping, the durable event consumer
and live permission callbacks remain open; user input is still gated by the transport
integration. The full 18-slice objective is not complete.

## Background

Ordinary history insertion has no native event identity or replay transaction. Persisting
history and a cursor separately could lose an event after a crash or duplicate a tool
result during replay. A transport ACK also needs durable attribution independently from
the work it admits. The [direct integration contract](../features/rara-direct-integration.md)
keeps these facts separate from tasks, mailbox messages and activation outcomes.

## Scope

- Additive per-agent tables for runtime ownership, native stream cursors, event receipts,
  history associations and control receipts. Existing text/blob history remains readable.
- Explicit launch/runtime/session binding; a closed runtime cannot reopen for writes.
- Atomic event receipt, normalized history and cursor persistence, with bounded projections.
- Prepared/send/ACK/unknown states, single-use send permits, turn checks and receipt limits.
- No new transport launcher, transcript store, canonical task ledger or public endpoint.

## Key Decisions

- Deduplicate by runtime, owning native session and event ID, with a unique sequence and
  canonical event fingerprint. Do not infer ownership from optional event provenance.
- Commit only the next contiguous event. A missing position returns a replay requirement;
  a retained-prefix gap or provider rewind records incomplete delivery without moving
  the committed cursor. Repeated tool output cannot create duplicate history rows.
- Preserve receipts after transcript retention. Their history associations use cascading
  cleanup, preventing expired content from reappearing on replay.
- Flush send intent with SQLite FULL synchronization before issuing one send permit.
  Cancellation or a crash cannot create another permit for the same identity.
- Keep operator intent, transport admission and execution completion distinct. Closing
  marks unresolved sends unknown; a correlated late ACK may settle the receipt. Native
  session creation and stream ownership commit together without advancing event progress.
- Store only typed safe metadata in receipts. Conversation content is supplied through
  the existing history codec; request bodies and raw rejection text are not metadata.

## Validation

The final storage revision has 20 focused cases covering migration, reopen, concurrent
duplicates, single-use sending, transaction failure, retention, identity/content conflicts,
out-of-order replay, unavailable/rewound cursors, ACK attribution, pending-turn fencing,
receipt bounds and unknown outcomes. The full database suite reports 214 passing tests.
All-target database Clippy with warnings denied passes.

```bash
cargo test --locked --offline -p agenthub-db runtime_events:: -- --test-threads=1
cargo test --locked --offline -p agenthub-db -- --test-threads=1
cargo clippy --locked --offline -p agenthub-db --all-targets -- -D warnings
cargo fmt --all --check
git diff --check
```

No dependencies or Bazel configuration change; existing Rust source globs include the
new modules. Remote CI for the complete slice 17 PR remains a delivery gate.

## Follow-Ups

- Compute canonical native fingerprints and translate events into the existing history
  vocabulary before enabling managed user input.
- Connect request receipts to the transport, bounded replay consumption and existing
  live permission callbacks. Reconcile abandoned runtime owners only after supervisor
  evidence establishes their process lifetime has ended.
- Prove disconnect around ACK, stale permissions, output compatibility and a native
  process round trip, then publish/validate the complete slice 17 PR.
- Keep slice 18 activation identity, role/source binding, semantic outcomes and nested
  worker isolation separate. See [the transition TODO](../todo.md).
