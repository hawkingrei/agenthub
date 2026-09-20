# Agent Loop Activation Contract

Status: implementation contract. The lifecycle service is delivered incrementally; the presence
of this specification does not enable automatic execution.

The control store implements policy configuration, idempotent trigger acceptance, pending
activation coalescing, safe event persistence, generation-fenced admission, structured finish,
continuation recording, and verified cleanup. Configured manual starts share durable reservations.
Local ACP execution now resolves a launch snapshot and uses generation-scoped actor control.
Offline Team configuration, explicit policy controls, and durable work-event intake are available.
Shared MCP tools, scoped Mem, role prompts, App tools/events, and product history are integrated.
No existing actor is implicitly opted in. Local Linux ACP is the current execution boundary;
verified restart recovery is implemented; real-adapter acceptance remains a rollout gate.

## Problem

The [loop runtime](agent-loop-runtime.md) needs concrete identity, persistence, and migration
boundaries before it can safely activate an offline member. Process-local start reservations,
mailbox delivery receipts, and provider completion cannot substitute for durable execution ownership.

## Scope

- Local Team member activation, with the existing `coordinator` and `worker` role identifiers.
- Additive control-store records, bounded admission, structured finish, and verified cleanup.
- Compatibility with manual sessions, run-partitioned inboxes, task claims, and daemon restart.
- Stable inspection data for subsequent tools, prompts, UI, Mem, and provider integration.

## Non-Goals

- Changing legacy `agent_loop` or reminder settings into automatic-start authorization.
- Replacing canonical task ownership, message consumption, or acceptance rules.
- Automatically migrating historical runs, memory files, or remote credentials.
- Treating a lease expiry or a process ID alone as proof that an old writer has stopped.

## Architecture

Domain records belong to the agent domain, with persistence in the control store. The daemon loop
service composes those boundaries with existing task claims, start admission, and process supervision.
Provider adapters execute an activation; they do not own scheduling or task state.

| Identity | Allocation and lifetime |
| --- | --- |
| Team/member/actor | Stable configured identity; actor remains addressable when offline |
| Trigger | Stable source kind/key and target scope; duplicate delivery returns the same receipt |
| Activation | One execution opportunity; accepted sources retain its ID before process launch |
| Execution generation | Monotonically increasing per actor; required by every ownership-sensitive write |
| Task attempt | Changes only under canonical task lifecycle rules |
| Mailbox run | Explicit Team execution partition; preserved across activations and provider sessions |
| Local session | A concrete launch and its persisted process/event history |
| Provider continuity | Optional adapter-specific resume reference; never substitutes for actor identity |

The first enabled Team work allocates its mailbox execution partition through the existing Team
service and records loop ownership explicitly. Initial allocation is idempotent and does not start
every member or create a task attempt. Subsequent activations reuse that partition. Rotation requires
an explicit scope transition that preserves access to outstanding inbox references; it is never a
side effect of fresh provider startup. Actor credentials identify exactly the eligible partition and
the current execution generation. An arbitrary caller-supplied run or activation ID grants no access.

## Contracts

### Policy and limits

Existing actors have no enabled loop policy after migration. Enabling requires a configured local
member, valid workspace/provider/role prompt, and required tool bindings. `disabled`, `enabled`, and
`suspended` describe admission policy independently from process status. Suspension preserves pending
work and does not itself cancel a running activation. An explicit stop/cancel operation handles the
running process and retains its reservation until cleanup. Resume does not revive canceled work.

Fresh sessions are the default for newly enabled loops. An explicit `resume` policy is accepted only
for adapters advertising supported continuity; both policies reread current task/IM/permission state.
Legacy manual-session preferences remain unchanged. Configuration changes create a new revision and
affect later launches; an activation records the prompt/configuration reference actually used.

Initial policy version 1 uses finite defaults:

| Limit | Default |
| --- | --- |
| Concurrent activations per actor | 1 |
| Pending activations per actor / Team | 32 / 256 |
| Retained trigger sources per pending activation | 64 |
| Admission lease / renewal interval | 60 seconds / 15 seconds |
| Startup attempt limit | 5 |
| Startup retry delay | Exponential, 1 second initially, capped at 60 seconds |
| Consecutive no-progress activations | 3, then an inspectable admission limit |
| Rolling activation window | 15 minutes |
| Activations per actor / Team per window | 12 / 120 |
| Standing registrations per actor / Team | 16 / 128 |
| Deferred admission reconsideration / candidates per scan | 5 seconds / 32 |

Limits are validated positive bounded configuration, snapshotted by revision. A caller cannot reset
budgets by submitting a new source ID. Startup failures and no-progress history survive restart.
When members configure different Team limits, the tightest non-disabled member limit applies to
all producers in that Team; rolling counts use the longest configured Team window. A more permissive
member cannot bypass the Team's existing bound. Deferral advances the next admission check without
changing the source deadline. Bounded scans move deferred work behind other eligible work, and
repeated unchanged deferrals do not append duplicate lifecycle events.
Reaching a limit retains accepted work and records the reason; excess *new* work is explicitly
rejected before acceptance. Operator resume/reset is explicit. A canonical progress transition or
new actionable dependency revision may reset consecutive no-progress accounting; elapsed polling
time or a provider's unsupported success assertion cannot. Time-window limits naturally expire.

### Additive persistence

Use separate tables for actor policy, activations, trigger sources, execution reservations, and
activation events. These records refer to existing actors, Teams, mailbox runs, tasks, and sessions;
they do not duplicate editable task state. Later scheduling and tool journals add their own records.

- Policy stores scope, revision, enabled/suspended state, session policy, budgets, and mailbox binding.
- Activation stores state, due time, generation/daemon ownership, launch/session references, outcome,
  continuation/wait references, and lifecycle timestamps.
- Trigger source uniqueness includes target scope, source kind, and stable source key. Immediate
  sources may coalesce into one pending activation up to the source limit. A future due-time source
  never becomes actionable early through coalescing. Every accepted source stays attributable.
- Reservation stores the single effective actor execution owner, generation, lease, and process
  identity. Lease expiry does not delete the reservation. Legacy/manual starts participate in the
  same reservation boundary once loop execution is enabled.
- Events store ordered IDs, activation/actor/scope/generation, typed event kind, safe references,
  status, and timestamps. Do not accept arbitrary diagnostic JSON, prompt bodies, tool arguments,
  outputs, credentials, or memory bodies into this trace.

Migrations are additive and repeatable. Do not infer loop ownership from an old `running` status,
an enabled watchdog, or a reminder row. Retain legacy rows and wire enum meanings. A deployment
rollback disables new admission and preserves the added records for inspection and later recovery.

### Admission and ownership

Claim pending work, validate policy/budgets, increment the actor generation, reserve execution, and
record admission in one control-store transaction. Reuse the existing spawn concurrency limiter
after durable admission. Do not hold a DB transaction while starting a provider or calling a tool.

Each state mutation validates actor/Team scope, activation, generation, daemon ownership, and the
live lease. Existing task claims must also authorize task effects; an actor reservation is not a
task claim. A newer daemon generation fences local control writes but does not by itself prove
that the old process tree cannot write files or external services.

An expired or uncertain executor remains reserved until supervision establishes process-tree
cleanup or another verified loss of execution authority. Missing in-memory handles are insufficient.
An uncertain local or remote writer blocks replacement and is visible as interrupted ownership.
Copy/move/remove/rebind operations cannot bypass reservations or pending work by checking only
`agents.status`; reconcile scope changes atomically under suspended intake.

### Offline configuration and scope changes

`spec.execution_mode = "loop"` separates Team configuration from process startup. Creating a Team,
adding or copying members, and editing its roster create disabled member policies without starting
providers. A coordinator with zero workers is valid. Loop defaults do not inject resident prompts or
synthetic phase steps. Omitted mode and `resident` retain the existing creation/start behavior.

`GET /api/teams/{team_id}/members/{member_id}/loop` returns policy and a safe preflight projection to
authorized inspectors. Owner-authorized `PUT` requires `expected_revision`, `state`, `session_policy`,
and bounded `limits`. Enabling or resuming admission requires successful preflight; suspension and
disablement remain available when preflight fails. Policy changes do not create work triggers, cancel
accepted work, or stop active execution. Session continuity changes wait for retained execution
cleanup. Budget updates preserve the lease duration already captured by an active reservation.

Preflight checks local provider executability, workspace policy, unique membership, actor control,
profile support, supervision, and declared required capabilities. It also runs before each provider
launch. Resume capability is negotiated before entry; static preflight does not promise a successful
provider handshake. Required Mem/MCP capabilities fail explicitly until the corresponding proxy
integration is available. Results expose bounded reason codes, not provider arguments or credentials.

Configuration operations serialize against starts using sorted per-actor gates and remain owned by
the daemon through caller disconnects. Membership and workspace guards share the actual SQLite write
transaction. A scope change requires suspended/disabled admission, no retained executor or pending
activation, no open provider session, and resolved task ownership, pending mail, reply obligations,
and permissions. Expired claims require canonical release/handoff; expiry alone is insufficient.
Direct ACP mode/model/config changes are rejected while an automatic activation owns its immutable
launch snapshot; persisted Card profile changes apply to later launches.

Copying a Card assigns a new Agent ID and disabled policy, without copying mailbox, activation,
claim, session, or credential state. Copying workspace contents also requires source quiescence.
Removing an empty identity detaches its policy. Retained activation history keeps its original Team
scope and blocks destructive deletion or identity reuse in another Team; copy the Card for a new
scope. This does not introduce a cross-Team transfer workflow. Team optimistic-update timestamps
advance monotonically so edits in the same second cannot reuse a stale compare-and-swap value.

### Durable work intake and source recovery

For Teams with `spec.execution_mode = "loop"`, canonical addressed messages, member mentions,
engaged-thread replies, and nonterminal task assignment changes stage work in the same SQLite
transaction as the canonical write. A failed intake budget rolls back the write and returns
HTTP 429 or gRPC `resource_exhausted`. A committed source survives process loss before mailbox
fan-out; delivery replicas reference that canonical message and do not create another wake.
Unaddressed discussion and self-authored messages do not automatically wake every member. Admission
retires obsolete assignment sources after reassignment or terminal status, while retaining addressed
discussion that still needs a response.

Mentions share the normal Team message parser: explicit mention arrays, `<at>` markup, and bounded
`@member_id` tokens. Thread engagement includes prior authors and prior mentions on the root and
its replies, within the same conversation. Members may opt out of implicit reply wakes with
`members[].loop_intake.engaged_thread_replies = false`; explicit addressing and current mentions
still apply. Disabled members retain canonical messages without automatic intake. Suspended members
retain accepted pending work, with admission paused.

`agenthub actor loop-activate --member-id <id> --source-key <key> [--task-id <id>]` requires current
activation-scoped credentials and the `loop:activate` permission. The server derives the requesting
actor, activation, and Team; request payloads cannot supply identity. The key identifies a business
request, scoped to the scheduling identity and target. Retrying from a later activation preserves
the original receipt and attribution; changing its task reference is an idempotency conflict.
Scheduling grants no membership, assignment, task claim, or acceptance authority.

An owner with `runtime:operate` may request work through
`POST /api/teams/{team_id}/members/{member_id}/loop/activate` with `source_key` and optional `task_id`.
Its source records the authenticated user, without fabricating an actor activation. Explicit
requests to disabled policies fail; suspended policies accept pending work.

`agenthub actor loop-context` reads the live activation and pages its sources, with a default of
64 and a maximum of 256 per page. Follow `next_cursor` through `--after-source-id` until exhausted.
`agenthub actor loop-source --source-id <id>` resolves an exact source message, including a message
older than the recent task detail window or one whose delivery replica is not available. The source
must belong to the current actor, Team, and activation under a live fence. Message bodies stay in
canonical stores and are hydrated through the existing body-store boundary. These reads do not
consume messages or accept tasks; current task state still comes from the task tools.

### Durable future work

[Loop scheduling](agent-loop-scheduling.md) defines due-time and recurring follow-ups, task-status
conditions, thread-reply watches, bounded firing reconciliation, and authenticated inspection and
revocation. Registration and dependency observation share canonical write transactions. Accepted
firings use the same intake and admission budgets; no provider process is kept alive to wait.
Active registrations block scope changes and retained registrations preserve original actor identity.

### Finish and recovery

States are `pending`, `starting`, `running`, `finalizing`, `finished`, `interrupted`, and `canceled`.
Outcome is independently `progress`, `handoff`, `waiting`, `no_actionable_work`, or
`completion_proposed`. Task acceptance remains the canonical task service's decision.

An authenticated finish operation requires the live activation generation and bounded task/evidence
references. Persist the outcome and required continuation/wait atomically before acknowledging it.
Repeated identical finish requests return the recorded receipt; conflicting requests fail without
repeating effects. The receipt remains readable after cleanup, but does not authorize new mutations.
The initial progress-evidence form references a canonical task note written by the executing actor
in the same Team during its execution. A note is credited once across activations; it establishes
recorded work, not acceptance or a guarantee about the note's claims. An outcome's `progress` label
without new evidence increments the no-progress counter. Invalid continuation admission rolls back
both the outcome and evidence credit. Wait reasons are typed; dependency and standing registrations
are delivered separately from the initial due-time self-continuation record.
Runnable remaining work requires a durable continuation; waiting requires an event/dependency or due
time with an actionable condition. Outcome recording does not automatically consume IM.

After acknowledgment, the supervisor reaps the process tree. Only verified cleanup releases the
execution reservation and moves finalization to `finished`. New triggers racing before, during, or
after release remain pending under the same admission boundary. Cancellation invalidates obsolete
continuations but still needs cleanup. An early process exit without an outcome is interrupted,
even with exit code zero. Unknown external effects require reconciliation, not exactly-once claims.
Revocation retains every source and its trace. Independently accepted coalesced work remains pending.
Admission rechecks whether referenced tasks remain actionable before starting their continuations.
Configured Linux executors run beneath a single-threaded subreaper guardian. It adopts and reaps
descendants that leave the provider process group, then sends a private cleanup receipt. Provider
stdio remains the ACP transport; the provider never inherits control or witness descriptors.
Closing the daemon control connection requests cleanup even if the daemon cannot receive a reply.

New reservations start with durable `unstarted` executor state. Before any OS spawn, the launcher
locks a private witness file and atomically changes the matching live reservation to `guarded`.
Recovery can retire an expired `unstarted` reservation in the same transaction that proves no spawn
was authorized; any delayed launch then fails its owner/generation check. Legacy reservations migrate
to `unknown`, never to `unstarted`.

The guardian inherits the witness lock, durably records `started` before spawning the provider, and
records `cleaned` only after reaping its entire descendant tree. On startup and subsequent scheduler
ticks, a replacement daemon reconciles expired foreign reservations. It must acquire the exclusive
witness lock and verify the exact Team, actor, activation, owner and generation identity. An unlocked
`prepared` witness proves no provider was started; an unlocked `cleaned` witness proves cleanup.
The recovery keeps the lock through the database compare-and-swap and retires the witness afterward.
Recorded outcomes remain finished, while executions without outcomes become interrupted. Recovery
does not replay unknown tool effects, reopen accepted tasks, or change suspension/pending intent.

A missing, corrupt, mismatched or `started` witness, a live lock, guardian crash or cleanup timeout
retains the reservation. This includes legacy reservations without evidence. PID absence, lease
expiry, provider output and bulk session-status changes are not substitutes for cleanup proof.
The event-store directory must retain the private `.executor-recovery` directory across daemon
restarts; do not delete or edit these files to clear a fence. Back up database and evidence together
after quiescing executors; restoring a snapshot while its original executors are alive is unsupported.
This is an execution-cleanup boundary, not a security sandbox for hostile same-user processes.
Other platforms do not admit loop execution until equivalent verification is available.

Restart first reconciles loop reservations and process authority. Legacy startup cancellation
continues for legacy runs only; it must not cancel loop-owned partitions or reopen their tasks.
Neither bulk session-status updates nor loss of a handle constitutes loop cleanup evidence.
Live native permission callbacks are settled or interrupted within adapter semantics; business
decisions may instead be durable waits on scoped IM. Expired permissions never carry authority into
a new session.

### Tool and inspection surface

Actor tools expose structured finish, scheduling, and wait operations through existing authenticated
internal transport. The server asserts actor, scope, activation, and generation; prompts only carry
bounded identity and recovery pointers. Separate operator controls enable, suspend, resume, inspect,
or cancel within existing Team/agent management authorization. New public shapes are additive.

Product history returns paginated redacted activation records and events under Team/agent read
authorization, including current policy, outcome, wait, and cleanup state. Debug-only doctor DB
diagnostics remain debug-only. Production history must not depend on that debug path.

Persist wall-clock timestamps and a durable ordering sequence. Measure durations with process-local
monotonic clocks; do not compare raw monotonic clock values across restarts. Correlate spans by
activation ID, but use bounded metric labels. Each later trigger/adapter/tool integration adds its
own safe trace evidence when the behavior is introduced.

### Local ACP Launch And Actor Control

Resolve command, runtime profile, role/context, workspace, and skill inputs once before provider
startup. The durable projection stores a version, SHA-256 configuration digest, provider, workspace,
profile references, and entry-contract version. It excludes arguments, prompt/skill bodies, and
credentials. Repeating the same launch snapshot is idempotent; conflicting retry configuration
requires a later activation rather than overwriting the earlier snapshot.

Fresh policy ignores provider continuity. Resume policy requires advertised ACP session loading;
a rejected load fails startup without a fresh-session fallback. Required ACP mode/profile requests
must succeed before the session is ready. One activation entry is submitted after the runtime session
is bound and the activation is running. Provider reasoning/tool rounds stay inside that submission.
A completed turn without a recorded outcome is interrupted and cleaned up. Legacy idle controllers,
reminders, and mailbox prompt hints cannot inject another turn into this path.

The Codex adapter configures a live thread through `thread/settings/update`; a fresh thread does
not yet have the persisted rollout required by `thread/resume`. Local settings change only after
the runtime accepts them. Prompt submission preserves receive order, but waiting for its stop
reason runs outside the ACP dispatch loop. Cancellation must remain reachable during a pending
permission request and invalidate that request before reporting the canceled prompt result.

The entry points to `agenthub actor loop-context` and `loop-source` for durable sources,
`team-members`, `team-tasks`, and `inbox` for current canonical state, and `agenthub actor loop-finish --outcome-file <path> --json` for the bounded outcome.
These commands recover the signed stable mailbox; they do not scan historical run partitions.
Legacy resident role skills are not attached to the loop contract. Loop ACP sessions mount approved
tools through the shared proxy and operation journal; direct static MCP servers stay disconnected.
Legacy sessions retain their configured MCP behavior.

A private file with mode 0600 in a mode-0700 runtime directory supplies short-lived actor credentials.
Renewal replaces it atomically using the reservation's lease duration; the token never appears in
activation trace data. Missing/expired loop credentials fail without falling back to a legacy token
or shared-secret configuration. Requests validate the signed actor, activation, generation, active
mailbox, membership, lease, and current daemon owner. A per-actor operation guard prevents cleanup
from releasing authority before admitted control requests settle. The daemon owns those requests
through caller disconnects. After cleanup, only idempotent finish-receipt replay remains available.
Native permission callbacks are interrupted when their local session is cleaned up.

The [MCP proxy](mcp-proxy-transport.md) has a restricted protocol-bootstrap admission before the
entry turn: after launch configuration and the local session are bound, `starting` may initialize
and discover its already approved MCP bindings and answer their registered callbacks. This does
not authorize tool sends or ordinary actor controls. Those still require `running`, and the
operation journal independently enforces that boundary. Bootstrap retains the same signed
identity, active mailbox, membership, owner, generation, lease, and operation-guard checks.

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| Storage | Existing DB upgrade, repeated/interrupted migration, reopen, source deduplication, scope isolation |
| Ownership | File-backed SQLite concurrent claims, stale-generation rejection, renewal, manual-start race |
| Finish | Atomic outcome/continuation, idempotent receipt, conflicting finish, no lost wake around cleanup |
| Recovery | Crash around claim/effect/outcome/cleanup; surviving child; uncertain writer blocks replacement |
| Compatibility | Legacy watchdog/reminders and manual startup unchanged; loop run not startup-canceled |
| Context | Fresh/resumed task and inbox recovery without new task attempts or mailbox rotation |
| Local adapter | Guardian receipt, detached descendants, strict resume/profile negotiation, one entry turn |
| Installed Codex | `scripts/verify_real_acp_runtime.py`: official 0.150.1, live settings, native commands, persisted load/history, canceled permission and ignored late approval |
| Assembled runtime | Opt-in `loop_real_acp_dispatch_worker_and_fresh_acceptance`: real ACP/app-server/CLI/MCP, deferred discovery, scoped Mem, App revocation, signed event deduplication, fresh acceptance, local progress during Mem outage |
| Actor control | Credential rotation, stale owner/generation rejection, disconnect ownership, finish replay |
| MCP bootstrap | Bound launch/session required; startup initialization/discovery cannot perform a journaled tool send |
| Configuration | Offline creation/copy, preflight and authority, disconnect/start exclusion, concurrent removal/intake, claim/reply/permission guards |
| Work intake | Message/task atomic rollback, duplicate delivery, thread opt-out, assignment retirement, file reopen, offline leader/worker/report cycle, scoped source reads and scheduling |
| Limits | Durable startup/no-progress limits, per-Team fan-out, due-time isolation and suspension |
| Visibility | Safe trace after exit, stable ordering, authorization, bounded pagination, debug/release separation |

`python3 scripts/check_loop_activation_contract.py` is a deterministic SQLite/fake-process
experiment for transaction and cleanup ordering. It is an executable design example, not a test of
the production runtime. Subsequent Rust store/service/adapter tests must establish the real behavior,
with normal Cargo/Bazel checks and browser evidence for the eventual UI.

## Operational Notes

The initial supported path is explicit opt-in, local Linux ACP execution with verified guardian
cleanup. Scoped tools, role prompts, policy controls, and retained history use the same admission
boundary. Follow the [operator guide](../../userdocs/docs/core/durable-execution.md) for configuration,
provider qualification, suspension and recovery. Unsupported capabilities fail explicitly.
Track implementation and remaining validation in
[TODO](../todo.md#agent-loop-product-transition).

## Open Risks

- Process authority after an unclean daemon exit needs platform-specific supervision evidence.
- Budget defaults may need tuning from observed progress; changes remain explicit and versioned.
- Provider continuity and durable permission capabilities vary; advertised support needs fixtures.
- Mem/app writes are separate transactions with possible unknown outcomes.
- Remote credential delivery and remote-writer fencing require a separate implementation.

## Source Journals

- [Bounded loop scheduling](../journal/2026-09-15-agent-loop-scheduling.md).

- [Durable work events and offline delegation](../journal/2026-09-15-agent-loop-work-events.md).

- [Product definition](../journal/2026-09-15-agent-loop-product-definition.md)
- [Activation contract checkpoint](../journal/2026-09-15-agent-loop-activation-contract.md)
- [Durable loop control store](../journal/2026-09-15-agent-loop-control-store.md)
- [Fenced loop admission](../journal/2026-09-15-agent-loop-admission.md)
- [Loop outcomes and cleanup](../journal/2026-09-15-agent-loop-lifecycle.md)
- [Offline loop configuration](../journal/2026-09-15-agent-loop-offline-configuration.md)
- [Shared MCP proxy and bootstrap](../journal/2026-09-15-shared-mcp-proxy.md)
