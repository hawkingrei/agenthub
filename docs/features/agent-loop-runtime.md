# Agent Loop Runtime

Status: target design, pending implementation. This refines the
[product model](agent-loop-product-model.md); state names below are design vocabulary, not shipped
API enum values or a database migration.

The [activation implementation contract](agent-loop-activation-contract.md) selects identity,
policy defaults, storage boundaries, and compatibility gates for the initial local implementation.
[Loop scheduling](agent-loop-scheduling.md) defines the implemented future-work and revocation boundary.

## Problem

Task progress must continue after process exit. Duplicate wakeups, restarts, and delayed tool results
must not create concurrent owners or lose work. The existing idle watchdog only prompts an existing
ACP handle and cannot provide this lifecycle.

## Scope

- Shared activation, outcome recording, and recovery for leader and worker roles.
- Stable actor/workspace identity across temporary provider sessions.
- Tool responsibilities and Nowledge Mem integration during a loop.
- Migration of current Team runs, mailbox partitions, and adapter lifecycle boundaries.

## Non-Goals

- A second task planner implemented in the scheduler.
- Exactly-once execution of external effects.
- Public API or SQL changes before compatibility design.
- Migrating all providers and remote nodes in one release.

## Architecture

| Responsibility | Owner |
| --- | --- |
| Persist accepted triggers with source, target, scope, and correlation | Activation intake |
| Enforce suspension, due time, concurrency, leases, and budgets | Scheduler |
| Resolve Card, prompt, provider, workspace, tools, and Mem binding | Launch resolver |
| Start/resume a provider and deliver one configured activation prompt | Provider adapter |
| Read state, choose actions, execute tools, report evidence, and finish | Agent |
| Persist outcome and continuation/wait state with the current fence | Outcome recorder |
| Reap the process tree and report verified exit | Process supervisor |

These are responsibilities, not a requirement to create seven crates or services. Reuse existing
managers, stores, scheduling, actor transport, and supervision where ownership matches. The daemon
hosts the scheduler; individual agent processes can be temporary.

Provider adapters include the existing ACP runtimes and the
[direct Rara integration](rara-direct-integration.md). Adapter capability differences — durable
permission waits, resumable provider continuity, replayable event cursors — change what the
scheduler may claim about a loop, never task or IM authority.

Identity mapping:

- Agent/member/actor mapping survives process exit.
- An activation identifies one admitted loop, its triggers, and touched tasks.
- Local launch identity and optional provider continuity remain distinct from activation identity.
- Existing Team `run_id` remains a mailbox/history partition. Do not rotate it solely because a
  process exits or a new activation begins.
- An activation may handle intake without a task or advance related tasks within granted scope.
  Task attempts follow task-state transitions, not activation count.

## Contracts

### 1. Admission

Persist each accepted trigger with a stable source identity. Duplicate delivery must not create new
logical work. Different triggers may coalesce for one actor while retaining every source reference.

Initially serialize activations per actor; different workers may execute concurrently under existing
task ownership and workspace isolation. An activation lease does not replace the task claim.
Claims carry renewable leases and incrementing generations; state updates require the live fence.

A lease alone cannot fence external writes. Before replacing an expired executor, stop or establish
loss of authority for its process/workspace. Uncertain remote ownership stays visible and cannot
permit a second writer. Suspension prevents admission; task cancellation invalidates obsolete
continuations while preserving their history. Ordinary process absence does not disable an actor.

### 2. State And Outcome

| State | Meaning | Next boundary |
| --- | --- | --- |
| Pending | Accepted work awaits admission | Claim or cancel |
| Starting | Lease held; configuration resolved | Running or startup failure |
| Running | Agent uses tools within scope | Outcome recorded or interruption |
| Finalizing | Outcome durable; cleanup outstanding | Finished after verified cleanup |
| Finished | Episode and cleanup recorded | Separate future activation |
| Interrupted | Runtime/cleanup failed or ownership uncertain | Bounded recovery or operator action |
| Canceled | Admission/execution revoked | No automatic continuation of canceled work |

Outcome is separate: progress, handoff, waiting, no actionable work, or task-completion proposal.
Existing acceptance rules decide task completion. Provider turn completion and exit code zero do
not manufacture a successful loop outcome. Cancellation during execution is not proof of cleanup;
the execution reservation remains until the old process is fenced and reaped.

### 3. Finish And Continue

The agent records completion through a structured tool boundary, not a magic phrase parsed from
free text. Its concrete API is deferred; required inputs are activation/fence identity, outcome,
task/evidence references, and continuation or wait information. Existing role authorization still
controls task completion/reassignment; the finish tool cannot grant additional task authority.

Validate the live claim and persist outcome plus required continuation before acknowledging finish.
Use one transaction when records share the control store, or a transactional outbox for derived
delivery. Do not hold a transaction over IM delivery or Mem network calls. Those have separate
outcomes. Repeated finish requests return the recorded result without repeating effects.

After acknowledgment, the supervisor stops the process if necessary. Mark the loop finished after
verified cleanup; failures remain visible and block unsafe replacement. Recheck pending work under
the admission/ownership boundary when releasing execution so a trigger racing with exit survives.
An active agent can read new IM through tools; every incoming message need not inject another prompt.

Runnable remaining work requires durable continuation. Waiting work records its dependency/event or
due time and the condition for becoming actionable. Budget exhaustion must record continuation or an
inspectable limit condition; it is not task completion.

### 4. Wait, Approval, And Recovery

Business decisions requested through IM can allow exit with durable wait state. Native ACP permission
requests may depend on live callbacks: do not claim they survive process exit before adapter support
exists. Keep the callback within its timeout or settle it explicitly and record interruption. A later
session must not reuse expired approval or treat an unrelated reply as authority.

Daemon restart preserves suspension and pending work. In-flight records require lease/process
reconciliation; do not reset live leases or blindly repeat unknown effects. Startup failures use
bounded backoff, while permanently missing configuration becomes an inspectable blocked condition.
Retry limits and no-progress budgets must be explicit before rollout.

### 5. Tool Responsibilities

| Tool surface | Agent behavior | Authority |
| --- | --- | --- |
| IM/inbox | Read, acknowledge, reply, delegate, reference threads | Actor/mailbox and conversation services |
| Task list | Read goals/dependencies; update allowed progress with evidence | Canonical task service |
| Execution/artifacts | Inspect/change assigned workspace; retain evidence references | Existing workspace/provider policy |
| Memory | Read bound context, retrieve decisions, retain selected learning | Nowledge Mem authorization |
| Follow-up | Register continuation or a dependency/due-time wait | Activation service |
| Scheduling | Request member activation; register member follow-ups and standing triggers | Activation service within role authority |
| Registered app tools | Call tools that external apps declared and an operator bound | [App tool registration](app-tool-registration.md) scopes |
| Loop completion | Persist outcome and release execution | Current activation claim |

Tools return explicit errors and stable references. Check required capabilities before execution.
Missing tools do not justify ad-hoc cross-workspace writes. Role prompts describe responsibilities
without depending on provider-specific tool spelling. The tool set is extensible by registration:
externally declared tools join an activation only through an operator binding and the enforcement
proxy defined in [app tool registration](app-tool-registration.md); they never gain task, IM, or
scheduling authority beyond their approved scopes.

### 6. Mem And Session Recovery

Use the [local MCP proxy](nowledge-mem-mcp-proxy.md) for the configured knowledge scope. Tasks and IM
supply current work; Mem supplies relevant knowledge. Fresh sessions must recover through these
authorities. Resumed sessions must still check current task, permission, and binding state.

Mem success is separate from the local outcome transaction. Retain operation identity and redacted
source pointers for reconciliation; do not blindly replay unknown non-idempotent writes. Learning is
selective, attributed knowledge with evidence references, not an automatic transcript copy.

### 7. Agent-Initiated Scheduling

Agents are trigger sources, not process schedulers. A leader advances delegation by creating
durable triggers through tools; the scheduler alone admits, starts, and supervises processes.

- Addressed IM is an activation trigger: a message that mentions a member durably activates that
  member with the thread reference once it is eligible. A thread the member is already engaged in
  may re-activate it on reply without a new mention, under intake policy. Broadcast conversation
  does not force activation; intake rules decide whether unaddressed discussion becomes work.
- A leader may explicitly request activation of a member it coordinates, schedule a due-time
  follow-up for itself or a member, and register dependency wakes such as "activate me when this
  task closes". These requests enter the same durable intake as user triggers.
- Standing triggers are supported but bounded: an agent may register a recurring schedule or a
  watched condition (task/dependency/thread events) within operator policy. Each firing enters
  normal intake as one trigger; registrations are inspectable, attributed, and revocable, and
  suspension pauses their admission without deleting them.
- Requests carry the scheduling actor, its current activation identity, and a reason reference
  (task, thread, or dependency). The activation trace records who scheduled whom and why.
- Admission policy is not delegated: serialization, suspension, leases, budgets, and coalescing
  apply to agent-created triggers exactly as to user triggers. Operator suspension outranks any
  agent request, and scheduling never bypasses task ownership or fence checks.
- Fan-out is bounded. Per-actor and per-team budgets cap agent-created pending activations, and
  cyclic wake patterns — leader wakes worker, the worker's report wakes the leader — must converge
  through coalescing and no-progress budgets instead of ping-pong activations.
- Changing the roster is a separate authority. A leader may propose adding or adopting a worker
  from an Agent Card through the existing [adoption flows](team-agent-adoption.md); instantiation
  respects operator policy and grants the new member no claims or inbox history.

Addressed work, assignment, explicit scheduling, source recovery, and thread intake policy follow
[the durable intake contract](agent-loop-activation-contract.md#durable-work-intake-and-source-recovery).
Canonical writes and trigger acceptance share one transaction; delivery copies are recoverable
projections of that source, not additional scheduling requests.

### 8. Observability And Activation Trace

The activation is the correlation spine for loop telemetry. Every lifecycle record — accepted
trigger, admission decision, resolved launch configuration, provider start, recorded outcome,
continuation, and cleanup — carries the activation identity plus stable actor/workspace identity,
touched task ids, the mailbox `run_id` partition that was read, and any provider continuity id.
Correlation uses durable records, not process memory: the trace of a finished or interrupted loop
must remain reconstructable after its process exits.

Historical storage reads bind both Team and actor IDs and do not require a live executor or current
membership. Callers must separately authorize access to the historical Team. Activation pages use
descending creation time plus ID; source and event pages use activation-scoped cursors. Each page
contains at most 100 records and an explicit continuation cursor. Source projections contain typed
references and revocation state, excluding original source keys and raw input objects.

Release builds expose `GET /api/teams/{team_id}/members/{actor_id}/loop/activations`, the individual
activation, and its `/sources`, `/events`, and `/tools` pages. Every surface requires `runtime:inspect`
and access to the historical Team. Revoked Team access is rejected even if the caller can inspect
other runtimes. An actor leaving the roster does not erase the Team's history. Invalid page limits
or cursors return a bounded 400 response; missing or foreign activation records return 404.

Tool boundary pages contain only an approved tool name, surface, optional safe target reference,
generation, status, wall-clock boundaries, and an optional monotonic duration in milliseconds.
MCP records also identify their canonical operation and attempt. History reads depend only on loop
records, so controller history does not require optional MCP storage. MCP projections commit in the
same transaction as the send or completion record. `input_required` and `task_accepted` preserve
nonterminal receipts without asserting success or granting replay permission. Legacy attempts are
backfilled once without invented durations. Restart recovery and asynchronous task settlement have
no surviving monotonic clock; their durations remain absent.
A `control_rpc` record describes the controller RPC return, including stream establishment when
applicable; it does not assert a terminal upstream tool effect. A missing completion remains
`started` with no duration after restart, and must not be interpreted as proof of a live call.
Late observations may complete their original record after executor cleanup without restoring
execution authority. Trace spans attach activation, actor, mailbox, and generation references to
the existing subscriber; these identities are not metric labels.

Each activation records, with monotonic timestamps and stable references:

- trigger acceptance with source identity and every coalesced source reference;
- admission or deferral with lease/fence generation and the queue/suspension reason when deferred;
- the resolved prompt/configuration reference (version identity, not the prompt body) and the
  provider/placement selection;
- tool-boundary summaries: tool surface, target reference, status, and duration — never prompt
  bodies, tool arguments, or tool outputs;
- the recorded outcome, continuation or wait condition, and the finish acknowledgment;
- verified cleanup, or the interruption/ownership-uncertainty finding.

Metrics implement the product observation contract: pending-activation age, admission latency,
running duration, outcome and exit-reason distributions, startup failures, retry counts, duplicate
suppression, no-progress loop rate per actor, wait-condition age, and Mem availability. Alerting
keys on stuck durable state — old pending work, expired leases without fencing resolution, growing
no-progress rates — not on process uptime.

`GET /api/teams/{team_id}/members/{actor_id}/loop/metrics` uses the same authorization as history.
The default event window is 24 hours; `window_seconds` must be between 1 and 604800. Counts and
duration aggregates use that window. Queue/wait gauges describe the current durable snapshot;
duplicate suppression is a cumulative observed counter. Categories are bounded enums, with no
actor, activation, task, tool-name, or workspace labels. No observations means unknown, not zero
service availability or a zero-sample progress rate.

Admission latency measures the first admission after work becomes due, excluding deliberate future
scheduling. Running duration spans each generation's running-to-verified-cleanup interval. These
cross-process intervals are wall-clock estimates: clock regressions are counted and excluded from
duration samples. A retained reservation contributes to unsettled age even after interruption;
it never proves provider liveness or verified exit. Exit distributions use only verified cleanup,
with separate startup-failure, canceled, recorded-outcome, and unexpected-exit categories.

No-progress rate is the fraction of finalized, non-canceled activations without newly credited
canonical progress evidence. Reusing a task note is not new progress. A newer admission clears the
previous business-wait observation, while future pending work does not. Active registration wait
age starts at registration or its latest firing, so recurring waits reset after each firing.
Historical cleanup reasons are not inferred. Pre-instrumentation source counters carry an unknown
baseline; duplicate totals are lower bounds when such sources remain. Duplicate acceptance updates
one counter in the intake transaction without expanding the lifecycle event log.

Runtime spans reuse the existing `tracing` subscriber and optional fastrace bridge from
[runtime diagnostics](runtime-diagnostics.md); the activation id becomes a span attribute so
wall-clock timelines join durable lifecycle records. `agenthub doctor agent-trace` extends from
session-centric stall analysis to activation-centric explanation: given an actor or activation
reference, it must answer why the agent was activated, what state it read, what it changed, why it
exited, and what will wake it next. Its stall classification gains loop layers such as
`pending_not_admitted`, `lease_expired_unfenced`, `waiting_dependency`, and `continuation_missing`
alongside the existing provider/persistence/SSE layers. The diagnostics redaction rules apply
unchanged to trace storage and doctor output; provider-native identifiers appear only through the
adapter's safe-metadata allowlist.

## Validation Matrix

| Boundary | Required future check |
| --- | --- |
| Prompt selection | Same engine, distinct roles, one recorded configured entry prompt |
| Admission | Duplicate trigger, simultaneous claim, renewal, stale-fence rejection |
| Exit race | Trigger before/during/after finalization remains recoverable |
| Cleanup | Early exit, surviving child, failed cleanup, cancellation during execution |
| Recovery | Crash before claim, after effect, after outcome, before cleanup |
| Ownership | No replacement alongside an uncertain old workspace writer |
| Waiting | Dependency change resumes; unchanged recheck and expired approval do not |
| Session policy | Fresh/resumed sessions read current tasks and eligible inbox partitions |
| Agent scheduling | Leader-created trigger activates an offline member under normal admission; suspension and budgets still apply |
| Wake cycles | Leader/worker reply loops coalesce and stop at no-progress budgets instead of ping-pong activations |
| Standing triggers | Registration attributed and revocable; each firing is one normal trigger; suspension pauses admission |
| Mem | Missing binding, unavailable server, cross-scope access, ambiguous write |
| Visibility | Separate task, activation, process, and next-wake state |
| Trace | Finished and interrupted loops reconstructable from durable records without the process |
| Redaction | Trace and doctor output carry ids/statuses/durations, never payload bodies |

Implementations require focused Rust and adapter protocol tests plus current Cargo/Bazel gates.
Provider fixtures must cover completion/permission boundaries before live provider smoke validation.

## Operational Notes

First prove lifecycle recovery with a deterministic fake adapter, then connect real tools/Mem and
align prompts, then expose the workbench controls. Each slice needs its own acceptance evidence;
[TODO](../todo.md#agent-loop-product-transition) carries implementation work.

Existing `agent_loop` settings do not authorize automatic startup. Legacy manual starts, reminders,
and run records keep their behavior until explicit opt-in and compatible migration are implemented.

## Open Risks

- Concrete persistence and inspection APIs need a reviewed additive design.
- Run-scoped inbox recovery must not introduce cross-Team reads.
- Session reuse default and activation cadence remain open; fresh sessions must work correctly.
- Remote execution needs authority fencing and Mem credential delivery before parity claims.
- Adapters differ in shutdown and durable permission-wait support.

## Source Journals

- [History storage checkpoint](../journal/2026-09-17-loop-history-storage.md)
- [Product redefinition checkpoint](../journal/2026-09-15-agent-loop-product-definition.md)
- [Start scheduling](../journal/2026-08-28-agent-start-scheduler.md)
- [Delivery receipts](../journal/2026-08-28-team-runtime-delivery-receipts.md)
