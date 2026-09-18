# Direct Runtime Event Storage

## Summary

The per-agent event database now provides atomic native event deduplication, history
associations, contiguous replay cursors and durable control-request receipts. This is
the storage foundation for slice 17. Typed projection and the managed durable event
consumer, managed text input, fenced browser answers, live permission callbacks and turn
cancellation are integrated. Recovery visibility remains open. The full 18-slice objective is not complete.

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

## Typed Projection Checkpoint

The protocol crate now maps typed controls and native events from the pinned upstream
revision. It adds canonical SHA-256 fingerprints, conversation/tool/plan/question history,
post-commit state effects, and allowlisted diagnostic observations. Open tool calls retain
their card identity across approval answer turns, while reused completed call IDs remain
separate. Question metadata retains the native waiting-turn fence; the input checkpoint
below connects it to browser submission and managed validation.

Validation records for this checkpoint report 34 protocol tests passing, one opt-in native
process test excluded, and all-target Clippy with warnings denied passing. Cases cover
wire methods and encoded limits, canonical digests, chunk rollback, tool identity, stale
answers, approval notices, snapshot ownership, semantic outcomes and secret redaction.
The existing workspace SHA-256 dependency is now used by the protocol crate; the lockfile
adds only that crate's dependency edge. No Bazel configuration changes are required.

```bash
cargo test --locked --offline -p agenthub-rara
cargo clippy --locked --offline -p agenthub-rara --all-targets -- -D warnings
```

## Follow-Ups

- Add durable receipt/cursor recovery visibility. Reconcile abandoned runtime owners only after supervisor
  evidence establishes their process lifetime has ended.
- Prove a native prompt/approval process round trip, then publish/validate the complete
  slice 17 PR. Fake-peer checks cover disconnect around ACK and stale permissions.
- Keep slice 18 activation identity, role/source binding, semantic outcomes and nested
  worker isolation separate. See [the transition TODO](../todo.md).

## Managed Consumer Checkpoint

Startup now durably records native session creation before accepting its events. The
consumer bounds reordering to 256 events and 8 MiB, requests live replay from the
committed cursor, and installs projection state only after history commits. Repeated
events produce no new history or effects. Explicit gaps preserve the cursor and fail
the owned launch. The existing supervisor still owns process cleanup; semantic exit
also waits for event drain. Control receipt tasks retain ownership through caller
disconnects, and close converts unresolved sends to unknown outcomes without retry.

Validation records report 13 focused consumer/managed-process cases passing, a separate
round trip against the pinned native binary passing, 128 manager regressions passing
with six fixture/opt-in tests excluded, 34 protocol cases passing, and root library/test
Clippy with warnings denied passing. The initial manager run had 12 local-listener
permission failures in the sandbox; rerunning with local test networking allowed
resolved all 12 without implementation changes.

```bash
cargo test --locked --offline -p agenthub --lib agent::manager:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::rara::tests::managed_native_process_transport -- --ignored --exact
cargo clippy --locked --offline -p agenthub --lib --tests -- -D warnings
```

The native check requires `AGENTHUB_RARA_TEST_BINARY` built from the pinned revision.
It creates a session and consumes initial events without issuing a paid model request.
The upstream prerequisite PR #885 was merged into its main branch on 2026-09-17;
the downstream prerequisite PR #1163 was merged into its dependency branch on
2026-09-18. Neither merge implies that the complete downstream stack is in main.

## Managed Input And Browser Checkpoint

Prompt, follow-up and explicit user answers now persist one attempted user message and
prepared receipt atomically. Caller message IDs are single-use request identities. Owned
background tasks complete ACK persistence after caller disconnects; ambiguous transport
failure stays `outcome_unknown`. Native question answers require their original runtime,
native session and waiting turn, and cannot be silently redirected after session mismatch.

The web projection matches receipts to their owned user messages even when receipt/history
pages arrive out of order. Identical text with distinct native request IDs remains distinct.
User-message conversion and bubble props now preserve delivery status. Malformed native
question targets disable submission; existing ACP submissions retain their callback shape.

Validation records: 22 focused database storage cases and database Clippy passed; 17 focused
managed/consumer cases passed (one opt-in native case excluded); 132 manager regressions
passed (six excluded); root library/test Clippy passed. The web suite passed 1,588 tests
across 171 files, followed by 115 focused rendering/input tests after the final user-bubble
prop fix. Type checking, lint and production build passed on that final web revision.

```bash
cargo test --offline --locked -p agenthub-db runtime_events:: -- --test-threads=1
cargo test --offline --locked -p agenthub --lib agent::manager:: -- --test-threads=1
cargo clippy --offline --locked -p agenthub --lib --tests -- -D warnings
cd web
npx tsc --noEmit
npm test -- --maxWorkers=2
npx vitest run src/native_input.test.ts src/components/native_input.test.tsx src/components/use_agents_workbench_panel.test.tsx src/acp_conversation.interaction.test.tsx src/acp_conversation_render.test.tsx src/pages/team_member_acp_panel.test.tsx
npm run lint
npm run build
```

Chrome DevTools MCP inspected the actual member thread page with isolated synthetic API
data at `/workspace/teams/team-native-input/members/agent-worker-1/thread`, using local
session `session-team-native-input-agent-worker-1`. Before the change the accepted receipt
had no visible label and the answer POST lacked its native target. After the change the
same message displayed `Accepted`, and the POST retained `fixture-runtime`, `native-session`
and `waiting-turn`. An injected session-mismatch response produced one attempt, retained
the original target and displayed the error in the card without retrying the new session.
The fixture lacks live SSE and prompt-default routes: existing fallback refreshes and 404
console entries remained, with no new JavaScript exception. Bounded `before_id=1` requests
returned an empty page; no deeper backfill was introduced. This is local fixture validation,
not production or native model-call evidence. Temporary browser processes were cleaned up.

## Permission And Cancellation Checkpoint

Committed pending plan/shell input now allocates a callback through the existing permission
service. Choices preserve their native semantics and the original tool card; unknown option
IDs deny execution. Timeout sends one explicit denial, while transport loss, superseded
input and cancellation expire callbacks without retargeting them. Operator selection history
stays separate from the native accepted/rejected/unknown receipt.

Native cancel/interrupt controls retain their captured turn fence. The existing manager cancel
route supports the direct runtime. Projection retires unfinished calls on terminal/discarded
turns, preserves approval handoffs and ignores old-turn cleanup against successor calls.

Validation records: 36 protocol cases passed (one opt-in native case excluded), 23 managed
runtime cases passed (one opt-in native case excluded), and protocol all-target plus root
library/test Clippy passed with warnings denied. Cases include all supported plan/shell choices,
unknown options, shared permission-service expiry, connection loss, superseded callbacks,
rejected native answers and both cancel/interrupt controls.

```bash
cargo test --offline --locked -p agenthub-rara
cargo clippy --offline --locked -p agenthub-rara --all-targets -- -D warnings
cargo test --offline --locked -p agenthub --lib agent::manager::rara:: -- --nocapture
cargo clippy --offline --locked -p agenthub --lib --tests -- -D warnings
```
