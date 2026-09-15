# Agent Loop Activation Contract

Status: implementation contract. The lifecycle service is delivered incrementally; the presence
of this specification does not enable automatic execution.

The control store implements policy configuration, idempotent trigger acceptance, pending
activation coalescing, safe event persistence, and generation-fenced admission/reservation methods.
Daemon/provider wiring, structured outcome recording, and process recovery remain subsequent slices.

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

### Finish and recovery

States are `pending`, `starting`, `running`, `finalizing`, `finished`, `interrupted`, and `canceled`.
Outcome is independently `progress`, `handoff`, `waiting`, `no_actionable_work`, or
`completion_proposed`. Task acceptance remains the canonical task service's decision.

An authenticated finish operation requires the live activation generation and bounded task/evidence
references. Persist the outcome and required continuation/wait atomically before acknowledging it.
Repeated identical finish requests return the recorded receipt; conflicting requests fail without
repeating effects. The receipt remains readable after cleanup, but does not authorize new mutations.
Runnable remaining work requires a durable continuation; waiting requires an event/dependency or due
time with an actionable condition. Outcome recording does not automatically consume IM.

After acknowledgment, the supervisor reaps the process tree. Only verified cleanup releases the
execution reservation and moves finalization to `finished`. New triggers racing before, during, or
after release remain pending under the same admission boundary. Cancellation invalidates obsolete
continuations but still needs cleanup. An early process exit without an outcome is interrupted,
even with exit code zero. Unknown external effects require reconciliation, not exactly-once claims.

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

## Validation Matrix

| Boundary | Required evidence |
| --- | --- |
| Storage | Existing DB upgrade, repeated/interrupted migration, reopen, source deduplication, scope isolation |
| Ownership | File-backed SQLite concurrent claims, stale-generation rejection, renewal, manual-start race |
| Finish | Atomic outcome/continuation, idempotent receipt, conflicting finish, no lost wake around cleanup |
| Recovery | Crash around claim/effect/outcome/cleanup; surviving child; uncertain writer blocks replacement |
| Compatibility | Legacy watchdog/reminders and manual startup unchanged; loop run not startup-canceled |
| Context | Fresh/resumed task and inbox recovery without new task attempts or mailbox rotation |
| Limits | Durable startup/no-progress limits, per-Team fan-out, due-time isolation and suspension |
| Visibility | Safe trace after exit, stable ordering, authorization, bounded pagination, debug/release separation |

`python3 scripts/check_loop_activation_contract.py` is a deterministic SQLite/fake-process
experiment for transaction and cleanup ordering. It is an executable design example, not a test of
the production runtime. Subsequent Rust store/service/adapter tests must establish the real behavior,
with normal Cargo/Bazel checks and browser evidence for the eventual UI.

## Operational Notes

Land storage, admission, and deterministic lifecycle recovery before real provider enablement.
Connect scoped tools before switching role prompts. Expose workbench controls after backend policy
and history are inspectable. App and Rara tracks reuse these boundaries; unsupported capabilities
fail explicitly rather than claim parity. Track implementation and remaining validation in
[TODO](../todo.md#agent-loop-product-transition).

## Open Risks

- Process authority after an unclean daemon exit needs platform-specific supervision evidence.
- Budget defaults may need tuning from observed progress; changes remain explicit and versioned.
- Provider continuity and durable permission capabilities vary; advertised support needs fixtures.
- Mem/app writes are separate transactions with possible unknown outcomes.
- Remote credential delivery and remote-writer fencing require a separate implementation.

## Source Journals

- [Product definition](../journal/2026-09-15-agent-loop-product-definition.md)
- [Activation contract checkpoint](../journal/2026-09-15-agent-loop-activation-contract.md)
- [Durable loop control store](../journal/2026-09-15-agent-loop-control-store.md)
- [Fenced loop admission](../journal/2026-09-15-agent-loop-admission.md)
