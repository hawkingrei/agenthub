# Team Mailbox Intake And Ownership Specification

## Problem

AgentHub Team mailbox currently models only delivery progress (`pending`, `delivered`,
`dead_letter`) and a coarse `ack` action. That is sufficient for transport, but insufficient for
multi-channel Team collaboration:

- human requests may arrive from chat, webhook, trigger, or future external channels rather than a
  single prompt `input`;
- an agent may read a message and decide it is not relevant, should be watched later, or should be
  claimed immediately;
- one mailbox topic may need a task link and a thread owner before execution can proceed;
- one agent may already own a thread, while others should observe without taking over.

Without a stable intake and ownership contract, mailbox consumers either over-ack messages too
early or silently leave important user-visible work unclaimed.

## Scope

- Canonical inbound-envelope contract for Team mailbox work regardless of source channel.
- Separation between delivery state, handling state, and thread/task ownership.
- Canonical handling actions for triage, claim, release, completion, and watch mode.
- Thread ownership rules for Team mailbox topics.
- Message-to-task linkage and reply-required handling for human-facing messages.

## Non-Goals

- Final provider-specific adapter implementations for Telegram, email, webhook, or push channels.
- Replacing Team conversation event bus, Team channels, or thread UI surfaces.
- Full ACL design for multi-user manual overrides.
- Final CLI flag names for every transitional compatibility path.

## Architecture

### 1) Layered Mailbox Model

Mailbox handling is split into four layers:

1. inbound envelope
   - normalized source message independent of channel origin
2. delivery state
   - transport-level persistence and replay status
3. handling disposition
   - whether an actor ignored, watched, claimed, released, or completed the work item
4. ownership/linkage
   - thread lease and task association for durable execution

These layers must remain separate. Transport delivery must not be overloaded to represent handling
intent.

### 2) Canonical Inbound Flow

Every external or internal source message should be normalized into one canonical inbound envelope
before Team routing:

- human message from Team conversation UI
- direct mailbox send from another agent
- future trigger event
- future webhook or external chat adapter
- system-generated review or follow-up request

After normalization:

1. persist the envelope as authoritative mailbox input;
2. fan out to mailbox recipients when mailbox routing is required;
3. expose the unread item through inbox reads;
4. let an actor triage the item before claiming execution;
5. attach durable task/thread state when the actor accepts ownership.

### 3) Topic Ownership Layer

Mailbox messages can represent standalone direct work, but many evolve into longer-lived topics.
When that happens, Team runtime should attach the message to:

- a thread root for focused discussion, and/or
- a canonical Team task for durable execution ownership.

Thread ownership is lease-based and independent from delivery ack. One actor may actively own a
thread while other actors remain watchers.

## Contracts

### 1) Inbound Envelope Contract

Every canonical inbound envelope should expose these stable fields:

- `message_id`
- `correlation_id`
- `source_kind`
  - `human`
  - `agent`
  - `trigger`
  - `system`
- `source_surface`
  - examples: `conversation`, `mailbox`, `thread`, `timer`, `webhook`
- `run_id`
  - optional for pre-run or conversation-scoped input
- `team_id`
- `conversation_id`
  - optional when the source is mailbox-only and not conversation-backed
- `thread_root_message_id`
  - optional until the topic is threaded
- `reply_target`
  - the canonical user-visible reply surface when one exists
- `message_kind`
- `payload`
- `requires_user_visible_reply`

`input` is only one possible source representation. Runtime must not assume that all user intent
originates from prompt `input`.

### 2) Message Kind Contract

Mailbox handling must classify messages by intent, not only by sender:

- `human_request`
- `coordination_request`
- `trigger_event`
- `task_signal`
- `thread_reply`
- `system_notice`

This kind should be available at read time without forcing every consumer to reverse-engineer raw
payload shape.

### 3) Delivery Status Contract

Delivery status remains transport-only:

- `pending`
- `delivered`
- `dead_letter`

Allowed transitions:

- `pending -> delivered`
- `pending -> dead_letter`

Delivery status must not imply that the target actor accepted, ignored, or completed the work.

### 4) Handling Disposition Contract

Handling disposition is separate from delivery status and models actor-level work decisions:

- `untriaged`
- `ignored`
- `watching`
- `claimed`
- `completed`
- `released`

Semantics:

- `untriaged`
  - no actor has made a handling decision yet
- `ignored`
  - actor read the message and decided it is not relevant for them
- `watching`
  - actor wants to observe the topic without taking ownership yet
- `claimed`
  - actor accepted responsibility for the next action on this item/topic
- `completed`
  - actor finished the claimed action and recorded durable outcome
- `released`
  - actor previously claimed the item/topic but explicitly gave up ownership

`ignored` and `watching` are not errors. They are first-class handling outcomes.

### 5) Canonical Handling Action Contract

Mailbox consumers should evolve toward these actions:

- `inbox`
  - read-only snapshot
- `triage`
  - set `ignored`, `watching`, or `claimed`
- `claim`
  - explicit ownership acquisition when the actor will act now
- `release`
  - explicit ownership surrender
- `resolve`
  - mark the claimed work complete and attach durable evidence

Compatibility path:

- existing `receive = inbox + ack` and `ack` may remain temporarily for repair or migration
- they are not the long-term canonical Team handling contract

### 6) Thread Ownership Contract

Thread ownership is topic-scoped, not message-scoped.

Canonical thread-claim fields:

- `thread_root_message_id`
- `owner_actor_id`
- `claim_status`
- `claimed_at`
- `lease_expires_at`

Rules:

- at most one active owner may hold a thread claim at a time;
- other actors may still read the thread and mark mailbox items as `watching`;
- a second actor must not silently take over while the first claim lease is active;
- takeover requires one of:
  - explicit release
  - lease expiry
  - coordinator/system reassignment

### 7) Message-To-Task Link Contract

Mailbox messages may create or relate to Team tasks.

Canonical link relation kinds:

- `spawned_task`
- `related_task`
- `evidence_for_task`

Rules:

- not every mailbox message becomes a task;
- when an actor claims execution work that should be tracked durably, the actor or coordinator
  should create or attach a canonical Team task;
- task linkage must be queryable without parsing free-form note text.

### 8) Human Reply Requirement Contract

For any inbound envelope with `requires_user_visible_reply = true`, runtime must not silently end
processing without one of these outcomes:

- user-visible reply emitted successfully;
- work transferred explicitly to another actor with preserved reply obligation;
- message marked ignored with an explicit allowed reason;
- issue escalated to coordinator or human operator with visible evidence.

The system should treat unresolved reply obligations as an explicit runtime state rather than as a
successful no-op.

### 9) Inbox Ordering Contract

Unread actionable work must remain visible ahead of historical replay needs.

Rules:

- default inbox reads prioritize unread actionable items;
- historical `include_delivered` style replay must not hide current pending work behind long
  delivered history;
- watch/claim state should be visible alongside delivery state so an actor can decide whether the
  item still needs attention.

### 10) Reply Target Fidelity Contract

- `reply_target` is the canonical reply surface for the current inbound item when one exists.
- If `thread_root_message_id` is present, reply should stay on that same thread by default rather
  than opening a new conversation lane.
- Widening the audience from a direct reply to `Conversation` / group chat is an explicit
  escalation choice, not the default reply path.
- Local stdout, tool logs, or internal reasoning do not satisfy a mailbox reply obligation for
  reply-required messages.

### 11) Actionability And Ownership Decision Contract

- A mailbox item that can be answered immediately without durable execution work may be replied to
  directly without spawning a new task.
- A mailbox item that requires tool execution, code changes, external side effects, or durable
  multi-step follow-up should be claimed before the actor begins extended work.
- When claimed work must remain visible on Kanban or survive the current turn, the actor or
  coordinator should create or attach a canonical Team task instead of leaving the work only in
  mailbox state.
- If an active thread owner already exists, a second actor should prefer `watching` or explicit
  handoff/escalation rather than racing a second ownership claim.

### 12) Deferred Follow-Up Contract

- Future follow-up that does not happen immediately should use a durable trigger/reminder path
  linked to the same mailbox topic, thread, or task.
- Deferred follow-up must not rely on long sleeps, idle loops, or unstated memory-only recall.
- Trigger-driven follow-up should preserve enough correlation metadata to reconnect the later wakeup
  with the original task/thread ownership chain.

## Validation Matrix

- Rust domain tests for inbound-envelope normalization, handling-disposition transitions, and
  thread-claim lease rules.
- Rust API/CLI tests for read-only inbox, explicit triage/claim/release flows, and compatibility
  behavior for legacy `receive`/`ack`.
- Focused tests that verify pending unread work remains visible even when delivered history is
  long.
- Team integration tests for:
  - claim -> task link creation
  - thread ownership preventing double takeover
  - required user-visible reply not being dropped silently
- Focused MCP/CLI prompt validation so coordinator and worker instructions match the canonical
  triage/ownership flow.

## Operational Notes

- Trigger-driven follow-up should reuse the same inbound-envelope contract as human or agent
  mailbox messages.
- Read/claim prompts should make the distinction between "quick reply" and "durable execution work"
  explicit so agents do not over-create tasks for answer-only messages.
- Mailbox metrics should distinguish transport state from handling state:
  - unread
  - watching
  - claimed
  - completed
  - released
- Coordinator visibility should include thread owner, linked task, and unresolved reply-required
  items.
- Historical replay remains valuable for audit/debugging, but replay must not become the primary
  unread-discovery surface.
- Existing `actor_receive` and `actor_ack` flows should be treated as compatibility behavior until
  explicit triage/claim commands replace them.

## Open Risks

- Introducing both delivery state and handling disposition increases schema and UI complexity.
- Thread lease expiry policy may be hard to tune across long-running or intermittently connected
  agents.
- Some trigger/system messages may legitimately require no user-visible reply; the allowlist for
  silent completion must stay narrow and explicit.
- Backfilling message kind and handling disposition for legacy rows may require compatibility
  inference during migration.

## Source Journals

- `docs/journal/2026-02-18-acp-actor-mailbox-native-tools.md`
- `docs/journal/2026-02-22-team-task-routing-user-actor-semantics.md`
- `docs/journal/2026-03-05-team-mcp-enforcement-external-review.md`
- `docs/journal/2026-05-21-team-spec-refresh-from-external-daemon-review.md`
