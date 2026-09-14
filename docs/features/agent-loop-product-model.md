# Agent Loop Product Model

Status: target product contract. Runtime migration is pending; this document does not claim that
the existing idle watchdog, reminders, or Mem policy helpers implement the complete loop model.

## Problem

The product has been described around keeping agent processes alive. Task progress must instead
survive process exit: an agent receives a loop activation, recovers relevant state through tools,
advances authorized work, records its outcome, and may exit. A later activation can continue that
work without requiring the previous process to remain alive.

AgentHub is an agent loop toolchain. It supplies activation, execution, IM, task-list tools, and
Nowledge Mem integration. Agents decide how to advance tasks within the operator's authority.

## Scope

The product definition establishes four requirements:

- Leader and worker agents remain, with different loop prompts on the same execution mechanism.
- Agent Cards remain part of startup and capability discovery.
- Each activation uses one configured role prompt; the agent process may exit after its loop ends.
- Nowledge Mem is an integrated knowledge service for work across loops.

This contract defines their lifecycle and state-ownership boundaries. Activation policy, provider
session reuse defaults, concrete tool APIs, and storage migrations need separate implementation
designs. The proposed delivery sequence is tracked in [TODO](../todo.md#agent-loop-product-transition).

The [runtime design](agent-loop-runtime.md) defines admission, outcome recording, shutdown races,
tool responsibilities, and recovery. `leader` is the product role name; the existing `coordinator`
identifier remains compatible. Domain specs link here where target behavior supersedes older
process-oriented contracts; unchanged API and storage rules continue to apply during migration.

## Non-Goals

- Requiring one model request or one tool call per activation.
- Implementing a fixed backend workflow for planning, delegation, execution, and review.
- Treating process exit, a completed model turn, or a delivered message as task completion.
- Replacing provider internals, the existing CLI/daemon packaging, or all runtime adapters.
- Moving canonical task and message state into Mem or creating another editable task ledger.
- Changing runtime code, public APIs, persisted formats, or production behavior in this definition.

## Architecture

### Product Objects

| Object | Responsibility | Lifetime |
| --- | --- | --- |
| Agent identity | Stable actor, role, ownership, and workspace association | Survives process exit |
| Agent Card and launch configuration | Describe identity/capabilities and resolve role prompt, provider, workspace, tool access, and memory binding | Reused across activations |
| Task | Goal, owner, status, dependencies, acceptance evidence, and next action | May span many loops |
| IM | Human intent, collaboration, replies, and durable inbox references | Survives sender/recipient exit |
| Loop activation | One admitted opportunity for an agent to advance work using its configured prompt and tools | Finite execution episode |
| Runtime session/process | Concrete provider execution and diagnostic history | May end with the loop |
| Nowledge Mem | Relevant prior knowledge, decisions, and reusable learning with provenance | Persists across loops |

Agent Card fields may refer to private launch configuration; discovery must not publish credentials.
Card presence and role identity do not require an online process. Existing discovery response shapes
remain unchanged until an explicit API migration is designed.

### Loop Lifecycle

```text
Accepted trigger -> Durable pending activation -> Admit and start agent
  -> One role prompt -> Agent reads IM/tasks/memory and uses execution tools
  -> Persist outcome, next action, and any wait condition -> Process may exit
  -> Later eligible trigger continues from durable state
```

During the tool-use phase, the provider may perform multiple reasoning and tool rounds. The single
prompt requirement applies to the product's activation entry: the runtime must not depend on a
sequence of bespoke planning, execution, review, and continuation prompts to advance the task.
Provider protocol messages, tool results, and runtime identity metadata remain necessary context.

The same mechanism runs both roles. The leader prompt guides intake, decomposition, ownership,
coordination, and acceptance. The worker prompt guides assigned execution, validation, reporting,
and blocker handling. Existing tool authorization remains enforced independently of prompt text.
The scheduler manages lifecycle and delivery; the agents choose the task plan. Provider adapters —
the ACP runtimes and the [direct Rara integration](rara-direct-integration.md) — execute
activations behind this same mechanism; adapter differences never change task or IM authority.

Tool surfaces are extensible by registration. External apps can declare tools and event triggers
through [app tool registration](app-tool-registration.md) and reach agents only through operator
bindings and the enforcement proxy; registered apps never become task, IM, or scheduling
authorities.

## Contracts

### 1. Stable Entry And Recoverable Context

- Each activation selects one versioned role prompt from the agent's effective configuration.
- Record the prompt/configuration reference used for the activation so history remains interpretable.
- Runtime context identifies the actor, workspace, applicable authority, trigger reference, and
  available tool entry points. It must remain bounded.
- Agents obtain changing assignments, inbox contents, task evidence, and knowledge through tools.
- No task may rely exclusively on an in-memory conversation for its owner, blocker, or next action.
- Prompt changes affect later activations; they do not silently change an active loop's instructions.

Fresh provider sessions and resumed sessions are both compatible with this definition. A fresh
session must be able to recover sufficient state through tools. Whether fresh sessions are the
default remains an explicit implementation choice, not an accepted requirement of this document.

### 2. Activation And Exit

- The control plane retains accepted work while the target process is absent.
- Closing a browser does not cancel a loop or remove pending work.
- A loop may finish after useful progress, a handoff, an external wait, or confirmed completion.
- Before a normal exit, persist the result and either completion evidence, the next actionable
  step, or a wait condition that explains how work can resume.
- Runnable remaining work needs a durable continuation trigger; it must not be stranded at exit.
- Waiting work needs a dependency/event reference or scheduled recheck. The agent need not keep
  a process open while waiting.
- Interrupted or crashed loops remain distinguishable from loops that persisted their outcome.
- Operator suspension prevents new automatic activations until explicitly resumed. Process absence
  alone must not be interpreted as either suspension or task completion.

The recommended initial activation policy combines explicit user actions, addressed IM, task
assignment/dependency events, and scheduled follow-ups. Periodic scans may reconcile missed wakeups.
This recommendation does not make every channel message, status read, or timer tick actionable work.
Subscriptions, cadence, retry ceilings, and execution budgets remain implementation choices.

Agents are themselves trigger sources. A leader delegates by assigning tasks, addressing members in
IM, and scheduling follow-ups or dependency wakes through tools; the addressed worker need not have
a live process, and the leader can exit after dispatching. Agent-created triggers pass the same
admission, suspension, and budget rules as user triggers, and never start processes directly — the
[runtime scheduling contract](agent-loop-runtime.md#7-agent-initiated-scheduling) defines the
authority and fan-out bounds.

### 3. Durable Coordination

- IM is the communication surface; task records are the authority for ownership and progress.
- A human message can remain discussion. Agent intake determines whether authorized work needs a
  canonical task under the existing ownership rules.
- The task-list tool operates on canonical tasks. Provider-local plans and workspace TODO files
  may be working aids or projections, but must not become competing Team task authorities.
- Inboxes remain addressable while recipients are offline. Agent identity must not depend on a
  particular process or provider session.
- Keep the existing distinction between trigger acceptance, runtime submission, message consumption,
  recorded loop outcome, and accepted task completion.
- Duplicate triggers require stable identity and idempotent admission. Recovery must fence stale
  execution and reconcile potentially completed effects before retrying them.
- Preserve one effective execution owner for a task. A trigger arriving during shutdown must remain
  pending or be admitted by the next owner; it must not disappear between the inbox check and exit.

Existing [task/attempt/run vocabulary](team-execution-vocabulary.md) remains authoritative for current
APIs. A loop activation is not automatically a new Team run, a new task attempt, or a renamed provider
session. In particular, current mailbox records use run partitions; a fresh provider session must
not make pending messages unreachable. The mapping needs an explicit recovery design.

### 4. Nowledge Mem Boundary

| Authority | Owned state |
| --- | --- |
| AgentHub | Tasks, ownership, IM/mailbox delivery, pending activations, execution outcomes, approvals, and output/artifact references |
| Nowledge Mem | Cross-loop knowledge, accepted decisions, relevant context, and selectively retained learning |
| Workspace/provider | Work products, temporary execution context, and provider session caches |

Use the existing [Mem MCP proxy contract](nowledge-mem-mcp-proxy.md) as the initial integration seam:

- Bind the Team/project to an explicit existing Mem space and a secure connection/profile reference.
- Keep stable actor mapping separate from temporary activation and provider-session identifiers.
- Discover current upstream tools and preserve their schemas and results. Inject `space_id` only
  where declared; tools without that field still require upstream authorization for the bound scope.
- Read context for the bound scope at activation and retrieve additional knowledge as work requires.
- Retain useful decisions and learning with task/activation/artifact provenance. Routine status
  updates remain canonical task writes, not duplicated memory task state.
- Preserve knowledge attribution and distinguish retrieved content from current user authority.
- Keep runtime outcomes and Mem-write outcomes separate. A failed memory write cannot erase local
  progress; an ambiguous non-idempotent write must not be blindly replayed by the next loop.
- Surface unavailable memory or a failed binding. Work requiring missing knowledge waits with a
  recorded reason; independently valid work may continue without claiming memory retrieval succeeded.

The existing integration contract initially covers local Team members. Broader standalone/remote
availability needs an explicit scope and credential-delivery design. Existing filesystem memory
remains a compatibility surface during migration; no automatic import or deletion is implied.

### 5. User Experience

The primary flow is: express a goal in IM, let agents maintain the task list and advance the work,
inspect progress/evidence, and provide decisions when needed. Agent Cards configure and explain
execution participants. Operators can inspect why an agent was activated, what it changed, why it
exited, and what will wake it next. That inspection is served by a durable per-activation trace
defined in the [runtime design](agent-loop-runtime.md#8-observability-and-activation-trace); it
must not require the process, the provider transcript, or a live browser session.

An agent with no running process is an ordinary state. Show task progress and the next wake/wait
reason independently from process health. Leader and worker history remains visible across exits.
Process controls and detailed session output remain available for diagnosis.

## Validation Matrix

These are acceptance requirements for subsequent implementation, not current test results.

| Scenario | Required evidence |
| --- | --- |
| Leader and worker execute | Same lifecycle engine, different selected role prompt, existing tool permissions enforced |
| One activation performs multiple tool rounds | One configured entry prompt drives work without phase-specific injected instructions |
| Process exits and is activated again | Stable actor/card; canonical task state and unread IM remain recoverable |
| Fresh provider session | Correct next action recovered without the previous transcript |
| Work remains runnable at exit | A durable continuation exists and later advances the task |
| Dependency is pending | Loop exits with a wait condition; unchanged checks do not falsely resume or complete the task |
| Trigger arrives while process exits | No lost activation and no concurrent unfenced task owner |
| Crash before/after an external effect | Explicit interruption and effect reconciliation; no unsupported exactly-once claim |
| Operator suspends an agent | Pending work stays inspectable and automatic startup stays disabled |
| Mem unavailable or scope denied | Visible bounded failure; no cross-space fallback or fabricated recalled context |
| Mem write outcome unknown | Local progress survives and the write is not automatically replayed |
| Browser closes and later reconnects | Execution/pending work survives; output and outcomes remain inspectable |
| Operator inspects a finished loop | Durable activation trace explains trigger, prompt reference, outcome, and next wake without a live process |
| Leader schedules a worker and exits | Agent-created trigger survives, activates the offline worker under normal admission, and the trace attributes the scheduling actor |

Lifecycle slices require focused Rust recovery/concurrency tests and the normal Cargo/Bazel gates.
Tool integration requires protocol and scope tests. UI slices require focused web checks and Chrome
DevTools inspection of an offline-to-active-to-exited agent and recovery history.

## Operational Notes

The current idle watchdog sends a prompt through an existing ACP handle. Current reminder delivery
explicitly does not start a stopped process. Both remain compatibility behavior until activation
semantics are implemented; turning them on does not enable this product model.

The Mem integration currently has policy helpers for scope binding, error classification, and
write-journal transitions. End-to-end proxy startup, context bootstrap, and write recovery still
need implementation evidence. Existing process supervision, admission control, task ownership,
mailbox persistence, and receipt fencing are reusable foundations.

Observe task progress, pending activation age, startup failures, exit reasons, retry counts, duplicate
suppression, and Mem availability. Process uptime alone does not show whether work is advancing.
The [runtime observability contract](agent-loop-runtime.md#8-observability-and-activation-trace)
defines the activation trace and metrics behind these observations, reusing the existing
tracing/fastrace and `agenthub doctor agent-trace` foundations from
[runtime diagnostics](runtime-diagnostics.md).

## Open Risks

- Session reuse policy and activation policy can materially affect cost and recovery complexity;
  select defaults in the lifecycle implementation design.
- Current run-scoped mailbox and permission flows may assume an available session. Recovery must
  preserve their delivery/authority boundaries while allowing process exit.
- Task state and Mem cannot be assumed to commit atomically. Keep explicit outcomes and reconcile
  partial success without duplicating ambiguous external writes.
- Multiple wake sources can cause repeated no-progress loops. Admission, coalescing, budgets, and
  wait conditions need explicit limits before rollout.
- Existing prompt and filesystem-memory contracts describe the old operating model. Align them
  incrementally with implementation; do not present a documentation change as a completed migration.

## Source Journals

These establish reusable implementation foundations, not completion of this target model:

- [Product redefinition checkpoint](../journal/2026-09-15-agent-loop-product-definition.md)
- [Start scheduling](../journal/2026-08-28-agent-start-scheduler.md)
- [Mailbox delivery receipts](../journal/2026-08-28-team-runtime-delivery-receipts.md)
- [Agent reminders](../journal/2026-09-06-agent-reminders.md)
- [Workspace memory](../journal/2026-04-10-team-workspace-memory-contract.md)
