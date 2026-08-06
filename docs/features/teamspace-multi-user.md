# Teamspace Multi-User Collaboration

## Problem

The current local account model and Team runtime are sufficient for a single operator, but they do
not define a safe shared-work boundary. A shared Team needs invite-based membership, lightweight
conversation, and review without turning a Task into a multi-writer coordination object.

The canonical execution rule is strict: one executable Task and one executable Step have exactly
one active responsible member. Collaboration must not create concurrent ownership implicitly.

## Scope

- Teamspace-scoped human and agent membership.
- External invite URLs for existing local accounts and new local-account registration.
- Lightweight channels, threads, mentions, evidence, and review around canonical Tasks.
- Single-owner Task and Step execution, lease fencing, explicit handoff, and dependency-based
  parallelism.
- Read-only fork consultation attached to an active goal.

## Non-Goals

- Organization or tenant billing, cross-instance federation, or external identity providers.
- Shared execution ownership, multi-assignee Tasks, or multiple writers for a Step.
- Making a channel, message, mention, or review comment an execution-control primitive.
- Giving a fork workspace, Git, credential, process-control, or external-write authority.
- Replacing the local account/session implementation in the first delivery phase.

## Architecture

### 1) Identity And Authority Layers

Keep the following identities separate:

| Layer | Canonical identity | Authority |
| --- | --- | --- |
| Local account | `users.id` | Instance authentication and platform capabilities. |
| Teamspace membership | `team_members` row | Visibility and Teamspace governance rights. |
| Runtime member | Team `member_id` | Agent/runtime identity and Task/Step assignment target. |
| Execution lease | Task or Step claim | The sole authority to actively execute and mutate its assigned work. |
| Read-only fork | Fork id | Bounded consultation only; never a Teamspace member or Task owner. |

A human account may be represented by a Teamspace member, and an agent may be bound to a runtime
member, but neither relationship grants execution authority by itself.

### 2) Teamspace Boundary

A Teamspace is the visibility and governance boundary for one Team. Team definitions remain owned
by a local account, while Teamspace membership determines who may read, discuss, plan, review, or
operate work in that Team.

Initial Teamspace roles are intentionally small:

| Role | Rights |
| --- | --- |
| `owner` | Manage membership, invites, Teamspace policy, and explicit handoff. |
| `planner` | Create and split work, assign owners, and manage dependencies. |
| `contributor` | Read Teamspace context, comment, and execute only assigned work. |
| `observer` | Read permitted Teamspace context and comment where policy permits; no task execution. |

Platform capabilities remain an outer gate. A Teamspace role can narrow access but cannot grant a
local account a platform capability it does not hold.

### 3) Invite And Registration Flow

An owner or planner with membership-management authority creates a single-use invite for a target
Teamspace role. The server stores only a digest of a high-entropy token, its expiry, intended role,
creator, and lifecycle state.

The invite URL carries the raw token in its fragment:

```
https://host/join#<token>
```

The browser reads and clears the fragment before submitting the token in a request body. This keeps
the token out of route paths, ordinary server logs, and referrer propagation.

Acceptance has two branches:

1. An authenticated existing local account accepts the invite and receives the Teamspace membership.
2. A new user completes local account registration and accepts the invite in one transaction.

The invite never grants access outside its Teamspace. It does not copy agents, workspaces, sessions,
credentials, caches, or private memory. Revocation, expiry, use, and acceptance are auditable and a
used or revoked invite cannot be replayed.

### 4) Task, Step, And Goal Control

`task.assigned_member_id` is the only executable Task owner slot. A canonical executable Task must
be assigned before it reaches `in_progress`; an unassigned work request may exist only before Task
materialization. A Task has one active lease generation and one active owner at any time.

`team_steps.member_id` is the only executable Step owner slot. A Step is a run-local compatibility
artifact, but its single responsible member remains mandatory. New product workflows should prefer
Task ownership; Steps must not introduce a second multi-owner model.

The owner starts or renews work through a compare-and-swap claim containing task-or-step identity,
owner identity, lease generation, and expiry. Stale owners are fenced from reporting progress or
terminal state. An active claim ends only through terminal completion, cancellation, expiry/recovery,
or explicit handoff.

### 5) Coordination Without Shared Ownership

Channels, threads, mentions, task comments, and review requests are collaboration records. They can
create evidence, raise a blocker, request a decision, or propose a work split, but they cannot
change an execution owner or start execution.

Parallel work is represented by a dependency graph of independent Tasks. Each materialized Task has
one assigned member and, when active, one lease. A planning request that needs several executors is
split before execution; it is not modeled as one jointly-owned Task.

Review is a state and evidence flow, not co-execution. A reviewer may accept, reject, or request
rework according to Teamspace policy. Rework returns to the current owner or requires an explicit
handoff before another member starts it.

### 6) Goal And Fork Interaction

An active goal is bound to the Task's current execution lease. Ordinary messages and review comments
are non-preemptive. Only cancellation, explicit handoff, lease conflict, or a declared safety or
external-operation conflict interrupts it.

A fork is a short-lived, read-only consultation child of one active goal. It returns immutable
evidence to the parent and may not write a workspace, invoke Git writes, mutate credentials or
external systems, create Tasks, change Task state, acquire resource claims, or create another fork.

## Contracts

### 1) Single-Owner Invariant

- Every canonical executable Task has exactly one `assigned_member_id`.
- Every executable Step has exactly one non-null `member_id`.
- At most one unexpired execution claim exists for a Task or Step.
- A member may observe, review, or comment on any permitted Task without becoming an executor.
- A channel membership, mention, message acknowledgement, or thread watch state never grants an
  execution claim.

### 2) Assignment And Handoff

- Planner assignment is explicit and records the assigning principal, previous owner, new owner,
  reason, and timestamp.
- A handoff is allowed only after the active owner releases the claim, the claim expires and is
  fenced, or an authorized owner performs a recorded forced handoff.
- The new owner must acquire a new lease generation before execution.
- A rejected compare-and-swap is a conflict, not a retryable permission to execute.
- Assignment changes and status transitions append durable Task evidence.

### 3) Task State Authority

| Transition | Required authority |
| --- | --- |
| `open -> in_progress` | Assigned member acquires current execution lease. |
| `in_progress -> waiting` | Current lease holder records the blocking evidence. |
| `in_progress -> in_review` | Current lease holder records result evidence and releases active execution. |
| `in_review -> completed` | Authorized reviewer or policy-defined acceptance path records a decision. |
| `in_review -> in_progress` | Current owner resumes with a new lease, or a handoff assigns a new owner first. |
| Any active state -> `canceled` | Current owner, authorized Teamspace authority, or policy-defined cancellation path. |

### 4) Membership And Visibility

- Every Teamspace-scoped read and write applies a server-side Teamspace membership predicate.
- Invite acceptance is atomic: consume invite, create or validate account, create membership, and
  write audit evidence together.
- Removing a member immediately prevents new task claims and new session access to Teamspace data.
- Removal does not silently delete historical messages, evidence, or audit records; active work is
  moved to `waiting` or explicitly handed off.
- Server-side authorization is authoritative; hidden UI controls are not access control.

### 5) Concurrency And Scheduling

- Capacity is reserved atomically when a Task goal starts and released idempotently at terminal
  completion, cancellation, or confirmed expiry.
- Limits count active execution leases, not Teamspace members, channel participants, or reviewers.
- A member has at most one active goal by default. Team policy may lower this limit.
- Independent Tasks may run concurrently only when dependency, capacity, workspace, and declared
  external-operation claims allow it.

## Validation Matrix

| Area | Required evidence |
| --- | --- |
| Invite security | Expiry, revocation, single use, digest-only persistence, fragment clearing, replay denial, and audit tests. |
| Teamspace isolation | Two humans, two agents, and two Teamspaces covering invite, accept, read, write, revoke, cache expiry, export, and direct-id access denial. |
| Task ownership | Reject unassigned execution, concurrent claim, stale lease update, implicit takeover, and multi-assignee writes. |
| Step ownership | Reject missing `member_id`, duplicate active Step claim, and stale Step completion. |
| Handoff and recovery | Verify release, expiry fencing, forced handoff audit, dependency preservation, and no duplicate active execution. |
| Fork safety | Verify read-only tool policy, no nested fork, no Task mutation, expiry, and immutable evidence return. |
| Web | Browser flows for invite acceptance, role-gated controls, ownership display, review/rework, and conflict rendering. |

## Operational Notes

- The initial delivery remains single-instance and uses the existing local account/session system.
- Invite tokens are secrets: never include them in audit detail, analytics, message bodies, browser
  history, or logs.
- Task/Step claims, capacity reservations, handoffs, and audit events are control-plane authority;
  they must not be reconstructed from mailbox text or ephemeral runtime state.
- Existing nullable `team_tasks.assigned_member_id` is a compatibility shape. Migration must backfill
  or quarantine legacy ownerless rows before enforcing executable-task non-null validation.

## Open Risks

- SQLite requires carefully scoped transactions and compare-and-swap predicates for claim fencing;
  in-process locks alone do not protect restart or multi-node paths.
- Mapping human Teamspace members to runtime members needs an explicit lifecycle so a removed human
  cannot leave an orphaned active agent authority.
- Invite URL delivery needs a product-level trusted-host configuration; accepting arbitrary Host
  headers would create phishing and token-routing risk.
- Review authority must stay narrow enough that reviewers can accept evidence without gaining
  arbitrary workspace or runtime control.

## Source Journals

- [2026-02-24 Team Operating Model Specification](../journal/2026-02-24-team-operating-model-spec.md)
- [2026-03-19 Team Task Ownership Contract](../journal/2026-03-19-team-task-ownership-contract.md)
- [2026-05-21 Team Task-First And Mailbox Envelope Slice](../journal/2026-05-21-team-task-first-and-mailbox-envelope-slice.md)
