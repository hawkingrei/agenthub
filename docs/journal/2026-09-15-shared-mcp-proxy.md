# Shared MCP Proxy Implementation

## Summary

Slice 9 now has the persistent operation journal, shared JSONL/HTTP/SSE transport, trusted call
preparation, actual HTTP/journal orchestration, daemon startup recovery, authenticated streaming
RPCs, and a local stdio shim. Existing Mem profile resolution, activation mounts, local ACP launch,
inherited-environment isolation, journaled March batches, and legacy GET/recovery/DELETE are
connected. Complete protocol
controller behavior and integration authorization remain pending; this checkpoint does not complete slice 9.

## Background

The loop foundation supplies fenced execution identities, but an upstream write can outlive the
activation that issued it. The journal must preserve uncertainty and late factual results without
giving an expired executor authority to send more work.

## Scope

- Shared agent-domain operation types, including compatibility aliases for existing Mem helpers.
- Additive operation, attempt, and ordered event tables with bounded scoped history reads.
- Atomic preparation/send admission, process-local send permits, immutable replay identity,
  old-daemon recovery, and factual completion independent of executor lifetime.
- A Cargo/Bazel transport crate with bounded framing, explicit protocol versions, legacy lifecycle
  state, HTTP request preparation, streamed responses, and modern parameter header mapping.
- Trusted discovery snapshots, scope-before-digest preparation, declared stable identity, and a
  journaled client that persists raw-response disposition before returning it to the provider.
- Startup recovery through the actual daemon generation, plus an authenticated daemon task fixture
  that holds the execution guard after the requesting caller disconnects.
- Activation-scoped MCP session storage and signed streaming RPCs, per-message shim credential
  refresh, live binding revocation, bounded discovery/callback state, and activation cleanup.
- Existing Mem configuration resolution, configuration fingerprinting, fresh/resumed ACP stdio
  descriptors, and removal of upstream secrets from provider and descendant environments.
- March batch admission through the real shim, with one HTTP POST, atomic tool send transitions,
  independently durable response receipts, and partial-result forwarding on disconnect.
- Legacy GET listeners and exact-stream recovery, readiness-driven shim subscription, and upstream
  DELETE after admitted exchanges settle, including caller loss and activation cleanup.

## Key Decisions

- Reuse the existing loop executor verification inside the journal transaction.
- Record `sent` with SQLite FULL synchronization before returning a permit; no network I/O in a
  transaction. Do not reset the pooled connection to weaker durability afterward.
- Keep daemon generation and attempt generation separate. Only old-daemon sends are recovered;
  ordinary database opens and repeated migrations do not declare a live call unknown.
- Preserve every attempt and transition. Late results can resolve uncertainty, but never replace
  another attempt's result or restore execution permission.
- Enforce original stable caller identity for write retries. A new request ID, profile alias,
  binding revision, or actor cannot hide a prior write in the same effective scope.
- Store no request/result bodies or arbitrary error strings. Policy derives canonical hashes from
  actual bound arguments and request parameters, preserving the real response separately.
- Build an owned HTTP request before issuing a send permit. Disable reqwest retries/redirects and
  ambient proxy configuration. Keep session/cursor controls outside provider JSONL messages.
- Preserve both legacy lifecycle and modern per-request protocol mechanics. Retain modern MRTR
  payloads unchanged; tool rounds use atomic journal links to their prior upstream receipt.
- Preserve a deferred input/task receipt without claiming terminal tool success. Its typed receipt
  cannot be downgraded by transport loss or used to replay the original request. Modern tool
  follow-ups, task lookups, cancellation, input updates, and modern task subscriptions require
  matching receipts. Legacy notifications use the authenticated ownership and receipt matching described below.
- Stream each admitted MCP message independently. Callback replies carry freshly read signed
  credentials, and the daemon retains operation ownership after RPC receiver loss.
- Serialize legacy lifecycle delivery while allowing registered callback responses through. Keep
  the upstream session header private even when initialization requests client roots first.
- Use detached bounded JSONL reader/writer threads so a credential or RPC failure can exit the
  shim without waiting for the provider to close stdin.
- Permit MCP protocol bootstrap during `starting` only after a launch snapshot and local session
  are bound. Keep journal sends and ordinary actor controls running-only, sharing the same live
  owner, generation, lease, membership, and mailbox validation for both phases.
- Resolve profiles in the daemon and mount them under the activation operation guard after the
  launch snapshot is durable. Keep keys and random credential-file paths out of configuration
  fingerprints. Reuse declared-space binding and reject supplied non-string scope values.
- Preserve the configured Mem tool-set header without treating it as authorization. Inspection
  of the local upstream Mem checkout at `fff6c631d3900e9991a7390865bd512a03f060a6`
  (`nmem-core/src/headers.rs`, `nmem-server/src/remote_gateway.rs`, and `mcp_gates.rs`) confirms
  Bearer support and that tool-set/space routing does not establish namespace authorization.
- Forward modern server discovery as a control request, retaining upstream capabilities, metadata,
  cache hints, and errors. It creates no journaled tool send and does not replace the client's
  version/fallback decision. The pinned July 2026 discovery schema places server identity under
  `result._meta`; the earlier draft's top-level `serverInfo` example is not the final contract.
- Share byte-credit pools across daemon sessions. Reserve working capacity before HTTP admission,
  with an independent callback allowance. Lease ingress, queued events/frames, retained discovery,
  and capabilities; transfer delivery leases across queues and bound the shim's combined queues.
- Hash retained request/callback/initialize IDs to fixed-size correlation keys, preserving the
  original IDs on the wire. Account serialized payloads and bounded working copies explicitly;
  do not present these counters as allocator or process RSS measurements.

Stable contract: [MCP operation journal](../features/mcp-operation-journal.md).
Transport contract: [MCP proxy transport](../features/mcp-proxy-transport.md).

## Validation

The following focused commands cover this checkpoint:

```bash
cargo test -p agenthub-db --locked --offline
cargo test -p agenthub-db --locked --offline mcp_operations
cargo test -p agenthub-acp-core -p agenthub-agent-domain --locked --offline
cargo test -p agenthub-mcp --locked --offline
cargo test -p agenthub --lib mcp_send_remains_daemon_owned_after_authenticated_request_disconnects --locked --offline
cargo clippy -p agenthub-db -p agenthub-acp-core -p agenthub-agent-domain --all-targets --locked --offline -- -D warnings
cargo clippy -p agenthub-mcp --all-targets --locked --offline -- -D warnings
cargo fmt --all --check
```

The initial full database suite passed 104 tests. After adding abrupt child-process exit coverage
and scoped event paging, the final journal selection passed 14 tests. ACP core passed 8 tests and
agent domain passed 7 tests. All-target Clippy with warnings denied, formatting, and whitespace
checks passed. The child fixture exits after prepared, sent, and completed commits without running
Rust destructors or closing SQLite. These fixtures do not yet prove a complete MCP transport or an
external service call.

The transport suite passes 22 tests with real local fake servers, including raw TCP truncation
after receipt of a write, redirect non-following, JSON/SSE/JSONL framing, versioned batches,
initialization state, and modern MRTR/header preservation. All-target transport Clippy passes.
The initial sandboxed test attempt could not bind loopback sockets; rerunning with socket permission
passed. This was the raw-transport checkpoint at `ee4ecaa4`.

The subsequent integration fixtures exercise the actual HTTP send and journal together: scoped
arguments, redacted durable rows, unchanged error results, same-identity write retries, rejection
of changed retry parameters, concurrent send exclusion, lost responses across reopen and a fresh
activation, and deferred input/task receipts. The root fixture uses signed execution authentication
and the real daemon task group and operation guard; it is not yet a real MCP RPC or CLI-shim test.
Provider environment isolation and the complete session bridge remain unproven.

Final integration validation passes 32 MCP tests, 15 database journal tests, 8 ACP-core tests,
7 agent-domain tests, the authenticated daemon HTTP ownership test, and 3 daemon-generation tests.
Root/MCP/database/domain all-target Clippy passes with warnings denied; formatting, whitespace,
and local document links pass. The final MCP suite also retains an HTTP JSON-RPC error that omits
its request ID, preserving the original error rather than reporting a transport disconnect.
No local Bazel run or complete provider-facing proxy validation is claimed at this checkpoint.

The streaming bridge checkpoint adds a real `agenthub mcp-proxy` subprocess connected to an actual
local gRPC service and fake HTTP upstream. It preserves an initialization roots callback and its
private session header, serializes initialized delivery before discovery, merges discovery pages,
and forwards progress and the original tool result after durable completion. Replacing the signed
credential envelope with a same-identity token lacking MCP permission rejects the next call even
though the original token remains valid; the process exits while stdin is still open.

A separate RPC fixture drops the response stream after the upstream observes `sent`, verifies that
the daemon still holds the execution guard, then observes durable success after releasing the
upstream. Scope isolation, binding revocation, activation cleanup, and notification/callback
rejection are covered. Core regressions preserve stale discovery responses without replacing the
current catalog, reject duplicate initialization IDs without poisoning lifecycle state, and bound
the blocking JSONL reader.

Bridge validation passes 3 focused root fixtures (also included in the 79 passing internal tests),
35 MCP tests, and root/MCP all-target Clippy with warnings denied. The real binary build,
formatting, generated-proto equality, whitespace, and local documentation links pass. A test-only
Mutex API mismatch was corrected before these passing selections. No production mount, ACP launch,
provider environment isolation, or complete protocol-controller claim follows from these results.

```bash
cargo build -p agenthub --bin agenthub --locked --offline
cargo test -p agenthub --lib internal::service::tests::loop_activation::mcp_shim --locked --offline
cargo test -p agenthub --lib internal:: --locked --offline
cargo test -p agenthub-mcp --locked --offline
cargo clippy -p agenthub -p agenthub-mcp --all-targets --locked --offline -- -D warnings
```

The bootstrap follow-up separates protocol preparation from tool execution. A signed startup
session must have an immutable launch snapshot and bound local session before opening MCP. Its
initialize/roots callback/initialized/discovery sequence succeeds while the activation remains
`starting`. Tools, resource reads, prompt reads, task operations, and batches containing them are
rejected without upstream I/O or journal entries; ordinary actor control remains rejected. After `mark_running`,
the same MCP session can perform its first journaled tool call and return the real result.

Database coverage also rejects missing launch/session, wrong owner/actor/Team/generation, expired
lease, inactive mailbox, removed membership, and revoked execution. This follow-up passes 42 loop
database tests, 15 journal tests, 35 MCP tests, and 80 internal tests including the new startup RPC
fixture. The real binary build, formatting, whitespace, and local documentation links pass.
Root/database/MCP all-target Clippy also passes with warnings denied.

Configured launch adds a fake ACP process that starts the real stdio shim during session creation,
initializes and discovers a fake HTTP Mem server, and performs one scoped write after startup.
The upstream observes a durable `sent` row before receiving the call, then the fixture observes
`succeeded`. Both provider and shim process environments exclude the configured credential, an
unused profile credential, and ambient Mem/header variables. The ACP request log excludes upstream
URLs, secrets, headers, and the tool body. A separate ACP fixture checks fresh and resumed session
descriptors and fingerprint stability across credential-file rotation.

This follow-up passes 12 root MCP tests (one child fixture is invoked by its parent rather than the
ordinary harness), 3 launch tests, 6 executor tests, 14 offline configuration tests, 7 ACP loop tests,
2 static MCP loader tests, 8 ACP-core tests, and 2 existing Mem configuration tests. These selections
overlap. Root/ACP/ACP-core/MCP all-target Clippy passes with warnings denied. The binary build,
formatting, whitespace, and changed-document local links pass. An initial test-only Option/Result
mismatch was fixed; a configuration-test filter that selected zero tests was replaced with the
compiled harness's `loop_configuration` selection before recording the 14-test result.

```bash
cargo test -p agenthub --lib mcp --locked --offline
cargo test -p agenthub --lib agent::manager::loop_launch::tests --locked --offline
cargo test -p agenthub --lib agent::manager::executor::tests --locked --offline
cargo test -p agenthub --lib loop_configuration --locked --offline
cargo test -p agenthub-acp --locked --offline loop_
cargo test -p agenthub-acp --locked --offline load_mcp_servers
cargo test -p agenthub-acp-core --locked --offline
cargo test -p agenthub-config --locked --offline nowledge_mem
cargo clippy -p agenthub -p agenthub-acp -p agenthub-acp-core -p agenthub-mcp --all-targets --locked --offline -- -D warnings
```

The modern discovery follow-up passes 13 root MCP tests (plus the parent-invoked child fixture),
35 MCP crate tests, and root/MCP all-target Clippy with warnings denied. The real binary build,
formatting, and whitespace checks pass. The startup fixture preserves the exact discovery result
and records no tool operation; tools remain denied before running. The real shim fixture forwards
an upstream HTTP 404 JSON-RPC `-32601` probe error, including its data, and subsequently completes
the existing legacy initialize/callback/discovery/write flow. The result shape follows the final
[July 2026 discovery specification](https://modelcontextprotocol.io/specification/2026-07-28/server/discover).

The payload-budget checkpoint passes 43 MCP crate tests and 14 root MCP tests, including the
parent-invoked configured-launch child fixture. It covers exact capacity versus one extra byte,
shared cross-session admission, initialization callbacks with ordinary workspace capacity full,
catalog refresh/invalidation, capability handoff, and large wire IDs with fixed-size retained keys.
The signed RPC fixture fills delivery capacity after discovery, observes `sent` before the upstream
write, and verifies a single send and durable `succeeded` after provider delivery closes. The
operation guard remains held until settlement. A raw SSE fixture verifies that short exponent
numbers expanding beyond the output limit in a progress event do not discard the later tool result.

The final real binary rebuild and root shim/HTTP regressions pass after that serialization fix.
Root/MCP all-target Clippy with warnings denied, formatting, whitespace, and changed-document local
links pass. No dependency, generated-proto, database-schema, or Bazel configuration change was
needed. The limits cover charged application payloads and bounded working allowances, not RSS.

The March batch follow-up uses one owned HTTP request with per-tool intents. A single transaction
admits every tool send or rolls all send transitions back. Response IDs identify independent
receipts, so out-of-order results remain factual and partial EOF makes only unfinished members
unknown. The bridge drains queued partial facts before closing without a completion marker.
Whole-batch scope/lifecycle/callback validation precedes send admission; discovery inside a batch
cannot grant authority to its other members. Every startup batch member must independently
qualify for protocol bootstrap.

Regression fixtures cover callback arrays during initialization, notification/discovery batches,
a batch response to a single discovery request, a same-frame callback, and two writes plus a ping
in one upstream POST. Invalid scope rejects all request members without journaling or HTTP. The
upstream checks that both send rows are already durable. Normal responses retain reversed order;
truncated responses reach real shim stdout before it exits. A separate pressure fixture commits
both factual write results despite exhausted event delivery credit. Callback responses now have
independent concurrency slots as well as workspace credit, preventing ordinary controls from
starving initialization replies. Existing single-message callback and tool paths remain covered.

```bash
cargo test -p agenthub-mcp --locked --offline
cargo test -p agenthub-db --locked --offline mcp_operations
cargo clippy -p agenthub -p agenthub-mcp -p agenthub-db --all-targets --locked --offline -- -D warnings
cargo build --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp --locked --offline
```

The final batch checkpoint passes 49 MCP crate tests, 17 journal store tests, and 17 root MCP
tests, with the configured-launch child fixture invoked by its parent. Root/MCP/database all-target
Clippy passes with warnings denied; the real binary build, formatting, whitespace, and local doc
links pass. One new fake upstream incorrectly placed a callback in an application/json response;
the fixture now uses SSE for that array and separately checks JSON response-only arrays. The
production validator remains strict. No dependency, schema, generated protocol, or Bazel change
was required; existing source globs include the new modules.

The legacy stream follow-up retains a separate cursor and server retry delay inside each HTTP
exchange. Recovery sends GET with the original session and deadline, retaining the same journal
permits. There is no second POST or new attempt. Metadata-only priming frames remain in recovery
state and do not consume provider delivery capacity. Concurrent writes and partial batches exercise
this path against real loopback HTTP servers.

The shim subscribes after the daemon reports actual protocol readiness on an internal frame field;
MCP payloads and stdout remain unchanged. Listeners use independent workspace credits and one
slot per session. They revalidate the live execution fence before each message and periodically
while idle, without holding the operation guard over idle reads. Raw reads and reconnect delays
stay pinned across validation ticks. GET 405 leaves ordinary MCP exchange available.

Session closure fences new admission, stops listening, and waits for admitted exchanges before
DELETE. Disconnected provider tasks and retired sessions also run cleanup. The actual HTTP fixture
checks that DELETE sees no pending send, even when a write completes through GET after RPC loss.
A real shim receives a GET callback, responds through POST, receives a resumed notification, and
terminates the upstream session on stdin EOF. A session-404 fixture preserves upstream error data,
closes the old stream, and proves a reopened proxy still requires initialization. Readiness is
published only after initialized HTTP delivery; a preparation-only regression keeps listening
closed. The real GET fixture uses a 1.1-second retry hint, spanning the one-second authority
check, and verifies that reconnection does not shorten the server delay.

Validation commands for this follow-up:

```bash
cargo test -p agenthub-mcp --locked --offline
cargo clippy -p agenthub -p agenthub-mcp --all-targets --locked --offline -- -D warnings
cargo build --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp --locked --offline
cargo fmt --all --check
```

The final legacy stream checkpoint passes 54 MCP crate tests and 20 root MCP tests, with the
configured-launch child fixture invoked by its parent. Root/MCP all-target Clippy passes with
warnings denied, and the real binary build, formatting, whitespace, and changed-document local
links pass. The tracked internal protobuf source matches build-script output for the new listen
RPC and readiness field. There are no dependency, database schema, or Bazel configuration changes.

The failed-handshake follow-up retains provisional HTTP context when initialize returns an
identified error, then retires that context after already admitted callback replies settle. It
clears pending callback IDs and discovery state while still holding lifecycle admission. A fresh
initialize can reuse upstream callback IDs without inheriting the old session. DELETE failure
preserves the original error, closes the provider stream without a completion marker, and leaves
the context available to final shutdown. The same cleanup covers malformed results, disconnects,
anonymous HTTP errors, and failed initialized notifications, including March batches.

Handshake identity is captured at admission. A repeated initialized notification after the
session reaches normal operation stays an ordinary notification, so its error cannot retire an
operating session with concurrent tool calls. This distinction also applies to notification
batches. Session termination is shared by handshake retirement and normal shutdown; the latter
still waits for all admitted exchanges.

The focused HTTP lifecycle fixture covers six failure shapes, fresh initialization with a reused
callback ID and a new HTTP session, an in-flight callback delaying DELETE, cleanup failure with
final shutdown, and repeated initialized errors in an operating session. All 58 MCP crate tests
and root/MCP all-target Clippy with warnings denied pass. The rebuilt real binary also passes
20 root MCP tests, including the parent-invoked configured-launch child. Formatting, whitespace,
and local links in all three changed documents pass. No public protocol, dependency, database
schema, or Bazel configuration changes are needed for this follow-up.

The modern tool MRTR follow-up adds digest-only input-receipt facts and an additive continuation
link table. One FULL-synchronous transaction resolves the current receipt under live daemon and
executor checks, verifies unchanged binding/schema/base parameters, and commits the next send
attempt plus its parent response digest. Later rounds retain the logical operation and original
stable caller identity. The provider supplies exact state and new input results; the proxy never
reconstructs or automatically resends them. Current admission caps a chain at ten continuation
rounds and a receipt at 64 input IDs.

Input results retain the direct MCP result shape, including declined/cancelled inputs, rather
than becoming JSON-RPC envelopes. State-only rounds omit earlier input responses. Malformed or
old receipts without valid correlation metadata remain known deferred outcomes and cannot grant
another send. A lost continuation result leaves an unknown attempt and blocks restarting the
original request, including for a read-only or stable-identity operation. Declared retries retain
that uncertain round through the linked retry path described below; task handles require the
separate lookup path described below.

Database fixtures cover additive migration, reopen, parent links, concurrent consumption, stale
intent/authority/state/IDs, round limits, recovery, and late factual results. HTTP fixtures cover
unchanged opaque state and bound arguments over three POSTs on one operation, independent input
sets, altered-intent rejection before HTTP, partial inputs, and unknown-round replay denial. A
real shim fixture exercises modern discovery and input-required/complete messages through signed
RPCs; its fake upstream checks that the continuation link exists before receiving the POST.

Review added a no-state parallel-read regression: consuming one receipt cannot make its input
IDs authorize another outstanding read. Without opaque state, admission requires an issued input
ID (or two empty maps); remaining missing/extra inputs still reach upstream unchanged. Final
validation passes 22 journal store tests, 61 MCP crate tests, and 21 root MCP tests with the
configured-launch child invoked by its parent. Root/MCP/database all-target Clippy, the real
binary build, formatting, whitespace, and local document links pass. The migration is additive;
there are no dependency, protobuf, or Bazel configuration changes.

```bash
cargo test -p agenthub-db --locked --offline mcp_operations
cargo test -p agenthub-mcp --locked --offline
cargo clippy -p agenthub -p agenthub-mcp -p agenthub-db --all-targets --locked --offline -- -D warnings
cargo build --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp --locked --offline
cargo fmt --all --check
```

The continuation retry follow-up adds a separate retry link table without changing existing
continuation or attempt rows. A retry refers to the round's first send and retains its parent
receipt and full semantic request digest. The caller must supply the same bound parameters,
state and input results with a fresh RPC ID. Trusted read-only or stable-identity policy is
required; a tool annotation does not grant replay authority. Failed/unknown outcomes permit
explicit retries, while a new input-required result starts another round. Each round has a
separate three-retry budget, and the ten-round chain bound remains intact.

Database fixtures exercise migration of populated continuation history, daemon restart, new
activations, concurrent retry admission, unchanged intent/authority, late result fencing, and
budget exhaustion across all ten rounds. Real HTTP fixtures preserve exact state (including
absence), input responses, original stable identity, and upstream JSON-RPC errors. The real shim
fixture adds a declared stable-identity binding, drops one continuation response after receiving
the request, rejects changed inputs before HTTP, and links the explicit retry before sending it.

Review also covers an ambiguous pair of parallel reads: an exact recorded retry with extra input
IDs cannot be discarded in favor of another receipt whose issued IDs happen to match. Neither
request sends when the association remains ambiguous. Final validation passes 26 journal store
tests, 62 MCP crate tests, and 22 root MCP tests, with the configured-launch child invoked by its
parent. Root/MCP/database all-target Clippy with warnings denied, the real binary build,
formatting, whitespace, and local document links pass using the commands above. No dependencies,
protobuf definitions, or Bazel configuration change.

Review against the released Tasks extension schema found that a modern flat `resultType: "task"`
receipt was incorrectly classified as a successful tool result. The HTTP regression reproduced
that error before the fix. The classifier now recognizes both modern and legacy receipt shapes
as deferred. The regression covers every initial task status, unchanged payload delivery,
durable unknown outcome, original-request replay denial, and task-ID redaction. The subsequent
lookup, cancellation, and input checkpoints resolve recorded handles and journal their control sends.

After the receipt fix, all 63 MCP crate tests and 22 root MCP tests pass, including the
parent-invoked configured-launch child. Root/MCP all-target Clippy with warnings denied and the
real binary build pass. Formatting, whitespace, and local document links also pass. The database
code is unchanged from the 26-test continuation retry checkpoint.

The task lookup follow-up adds immutable digest-only task links and separately journaled queries.
Queries recheck current daemon/executor authority, Team/actor/scope/binding, discovered tool schema,
protocol version, and the legacy HTTP session before committing a FULL-synchronous send. A modern
`tasks/get` can settle the original attempt from its embedded result. A legacy `tasks/get` reporting
`completed` stays pending until `tasks/result` supplies the actual tool result. Neither path repeats
the originating write or creates another tool attempt. Query errors leave the tool pending; a valid
failure/cancellation fact is distinct from an outer RPC error or cancellation acknowledgment.

Database fixtures cover additive migration, concurrent query admission, old receipts without handle
metadata, scoped inspection, restart recovery, and late/conflicting terminal facts. HTTP fixtures
cover both protocol eras, fresh activations, malformed/foreign handles, missing capability metadata,
tool errors, and failed/cancelled task statuses. March batch admission rejects task members before
changing lifecycle or consuming request IDs. The real binary/shim fixture checks the persisted query
before upstream I/O, unchanged terminal result delivery, and exactly one originating tool attempt.

The lookup checkpoint passes 30 database journal tests, 68 MCP crate tests, and 23 root MCP tests, including the configured-launch
child invoked by its parent. Root/MCP/database all-target Clippy with warnings denied and the real
binary build pass with the validation commands above. Formatting, whitespace, and local document
links pass. Subsequent checkpoints add cancellation, input updates, and notification settlement.

Task cancellation now commits its own durable intent before HTTP while sharing the lookup path's
authority resolution, request preparation, bounded transport drain, and first-terminal-fact rule.
One cancellation per task attempt survives errors, lost acknowledgments, and restart. Modern
acknowledgments leave the tool pending; a valid legacy cancelled status can settle the originating
attempt. The real shim fixture follows a modern acknowledgment with a successful tool lookup and
rejects another cancellation before upstream I/O.

Three database fixtures cover additive migration, concurrent admission, scoped inspection, restart,
and late or conflicting facts. HTTP fixtures cover both wire eras, foreign handles, lost responses,
RPC errors, malformed responses, and fresh-activation resend rejection. Bridge tests cover the
legacy cancel capability and prevent March task batches from bypassing receipt admission.

Cancellation validation passes 33 database journal tests, 71 MCP crate tests, and 23 root MCP tests,
including the parent-invoked configured-launch child. Root/MCP/database all-target Clippy with
warnings denied, the real binary build, formatting, whitespace, and local document links pass.
No dependencies, protobuf definitions, or Bazel configuration changed.

Task input observations now commit digest-only input IDs and request identities before delivery.
Update sends consume their supplied inputs atomically under the recorded task's current authority,
with partial answers preserved unchanged. Unknown/used IDs roll back the complete update, and a
lost acknowledgment does not release input consumption. Repeated polls retain both identity and
consumption. If the upstream changes a request under the same ID, the query records invalid input
and the conflicting ID remains persisted across reopen, denying further updates for the task.

Database fixtures cover migration, partial/concurrent consumption, rollback, scoped sequence pages,
equivocation, restart, late acknowledgments, stale authority, and cancellation intent. HTTP fixtures
exercise partial answers across activations, raw payload preservation, stale polls, lost responses,
RPC errors, and changed request payloads. The real shim fixture now receives elicitation input,
sends its answer, rejects another answer for the consumed ID, then observes cancellation and final
tool results independently. Modern subscription delivery is covered by the follow-up below.

Review against the current HTTP binding found that task methods omitted `Mcp-Name`. A real HTTP
regression reproduced the missing header. The transport now mirrors `taskId` for get/update/cancel,
including the standard encoding for padded values, while preserving the request body. Journal HTTP
fixtures also check the name header on actual task exchanges.

Input/header validation passes 37 database journal tests, 75 MCP crate tests, and 23 root MCP tests,
including the configured-launch child invoked by its parent. Root/MCP/database all-target Clippy
with warnings denied, the rebuilt real binary, formatting, whitespace, and local document links
pass. No dependencies, protobuf definitions, or Bazel configuration changed.

### Subscription follow-up (2026-09-16)

Modern subscriptions use separate daemon-owned streams with short authority guards and no ordinary
request body deadline. Acknowledgments must precede notifications and honor only requested filters.
Known task IDs receive private observation permits under the original scope/binding/schema.
Notification and input facts commit before delivery, with bounded digest-only history and
deduplication across subscription IDs. Polls, cancellation responses, and notices share first-fact
settlement. Stdio cancellation closes the selected HTTP stream locally. The shim closes idle
subscriptions on EOF while ordinary calls finish their durable drain.

Pinned core schema review confirmed that `clientInfo` is optional while client capabilities remain
required. The transport now accepts that shape and validates supplied implementation fields. Task
notification parsing shares lookup classification for tool errors, task errors, cancellation, and
input-required observations.

Subscription validation commands:

```bash
cargo test -p agenthub-db --locked --offline mcp_operations
cargo test -p agenthub-mcp --locked --offline
cargo clippy -p agenthub -p agenthub-mcp -p agenthub-db --all-targets --locked --offline -- -D warnings
cargo build -p agenthub --bin agenthub --locked --offline
cargo test -p agenthub --locked --offline mcp_
cargo fmt --all --check
```

Database fixtures cover migration/deduplication, scope admission, input conflict, late settlement,
and history capacity. HTTP fixtures cover ordering, filters, typed IDs, cancellation, concurrent
subscriptions, idle revocation, and abandoned preparation cleanup. The real binary fixture receives
subscribed task inputs and results through signed RPC, checks receipts before provider delivery,
and closes with an idle subscription. An authenticated startup fixture proves list-only subscription
admission, task/resource denial before running, and cleanup without a retained executor guard.

Validation passes 40 database journal tests, 80 MCP crate tests, and 25 root MCP tests. The configured
launch child remains explicitly invoked by its parent. Root/MCP/database all-target Clippy with
warnings denied, the actual binary build, formatting, whitespace, and changed-document local links
pass. New Rust files are covered by existing Bazel source globs; dependencies, protobuf definitions,
and Bazel configuration are unchanged. Slice 9 is still incomplete and unpublished.

### Legacy notification follow-up (2026-09-16)

Legacy task status messages now pass through the durable journal on ordinary POST exchanges and
independent GET streams. Session opening captures a private observation owner under the existing
executor fence. Incoming facts must match the original accepted task's Team/actor, server, scope,
binding, protocol, and private HTTP session. Catalog invalidation cannot erase an admitted fact,
and executor exit does not give the observation owner permission to send another request.

When a status precedes its creation receipt, a bounded transient queue lets callbacks continue.
Receipt completion wakes waiting streams, which commit correlated facts before provider delivery.
Count, byte, and time limits reject unmatched delivery; the original tool response still drains
and commits. Legacy completed status stays pending until an actual result fetch. March batches
reject task notifications while retaining factual results from the same response frame.

Focused validation covers scoped ownership and late facts, early notices with a required callback,
pending count/byte/deadline limits, delivery loss, and unsupported protocol shapes. The actual
binary fixture places notices on GET and POST before the creation receipt, answers the callback
through signed RPC, checks receipt persistence before stdio delivery, fetches the actual result,
and preserves it after a later cancellation. It also exercises a control POST notice and normal
session deletion. Validation uses the database, MCP, real binary, root MCP, Clippy, and formatting
commands above.

This checkpoint passes 41 database journal tests, 85 MCP crate tests, and 26 root MCP tests, with
the configured-launch child invoked by its parent. The final root fixture explicitly waits for a
GET marker after the early status before releasing the POST creation receipt. All-target Clippy
passes with warnings denied, as do formatting, whitespace, and 75 local links in the changed docs.
The actual binary was built before the CLI fixtures; the only subsequent production edit replaces
an unnecessary lazy `Option` closure with its equivalent `and` expression. No dependencies,
protobuf definitions, tables, or Bazel configuration changed. Slice 9 remains incomplete and unpublished.

### Integration access follow-up (2026-09-16)

Bindings now supply an explicit trusted policy for tool names, exact resource URIs, resource
reference templates, prompt names, callback methods, and logging controls. Single requests,
March batch members, and modern subscription filters check these grants before consuming IDs
or changing lifecycle state. Discovery filters unauthorized entries without rebuilding approved
schemas or losing pagination/extension fields. Server capabilities reflect disabled surfaces;
resource reads with an unauthorized returned URI fail before provider delivery. An opaque deferred
envelope remains intact, but it cannot bypass checks on resource content included alongside it.

Callback registration checks incoming methods before granting reply authority. Deferred input
requests in continuation responses and task responses/notifications use the same grants; an
unauthorized callback cannot reach the provider through a different protocol envelope. Revoked prepared
exchanges stop before execution, while admitted operations retain their factual drain. The configured
Mem resolver now selects the tool-only policy and includes its version in the configuration
fingerprint. This closes unchecked non-tool forwarding, but is an interim restriction: full Mem
namespace authorization and scoped non-tool access remain requirements of the original goal.

Focused tests cover exact grants and template/read separation, discovery and capability projection,
unchanged errors/deferred envelopes, callback registration, revoked preparation, and atomic batch
rejection. Authenticated RPC fixtures exercise permitted and denied resource/prompt/completion and
subscription requests against a real HTTP server, prove denied IDs remain reusable, and check
March response projection per request ID. Deferred prompt responses cover an allowed roots request
and a denied sampling request without forwarding the forbidden input. Revocation returns a correlated
JSON-RPC admission error and dispatches no HTTP request. Existing real-binary fixtures remain regression gates
for the configured Mem tool and callback flow.

Validation commands:

```bash
cargo test -p agenthub-mcp --locked --offline
cargo build -p agenthub --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp_ --locked --offline
cargo clippy -p agenthub -p agenthub-mcp --all-targets --locked --offline -- -D warnings
cargo fmt --all --check
```

This access checkpoint passes 92 MCP crate tests and 28 root MCP tests, with the configured-launch
child explicitly invoked by its parent. The actual binary build and root/MCP all-target Clippy
with warnings denied also pass. The focused access tests include direct and deferred callbacks,
task input envelopes, resource output rejection, and post-revocation admission. No database,
protobuf, dependency, or Bazel configuration changes are included.

### Process crash follow-up (2026-09-16)

A new root regression runs the production control service and operation journal in a separately
killable test process, with the actual stdio shim and an independent HTTP upstream. The journal is
file-backed. Each case terminates the control process without shutdown handlers and then stops
the old shim before releasing its executor reservation.

The four checkpoints cover no tool call yet, an upstream-received write with its response withheld,
parsed success before commit, and committed success whose provider output has not been consumed. Recovery reacquires the OS
daemon lock and claims a new generation. It records `daemon_restart` ambiguity exactly once,
retains successful receipts, rejects old executor credentials, and starts a fresh activation/shim.
A new RPC ID cannot resend the ambiguous write. A write that had never been called can run normally.
Neither discovery nor startup performs an automatic tool send, and stored records omit payloads.

The uncommitted-success case installs a TEMP SQLite trigger and update hook on the single test
connection. The hook parks its SQLite worker only after the attempt changes to `succeeded` inside
the uncommitted transaction. The parent waits for that marker, checks the still-committed `sent`
state and absence of provider output, then kills the process. Reopen proves the partial success
rolled back and the unresolved write remains unreplayable. The hook and TEMP objects disappear
with the child; the production journal implementation is unchanged.

The crash fixture passes all four cases. Validation also covers the shared activation
fixture used to create the original and replacement reservations:

```bash
cargo test -p agenthub --lib real_mcp_proxy_survives_daemon_crashes --locked --offline
cargo test -p agenthub --lib internal::service::tests::loop_activation --locked --offline
cargo clippy -p agenthub --all-targets --locked --offline -- -D warnings
cargo fmt --all --check
```

The process test complements the existing database child-exit checks for committed prepared,
sent, and completed rows. All 28 activation-related root tests pass, including the parent that
executes the ignored crash helper once per checkpoint. Root all-target Clippy with warnings denied,
formatting, whitespace, and 75 local document links pass. The actual shim binary matches the
unchanged production implementation from the access checkpoint. Static MCP launch compatibility
is covered by the following follow-up.

### Static launch compatibility follow-up (2026-09-16)

The ACP startup implementation now accepts its static MCP loader through a private helper. The
public entry continues supplying the existing loader; loop launch configuration still bypasses it.
This lets the launch fixture use an isolated configuration file without changing the process home
directory or installing a production test switch.

A fake ACP provider exercises fresh and resumed sessions with and without HTTP MCP support. It
starts the configured native stdio service, performs MCP initialization/discovery, calls its tool,
and checks the response and extension fields. The fixture verifies unchanged command arguments,
environment entries, HTTP URL/headers, and the provider capability filter. Two additional loop
launch cases verify the static loader is never invoked. Only fixture observations are recorded;
the provider fixture does not store prompts or full ACP request payloads.

Validation commands:

```bash
cargo test -p agenthub-acp static_mcp_tests --locked --offline
cargo test -p agenthub-acp loop_ --locked --offline
cargo clippy -p agenthub-acp -p agenthub --all-targets --locked --offline -- -D warnings
cargo build -p agenthub --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp_ --locked --offline
cargo fmt --all --check
```

The static tests pass all six launch cases; eight loop-selected tests also pass (one overlaps the
static selection). The actual binary rebuild and 29 root MCP tests also pass; the two ignored
process helpers are explicitly executed by their parent tests. Root/ACP all-target Clippy passes
with warnings denied. No public API,
configuration schema, dependency, protobuf, or Bazel configuration changed.

### Read continuation follow-up (2026-09-16)

Resource reads and prompt retrieval now use a bounded session-local MRTR controller. Previously,
the generic control path could forward caller-supplied state and inputs without associating them
with an upstream receipt. The controller binds the method and original parameters, compares opaque
state by digest, and requires an unambiguous issued input anchor when no state exists. Reservations
prevent concurrent consumption while an unsent dropped request leaves its receipt usable.

Sending consumes the old receipt. A transport failure or upstream error cannot authorize another
continuation send. A successful intermediate response replaces it; a final response releases it.
Fresh reads remain available independently. Ten requests per chain, 64 active chains per session,
and shared retained-byte credits bound the state. Read receipts are transient correlation data;
the existing durable tool journal remains responsible for external write uncertainty.

The pinned final MRTR specification permits only tool calls, resource reads, and prompt retrieval.
The pinned tasks extension permits task creation only for tool calls. Generic controls now reject
other deferred shapes; read input methods also require their client capability family and existing
integration callback grant. Legacy requests and batches cannot carry modern continuation fields.

Validation commands:

```bash
cargo test -p agenthub-mcp --locked --offline
cargo clippy -p agenthub-mcp -p agenthub --all-targets --locked --offline -- -D warnings
cargo build -p agenthub --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp_ --locked --offline
cargo fmt --all --check
```

The HTTP fixtures exercise both read methods, exact wire preservation, changed parameters/state,
partial and extra inputs, ambiguous parallel requests, reserved-receipt ambiguity, unsent drop,
cross-session rejection, upstream errors and connection loss, bounded rounds, and capacity shared
between sessions. They use a store without a tool-journal schema to prove that reads do not create
synthetic tool operations. No database, dependency, protobuf, or Bazel configuration changes occur.

All 102 MCP crate tests and 29 root MCP tests pass. The root access fixture now declares the client
input capabilities it exercises and verifies a complete prompt continuation through signed RPC,
including changed-parameter and consumed-receipt rejection without another HTTP request. The two
ignored process helpers run through their parent tests. Root/MCP all-target Clippy with warnings
denied, the actual binary rebuild, formatting, whitespace, and 82 local document links pass.
A full temporary filesystem interrupted one root run; after archiving inactive test artifacts
with their original paths retained as symlinks, the unchanged test selection passes.

### Client capability follow-up (2026-09-16)

The shared observer now checks the client's declared support independently of the integration's
access policy. Legacy initialization keeps a fixed-size projection and clears it after handshake
failure. Modern exchanges retain only their own request's declaration, including later task queries
and subscriptions. Roots, sampling tools, and elicitation form/URL support use the same checks for
direct callbacks, MRTR inputs, task responses, and task notifications. Opaque content remains data.

Modern logging requires the request's opt-in and minimum severity, as well as the integration's
logging grant. Retired standard methods fail before request-state mutation; corresponding legacy
operations keep their existing admission. Discovery hides the modern tasks extension when tools
are disabled. None of these changes rewrite upstream capability metadata.

Unsupported callbacks cannot register response authority or prevent a previously admitted write
from reaching its durable result. RPC fixtures also prove that task input observations persist even
when the querying or subscribing request lacks elicitation support. Concurrent HTTP requests test
that an unrelated declaration cannot remove or grant another request's support.

Validation commands:

```bash
cargo test -p agenthub-mcp --locked --offline
cargo clippy -p agenthub-mcp -p agenthub --all-targets --locked --offline -- -D warnings
cargo build -p agenthub --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp_ --locked --offline
cargo fmt --all --check
```

All 110 MCP crate tests and 31 root MCP tests pass; the two ignored process helpers are executed
by their parent tests. Root/MCP all-target Clippy with warnings denied and the actual binary rebuild
pass. The first final Clippy run identified a nonminimal logging predicate; the equivalent simplified
predicate passes the full repeated validation. No database, dependency, protobuf, or Bazel changes
were required.

### Mem namespace authorization follow-up (2026-09-16)

Configured Mem launches now require the deployment's existing authenticated narrowed-key contract.
The daemon checks `GET /members/me` with the exact credential retained for MCP before recording
the launch snapshot, mounting tools, or starting the provider. It requires a workspace UUID, one
grant equal to the configured Team space, and an active effective write target in that space.
An actor's credential-profile override cannot select a different Team namespace. Full keys and
deployments without this contract fail explicitly; no new upstream API or credential is created.

The source gate is `nowledge-co/mem` at `f2d52afa86e5f17895f62d9d94608097f5581f8b`:
`nmem-cloud/src/routes/members.rs` exposes the authenticated membership/key-scope projection;
`auth.rs`, `key_scope.rs`, and `scope_access.rs` carry the key's live grants into MCP read/mutation
admission. `docs/design/DESKTOP_TEAM_PARITY.md` explicitly scopes key narrowing to Cloud and
excludes desktop/Family. The space-protocol acknowledgment remains a selector compatibility
declaration, not an authorization credential.

Authorization permits the standard scoped resource/prompt surfaces alongside dynamic tools. The
proxy still binds `space_id` only where the discovered schema declares it. It relies on the verified
upstream key to reject foreign opaque IDs and resources, preserving those native MCP/JSON-RPC
errors. The membership response remains private and transient; only its workspace identity enters
the launch fingerprint. Endpoint-based operation scope identity is unchanged pending alias work.

The probe has a ten-second deadline and a 64 KiB body limit, keeps the configured endpoint prefix,
marks credentials sensitive, and disables redirects, retries, and ambient HTTP proxies. Offline
configuration preflight uses the original synchronous validation without network access.

Validation commands:

```bash
cargo test -p agenthub --lib mcp_proxy::configured --locked --offline
cargo clippy -p agenthub --all-targets --locked --offline -- -D warnings
cargo build -p agenthub --bin agenthub --locked --offline
cargo test -p agenthub --lib mcp_ --locked --offline
cargo test -p agenthub --lib loop_launch --locked --offline
cargo test -p agenthub --lib loop_configuration --locked --offline
cargo fmt --all --check
```

HTTP fixtures exercise malformed/full/multiple/foreign/empty authorization, inactive placement,
large bodies with and without Content-Length, unsupported endpoints, redirection, private failure
redaction, rotating credentials, and actor-profile confinement. The isolated configured-launch
fixture now covers both rejection before provider/MCP startup and a successful actual ACP/shim/RPC
path with declared/undeclared tool scope, resources, prompts, and preserved upstream denials.
These fixtures validate integration behavior without a personal Mem account; they do not claim
validation of a running Mem Cloud deployment.

All six configured-Mem tests, 34 root MCP tests, three loop launch tests, and 14 configuration tests
pass (the selections overlap). Parent tests execute the two otherwise ignored process helpers.
Root all-target Clippy passes with warnings denied, and the actual shim binary was rebuilt after
the production change. An initial `loop_preflight` filter selected no tests; the final
`loop_configuration` selection includes both preflight regression cases. No dependencies, database
schema, protobuf, or Bazel configuration changed.

## Follow-Ups

- Complete slice 9's authority-alias reconciliation without changing immutable operation intents
  or authorizing replay of unresolved writes after an endpoint move.
- Integrate existing Mem scope/context bootstrap in slice 10 and app bindings in slice 14 through
  this same journal. Slice 9 remains open in [TODO](../todo.md).
