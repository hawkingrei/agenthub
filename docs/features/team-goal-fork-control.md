# Team Goal And Fork Control

## Problem

An actionable Team task needs durable ownership until a terminal outcome. Bounded research must run
without preempting the task owner, and coordinator planning must keep active work within capacity.

## Scope

- task-backed goal leases, recovery, and terminal state
- short-lived read-only research forks attached to active goals
- conflict escalation, non-preemptive informational messages, and concurrency budgets
- single-owner Task and Step execution; dependency-based splitting instead of shared execution

## Non-Goals

- forks are not persistent Team members or standalone canonical tasks
- forks never write a workspace, run Git write commands, or perform external mutations
- mailbox remains the canonical coordination transport

## Architecture

A canonical Task becomes a goal only when the backend atomically reserves Team and member capacity.
Its assigned member holds the lease until `completed`, `blocked`, `cancelled`, or explicit handoff.
Ordinary mailbox messages are recorded without interrupting that lease.

`team_goal_leases` is the durable capacity reservation. An active row is one with no `released_at`
and an `expires_at` in the future; Team and member capacity are counted from those rows inside the
same transaction that creates the Task execution claim. Handoff and terminal Task transitions retain
the row for audit while recording its release reason.

One Task and one Step each have one active responsible member. Collaboration is modeled through
messages, evidence, review, dependency-linked child Tasks, or read-only forks; it never creates a
second concurrent owner for the same executable work.

A fork is a child of one active goal with a narrow question, deadline, acceptance criteria, and a
fixed read-only profile. It returns immutable evidence to its parent and cannot change task state,
create tasks or forks, claim workspace resources, or write externally.

`team_goal_forks` binds each fork to the parent lease generation. Creation checks the active parent
and the Team-wide fork limit in one transaction. Completion succeeds only while that exact parent
generation remains active, then atomically stores the immutable result, appends a `result` Task note,
and records an audit event. The v1 result schema is a bounded JSON object; broader schema support is
deferred until validation can remain explicit and deterministic. Sensitive result fields pass
through the same recursive redaction boundary as Task conversation payloads before persistence.

The coordinator plans owned, prioritized tasks with dependencies. Backend capacity reservations and
declared external-operation claims decide whether queued work can start.

## Contracts

### Goal Lifecycle

- States are `queued`, `active`, `waiting`, `blocked`, `completed`, and `cancelled`.
- An active goal has exactly one owner and one fenced lease generation.
- A canonical executable Task must have exactly one assigned member before it can become active.
- An executable Step has exactly one responsible member and cannot add a multi-owner execution path.
- Reconnect resumes from persisted goal state and latest task evidence, never an inferred session
  transcript.
- Only cancellation, explicit handoff, or a persisted conflict can interrupt an active goal.

### Read-Only Forks

- Every fork has one parent goal, a question, acceptance criteria, expiry, and result schema.
- Fork questions, acceptance criteria, result payloads, expiry, and Team concurrency are bounded.
- The read-only profile denies workspace writes, Git writes, process control, credential mutation,
  and external write operations.
- Completion appends a result with fork identity and timestamp to the parent evidence stream.
- A stale fork cannot complete after parent expiry, release, handoff, or generation replacement.
- Teamspace members with runtime-inspection capability may list fork state. Only owners and planners
  with Team management capability may create or complete forks through the HTTP control plane.
- The canonical HTTP routes are `GET|POST /api/teams/{id}/tasks/{task_id}/forks` and
  `POST /api/teams/{id}/tasks/{task_id}/forks/{fork_id}/complete`.

### Conflicts And Capacity

- Forks cannot cause workspace-write conflicts by contract.
- Parallelism requires independent dependency-linked Tasks with distinct owners, not several owners
  attached to one Task or Step.
- The backend rejects or queues starts that conflict on owner, capacity reservation, or declared
  external-operation claim.
- A discovered conflict freezes only the affected goal and notifies the coordinator.
- Defaults are three active goals per Team, two active forks per Team, and one active goal per
  member. Team policy may lower these limits.
- Reservation, release, and retry are transactional and idempotent.

## Validation Matrix

| Area | Required evidence |
| --- | --- |
| Goal control | Rust tests for reservation, terminal release, recovery, and stale lease fencing |
| Fork safety | read-only policy, expiry, no nested fork, and immutable parent result tests |
| Scheduling | capacity, dependency, owner, and external-claim conflict tests |
| Runtime | mailbox non-preemption and reconnect/resume tests |
| Web | goal/fork/conflict state rendering and browser flow |

## Operational Notes

Fork results are evidence rather than authority. The owner or coordinator decides whether they alter
the parent plan. Capacity and conflict records are control-plane state, not transient runtime data.

## Open Risks

- The read-only profile requires adapter-specific command and tool enforcement.
- External-operation claims need a small typed vocabulary before enforcement.
- Lease expiry needs fencing so stale owners cannot report terminal results.
- Workbench rendering and operator controls for fork state are not implemented yet.

The Teamspace membership, invite, review, and single-owner contracts are defined in
[teamspace-multi-user.md](teamspace-multi-user.md).

## Source Journals

- [Team collaboration playbook](teams-collaboration-playbook.md)
- [Team architecture](agents-teams.md)
- [Goal lease and read-only fork foundation](../journal/2026-08-07-team-goal-lease-foundation.md)
