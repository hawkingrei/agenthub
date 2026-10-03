# Direct Runtime Event Storage

## Summary

The per-agent event database now provides atomic native event deduplication, history
associations, contiguous replay cursors and durable control-request receipts. This is
the storage foundation for slice 17. Typed projection and the managed durable event
consumer, managed text input, fenced browser answers, live permission callbacks and turn
cancellation are integrated. Authorized history and startup transport retirement are implemented;
their current validation is recorded below. The full 18-slice objective is not complete.

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
- No new transport launcher, transcript store or canonical task ledger. Safe delivery evidence
  uses an authorized session history endpoint.

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

- Publish and validate the complete slice 17 PR. Native and fake-peer checks cover prompt,
  approval, disconnect around ACK and stale permissions.
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

## Delivery Status Accessibility Follow-Up (2026-10-03)

Delivery labels now expose `role="status"`, preserving the same polite, atomic live region as
receipts change. The message body stays outside that region, and messages without a receipt
have no delivery status. The regression failed before the fix because no status was exposed;
all 19 focused input/workbench cases pass afterward. Web lint, TypeScript and production build pass.

Chromium's DevTools Protocol accessibility tree confirmed the before/after behavior for sending,
accepted, rejected and unknown delivery. All four transitions retain one receipt node and one
message; the fixed status has `live: polite` and `atomic: true`. The isolated real-component fixture
produced no browser exceptions. This validates browser accessibility semantics, not spoken output
from a screen reader or the full runtime. Chrome DevTools MCP was unavailable in this session;
Playwright drove local Chromium and its accessibility protocol instead. Temporary fixtures and
browser/server processes were removed after validation.

```bash
cd web
npm exec vitest -- run src/native_input.test.ts src/components/native_input.test.tsx src/components/use_agents_workbench_panel.test.tsx
npm run lint
npm exec tsc -- --noEmit
npm run build
```

## Control Admission And Source Ordering Follow-Up (2026-10-03)

Review identified three admission-boundary defects and one contradictory task-routing statement:

- A connection closing before dispatch recorded an unknown outcome. It now records `not_sent`
  without issuing a provider request. A foreign-runtime response after dispatch still records
  `outcome_unknown`; only a request addressed to the wrong runtime before dispatch is `not_sent`.
- A source batch could send its next control before the previous ACK's event prefix committed.
  Each registration now waits for that durable prefix. A gap retires the failed bootstrap without
  preparing or sending the next registration.
- A question submitted while another input was in flight resolved without sending. Question
  callbacks now reject an unavailable send, allowing the card to show an error and retain its
  selection for an explicit retry. Missing sessions, callbacks and empty answers also reject.
- The task-prefix contract now consistently follows task-ID lifetime: retitling preserves the
  stored prefix; a new or rekeyed task ID selects a new prefix. Storage behavior is unchanged.

Before the fixes, the focused Closing regression recorded `outcome_unknown`, the source regression
observed two prepared controls before the first source event, and all five input-gate cases resolved
instead of rejecting. Afterward, 29 runtime/receipt cases passed (two opt-in cases ignored), six
native activation cases passed, and both task-context cases passed. The 28 focused web cases, lint,
TypeScript, production build, root library/test Clippy with warnings denied and formatting passed.

An isolated Chromium fixture used the real Team input hook and question card. Before the change,
a blocked answer returned with no error and no transport call. Afterward, the card displayed a
retryable error, retained the selected option and sent the original native target exactly once
after an explicit retry. Neither version retried automatically, and no browser exceptions occurred.
Validation used Playwright with local Chromium because Chrome DevTools MCP was unavailable; it
does not establish production backend or real-provider behavior. Temporary fixtures were removed.

```bash
cargo test --locked --offline -p agenthub --lib agent::manager::rara:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::native:: -- --test-threads=1
cargo test --locked --offline -p agenthub-db loop_task_context -- --test-threads=1
cargo clippy --locked --offline -p agenthub --lib --tests -- -D warnings
cargo fmt --all --check
cd web
npm exec vitest -- run src/pages/team/use_team_member_acp_input.test.tsx src/components/native_input.test.tsx src/native_input.test.ts src/components/use_agents_workbench_panel.test.tsx src/pages/team/use_team_member_acp_view_model.test.tsx
npm run lint
npm exec tsc -- --noEmit
npm run build
```

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

## History And Recovery Checkpoint

The release-visible runtime history route checks `runtime:inspect` and local session ownership,
then returns a bounded snapshot of safe receipts and event cursors. It neither allocates runtime
authority nor exposes transcript bodies. Request-ID pagination remains stable across ACK updates.

Startup recovery uses the existing exclusive daemon generation, closes old stdio ownership,
settles unsent/uncertain requests and expires old permissions. It covers native sessions left
waiting for approval. This does not prove process-tree cleanup or grant loop admission; existing
execution reservations remain fenced. The earlier recovery plan required supervisor evidence
for process retirement; this implementation deliberately settles transport receipts only.

An ACK cursor now fences routing of the next input/control until the consumer has committed
that sequence. It cannot advance the history cursor or make a second prompt race ahead of
the first turn's delayed start event.

Database validation reports 24 focused cases and all 218 database tests passing, including
bounded history, stable paging, gap visibility and exclusion of payload bodies. Final root
validation reports 142 manager regressions passing (seven fixture/opt-in cases excluded), both
runtime-history/authz API cases passing, and library/test Clippy passing. Database all-target
Clippy also passes. The managed regressions include durable replay of the approval tool lifecycle:
one card, one terminal result, and no extra history after reordered duplicate delivery.

The real fixture first exposed a mode assumption: structured native questions require planning
mode, so the scripted model now enters that mode through the native tool and later requests plan
approval. It then exposed a projection defect: approved shell execution emits another tool-start
event for its original call ID. That event now updates the original card exactly once in the
answer turn, while unapproved duplicates remain conflicts. Native plan answers also settle their
interaction card without waiting for a tool-result event the native runtime does not emit.
The resulting protocol suite has 38 passing cases and all-target Clippy passes.

The final real-process fixture passes both shell decisions against the pinned binary. It uses
five local model requests per decision to enter planning mode, ask a question, receive the fenced
answer, approve the plan, approve or deny the command, and finish. Approval creates one marker
inside the fixture workspace; denial creates none. Both paths retain one tool card/result,
complete the plan card and close with accepted receipts and a contiguous persisted cursor.
No external provider is called. Unlike the earlier handshake-only smoke, this proves native
conversation, input and permission mapping through the managed process and storage paths.

```bash
cargo test --offline --locked -p agenthub --lib agent::manager::rara::tests::native::managed_native_question_and_shell_approval_round_trip -- --ignored --exact --nocapture
cargo test --offline --locked -p agenthub --lib agent::manager:: -- --test-threads=1
cargo test --offline --locked -p agenthub --lib runtime_history_route
cargo test --offline --locked -p agenthub --lib agent_inspect_routes_require_runtime_inspect_capability
cargo test --offline --locked -p agenthub-db -- --test-threads=1
cargo clippy --offline --locked -p agenthub-db --all-targets -- -D warnings
```

The ignored fixture requires `AGENTHUB_RARA_TEST_BINARY` from the pinned upstream and localhost
HTTP access; the test configuration routes model traffic exclusively to its local server.

## CI Fixture Follow-Up

Bazel coverage exposed a race in the existing member auto-start test: its default
`agenthub actor` command has no subcommand and exits with an error before or after the
startup status assertion. That test now uses `/bin/cat` for both members, retaining the
worker worktree policy and letting the supervisor own their lifetime. It also checks that
both running session handles exist after creation and disappear after Team deletion.

The focused test passes. The native adapter is unchanged; current-head CI must confirm
the coverage upload and aggregate checks after this fixture correction.

```bash
cargo test --offline --locked -p agenthub --lib api::teams::tests::teams_api_create_team_auto_starts_member_runtime -- --exact
```

## Delivery Checkpoint (2026-09-20)

All applicable CI checks passed at `c6e2bbd15a35df95b806d0926360d38a4419bdb3`,
including Bazel coverage and both Codecov checks. No review feedback remained outstanding.
[PR #1164](https://github.com/hawkingrei/agenthub/pull/1164) was merged by the user into
`codex/loop-16-rara` at `95447346f3e559133c293f91c3f97480b6ac4175`. Its squash-merge
tree matches the validated PR head. Slice 18 preserves its subsequent changes while merging
that dependency forward; the complete stack is not thereby claimed to be on main.

## Startup Recovery Review (2026-10-03)

Startup receipt recovery now includes sessions held by loop execution reservations. The old
stdio owner cannot reconnect, so its preparations become `not_sent` and unresolved sends become
`outcome_unknown`. The session exit update retains the reservation exclusion; guardian cleanup
still gates replacement execution. Permission callbacks expire without completing an activation.

Conversation history reads reconcile only retained input and receipt events with durable
receipts, including late ACKs and a crash before receipt projection. This covers SQLite pages,
fresh indexed pages and individual events without changing event identity, pagination or
compressed storage. Repeated reads cannot append history or restore expired conversation rows.
Local session, runtime, native session and request identity must all match.

The pre-fix regressions reproduced an open transport under a retained reservation and a
conversation message still showing `pending` after its receipt had settled. The guardian
regression uses a real detached descendant and checks transport retirement independently
from unchanged activation/session state and blocked replacement admission.

Focused validation commands:

The first recovery revision passed 29 runtime/receipt tests (two opt-in cases ignored), two guardian recovery
tests, one indexed native-history regression, two existing indexed-history regressions and
25 runtime-event storage tests. Root library/test Clippy with warnings denied, formatting,
diff checks and all nine local documentation link targets pass. These checks use local
fixtures; deferred real-provider acceptance was not repeated.

```bash
cargo test --locked --offline -p agenthub --lib agent::manager::rara:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::recovery:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::tests::native_history:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib list_agent_events -- --test-threads=1
cargo test --locked --offline -p agenthub-db runtime_events:: -- --test-threads=1
cargo clippy --locked --offline -p agenthub --lib --tests -- -D warnings
cargo fmt --all --check
```

The next review found repeated retirement of already closed owners. Startup now visits each
local event database once and reads open owners in 100-row keyset pages using a partial index.
The session association and reservation checks remain in place. Main-database session and
permission cleanup completes before closing the event-store owner, so an interrupted cleanup
remains eligible for retry. No existing row format or protocol changes.

A related API regression blocked explicitly targeted question answers during a loop activation.
The loop admission gate now allows these answers into the existing local-session, runtime,
native-session and pending-turn checks. Ordinary prompts still require durable work intake.
The real API router regression uses a local fake provider and confirms exactly one answer is
sent, wrong identities and an old turn return conflict, and untargeted input remains rejected.

Both regressions reproduce against the prior implementation: a valid question answer returns
HTTP 500, and a trigger detects another write to a closed owner during recovery. Added storage
checks cover migration/reopen, the indexed query plan, three pages while closing owners, retained
ownership evidence, and invalid cursors. Manager recovery checks include closed-owner write
rejection, unassociated owner exclusion and retry after injected permission-cleanup failure.

The follow-up passes 26 runtime storage tests, seven native activation/API tests, 30 runtime
adapter tests (two real-provider opt-in cases ignored) and both guardian recovery tests.
Root/database library/test Clippy with warnings denied, workspace formatting, diff checks
and nine local documentation links pass. Existing real-provider acceptance remains deferred.

```bash
cargo test --locked --offline -p agenthub-db runtime_events:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::native:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::rara:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::recovery:: -- --test-threads=1
cargo clippy --locked --offline -p agenthub -p agenthub-db --lib --tests -- -D warnings
cargo fmt --all --check
```

## Event Bounds And Source Prefix Review (2026-10-03)

Review found three failures at existing event-processing boundaries. Terminal projection can
retire 512 open tools, but storage admitted only 16 history rows. The per-event row bound is now
514, including the status and diagnostic rows of a failed turn. The 2 MiB encoded payload bound
and single transaction for event identity, history and cursor are preserved.

A required source ACK without `last_sequence` now aborts bootstrap before preparing or sending
the next source. Successful registration still waits for each acknowledged prefix to commit.
The consumer also admits the next contiguous frame when its reorder buffer is full; the extra
frame keeps the 1 MiB frame cap, and other out-of-order events retain the 256-event/8 MiB limits.

Four regression cases fail on the preceding implementation: missing-prefix recovery at count
capacity, the same recovery at byte capacity, terminal projection with 16 unfinished tools,
and source registration with an absent ACK cursor. The terminal regression also covers all
512 supported tools, rollback after inserting the tool/status rows, retry and duplicate replay.
The source regression verifies that only one control reaches both the durable ledger and wire.

All 66 focused cases pass: 26 runtime storage, 33 runtime adapter (two opt-in real-provider
cases ignored), and seven native activation/API tests. The final storage-limit assertion also
passes independently and requires the specific projection-limit error. Root/database Clippy
with warnings denied, workspace formatting, diff checks and nine local documentation links pass.
These are fixture-based checks; the deferred real-provider acceptance scope is unchanged.

Focused validation commands:

```bash
cargo test --locked --offline -p agenthub-db runtime_events:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::rara:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::native:: -- --test-threads=1
cargo clippy --locked --offline -p agenthub -p agenthub-db --lib --tests -- -D warnings
cargo fmt --all --check
```

## Answer Admission And Advancing Source Cursors (2026-10-03)

An accepted user-answer ACK can omit its event cursor or point to an already committed
prefix. Waiting only on that cursor left the same question open to another dispatch with
a new request ID. The input gate now records the accepted waiting-turn identity before
returning. A second answer to that turn fails before receipt preparation or provider dispatch.
This bounded marker does not advance the event cursor, clear pending state, admit ordinary
input, or retire a successor question. Rejected answers remain available for explicit retry.

Source registration now requires its ACK cursor to advance beyond the committed cursor
observed before dispatch. Missing, zero and stale cursors abort bootstrap before a second
source is prepared or sent. Successful registration still waits for its event prefix to commit.

Before the fix, the duplicate-answer regression returned success and both zero/stale source
regressions allowed the batch to continue. The delayed-event answer cases cover absent, zero
and old cursors, unchanged committed history, ordinary-input rejection, no duplicate ledger
entry or wire control, and a subsequent question that can still be answered. A separate case
covers a rejected answer followed by an explicit retry.

All 44 focused cases pass: 37 runtime adapter tests (two real-provider opt-in cases ignored)
and seven native activation/API tests. Root library/test Clippy with warnings denied,
workspace formatting, diff checks and nine local documentation links pass. Deferred
real-provider acceptance was not repeated.

```bash
cargo test --locked --offline -p agenthub --lib agent::manager::rara:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::native:: -- --test-threads=1
cargo clippy --locked --offline -p agenthub --lib --tests -- -D warnings
cargo fmt --all --check
```

## Receipt Projection And Recovery Pool Lifetime (2026-10-03)

The input response now follows the ACK already committed by the receipt owner. Reading or
writing the derived receipt-history event is a separate, logged projection step; its failure
cannot report accepted or queued work as failed. Rejected and uncertain outcomes remain
failures. Existing history reconciliation renders the durable status without another send
or any rewrite of the stored attempted message.

Startup recovery opens an initialized pool outside the event router's cache and closes it
after each agent on success or error. The normal cached path shares the same opener and SQLite
defaults. Schema initialization failure explicitly closes the pool, and recovery does not
close pools already held by readers.

Both regressions fail before the change: an accepted input returns the injected projection
error, and a failed recovery increases open event-database file handles from five to ten.
The projection fixture covers accepted, queued, rejected and unknown outcomes with only one
wire request. The Linux recovery fixture checks that cold database handles return to zero after failure,
success and repeated recovery while retaining an existing cached reader. SQLite shared WAL
handles held by that existing reader are excluded from the cold-database count. A portable router test covers uncached
initialization/reopen, retained data, invalid schema, unchanged cache membership and reader reuse.

Focused validation covers 76 passing cases: 28 database/router cases, 39 runtime adapter/event
cases, seven activation/API cases and two guardian recovery cases. Two real-provider opt-in
cases remain ignored; the fixture evidence does not claim final provider acceptance.
Library/test Clippy with warnings denied, workspace formatting, diff checks and nine local
documentation links pass.

```bash
cargo test --locked --offline -p agenthub-db uncached_event_pools -- --test-threads=1
cargo test --locked --offline -p agenthub-db remove_agent_db_retries -- --test-threads=1
cargo test --locked --offline -p agenthub-db runtime_events:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::rara:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::native:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::recovery:: -- --test-threads=1
cargo clippy --locked --offline -p agenthub -p agenthub-db --lib --tests -- -D warnings
cargo fmt --all --check
```

## Recovery Query-Plan Fixture (2026-10-03)

Bazel coverage exposed a connection-dependent failure in the migration query-plan assertion.
The two-connection pool could run the migration on one connection and `EXPLAIN QUERY PLAN`
on another connection that still cached the schema without the partial index. A deterministic
reproduction held that old-schema connection across migration and reported the unique session
index instead of `idx_runtime_event_owners_open`.

The fixture now executes the actual recovery query on the held connection before explaining
that query on the same connection. This exercises SQLite's schema refresh before inspecting
the plan. The exact partial-index assertion, 100/100/5 pagination, close-during-scan behavior,
invalid cursor rejection and retained ownership count remain enforced. The assertion also
includes the observed plan when it fails. Production SQL and schema are unchanged.

The pre-fix regression fails deterministically with the old-schema connection. After the fix,
all 227 database tests pass, along with database library/test Clippy with warnings denied,
workspace formatting and whitespace checks.

```bash
cargo test --locked --offline -p agenthub-db --lib
cargo clippy --locked --offline -p agenthub-db --lib --tests -- -D warnings
cargo fmt --all --check
bazel coverage --combined_report=lcov --test_output=errors --nocache_test_results //crates/agenthub-db:agenthub_db_tests
```

The local default-config Bazel coverage attempt stopped during repository analysis: its cached
`remote_coverage_tools` extraction lacked a `MODULE.bazel`, `REPO.bazel` or `WORKSPACE` marker.
No test target executed in that attempt. Current-head CI remains the Bazel coverage verification
surface; the fixture fix does not change Bazel configuration or the shared repository cache.

## Chronological Receipts And Transport Cleanup (2026-10-03)

Caller-supplied request IDs are not chronological. Receipt history now orders by descending
creation timestamp and request ID, resolving the existing ID cursor inside the owned runtime
and the same read transaction. Unknown and foreign cursors return the same invalid-cursor
error. ACK updates do not move a receipt between pages, and new receipts ahead of a cursor
remain on the newest page. The existing 4096-receipt limit bounds each runtime's sort without
a schema migration or a new API field.

Transport failure previously stopped the supervised child directly. That clears the shared
child handle, allowing the exit watcher to return before releasing the loop reservation.
The failure path now uses the common observed-session cleanup helper under its existing
configuration gate. Verified process and descendant cleanup precedes reservation, credential
and activation-authority release, followed by finalization of only the matching local session.
Transport loss interrupts an unfinished activation without inventing a semantic outcome.

Both focused regressions fail before the fix. The storage fixture exposes a lexicographically
high older ID displacing the newest receipt. It also covers timestamp ties, cross-runtime ID
collisions, an insertion between pages, late ACK updates, closed-owner reads and invalid
cursors. The API fixture covers newest-first ordering and missing-cursor rejection after exit.
The transport fixture aborts a connection while its provider and a detached descendant remain
alive, without an activation monitor. It observes the leaked reservation before explicitly
cleaning the fixture. After the fix, it requires process and descendant removal, interruption,
durable and in-memory reservation removal, credential retirement and replacement admission at
a newer generation.

Focused validation passes 77 cases: 27 runtime storage, eight native activation, one history
API, 39 runtime adapter/event and two guardian recovery cases. Two real-provider opt-in cases
remain ignored; these fixtures do not establish final provider acceptance.
Library/test Clippy with warnings denied, workspace formatting and diff checks also pass.

```bash
cargo test --locked --offline -p agenthub-db --lib runtime_events:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::native:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib runtime_history_route_is_authorized_scoped_and_available_after_exit -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::rara:: -- --test-threads=1
cargo test --locked --offline -p agenthub --lib agent::manager::loop_launch::tests::recovery:: -- --test-threads=1
cargo clippy --locked --offline -p agenthub -p agenthub-db --lib --tests -- -D warnings
cargo fmt --all --check
```
