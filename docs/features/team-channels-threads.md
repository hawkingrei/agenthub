# Team Channels And Threads Specification

## Problem

The Team workspace exposes `# all` plus API-backed non-default Team channels, channel-rooted
threads, and the task/run/member surfaces. The current contract keeps the compact
`channel + thread` model as the canonical Team communication shape.

The model exists to avoid these regressions:

- Team communication collapsing back into one flat conversation instead of discoverable
  communication lanes;
- replies duplicating into the parent channel timeline instead of staying in a focused thread pane;
- channel roots turning into full context dumps instead of summary entrypoints;
- threads becoming a top-level workspace lens or replacing canonical Team task/run ownership.

We want to learn from the Slock channel/thread layout without collapsing AgentHub Team semantics or
losing the current Notion-style content-first shell.

## Scope

- Team communication-lane information architecture.
- Canonical `channel` and `thread` shell behavior inside Team workspaces.
- Team actor capability direction for opening and replying in threads.
- URL/deep-link behavior for channel and thread selection.
- Right-side thread pane behavior.
- Composer-level task affordance direction.

## Non-Goals

- Replacing Team canonical task ownership or Team run/step semantics.
- Defining the full backend storage schema for every future channel feature.
- Implementing reactions, rich moderation, or arbitrary Slack-style channel administration.
- Turning human chat input directly into canonical Team task records.

## Architecture

### 1) Core Principle

Team should support a `channel timeline + thread pane` structure.

The shell should follow this shape:

- left rail: Team directory plus Team channels;
- center pane: selected channel timeline;
- right pane: optional selected thread;
- composer: always attached to the currently selected conversation context.

This should feel structurally closer to Slock while still preserving AgentHub-specific Team depth.

### 2) Canonical Entity Model

Inside a Team workspace, communication should distinguish:

- `channel`
  - a human-facing communication lane inside one Team
  - examples:
    - `# all`
    - future review / topic / onboarding channels
- `thread`
  - a focused reply context rooted in one channel message
  - always belongs to one parent `channel`
- `task`
  - the canonical Team work object
  - remains distinct from both `channel` and `thread`

Important constraint:

- channels and threads are communication/review surfaces
- they are not the canonical Team task surface

### 3) Channel Contract

The Team shell should treat channels as first-class Team communication lanes.

Required baseline:

- `# all` remains the default channel
- `# all` is the broad coordination/default lane, not the only lane
- left rail groups channels under `Channels`
- selecting a channel replaces the center timeline without leaving the Team workspace shell
- channel header stays light and directory-like:
  - channel name
  - optional short description
  - optional compact member/visibility metadata

Multi-channel contract:

- a Team may expose multiple channels in addition to `# all`
- every non-default channel should have a short, explicit work-scoping description
- channels should narrow context, not duplicate Kanban/task ownership
- non-default channels may be created explicitly by the Team coordinator or a human operator
- non-default channels may also be archived or deleted explicitly when the work lane is no longer
  needed

Recommended examples:

- `# all`
  - default coordination lane
  - human requests, planning, announcements, cross-cutting updates
- `# review`
  - focused review / approval / PR / decision follow-up
- `# research`
  - investigation notes, paper or issue exploration, structured findings
- `# rollout`
  - launch, migration, deployment, operational checklists

Constraints:

- do not create channels as arbitrary folders for every task
- channels should describe a durable work mode or work stream, not replace canonical Team tasks
- each channel description should make it obvious why a message belongs there instead of `# all`
- `# all` is the system default channel and must not be deletable
- deleting a non-default channel must not delete canonical Team tasks or execution history
- deleting a non-default channel should remove only the communication lane and its thread context,
  or archive it first if hard deletion is too destructive for the current rollout phase

Constraints:

- do not add thick dashboard chrome or channel-level runtime metrics that Slock does not need
- keep channel rows compact: `title + one compact meta line` at most

### 4) Thread Contract

Threads should become first-class secondary conversation contexts.

Canonical meaning:

- `channel`
  - the durable Team communication lane
- `thread`
  - a focused collaboration context rooted in one event inside a channel
  - initially, that root event is a channel message
  - later, the same concept may also index review/execution follow-up events without changing the
    channel-first shell

Product consequence:

- thread is not the primary Team entrypoint
- thread is the reusable “recent active context” unit that can be reopened from either:
  - the current channel timeline
  - a future recent-threads index
- this matches the useful part of the Slock model without collapsing AgentHub into a chat-only
  product

Required behavior:

- selecting a reply target opens a right-side `ThreadPane`
- the center channel timeline remains visible
- thread replies should render only in the right-side thread pane; they should not duplicate as
  inline rows in the parent channel timeline
- the right pane header should show:
  - `Thread`
  - parent channel reference (for example `#papers`)
  - `View in channel`
  - `Close thread`
- thread messages render as a focused sub-conversation instead of expanding inline into a large
  nested tree

Important constraint:

- a thread is a communication context, not a separate Team object type
- a thread is not a top-level Workspace lens and must stay subordinate to its parent channel
- closing the thread should return the layout to channel-only mode without losing the selected
  channel
- the first version should derive `thread` from an existing channel root message instead of
  inventing a detached empty thread object
- a later `Threads` or `Recent threads` view, if introduced, should be an index of active thread
  contexts rather than a new creation surface

#### 4.1) Summary Versus Full Context

Channel and thread should deliberately split summary from full context.

Required product meaning:

- the parent `channel` message is the summary entrypoint for one issue, request, review point, or
  task-oriented discussion
- the `thread` rooted at that message is the full context container for that specific topic

This means:

- the channel timeline should stay scannable and low-noise
- the thread should carry the deeper context:
  - detailed background
  - longer explanations
  - progress updates
  - logs, references, and follow-up discussion

Product goal:

- avoid turning the main channel into one long high-volume context dump
- let humans and agents discover that a topic exists from the channel summary first
- let only the interested or relevant participants open the thread and read the full context

#### 4.2) Agent Relevance And Context Budget

Threads should help AgentHub avoid unnecessary shared-channel context explosions.

Required behavioral direction:

- agents should not need to ingest the entire recent channel history by default in order to handle
  one focused issue
- a channel root message should be sufficient to advertise that a more detailed context exists
- agents that are explicitly mentioned, own the work, or otherwise judge the topic as relevant may
  then open the thread and read the full discussion

This preserves two useful properties at the same time:

- broad shared visibility in the parent channel
- bounded deep context in the thread

Current concurrency boundary:

- seeing a root channel message does not automatically make an agent or user a thread participant
- if another participant later opens the thread and replies, automatic thread forwarding targets:
  - existing thread participants
  - members mentioned on the root message
  - members mentioned on earlier thread replies
  - newly mentioned members in the current reply
- a passive root reader therefore must explicitly open the thread later, or be mentioned, before it
  can rely on receiving the deeper follow-up automatically
- this is acceptable for the current rollout because the root message is intentionally summary-first,
  but prompt/runtime guidance must make that boundary explicit

Important product consequence:

- thread is not just a UI nesting affordance
- thread is part of the Team context-management model
- it should reduce unnecessary context fan-in for both humans and agents

#### 4.3) Prompt And Agent Behavior Contract

The channel/thread split should be reflected explicitly in Team prompt and behavior guidance.

Required direction:

- coordinator and worker prompts should describe the channel root message as the summary entrypoint
- prompts should describe the thread as the place for the full context
- agents should learn that long background, logs, evidence, and detailed follow-up belong in the
  thread instead of being pasted into the main channel by default
- the thread pane itself should reinforce the same split with summary-first helper copy instead of
  presenting reply nesting as a purely mechanical UI affordance
- the main channel composer should reinforce the same split with summary-first helper copy so a new
  root post does not feel like the place to dump the entire working context

Expected agent behavior:

- when posting a new topic in a channel, keep the root message summary-first
- when deeper context is needed, continue inside the thread
- when an agent is mentioned or otherwise judges the topic relevant, it should open the thread
  before assuming the root message contains the complete working context
- thread replies should be preferred for topic-specific back-and-forth so the main channel remains
  scannable
- prompt/runtime guidance should treat `agenthub actor team-thread-open` and
  `agenthub actor team-thread-reply` as the canonical agent-side way to proactively move a topic
  from summary root into thread-scoped deep context

This is not only a UX rule.

It is part of the Team context-budget contract:

- channel = broad visibility
- thread = bounded deep context

### 5) Actor Capability Contract

`Open thread` should not remain only a front-end affordance.

It should become a Team actor capability with shell projection.

Recommended capability surface:

- `team_channel_create`
  - input:
    - `team_id`
    - `channel_id`
    - `title`
    - optional `description`
  - behavior:
    - creates a non-default Team communication lane
    - reserved for Team coordinator and human operator flows by default
- `team_channel_update`
  - input:
    - `team_id`
    - `channel_id`
    - optional `title`
    - optional `description`
    - optional ordering / visibility metadata
- `team_channel_archive`
  - input:
    - `team_id`
    - `channel_id`
  - behavior:
    - hides a non-default channel from the active rail without destroying canonical Team work
- `team_channel_delete`
  - input:
    - `team_id`
    - `channel_id`
  - behavior:
    - permanently removes a non-default communication lane when explicit deletion is desired
    - must reject deletion of `# all`

- `team_thread_open`
  - input:
    - `team_id`
    - `channel_id`
    - `root_message_id`
    - optional `reason`
  - behavior:
    - if a thread already exists for the root message, return it
    - otherwise lazily materialize/open a thread bound to that root message
- `team_thread_reply`
  - input:
    - `team_id`
    - `channel_id`
    - `root_message_id`
    - reply `text`
  - behavior:
    - appends a reply scoped to the thread rooted at the existing channel message
    - first rollout may persist replies inside the parent channel conversation as long as payloads
      carry canonical `thread_root_message_id` metadata for filtering
    - shell and public HTTP clients should use a stable Team API path instead of depending on
      internal gRPC directly:
      - `POST /api/teams/:team_id/channels/:channel_id/threads/:root_message_id/replies`
- `team_thread_view_in_channel`
  - shell/navigation projection back to the parent channel timeline

Important constraints:

- `team_channel_create` / `team_channel_delete` should default to coordinator + human operator authority
- worker actors should not create or delete Team channels unless a later policy explicitly grants it
- thread open must be anchored to an existing channel message
- agent actor must not create detached empty threads without a `root_message_id`
- thread capability does not change canonical Team task ownership or task materialization rules
- the shell `Open thread` button should become a projection of this actor capability rather than an
  isolated UI-only construct

### 6) URL And Deep-Link Contract

The Team workspace should support stable deep links for channels and threads.

Target direction:

- `/workspace/teams/:team_id?channel=:channel_id`
- `/workspace/teams/:team_id?channel=:channel_id&thread=:thread_id`

Guidance:

- `channel` controls the center timeline
- `thread` controls the optional right-side pane
- a missing `thread` means the thread pane is closed
- a missing `channel` defaults to the Team default lane (`# all`)
- `thread` should initially resolve from the root channel message identity
  (`root_message_id`-backed deep link)

### 7) Composer Task Affordance

We should learn from Slock's composer-adjacent `As Task` affordance, but keep AgentHub semantics.

Required constraint:

- human chat input must not directly create canonical Team task records

Core product goal:

- let a human mark one outgoing conversation message as "task-oriented intent"
- keep that intent close to the composer instead of hiding it in Kanban-only workflows
- preserve the existing Team rule that coordinator planning/runtime decide whether and how a canonical
  Team `task` should be materialized

Recommended direction:

- composer may expose a lightweight task affordance such as:
  - `Create task draft`
  - `Promote to task request`
- this affordance should live next to the message composer, not as a thick page-level toolbar
- resulting behavior should still flow through Team planning/runtime materialization before becoming
  a canonical Team `task`
- non-default channels may shape the draft/request context, but must not bypass coordinator/runtime
  canonicalization

Product interpretation:

- this is a `conversation -> task-intent` affordance
- it is not a direct `create canonical task now` action
- the first rollout should optimize clarity of intent, not attempt to expose full task
  configuration in the composer

#### 7.1 Placement And Surface Rules

The affordance should stay attached to the active Team conversation composer.

Recommended first-rollout placement:

- show the affordance as a compact secondary control adjacent to the send action
- keep it in the same visual language as other lightweight Team controls
- do not add a thick horizontal toolbar or a multi-row composer chrome layer

Allowed surfaces:

- channel composer
- thread reply composer

Disallowed surfaces for the first rollout:

- ACP/agent runtime input docks
- Kanban canonical task editor
- global workspace shell header

#### 7.2 Interaction Model

The affordance should behave like a lightweight mode toggle, not a separate form page.

Default state:

- composer behaves like ordinary Team conversation input
- the task affordance is visible but not active

Armed state:

- after the user activates the affordance, the composer enters a lightweight `task request` mode
- show a thin context strip above or inside the composer, for example:
  - `Task request`
  - `Send as a task-oriented request for coordinator planning`
- keep the main text area unchanged
- allow the user to cancel the mode before sending

Send behavior in armed state:

- sending still creates one conversation message in the current channel/thread context
- that message carries task-request intent metadata
- sending does not immediately create a canonical Team task row in Kanban

Exit behavior:

- after successful send, the composer returns to ordinary conversation mode
- if the user cancels before send, the mode clears without mutating the typed text

#### 7.3 Visual Direction

We should learn from Slock's lightweight "As Task" pattern, but preserve AgentHub's current shell.

Required visual direction:

- compact secondary control
- minimal additional height
- one thin context strip when active
- no modal, no drawer, no expanded inline task form in the first rollout

Explicit anti-goals:

- do not add Slack-like rich composer toolbars
- do not open a task-configuration side panel from the composer
- do not ask for assignee, run policy, channel selection, or execution metadata before send
- do not make the affordance visually heavier than the primary send action

#### 7.4 Semantic Contract

The first rollout should treat the affordance as an intent marker on a conversation message.

Required semantic properties:

- the message remains part of the selected channel/thread conversation history
- the message is clearly distinguishable to Team planning/runtime as a task-oriented request
- Team coordinator/runtime may later:
  - materialize a canonical Team task
  - ignore the request
  - ask for clarification
  - merge it into an existing task or lane

Important constraint:

- the affordance should not promise that every marked message becomes a task
- the affordance should only promise that the message is delivered as task-oriented intent

#### 7.5 Relationship To Channel And Thread Context

The affordance should inherit the active communication context rather than invent a separate one.

Rules:

- in a channel composer, the task-request message belongs to that selected channel
- in a thread composer, the task-request message belongs to that thread context while still
  remaining subordinate to the parent channel
- a non-default channel may shape the meaning of the request
  - for example `# review` implies review-oriented task requests
  - for example `# research` implies investigation-oriented task requests
- but channel choice alone must not create or classify canonical Team tasks automatically

#### 7.6 Relationship To Kanban

Kanban remains the canonical task surface.

Still true after this affordance ships:

- Kanban is where canonical tasks live
- coordinator planning/runtime own materialization into Kanban
- composer-level task intent is an upstream signal, not a replacement for Kanban

This means:

- the first rollout should not insert optimistic task cards directly into Kanban on send
- if later product work wants a stronger link, it should surface as:
  - `requested task created`
  - `linked to existing task`
  - or another explicit follow-up event after canonicalization

#### 7.7 URL And Persistence Guidance

The first rollout should avoid introducing a dedicated URL mode for task-request composer state.

Guidance:

- ordinary `channel` / `thread` route state remains canonical
- the temporary armed-state of the composer does not need its own query parameter
- drafts may stay local to the current page session in the first rollout

This keeps the feature lightweight and prevents task-intent UX from overcomplicating shell routing.

#### 7.8 Rollout Slice

The first implementation slice should be intentionally narrow.

Phase 5A:

- add the lightweight composer affordance
- add the thin active-state strip
- send one conversation message with task-request intent metadata
- do not change Kanban materialization semantics yet

Phase 5B:

- surface clearer follow-up feedback when coordinator/runtime later materialize a task from that request
- optionally show a compact linkage from the request message to the resulting canonical task

Phase 5C:

- evaluate whether richer task-draft affordances are needed
- only after the lightweight mode proves useful and does not blur the Team task contract

### 8) Relationship To Existing Team Semantics

This design must preserve the existing Team operating model.

Still true after rollout:

- human intent enters through conversation
- coordinator/runtime own canonical task materialization
- `Kanban` remains the canonical Team task lane
- `Execution Runs` remains the canonical execution-history/debug lane
- `run` and `step` remain execution/debug artifacts, not communication objects

### 9) Notion-Style Constraints

The channel/thread rollout must keep the current visual direction:

- restrained chrome
- thin headers
- compact directory rows
- low badge density
- content-first panes

Explicit anti-goals:

- do not add Slack-like thick composer toolbars
- do not add persistent runtime/status panels around every channel
- do not overload thread headers with debugging metadata

## Rollout Phases

### Phase 1: Channel/Thread Spec And Routing

- define canonical Team `channel` and `thread` route/query grammar
- introduce shell-level selected-channel / selected-thread state
- keep the current single channel (`# all`) behavior working through the new shell model

### Phase 2: Thread Split View

- add a real right-side `ThreadPane` in Team conversation
- keep the center channel timeline visible
- wire message reply/thread-count affordances to open/close the pane

### Phase 3: Actor Capability Integration

- add Team actor-level `thread_open` / `thread_reply` behavior
- make shell thread open/close a projection of actor-side thread identity instead of pure UI state

### Phase 4: Additional Team Channels

- make `Channels` in the Team rail a true Team-local directory instead of a fixed two-item list
- support more than `# all` while preserving current Team defaults
- require channel descriptions that explain the work focus of each non-default lane

### Phase 5: Composer Task Draft Affordance

- add a lightweight task affordance near the composer
- keep canonical task creation behind Team planning/runtime semantics

## Validation

- route-level tests for `channel` and `thread` query/deep-link behavior
- Team shell tests for `channel-only` vs `channel + thread` layouts
- conversation tests for opening and closing the right thread pane
- focused tests or prompt-contract checks proving summary-first root messages and thread-first deep
  context guidance stay encoded in the Team behavior surface
- actor command / capability tests for thread-open anchored to `root_message_id`
- channel-directory tests for `# all` plus descriptive non-default channels
- regression tests confirming `Kanban` and `Execution Runs` stay canonical for task/run ownership
- Chrome DevTools MCP checks against:
  - Slock channel/thread reference page
  - local Team workspace regression after implementation

## Source Journals

- `docs/journal/2026-05-03-team-thread-prompt-contract.md`
