# Team Channels And Threads Specification

## Problem

AgentHub Team already has canonical task, mailbox, and conversation contracts, but the boundary for
`channel` and `thread` can still drift:

- `thread` can be mistaken for a top-level workflow object instead of a focused reply lane;
- long background and execution detail can spill back into the parent channel timeline;
- mailbox ownership and task ownership can be conflated with thread participation;
- operators and agents can create parallel task or discussion lanes when one rooted thread already
  exists.

We need one stable contract for how Team communication lanes are organized so that `channel`,
`thread`, `task`, and `mailbox` keep distinct responsibilities.

## Scope

- Team communication-lane information architecture.
- Canonical `channel` and `thread` behavior inside Team workspaces.
- Thread identity, reply-target, and participation rules.
- Boundary between `thread`, `task`, and `mailbox`.
- URL/deep-link behavior for Team channel and thread selection.
- Conversation-level task-intent direction at the composer surface.

## Non-Goals

- Replacing canonical Team task ownership, Kanban, or mailbox execution semantics.
- Defining the full backend storage schema for every future channel feature.
- Full ACL policy for multi-user moderation, retention, or channel administration.
- Arbitrary nested threads or detached empty thread objects.
- Provider-specific external chat adapters.

## Architecture

### 1) Layered Team Communication Model

Team communication should distinguish four different things:

- `channel`
  - durable human-facing communication lane
  - broad visibility and summary-first discovery
- `thread`
  - focused reply lane rooted in one existing channel message
  - carries deeper context for one concrete topic
- `task`
  - canonical Team work object
  - owns assignee, priority, lifecycle, and durable note journal
- `mailbox`
  - canonical delivery and triage transport
  - carries handling disposition, thread claim, task link, and reply obligation state

The important boundary is:

- `channel` and `thread` are communication surfaces;
- `task` is the canonical work surface;
- `mailbox` is the canonical execution transport.

### 2) Summary-First Communication

The parent channel timeline should remain scannable and summary-oriented.

Required split:

- channel root message
  - summary entrypoint for one topic, issue, review point, or work request
- thread
  - full context for that topic:
    - background
    - progress updates
    - blocker discussion
    - logs
    - references
    - review follow-up

This lets Team communication stay discoverable without forcing every participant to ingest the full
deep context by default.

### 3) Thread Identity Model

A thread is derived from an existing channel message.

Canonical properties:

- one parent `channel`
- one `root_message_id`
- one stable thread reply target

This means:

- the first rollout should not create detached empty threads;
- the first-class model should not support nested thread trees;
- reopening a thread should resolve from the same rooted message identity rather than inventing a
  second logical topic.

### 4) Participation, Attention, And Ownership

Thread participation is not the same as task ownership or mailbox ownership.

The communication model should distinguish:

- thread participant
  - someone who is reading or replying in the discussion lane
- watcher
  - someone who wants visibility without current ownership
- thread owner / mailbox claimant
  - actor currently responsible for execution follow-up on that topic
- task owner
  - actor assigned to the canonical Team task

These concepts may overlap, but they must not be silently merged.

### 5) Shared Default Lane And Non-Default Channels

`# all` remains the default coordination lane.

Non-default channels are allowed when they represent durable work streams such as review, research,
or rollout. They must not become one channel per task or a substitute for Kanban ownership.

Channel taxonomy should stay sparse:

- `# all`
  - default coordination and broad visibility
- optional work-stream channels
  - explicit, durable scope
  - not arbitrary folders for every task

## Contracts

### 1) Channel Contract

Required baseline:

- `# all` is the default Team channel and must remain undeletable;
- a Team may expose additional channels with short explicit descriptions;
- selecting a channel changes the center communication lane without leaving the Team workspace shell;
- deleting or archiving a non-default channel must not delete canonical Team tasks or execution
  history.

Channel purpose:

- broad visibility
- summary-first discussion
- cross-cutting coordination
- durable work-stream grouping

Channel anti-goals:

- one channel per task
- channel as canonical task board
- channel as replacement for mailbox ownership

### 2) Thread Contract

Canonical meaning:

- a `thread` is a focused reply lane rooted in one existing channel message;
- a `thread` is subordinate to its parent channel;
- a `thread` is not a top-level Team lens or primary work object.

Required behavior:

- opening a reply target should open a dedicated thread context rather than inline-nesting a deep
  tree in the parent timeline;
- thread replies should render in the thread context, not duplicate as full inline rows in the
  parent channel timeline;
- closing the thread should return to the selected parent channel without losing channel context;
- the thread root should stay visible as the summary entrypoint into the deeper discussion.

### 3) Reply-Target Fidelity Contract

If an inbound message already has a concrete reply target, the default visible reply should stay on
that same target.

Rules:

- if a message arrives on a thread target, reply on that same thread by default;
- if a message arrives on a channel root, follow-up may continue in the channel unless the topic is
  explicitly moved into a thread;
- escalation, transfer, or cross-actor takeover may change the reply surface, but that change must
  be explicit and operator-visible.

This is a runtime correctness rule, not only a UI preference.

### 4) Thread Participation Contract

Seeing a root channel message does not automatically make an actor a full thread participant.

Participants should grow through:

- authoring the root message
- opening or following the thread
- being mentioned on the root or a later thread reply
- replying in the thread directly

Actor-local watching is allowed without ownership. Ownership remains governed by mailbox and task
contracts, not by passive thread visibility.

### 5) Thread-To-Task Boundary Contract

`task` remains the canonical Team work object.

Rules:

- thread is the preferred deep-context lane for a topic-specific execution discussion;
- task is the preferred durable surface for assignee, priority, state, and note journal;
- creating a new thread does not create a canonical task automatically;
- thread messages should normally attach to an existing task or remain discussion-only, rather than
  spawn parallel canonical tasks by default;
- top-level conversation or channel messages are the normal place to introduce new work intent;
- if a topic already has a canonical task and a rooted thread, follow-up should deepen that same
  lane instead of opening parallel tasks or parallel threads.

This keeps Team task-first while still allowing communication to stay contextual.

### 6) Thread-To-Mailbox Boundary Contract

Mailbox is the execution transport and triage surface for actor work; thread is the visible
discussion lane.

Required boundary:

- mailbox may carry `thread_root_message_id`, thread claim, task link, and reply obligation
  metadata;
- thread claim and mailbox handling disposition govern execution responsibility;
- thread participation alone does not claim execution ownership;
- `watching` is a visibility state, not a promise to act;
- unresolved human reply obligations should stay visible through mailbox-derived state even when
  the visible discussion happens in a thread.

See [team-mailbox-intake-and-ownership.md](./team-mailbox-intake-and-ownership.md) for the durable
triage and ownership contract.

### 7) Shared Default Thread Contract

The Team default communication lane may use a canonical shared-thread target under `# all`.

Required behavior:

- shared Team conversation remains one stable default discussion target;
- public Team clients should resolve that shared target through stable Team APIs instead of
  reconstructing it ad hoc;
- shared-thread canonicalization must prefer one durable rooted target rather than letting multiple
  equivalent “default thread” records drift.

This keeps the default lane predictable for both humans and agents.

### 8) URL And Deep-Link Contract

The Team workspace should support stable deep links for channel and thread selection.

Target direction:

- `/workspace/teams/:team_id?channel=:channel_id`
- `/workspace/teams/:team_id?channel=:channel_id&thread=:root_message_id`

Rules:

- `channel` controls the center timeline;
- `thread` controls the optional focused reply pane;
- missing `thread` means thread pane closed;
- missing `channel` falls back to `# all`;
- deep-link resolution should use rooted message identity instead of a second detached thread id
  namespace unless product evolution proves a separate namespace is necessary.

### 9) Actor Capability Contract

Thread handling must not remain a UI-only affordance.

Canonical actor capability direction:

- `team_thread_open`
  - open or resolve the thread rooted at an existing channel message
- `team_thread_reply`
  - append one reply in that rooted thread context
- `team_thread_view_in_channel`
  - jump back to the parent channel summary lane

Channel lifecycle capability direction:

- `team_channel_create`
- `team_channel_update`
- `team_channel_archive`
- `team_channel_delete`

Constraints:

- worker actors should not create or delete channels by default;
- thread open must stay anchored to an existing root message;
- actor/runtime capability should project the same canonical behavior as the UI shell.

### 10) Conversation-To-Task Intent Contract

Conversation surfaces may expose a lightweight task-intent affordance near the composer, but that
affordance must remain upstream of canonical task creation.

Rules:

- sending a task-intent message still creates a conversation or thread message first;
- task-intent metadata is a signal to coordinator/runtime, not a direct canonical task create;
- workers and other non-coordinator actors may propose task intent or suggest that existing work
  should be split, linked, or promoted, but they do not materialize new canonical Team tasks
  themselves;
- only coordinator/runtime may turn that task-intent signal into a new canonical Team task after
  choosing explicit ownership, priority, and linkage;
- the first rollout should not ask for full task configuration inside the composer;
- channel or thread context may shape the meaning of the request, but must not bypass coordinator
  materialization of canonical task records.

This preserves:

- conversation clarity
- thread continuity
- task-first ownership discipline

## Validation Matrix

1. Channel and thread routing
- opening a thread keeps the parent channel visible and routes replies into the thread context;
- closing a thread preserves the selected channel;
- deep links reopen the expected `channel + root_message_id` pair.

2. Reply-target fidelity
- human and agent follow-up on an existing thread stays on that same thread by default;
- escalation or transfer changes the visible surface only through explicit state transitions.

3. Task/thread boundary
- creating a thread does not create a canonical task automatically;
- task detail, note journal, and mailbox task links continue to treat task as the primary work
  object;
- thread messages attach to existing task context rather than creating parallel task truth by
  default.

4. Mailbox/thread boundary
- mailbox claim and watching state remain authoritative for execution ownership;
- thread visibility alone does not satisfy reply obligation or ownership rules;
- thread claim state, linked task, and open reply obligations are visible in Team runtime surfaces.

5. Shared default lane
- the default shared-thread target is stable and idempotent across reload, restart, and replay;
- human and agent replies in the default lane remain visible through the same canonical target.

6. UI behavior
- channel timeline remains summary-first and scannable;
- thread pane carries the deeper context;
- narrow-screen shell preserves the same distinction without turning thread into a top-level lens.

## Operational Notes

- Keep channel roots summary-first; move long logs, detailed evidence, and extended back-and-forth
  into the thread.
- Prefer expanding an existing task/thread lane over creating parallel lanes for the same work.
- Keep channel taxonomy sparse and durable.
- Treat thread routing as a correctness contract for prompts, runtime fan-out, and UI actions.
- When task, mailbox, and thread disagree, task ownership and mailbox triage remain authoritative
  for execution responsibility.

## Open Risks

- Channel sprawl can recreate folder-like clutter and hide work instead of clarifying it.
- Weak reply-target discipline can still cause replies to leak into the wrong visible surface.
- Long-lived threads may require stronger history windowing and replay optimization.
- Cross-actor takeover rules can become confusing if thread participation and mailbox ownership are
  presented as the same thing in the UI.
- Conversation-level task-intent can over-promise if the UI does not make coordinator
  canonicalization explicit.

## Source Journals

- [docs/journal/2026-03-08-teams-conversation-tasks-mailbox-workflow.md](../journal/2026-03-08-teams-conversation-tasks-mailbox-workflow.md)
- [docs/journal/2026-03-13-team-shared-thread-canonical-replies.md](../journal/2026-03-13-team-shared-thread-canonical-replies.md)
- [docs/journal/2026-05-18-team-workspace-p0-matrix.md](../journal/2026-05-18-team-workspace-p0-matrix.md)
- [docs/journal/2026-05-20-team-mailbox-phase2-ownership-and-task-links.md](../journal/2026-05-20-team-mailbox-phase2-ownership-and-task-links.md)
- [docs/journal/2026-05-21-team-spec-refresh-from-external-daemon-review.md](../journal/2026-05-21-team-spec-refresh-from-external-daemon-review.md)
- [docs/journal/2026-05-21-team-prompt-operating-contract-refresh.md](../journal/2026-05-21-team-prompt-operating-contract-refresh.md)
- [docs/journal/2026-05-21-team-task-first-and-mailbox-envelope-slice.md](../journal/2026-05-21-team-task-first-and-mailbox-envelope-slice.md)
